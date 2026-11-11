use crate::LockExt;
use std::{fs, path::{Path, PathBuf}, sync::Mutex};

type Result<T> = std::result::Result<T, String>;
static EXPORT: Mutex<()> = Mutex::new(());
const TRACKS: [&str; 2] = ["microphone.wav", "system.wav"];

fn reader(path: &Path) -> Result<hound::WavReader<std::io::BufReader<fs::File>>> {
    let reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16000 || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int {
        return Err("Formato audio registrato non supportato".into());
    }
    Ok(reader)
}

// Stream both synchronized tracks, padding the shorter one with silence.
// A first pass computes headroom so overlapping voices never clip.
fn mixed_samples(paths: &[PathBuf], mut write: impl FnMut(i32) -> Result<()>) -> Result<()> {
    let mut readers = paths.iter().map(|path| reader(path)).collect::<Result<Vec<_>>>()?;
    let mut samples: Vec<_> = readers.iter_mut().map(|r| r.samples::<i16>()).collect();
    loop {
        let mut present = false;
        let mut sum = 0i32;
        for track in &mut samples {
            if let Some(sample) = track.next() {
                present = true;
                sum += sample.map_err(|e| e.to_string())? as i32;
            }
        }
        if !present { break; }
        write(sum)?;
    }
    Ok(())
}

pub fn consolidate(directory: &Path, discard_sources: bool) -> Result<PathBuf> {
    let _guard = EXPORT.guard();
    let output = directory.join("registrazione.wav");
    if !output.is_file() {
        let paths: Vec<_> = TRACKS.iter().map(|name| directory.join(name)).filter(|p| p.is_file()).collect();
        if paths.is_empty() { return Err("Audio conservato non disponibile".into()); }
        let mut peak = 0i32;
        mixed_samples(&paths, |sample| { peak = peak.max(sample.abs()); Ok(()) })?;
        let gain = if peak > 32000 { 32000. / peak as f64 } else { 1. };
        let temporary = directory.join(format!("registrazione-{}.part", uuid::Uuid::new_v4()));
        let result = (|| {
            let spec = hound::WavSpec { channels: 1, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
            let mut writer = hound::WavWriter::create(&temporary, spec).map_err(|e| e.to_string())?;
            mixed_samples(&paths, |sample| writer.write_sample((sample as f64 * gain).round() as i16).map_err(|e| e.to_string()))?;
            writer.finalize().map_err(|e| e.to_string())?;
            fs::OpenOptions::new().write(true).open(&temporary).and_then(|file| file.sync_all()).map_err(|e| e.to_string())?;
            fs::rename(&temporary, &output).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }
    // Only discard the source tracks after the complete WAV has been committed.
    if discard_sources {
        for name in TRACKS {
            let path = directory.join(name);
            if path.is_file() { fs::remove_file(path).map_err(|e| e.to_string())?; }
        }
    }
    Ok(output)
}
