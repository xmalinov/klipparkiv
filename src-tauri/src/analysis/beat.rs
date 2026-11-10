use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use beat_this::{BeatThis, RtenRuntime};

use crate::analysis::models::ModelPaths;

const SAMPLE_RATE: u32 = 22_050;
const READ_BUFFER: usize = 64 * 1024;

pub struct BeatRun {
    pub beats: Vec<f32>,
    pub downbeats: Vec<f32>,
}

/// Decode the file with ffmpeg and feed mono f32 chunks to Beat This.
///
/// The stream keeps only the mel spectrogram plus one chunk, so long tracks do
/// not require the whole PCM buffer in memory.
pub fn analyze(
    ffmpeg: &Path,
    models: &ModelPaths,
    audio: &Path,
    duration_secs: Option<f64>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32),
) -> Result<BeatRun, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("analysis cancelled".into());
    }

    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(audio)
        .args([
            "-map", "0:a:0", "-vn", "-ac", "1", "-ar", "22050", "-f", "f32le", "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::process::hide_console_std(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start ffmpeg for analysis: {error}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg analysis stdout is unavailable".to_string())?;

    let mut tracker = BeatThis::new(&RtenRuntime, &models.mel, &models.beat)
        .map_err(|error| format!("cannot load beat grid model: {error}"))?;
    progress(0.08);

    let mut stream = tracker
        .stream(SAMPLE_RATE)
        .map_err(|error| format!("cannot start beat grid inference: {error}"))?;

    let expected_bytes = duration_secs
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value * SAMPLE_RATE as f64 * 4.0);

    let mut buffer = vec![0u8; READ_BUFFER];
    let mut carry: Vec<u8> = Vec::new();
    let mut decoded_bytes = 0u64;
    let mut last_progress = 0.08f32;

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("analysis cancelled".into());
        }

        let read = stdout
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        carry.extend_from_slice(&buffer[..read]);
        decoded_bytes += read as u64;

        let complete = carry.len() / 4;
        if complete > 0 {
            let mut samples = Vec::with_capacity(complete);
            for chunk in carry[..complete * 4].as_chunks::<4>().0 {
                samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
            stream
                .push(&samples)
                .map_err(|error| format!("beat grid inference failed: {error}"))?;
            carry.drain(..complete * 4);
        }

        if let Some(expected) = expected_bytes {
            let ratio = (decoded_bytes as f64 / expected).clamp(0.0, 1.0) as f32;
            let next = 0.08 + ratio * 0.82;
            if next - last_progress >= 0.02 {
                last_progress = next;
                progress(next);
            }
        }
    }

    let analysis = stream
        .finish()
        .map_err(|error| format!("beat grid inference failed: {error}"))?;
    let status = child.wait().map_err(|error| error.to_string())?;
    if !status.success() && analysis.beats.is_empty() {
        return Err("ffmpeg could not decode the track for analysis".into());
    }
    progress(0.95);

    Ok(BeatRun {
        beats: analysis.beats,
        downbeats: analysis.downbeats,
    })
}
