use crate::model::{Capture, Settings};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Instant,
};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
struct LevelMeter {
    sum: f64,
    count: usize,
}
impl LevelMeter {
    fn add(&mut self, sample: f32) {
        self.sum += (sample as f64).powi(2);
        self.count += 1;
    }
    fn read(&mut self) -> (f64, f64) {
        let rms = (self.sum / self.count.max(1) as f64).sqrt();
        let dbfs = if rms > 0. { 20. * rms.log10() } else { -60. };
        self.sum = 0.;
        self.count = 0;
        let dbfs = dbfs.clamp(-60., 0.);
        ((dbfs + 60.) / 60., dbfs)
    }
}

fn open_track(file: &Path) -> Result<hound::WavWriter<std::io::BufWriter<std::fs::File>>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file, spec).map_err(|e| e.to_string())?;
    // Loopback can receive no packets for an entire call. Publish a valid empty
    // header before marking the source ready so the preview can read both tracks.
    writer.flush().map_err(|e| e.to_string())?;
    Ok(writer)
}

pub struct Recording {
    pub info: Capture,
    pub stop: Arc<AtomicBool>,
    pub start: Instant,
    pub handles: Vec<thread::JoinHandle<Result<Option<String>, String>>>,
    pub dir: PathBuf,
    pub video_input: Option<Arc<std::sync::Mutex<std::process::ChildStdin>>>,
}
#[derive(serde::Serialize)]
pub struct AudioOutput { pub id: String, pub name: String }
pub fn outputs() -> Result<Vec<AudioOutput>, String> {
    #[cfg(windows)]
    { windows_capture::outputs().map_err(|e|e.to_string()) }
    #[cfg(not(windows))]
    { Ok(vec![]) }
}
impl Recording {
    pub fn finish(self) -> Result<(Capture, i64, PathBuf, Option<String>), String> {
        self.stop.store(true, Ordering::SeqCst);
        let duration = self.start.elapsed().as_millis() as i64;
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        for handle in self.handles {
            match handle.join() {
                Ok(Ok(Some(warning))) => warnings.push(warning),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => errors.push(e),
                Err(_) => errors.push("Thread audio interrotto".into()),
            }
        }
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        Ok((
            self.info,
            duration,
            self.dir,
            if warnings.is_empty() {
                None
            } else if warnings.iter().any(|warning| warning.starts_with("Video:")) {
                Some(format!("Discontinuità registrazione: {}", warnings.join(" ")))
            } else {
                Some(format!("Discontinuità audio: {}", warnings.join(" ")))
            },
        ))
    }
}
pub fn start(
    app: AppHandle,
    settings: &Settings,
    info: Capture,
    dir: PathBuf,
) -> Result<Recording, String> {
    #[cfg(target_os = "macos")]
    return crate::mac_audio::start(app, settings, info, dir);
    #[cfg(not(any(windows, target_os = "macos")))]
    return Err("Cattura audio disponibile su Windows e macOS.".into());
    #[cfg(windows)]
    {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let mut handles = Vec::new();
        let clock_origin = clock_hns()?;
        let (tx, rx) = mpsc::channel();
        for source in ["microphone", "system"] {
            if (source == "microphone" && !settings.microphone)
                || (source == "system" && !settings.system_audio)
            {
                continue;
            }
            let tx = tx.clone();
            let app = app.clone();
            let stop = stop.clone();
            let file = dir.join(format!("{source}.wav"));
            let output = settings.system_audio_output.clone();
            handles.push(thread::spawn(move || {
                let result = capture(&app, source, &file, stop.clone(), start, clock_origin, tx, &output);
                if let Err(ref e) = result {
                    stop.store(true, Ordering::SeqCst);
                    let _ = app.emit(
                        "app-error",
                        format!("Registrazione interrotta ({source}): {e}"),
                    );
                }
                result
            }));
        }
        drop(tx);
        for _ in 0..handles.len() {
            match rx.recv_timeout(std::time::Duration::from_secs(8)) {
                Ok(Ok(())) => {}
                other => {
                    stop.store(true, Ordering::SeqCst);
                    for h in handles {
                        let _ = h.join();
                    }
                    return Err(match other {
                        Ok(Err(e)) => e,
                        _ => "La sorgente audio non risponde.".into(),
                    });
                }
            }
        }
        Ok(Recording {
            info,
            stop,
            start,
            handles,
            dir,
            video_input: None,
        })
    }
}
#[cfg(windows)]
fn clock_hns() -> Result<u64, String> {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let mut counter = 0i64;
    let mut frequency = 0i64;
    unsafe {
        QueryPerformanceCounter(&mut counter).map_err(|e| e.to_string())?;
        QueryPerformanceFrequency(&mut frequency).map_err(|e| e.to_string())?;
    }
    Ok((counter as u128 * 10_000_000 / frequency as u128) as u64)
}
#[cfg(not(windows))]
fn clock_hns() -> Result<u64, String> {
    Ok(0)
}

#[cfg(windows)]
#[path = "audio_windows.rs"]
mod windows_capture;
#[cfg(windows)]
fn capture(
    app: &AppHandle,
    source: &str,
    file: &Path,
    stop: Arc<AtomicBool>,
    start: Instant,
    clock_origin: u64,
    ready: mpsc::Sender<Result<(), String>>,
    output: &str,
) -> Result<Option<String>, String> {
    windows_capture::capture(app, source, file, stop, start, clock_origin, ready, output)
}
#[derive(Default)]
struct PacketTimeline {
    written: u64,
    seen: bool,
    fresh_stream: bool,
    glitches: u64,
    gap_frames: u64,
    gap_count: u64,
    gap_positions: Vec<u64>,
    device_end: Option<u64>,
    missing_frames: u64,
    missing_count: u64,
    longest_missing: u64,
    missing_positions: Vec<u64>,
    long_missing_positions: Vec<u64>,
    uncertain_positions: u64,
    clock_mismatch_packets: u64,
    largest_clock_delta: u64,
}
// AUTOCONVERTPCM can return 16 kHz samples with positions still counted in
// endpoint frames (48 kHz on the tested Realtek microphone).
struct DeviceFrameClock {
    native_rate: u32,
    rate: u32,
    calibrated: bool,
    previous: Option<(u64, u64, u64)>,
    position: u64,
    remainder: u64,
}
impl DeviceFrameClock {
    fn new(native_rate: u32) -> Self {
        let native_rate = native_rate.max(1);
        Self { native_rate, rate: native_rate, calibrated: native_rate == 16000,
            previous: None, position: 0, remainder: 0 }
    }
    fn normalize(&mut self, raw: u64, qpc: u64, frames: u64, flags: u32) -> u64 {
        if flags & 4 != 0 { return self.position; }
        if let Some((previous, timestamp, previous_frames)) = self.previous {
            if raw >= previous {
                let delta = raw - previous;
                // Some drivers already use the requested format's units.
                // Check a continuous packet against QPC once to distinguish it.
                if !self.calibrated && flags & 5 == 0 && qpc > timestamp {
                    let expected = ((qpc - timestamp) as u128 * 16000 / 10_000_000) as u64;
                    let native = (delta as u128 * 16000 / self.native_rate as u128) as u64;
                    let tolerance = (expected / 10).max(4);
                    if expected >= 16 && expected <= 16000 && expected.abs_diff(previous_frames) <= tolerance {
                        if delta.abs_diff(expected) <= tolerance && delta.abs_diff(expected) <= native.abs_diff(expected) {
                            if self.rate != 16000 { self.remainder = 0; }
                            self.rate = 16000;
                            self.calibrated = true;
                        } else if native.abs_diff(expected) <= tolerance {
                            self.rate = self.native_rate;
                            self.calibrated = true;
                        }
                    }
                }
                let scaled = delta as u128 * 16000 + self.remainder as u128;
                self.position = self.position.saturating_add((scaled / self.rate as u128).min(u64::MAX as u128) as u64);
                self.remainder = (scaled % self.rate as u128) as u64;
            } else {
                self.position = self.position.saturating_add(previous_frames);
                self.remainder = 0;
            }
        }
        self.previous = Some((raw, qpc, frames));
        self.position
    }
}
struct PacketPlan {
    padding: u64,
    skip: usize,
}
impl PacketTimeline {
    fn begin_stream(&mut self) {
        // Retain the call timeline; only the new stream's initial packet is new.
        self.fresh_stream = true;
        self.device_end = None;
    }
    fn observe_device_position(&mut self, position: Option<u64>, frames: u64, discontinuity: bool) {
        // QPC spacing also contains clock jitter. Only the stream's frame
        // counter can corroborate missing samples; a flag alone cannot.
        if let Some(position) = position {
            if let Some(end) = self.device_end {
                let missing = position.saturating_sub(end);
                if discontinuity && missing > 160 {
                    self.missing_frames += missing;
                    self.missing_count += 1;
                    self.longest_missing = self.longest_missing.max(missing);
                    if self.missing_positions.len() < 6 {
                        self.missing_positions.push(self.written);
                    }
                    if missing >= 4000 && self.long_missing_positions.len() < 6 {
                        self.long_missing_positions.push(self.written);
                    }
                }
            }
            self.device_end = Some(position.saturating_add(frames));
        } else {
            self.uncertain_positions += 1;
            // Do not charge an uncertain packet as lost audio on the next read.
            self.device_end = None;
        }
    }
    fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({
            "device_flags": self.glitches,
            "timeline_padding_intervals": self.gap_count,
            "timeline_padding_ms": self.gap_frames as f64 / 16.,
            "timeline_padding_positions_ms": self.gap_positions.iter().map(|f|f/16).collect::<Vec<_>>(),
            "missing_sample_intervals": self.missing_count,
            "missing_sample_ms": self.missing_frames as f64 / 16.,
            "longest_missing_sample_ms": self.longest_missing as f64 / 16.,
            "missing_sample_positions_ms": self.missing_positions.iter().map(|f|f/16).collect::<Vec<_>>(),
            "uncertain_position_packets": self.uncertain_positions,
            "clock_mismatch_packets": self.clock_mismatch_packets,
            "largest_clock_delta_ms": self.largest_clock_delta as f64 / 16.,
        })
    }
    fn start_frame(&self, timestamp: Option<u64>, arrival: u64, frames: u64) -> u64 {
        // Invalid timestamps must not turn scheduling jitter into discarded speech.
        timestamp.unwrap_or_else(|| {
            if self.seen && !self.fresh_stream {
                self.written
            } else {
                arrival.saturating_sub(frames)
            }
        })
    }
    fn device_start_frame(&mut self, timestamp: Option<u64>, device_position: Option<u64>, arrival: u64, frames: u64) -> u64 {
        if let (Some(position), Some(end)) = (device_position, self.device_end) {
            // GetBuffer consumes each packet once. Do not discard queued speech
            // or insert gaps merely because successive QPC readings jitter.
            // A positive frame-counter gap preserves actual stream spacing.
            let expected = self.written.saturating_add(position.saturating_sub(end));
            if let Some(timestamp) = timestamp {
                let delta = timestamp.abs_diff(expected);
                self.largest_clock_delta = self.largest_clock_delta.max(delta);
                if delta > 160 { self.clock_mismatch_packets += 1; }
            }
            // Hardware mute/device resets can return a wildly advanced frame
            // counter without setting TIMESTAMP_ERROR. Never pad toward a time
            // beyond the audio that could have arrived in real time.
            return if expected > arrival.saturating_add(16000) {
                self.written.max(arrival.saturating_sub(frames))
            } else { expected };
        }
        let start = self.start_frame(timestamp, arrival, frames);
        let start = if start > arrival.saturating_add(16000) { arrival.saturating_sub(frames) } else { start };
        if self.seen && !self.fresh_stream { start.max(self.written) } else { start }
    }
    fn bounded_packet(&mut self, expected_start:u64, frames:usize, arrival:u64, discontinuity:bool) -> Result<PacketPlan,String> {
        if self.written.max(expected_start.saturating_add(frames as u64)) > arrival.saturating_add(16000) {
            return Err("Il dispositivo ha restituito tempi audio non validi. Registrazione interrotta per evitare sovraccarico; audio acquisito conservato.".into());
        }
        Ok(self.packet(expected_start,frames,discontinuity))
    }
    fn packet(&mut self, expected_start: u64, frames: usize, discontinuity: bool) -> PacketPlan {
        let had_packets = self.seen && !self.fresh_stream;
        if had_packets && discontinuity {
            self.glitches += 1;
        }
        self.seen = true;
        self.fresh_stream = false;
        let padding = if expected_start > self.written.saturating_add(2) {
            expected_start - self.written
        } else {
            0
        };
        // A driver flag alone doesn't prove lost speech. Measure flagged gaps
        // longer than 10 ms; initial alignment and normal loopback silence are
        // deliberately excluded. Still insert all padding into the WAV.
        if had_packets && discontinuity && padding > 160 {
            self.gap_frames += padding;
            self.gap_count += 1;
            if self.gap_positions.len() < 6 {
                self.gap_positions.push(self.written);
            }
        }
        self.written += padding;
        let skip = self
            .written
            .saturating_sub(expected_start)
            .min(frames as u64) as usize;
        self.written += (frames - skip) as u64;
        PacketPlan { padding, skip }
    }
    fn warning(&self, source: &str) -> Option<String> {
        // Product thresholds, not a guarantee of transcript accuracy. Tiny
        // driver glitches remain diagnostic; sustained outages deserve review.
        if self.longest_missing < 4000 && self.missing_frames < 48000 {
            return None;
        }
        let source = if source == "microphone" {
            "Microfono"
        } else {
            "Audio PC"
        };
        let warning_positions = if self.long_missing_positions.is_empty() {
            &self.missing_positions
        } else { &self.long_missing_positions };
        let positions = warning_positions
                .iter()
                .map(|frame| {
                    let seconds = frame / 16000;
                    format!("{:02}:{:02}", seconds / 60, seconds % 60)
                })
                .collect::<Vec<_>>()
                .join(", ");
        Some(format!(
                "{source}: interruzioni audio per {:.2} s complessivi. Verifica vicino a {}{}.",
                self.missing_frames as f64 / 16000.,
                positions,
                if self.missing_count > warning_positions.len() as u64 {
                    " e altri punti"
                } else {
                    ""
                }
            ))
    }
}

#[cfg(test)]
mod meter_tests {
    #[test]
    fn corrupted_driver_times_cannot_request_unbounded_padding() {
        let mut timeline = super::PacketTimeline::default();
        let expected = timeline.device_start_frame(Some(u64::MAX), Some(0), 160, 160);
        let plan = timeline.bounded_packet(expected, 160, 160, false).unwrap();
        assert_eq!(plan.padding, 0);
        timeline.observe_device_position(Some(0), 160, false);
        let expected = timeline.device_start_frame(Some(u64::MAX), Some(u64::MAX), 320, 160);
        let plan = timeline.bounded_packet(expected, 160, 320, true).unwrap();
        assert_eq!(plan.padding, 0);
        assert_eq!(timeline.written, 320);
        let before = timeline.written;
        assert!(timeline.bounded_packet(u64::MAX, 160, 320, true).is_err());
        assert_eq!(timeline.written, before);
    }
    #[test]
    fn runaway_packets_stop_before_writing_and_overlap_is_allowed() {
        let mut timeline = super::PacketTimeline::default();
        for _ in 0..100 {
            timeline.bounded_packet(timeline.written, 160, 0, false).unwrap();
        }
        assert_eq!(timeline.written, 16000);
        assert!(timeline.bounded_packet(16000, 160, 0, false).is_err());
        assert_eq!(timeline.written, 16000);
        assert_eq!(timeline.bounded_packet(0, 160, 0, false).unwrap().skip, 160);
    }
    #[test]
    fn native_48khz_positions_do_not_insert_silence_into_16khz_speech() {
        let mut clock = super::DeviceFrameClock::new(48000);
        let mut timeline = super::PacketTimeline::default();
        let mut output = Vec::new();
        for i in 0..500u64 {
            let position = clock.normalize(i * 480, i * 100000, 160, 0);
            let expected = timeline.device_start_frame(Some(i * 160), Some(position), i * 160 + 160, 160);
            timeline.observe_device_position(Some(position), 160, false);
            let plan = timeline.packet(expected, 160, false);
            assert_eq!(plan.padding, 0);
            assert_eq!(plan.skip, 0);
            output.extend(std::iter::repeat_n(123i16, 160));
        }
        assert_eq!(output.len(), 80000);
        assert_eq!(timeline.written, 80000);
        assert_eq!(timeline.clock_mismatch_packets, 0);
        assert_eq!(clock.rate, 48000);
        let position=clock.normalize(500*480+960,500*100000+200000,160,1);
        assert_eq!(position,80320); // A genuine 20 ms gap stays 20 ms.
        let expected=timeline.device_start_frame(Some(80320),Some(position),80480,160);
        assert_eq!(timeline.packet(expected,160,true).padding,320);
    }
    #[test]
    fn converted_positions_and_fractional_44100hz_positions_keep_correct_units() {
        let mut converted=super::DeviceFrameClock::new(48000);
        let mut fractional=super::DeviceFrameClock::new(44100);
        for i in 0..500u64 {
            assert_eq!(converted.normalize(i*160,i*100000,160,0),i*160);
            assert_eq!(fractional.normalize(i*441,i*100000,160,0),i*160);
        }
        assert_eq!(converted.rate,16000);
        assert_eq!(fractional.rate,44100);
        assert_eq!(converted.normalize(0,500*100000,160,0),500*160);
        assert_eq!(converted.normalize(u64::MAX,0,160,4),500*160);
        assert_eq!(converted.normalize(160,501*100000,160,0),501*160);
    }
    use super::*;
    #[test]
    fn replacing_endpoint_keeps_wav_content_and_alignment_even_without_new_timestamp() {
        let file =
            std::env::temp_dir().join(format!("parlo-route-{}.wav", uuid::Uuid::new_v4()));
        let mut writer = open_track(&file).unwrap();
        let mut timeline = PacketTimeline::default();
        let before = vec![123i16; 1600];
        let plan = timeline.packet(0, before.len(), true);
        for sample in &before[plan.skip..] {
            writer.write_sample(*sample).unwrap();
        }
        timeline.begin_stream();
        // The replacement endpoint has a new session but shares the call clock.
        let after = vec![456i16; 1600];
        let start = timeline.start_frame(None, 4800, after.len() as u64);
        assert_eq!(start, 3200);
        let plan = timeline.packet(start, after.len(), true);
        assert_eq!(plan.padding, 1600);
        assert_eq!(plan.skip, 0);
        for _ in 0..plan.padding {
            writer.write_sample(0i16).unwrap();
        }
        for sample in &after[plan.skip..] {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
        let mut reader = hound::WavReader::open(&file).unwrap();
        let samples = reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(&samples[..1600], &before);
        assert!(samples[1600..3200].iter().all(|sample| *sample == 0));
        assert_eq!(&samples[3200..], &after);
        assert_eq!(timeline.written, 4800);
        assert_eq!(timeline.glitches, 0); // New-session driver flags are not packet loss.
        assert_eq!(timeline.start_frame(None, 9999, 160), 4800);
        timeline.packet(4800, 160, true);
        assert_eq!(timeline.glitches, 1); // Later real discontinuities still count.
        drop(reader);
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn uncertain_timestamps_keep_queued_speech_instead_of_trimming_it() {
        let mut timeline = PacketTimeline::default();
        timeline.packet(0, 1600, false);
        for arrival in [1600, 1800, 1400, 3200] {
            let start = timeline.start_frame(None, arrival, 160);
            let plan = timeline.packet(start, 160, false);
            assert_eq!(plan.skip, 0);
            assert_eq!(plan.padding, 0);
        }
        assert_eq!(timeline.written, 2240);
        assert_eq!(timeline.start_frame(Some(2400), 9999, 160), 2400);
    }
    #[test]
    fn audio_continues_after_glitch_with_silence_in_gap_and_no_duplicate_samples() {
        let mut timeline = PacketTimeline::default();
        let mut output = Vec::new();
        for (start, samples, glitch) in [
            (0, vec![1i16, 2, 3], true),
            (6, vec![4, 5], true),
            (7, vec![5, 6], false),
        ] {
            let plan = timeline.packet(start, samples.len(), glitch);
            output.extend(std::iter::repeat_n(0, plan.padding as usize));
            output.extend_from_slice(&samples[plan.skip..]);
        }
        assert_eq!(output, vec![1, 2, 3, 0, 0, 0, 4, 5, 6]);
        assert_eq!(timeline.written, 9);
        assert_eq!(timeline.glitches, 1);
        assert!(timeline.warning("microphone").is_none());
    }
    #[test]
    fn initial_discontinuity_and_regular_loopback_silence_are_not_capture_failures() {
        let mut timeline = PacketTimeline::default();
        assert_eq!(timeline.packet(16000, 160, true).padding, 16000);
        assert!(timeline.warning("system").is_none());
        assert_eq!(timeline.packet(32000, 160, false).padding, 15840);
        assert!(timeline.warning("system").is_none());
    }
    #[test]
    fn driver_flags_are_distinguished_from_measured_gaps() {
        let mut timeline = PacketTimeline::default();
        timeline.packet(16000, 160, true);
        timeline.packet(16160, 160, true);
        assert_eq!(timeline.gap_frames, 0);
        assert!(timeline.warning("microphone").is_none());
        timeline.packet(24320, 160, true);
        assert_eq!(timeline.gap_count, 1);
        assert_eq!(timeline.gap_frames, 8000);
        // QPC gaps alone are recorded, not presented as lost samples.
        assert!(timeline.warning("microphone").is_none());
        timeline.packet(40000, 160, false);
        assert_eq!(timeline.gap_count, 1); // ordinary loopback silence
    }
    #[test]
    fn contiguous_samples_with_jittery_timestamps_do_not_warn() {
        let mut timeline = PacketTimeline::default();
        for i in 0..67 {
            timeline.observe_device_position(Some(i * 160), 160, true);
            timeline.packet(i * 480, 160, true);
        }
        assert_eq!(timeline.glitches, 66);
        assert!(timeline.gap_frames > 16000);
        assert_eq!(timeline.missing_frames, 0);
        assert!(timeline.warning("system").is_none());
        assert_eq!(timeline.diagnostics()["device_flags"], 66);
    }
    #[test]
    fn device_counter_keeps_all_samples_despite_forward_and_backward_qpc_jitter() {
        let mut timeline = PacketTimeline::default();
        for (i, timestamp) in [1600, 2080, 1840, 2400, 2160, 2560].into_iter().enumerate() {
            let position = i as u64 * 160;
            let expected = timeline.device_start_frame(Some(timestamp), Some(position), 99999, 160);
            timeline.observe_device_position(Some(position), 160, true);
            let plan = timeline.packet(expected, 160, true);
            assert_eq!(plan.skip, 0);
            assert_eq!(plan.padding, if i == 0 {1600} else {0});
        }
        assert_eq!(timeline.written, 1600 + 6*160);
        assert!(timeline.clock_mismatch_packets > 0);
        // Counter gaps and endpoint changes still keep both sources aligned.
        let expected = timeline.device_start_frame(Some(0), Some(1280), 99999, 160);
        assert_eq!(expected, timeline.written + 320);
        timeline.begin_stream();
        assert_eq!(timeline.device_start_frame(Some(8000), Some(0), 99999, 160), 8000);
    }
    #[test]
    fn brief_gaps_remain_diagnostic_but_sustained_sample_loss_warns() {
        let mut timeline = PacketTimeline::default();
        timeline.observe_device_position(Some(0), 160, true);
        let mut position = 160;
        for _ in 0..44 {
            position += 280; // 17.5 ms each, 0.77 s total, like the report.
            timeline.observe_device_position(Some(position), 160, true);
            timeline.packet(position, 160, true);
            position += 160;
        }
        assert_eq!(timeline.missing_frames, 12320);
        assert!(timeline.warning("system").is_none());
        position += 8000;
        timeline.observe_device_position(Some(position), 160, true);
        let warning = timeline.warning("system").unwrap();
        assert!(warning.contains("1.27 s"));
        assert!(warning.contains("Verifica vicino"));
        assert!(!warning.contains("segnalazioni"));
    }
    #[test]
    fn silence_stream_changes_and_invalid_positions_do_not_prove_loss() {
        let mut timeline = PacketTimeline::default();
        timeline.observe_device_position(Some(0), 160, true);
        timeline.observe_device_position(Some(16000), 160, false); // silent loopback
        timeline.observe_device_position(None, 160, true);
        timeline.observe_device_position(Some(32000), 160, true);
        timeline.begin_stream();
        timeline.observe_device_position(Some(48000), 160, true);
        timeline.observe_device_position(Some(0), 160, true); // driver counter reset
        assert_eq!(timeline.missing_frames, 0);
        assert_eq!(timeline.uncertain_positions, 1);
        assert!(timeline.warning("microphone").is_none());
    }
    #[test]
    fn many_short_confirmed_gaps_warn_when_cumulative_loss_reaches_three_seconds() {
        let mut timeline = PacketTimeline::default();
        timeline.observe_device_position(Some(0), 160, false);
        let mut position = 160;
        for _ in 0..150 {
            position += 320;
            timeline.observe_device_position(Some(position), 160, true);
            position += 160;
        }
        assert_eq!(timeline.longest_missing, 320);
        assert_eq!(timeline.missing_frames, 48000);
        assert!(timeline.warning("microphone").unwrap().contains("3.00 s"));
    }
    #[test]
    fn track_is_readable_before_first_packet_and_while_writer_remains_open() {
        let file =
            std::env::temp_dir().join(format!("parlo-track-{}.wav", uuid::Uuid::new_v4()));
        let mut writer = open_track(&file).unwrap();
        let reader = hound::WavReader::open(&file).unwrap();
        assert_eq!(reader.duration(), 0);
        assert_eq!(reader.spec().sample_rate, 16000);
        drop(reader);
        writer.write_sample(123i16).unwrap();
        writer.flush().unwrap();
        let mut reader = hound::WavReader::open(&file).unwrap();
        assert_eq!(reader.duration(), 1);
        assert_eq!(reader.samples::<i16>().next().unwrap().unwrap(), 123);
        drop(reader);
        writer.finalize().unwrap();
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn normal_voice_uses_the_db_scale_and_all_packets_are_averaged() {
        let mut meter = LevelMeter::default();
        assert_eq!(meter.read(), (0., -60.));
        for _ in 0..100 {
            meter.add(0.1);
        }
        let (level, dbfs) = meter.read();
        assert!((dbfs + 20.).abs() < 0.01);
        assert!((level - 2. / 3.).abs() < 0.01);
        assert_eq!(meter.read(), (0., -60.));
        meter.add(1.);
        for _ in 0..3 {
            meter.add(0.);
        }
        assert!((meter.read().1 + 6.0206).abs() < 0.001);
    }
}
#[cfg(not(windows))]
fn capture(
    _: &AppHandle,
    _: &str,
    _: &Path,
    _: Arc<AtomicBool>,
    _: Instant,
    _: u64,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<Option<String>, String> {
    let e = "Cattura disponibile soltanto su Windows nella versione 0.1".to_string();
    let _ = ready.send(Err(e.clone()));
    Err(e)
}
