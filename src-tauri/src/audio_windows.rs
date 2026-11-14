//! WASAPI endpoint routing. A device change replaces the native stream, never
//! the WAV writer or the call clock. COM callbacks only signal the owning thread.
use super::{open_track, DeviceFrameClock, LevelMeter, PacketTimeline};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use windows::{
    core::{Result as WinResult, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE, PROPERTYKEY},
        Media::Audio::*,
        System::{Com::*, Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc}, Threading::*},
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
    },
};

struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn take_string(value: PWSTR) -> WinResult<String> {
    let result = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())); }
    Ok(result?)
}
pub(super) fn outputs() -> WinResult<Vec<super::AudioOutput>> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }?;
    let _com = Com;
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
    let devices = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }?;
    let mut result = Vec::new();
    for index in 0..unsafe { devices.GetCount() }? {
        let device = unsafe { devices.Item(index) }?;
        let id = take_string(unsafe { device.GetId() }?)?;
        let name = (|| -> WinResult<String> {
            let store = unsafe { device.OpenPropertyStore(STGM_READ) }?;
            let mut value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }?;
            let name = unsafe { PropVariantToStringAlloc(&value) }.and_then(take_string);
            unsafe { PropVariantClear(&mut value) }?;
            name
        })().unwrap_or_else(|_| id.clone());
        result.push(super::AudioOutput { id, name });
    }
    result.sort_by_key(|device| device.name.to_lowercase());
    Ok(result)
}
fn output_role(direction: EDataFlow, output: &str) -> Option<ERole> {
    if direction != eRender || output.is_empty() { Some(eConsole) }
    else if output == "@communications" { Some(eCommunications) }
    else { None }
}
struct Event(HANDLE);
impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct AudioPriority(Option<HANDLE>);
impl Drop for AudioPriority {
    fn drop(&mut self) {
        if let Some(handle) = self.0 {
            unsafe {
                let _ = AvRevertMmThreadCharacteristics(handle);
            }
        }
    }
}

#[windows::core::implement(IMMNotificationClient)]
struct EndpointNotifications {
    direction: EDataFlow,
    changed: Arc<AtomicBool>,
}
impl IMMNotificationClient_Impl for EndpointNotifications_Impl {
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _: &PCWSTR) -> WinResult<()> {
        if flow == self.direction && (role == eConsole || role == eCommunications) {
            self.changed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> WinResult<()> {
        self.changed.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> WinResult<()> {
        self.changed.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> WinResult<()> {
        self.changed.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> WinResult<()> {
        Ok(())
    }
}
struct EndpointWatch {
    enumerator: IMMDeviceEnumerator,
    callback: IMMNotificationClient,
    registered: bool,
    changed: Arc<AtomicBool>,
    output: String,
}
impl EndpointWatch {
    fn new(direction: EDataFlow, output: &str) -> WinResult<Self> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
        let changed = Arc::new(AtomicBool::new(false));
        let callback: IMMNotificationClient = EndpointNotifications {
            direction,
            changed: changed.clone(),
        }
        .into();
        // Periodic checks below remain available if a driver refuses callbacks.
        let registered =
            unsafe { enumerator.RegisterEndpointNotificationCallback(&callback) }.is_ok();
        Ok(Self {
            enumerator,
            callback,
            registered,
            changed,
            output: output.into(),
        })
    }
    fn endpoint(&self, direction: EDataFlow) -> WinResult<(String, IMMDevice)> {
        let device = if let Some(role) = output_role(direction, &self.output) {
            unsafe { self.enumerator.GetDefaultAudioEndpoint(direction, role) }?
        } else {
            let id: Vec<u16> = self.output.encode_utf16().chain(Some(0)).collect();
            let device = unsafe { self.enumerator.GetDevice(PCWSTR(id.as_ptr())) }?;
            if unsafe { device.GetState() }? != DEVICE_STATE_ACTIVE {
                return Err(windows::core::Error::from_hresult(AUDCLNT_E_DEVICE_INVALIDATED));
            }
            device
        };
        let id = take_string(unsafe { device.GetId() }?)?;
        Ok((id, device))
    }
}
impl Drop for EndpointWatch {
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                let _ = self
                    .enumerator
                    .UnregisterEndpointNotificationCallback(&self.callback);
            }
        }
    }
}

struct Stream {
    id: String,
    client: IAudioClient,
    reader: IAudioCaptureClient,
    event: Event,
    position_clock: DeviceFrameClock,
}
struct Packet {
    samples: Vec<f32>,
    flags: u32,
    timestamp: u64,
    device_position: u64,
}
impl Stream {
    fn open(id: String, device: &IMMDevice, loopback: bool) -> WinResult<Self> {
        let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }?;
        let native_format = unsafe { client.GetMixFormat() }?;
        let native_rate = unsafe { (*native_format).nSamplesPerSec };
        unsafe { CoTaskMemFree(Some(native_format.cast())); }
        let format = WAVEFORMATEX {
            wFormatTag: 3,
            nChannels: 1,
            nSamplesPerSec: 16000,
            nAvgBytesPerSec: 64000,
            nBlockAlign: 4,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
            | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
            | if loopback {
                AUDCLNT_STREAMFLAGS_LOOPBACK
            } else {
                0
            };
        unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 5_000_000, 0, &format, None) }?;
        let event = Event(unsafe { CreateEventW(None, false, false, None) }?);
        unsafe { client.SetEventHandle(event.0) }?;
        let reader = unsafe { client.GetService() }?;
        unsafe { client.Start() }?;
        Ok(Self {
            id,
            client,
            reader,
            event,
            position_clock: DeviceFrameClock::new(native_rate),
        })
    }
    fn packet(&mut self) -> WinResult<Option<Packet>> {
        if unsafe { self.reader.GetNextPacketSize() }? == 0 {
            return Ok(None);
        }
        let (mut pointer, mut frames, mut flags, mut timestamp, mut device_position) = (std::ptr::null_mut(), 0, 0, 0, 0);
        unsafe {
            self.reader.GetBuffer(
                &mut pointer,
                &mut frames,
                &mut flags,
                Some(&mut device_position),
                Some(&mut timestamp),
            )
        }?;
        // Always release an acquired buffer, including zero-sized or bad buffers.
        let samples = if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || frames == 0 {
            Some(vec![0.; frames as usize])
        } else if pointer.is_null() {
            None
        } else {
            Some(
                unsafe { std::slice::from_raw_parts(pointer as *const f32, frames as usize) }
                    .to_vec(),
            )
        };
        unsafe { self.reader.ReleaseBuffer(frames) }?;
        let samples = samples.ok_or_else(|| {
            windows::core::Error::from_hresult(windows::Win32::Foundation::E_POINTER)
        })?;
        let device_position = self.position_clock.normalize(device_position, timestamp, frames as u64, flags);
        Ok((frames > 0).then_some(Packet {
            samples,
            flags,
            timestamp,
            device_position,
        }))
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

fn recoverable(error: &windows::core::Error) -> bool {
    [
        AUDCLNT_E_DEVICE_INVALIDATED,
        AUDCLNT_E_RESOURCES_INVALIDATED,
        AUDCLNT_E_SERVICE_NOT_RUNNING,
    ]
    .contains(&error.code())
}

#[derive(Default)]
struct ReconnectHistory {
    since: Option<u64>,
    count: u64,
    frames: u64,
    positions: Vec<u64>,
    longest: u64,
}
impl ReconnectHistory {
    fn begin(&mut self, frame: u64) {
        if self.since.is_none() {
            self.since = Some(frame);
            self.count += 1;
            if self.positions.len() < 6 {
                self.positions.push(frame);
            }
        }
    }
    fn end(&mut self, frame: u64) {
        if let Some(start) = self.since.take() {
            let missing = frame.saturating_sub(start);
            self.frames += missing;
            self.longest = self.longest.max(missing);
        }
    }
    fn warning(&mut self, source: &str, end: u64) -> Option<String> {
        let still_missing = self.since.is_some();
        self.end(end);
        if self.count == 0 || (!still_missing && self.longest < 4000 && self.frames < 48000) {
            return None;
        }
        let positions = self
            .positions
            .iter()
            .map(|frame| format!("{:02}:{:02}", frame / 16000 / 60, frame / 16000 % 60))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!("{}: {} riconnessioni del dispositivo, {:.2} s complessivi in attesa. Verifica vicino a {}{}. {}",
            if source=="system" {"Audio PC"} else {"Microfono"},self.count,self.frames as f64/16000.,positions,
            if self.count>self.positions.len() as u64 {" e altri punti"} else {""},
            if still_missing {"Sorgente ancora indisponibile allo stop."} else {"La sorgente è stata ripristinata."}))
    }
}

pub(super) fn capture(
    app: &AppHandle,
    source: &str,
    file: &Path,
    stop: Arc<AtomicBool>,
    start: Instant,
    clock_origin: u64,
    ready: mpsc::Sender<Result<(), String>>,
    output: &str,
) -> Result<Option<String>, String> {
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        let message = error.to_string();
        let _ = ready.send(Err(message.clone()));
        return Err(message);
    }
    let _com = Com;
    let mut task = 0;
    let priority =
        unsafe { AvSetMmThreadCharacteristicsW(windows::core::w!("Audio"), &mut task) }.ok();
    if let Some(handle) = priority {
        unsafe {
            let _ = AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH);
        }
    } else {
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
        }
    }
    let _priority = AudioPriority(priority);
    let direction = if source == "system" {
        eRender
    } else {
        eCapture
    };
    let initialized = (|| -> Result<_, String> {
        let watch = EndpointWatch::new(direction, output).map_err(|e| e.to_string())?;
        let (id, device) = watch.endpoint(direction).map_err(|e| {
            if source == "system" && !output.is_empty() { format!("Uscita audio selezionata non disponibile. Ricollegala o scegli un'altra uscita nelle impostazioni. ({e})") }
            else { e.to_string() }
        })?;
        let writer = open_track(file)?;
        let stream = Stream::open(id, &device, source == "system").map_err(|e| e.to_string())?;
        Ok((watch, writer, stream))
    })();
    let (watch, mut writer, stream) = match initialized {
        Ok(value) => {
            let _ = ready.send(Ok(()));
            value
        }
        Err(message) => {
            let _ = ready.send(Err(message.clone()));
            return Err(message);
        }
    };
    let mut stream = Some(stream);
    let mut timeline = PacketTimeline::default();
    let mut reconnect = ReconnectHistory::default();
    let (mut last_flush, mut last_meter, mut next_check) =
        (Instant::now(), Instant::now(), Instant::now());
    let mut meter = LevelMeter::default();
    let frame_now = || (start.elapsed().as_secs_f64() * 16000.) as u64;
    while !stop.load(Ordering::SeqCst) {
        if let Some(current) = &mut stream {
            unsafe {
                WaitForSingleObject(current.event.0, 50);
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let mut lost = false;
        if let Some(current) = &mut stream {
            // Bound draining so route/stop notifications are never starved by a backlog.
            for _ in 0..64 {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let packet = match current.packet() {
                    Ok(Some(packet)) => packet,
                    Ok(None) => break,
                    Err(error) if recoverable(&error) => {
                        lost = true;
                        break;
                    }
                    Err(error) => return Err(error.to_string()),
                };
                let valid = (packet.flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0
                    && packet.timestamp >= clock_origin)
                    .then(|| {
                        ((packet.timestamp - clock_origin) as u128 * 16000 / 10_000_000) as u64
                    });
                let device_position = (packet.flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0)
                    .then_some(packet.device_position);
                let expected = timeline.device_start_frame(valid, device_position, frame_now(), packet.samples.len() as u64);
                timeline.observe_device_position(
                    device_position,
                    packet.samples.len() as u64,
                    packet.flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0,
                );
                let plan = timeline.bounded_packet(
                    expected,
                    packet.samples.len(),
                    frame_now(),
                    packet.flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0,
                )?;
                for _ in 0..plan.padding {
                    writer.write_sample(0i16).map_err(|e| e.to_string())?;
                }
                for sample in &packet.samples[plan.skip..] {
                    let value = if sample.is_finite() {
                        sample.clamp(-1., 1.)
                    } else {
                        0.
                    };
                    meter.add(value);
                    writer
                        .write_sample((value * 32767.) as i16)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        if lost {
            reconnect.begin(frame_now());
            drop(stream.take());
            meter = LevelMeter::default();
            watch.changed.store(true, Ordering::SeqCst);
        }
        // Drain queued packets before querying endpoint COM state. Device
        // discovery can block; it must not delay releasing capture buffers.
        if watch.changed.swap(false, Ordering::SeqCst) || Instant::now() >= next_check {
            next_check =
                Instant::now() + Duration::from_millis(if stream.is_some() { 500 } else { 250 });
            match watch.endpoint(direction) {
                Ok((id, device)) if stream.as_ref().is_none_or(|current| current.id != id) => {
                    reconnect.begin(frame_now());
                    // Release only this endpoint; the other source runs independently.
                    drop(stream.take());
                    meter = LevelMeter::default();
                    if let Ok(next) = Stream::open(id, &device, source == "system") {
                        timeline.begin_stream();
                        reconnect.end(frame_now());
                        stream = Some(next);
                    }
                }
                Err(_) => {
                    reconnect.begin(frame_now());
                    drop(stream.take());
                    meter = LevelMeter::default();
                }
                _ => {}
            }
        }
        if last_flush.elapsed() >= Duration::from_secs(1) {
            writer.flush().map_err(|e| e.to_string())?;
            last_flush = Instant::now();
        }
        if last_meter.elapsed() >= Duration::from_millis(150) {
            let (level, dbfs) = meter.read();
            let _ = app.emit(
                "audio-level",
                serde_json::json!({"source":source,"level":level,"dbfs":dbfs}),
            );
            // Repeated state reconciles minimized WebViews without stopping native capture.
            let _ = app.emit(
                "audio-route",
                serde_json::json!({"source":source,"reconnecting":stream.is_none()}),
            );
            last_meter = Instant::now();
        }
    }
    let clock_rates = stream.as_ref().map(|current| serde_json::json!({
        "endpoint_rate":current.position_clock.native_rate, "position_rate":current.position_clock.rate,
        "calibrated":current.position_clock.calibrated,
    }));
    drop(stream);
    let final_frames = frame_now();
    for _ in timeline.written..final_frames {
        writer.write_sample(0i16).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    // Preserve metadata before temporary audio is deleted after transcription.
    // Diagnostics contain counters/times only, never audio or transcript text.
    let still_missing = reconnect.since.is_some();
    let routing_warning = reconnect.warning(source, final_frames);
    let diagnostics = serde_json::json!({
        "version": 1, "source": source, "duration_ms": final_frames / 16,
        "written_frames": timeline.written, "sample_rate": 16000,
        "clock_rates": clock_rates,
        "packets": timeline.diagnostics(),
        "routing": {"reconnections": reconnect.count, "unavailable_ms": reconnect.frames as f64 / 16.,
            "longest_unavailable_ms": reconnect.longest as f64 / 16., "missing_at_stop": still_missing},
    });
    if let Err(error) = std::fs::write(file.with_extension("diagnostics.json"), diagnostics.to_string()) {
        eprintln!("Diagnostica cattura non salvata ({source}): {error}");
    }
    let warnings = [
        timeline.warning(source),
        routing_warning,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    Ok((!warnings.is_empty()).then(|| warnings.join(" ")))
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Requires default Windows audio devices and a 997 Hz test tone on the default output"]
    fn default_devices_deliver_wasapi_packets() {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.unwrap();
        let _com = Com;
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.unwrap();
        let mic = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) }.unwrap();
        let output = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }.unwrap();
        let mut mic = Stream::open(String::new(), &mic, false).unwrap();
        let mut loopback = Stream::open(String::new(), &output, true).unwrap();
        let end = Instant::now() + Duration::from_secs(4);
        let mut mic_frames = 0;
        let mut loopback_frames = 0;
        let mut tone_detected = false;
        let phase_step = std::f64::consts::TAU * 997.0 / 16000.0;
        while Instant::now() < end {
            // The microphone check keeps only a frame count.
            while let Some(packet) = mic.packet().unwrap() {
                mic_frames += packet.samples.len();
            }
            while let Some(packet) = loopback.packet().unwrap() {
                let (mut cosine, mut sine) = (0.0, 0.0);
                for (index, sample) in packet.samples.iter().enumerate() {
                    let (s, c) = (index as f64 * phase_step).sin_cos();
                    cosine += f64::from(*sample) * c;
                    sine += f64::from(*sample) * s;
                }
                loopback_frames += packet.samples.len();
                if packet.samples.len() >= 128 {
                    tone_detected |= cosine.hypot(sine) / packet.samples.len() as f64 > 0.002;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(mic_frames >= 16000, "Microphone frames: {mic_frames}");
        assert!(loopback_frames >= 16000, "Loopback frames: {loopback_frames}");
        assert!(tone_detected, "Loopback missed the synthetic tone");
        println!("WASAPI frames: microphone={mic_frames}, loopback={loopback_frames}");
    }

    #[test]
    fn output_route_supports_default_communications_and_fixed_device() {
        use super::*;
        assert_eq!(output_role(eRender, ""), Some(eConsole));
        assert_eq!(output_role(eRender, "@communications"), Some(eCommunications));
        assert_eq!(output_role(eRender, "synthetic-endpoint"), None);
        assert_eq!(output_role(eCapture, "synthetic-endpoint"), Some(eConsole));
        assert_eq!(output_role(eCapture, "@communications"), Some(eConsole));
    }
    use super::*;
    #[test]
    fn fast_route_switch_is_diagnostic_but_missing_at_stop_still_warns() {
        let mut history = ReconnectHistory::default();
        history.begin(0);
        history.end(1600);
        assert!(history.warning("system", 16000).is_none());
        assert_eq!(history.count, 1);
        assert_eq!(history.frames, 1600);
        history.begin(16000);
        assert!(history.warning("system", 16160).unwrap().contains("ancora indisponibile"));
    }
    #[test]
    fn output_switch_notification_does_not_switch_the_microphone() {
        let output = Arc::new(AtomicBool::new(false));
        let input = Arc::new(AtomicBool::new(false));
        let out: IMMNotificationClient = EndpointNotifications {
            direction: eRender,
            changed: output.clone(),
        }
        .into();
        let mic: IMMNotificationClient = EndpointNotifications {
            direction: eCapture,
            changed: input.clone(),
        }
        .into();
        unsafe {
            out.OnDefaultDeviceChanged(eRender, eConsole, PCWSTR::null())
                .unwrap();
            mic.OnDefaultDeviceChanged(eRender, eConsole, PCWSTR::null())
                .unwrap();
        }
        assert!(output.swap(false, Ordering::SeqCst));
        assert!(!input.load(Ordering::SeqCst));
        unsafe {
            out.OnDefaultDeviceChanged(eRender, eCommunications, PCWSTR::null())
                .unwrap();
        }
        assert!(output.swap(false, Ordering::SeqCst));
        assert!(!input.load(Ordering::SeqCst));
        unsafe {
            out.OnDefaultDeviceChanged(eRender, eMultimedia, PCWSTR::null()).unwrap();
        }
        assert!(!output.load(Ordering::SeqCst));
        unsafe {
            mic.OnDefaultDeviceChanged(eCapture, eConsole, PCWSTR::null())
                .unwrap();
        }
        assert!(input.load(Ordering::SeqCst));
    }
    #[test]
    fn unavailable_period_is_counted_once_and_stop_reports_missing_source() {
        let mut history = ReconnectHistory::default();
        history.begin(16000);
        history.begin(24000);
        history.end(32000);
        history.begin(48000);
        history.begin(56000);
        let warning = history.warning("system", 64000).unwrap();
        assert_eq!(history.count, 2);
        assert_eq!(history.frames, 32000);
        assert!(warning.contains("2.00 s"));
        assert!(warning.contains("00:01, 00:03"));
        assert!(warning.contains("ancora indisponibile"));
        let mut history = ReconnectHistory::default();
        history.begin(0);
        history.end(8000);
        assert!(history
            .warning("microphone", 16000)
            .unwrap()
            .contains("ripristinata"));
        assert!(ReconnectHistory::default()
            .warning("system", 16000)
            .is_none());
    }
    #[test]
    fn only_native_device_service_invalidation_is_recoverable_read_error() {
        for code in [
            AUDCLNT_E_DEVICE_INVALIDATED,
            AUDCLNT_E_RESOURCES_INVALIDATED,
            AUDCLNT_E_SERVICE_NOT_RUNNING,
        ] {
            assert!(recoverable(&windows::core::Error::from_hresult(code)));
        }
        assert!(!recoverable(&windows::core::Error::from_hresult(
            windows::Win32::Foundation::E_POINTER
        )));
    }
}
