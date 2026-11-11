//! Initializes default audio endpoints without starting capture or reading any audio.
#[cfg(windows)]
pub fn audio_formats() -> Result<(), String> {
    use windows::Win32::{Media::Audio::*, System::Com::*};
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _guard = ComGuard;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| e.to_string())?;
    for (direction, label) in [(eCapture, "microfono"), (eRender, "audio di sistema")] {
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(direction, eConsole) }
            .map_err(|e| format!("{label}: {e}"))?;
        let client: IAudioClient =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| e.to_string())?;
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
            | if direction == eRender {
                AUDCLNT_STREAMFLAGS_LOOPBACK
            } else {
                0
            };
        unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 5_000_000, 0, &format, None) }
            .map_err(|e| format!("Formato {label}: {e}"))?;
        // No Start(), GetBuffer() or microphone recording is performed by this diagnostic.
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn audio_formats() -> Result<(), String> {
    Err("Diagnostica WASAPI disponibile su Windows".into())
}
