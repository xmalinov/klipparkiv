use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use beat_this::{BeatThis, RtenRuntime};

use crate::analysis::models::ModelPaths;

const SAMPLE_RATE: u32 = 22_050;
const READ_BUFFER: usize = 64 * 1024;
const MODEL_FPS: f32 = 50.0;

#[derive(Debug)]
pub struct BeatRun {
    pub beats: Vec<f32>,
    pub downbeats: Vec<f32>,
    pub beat_confidence: Vec<f32>,
    pub downbeat_confidence: Vec<f32>,
}

fn sigmoid(value: f32) -> f32 {
    if value.is_finite() {
        1.0 / (1.0 + (-value).exp())
    } else {
        0.0
    }
}

/// Confidence of a predicted event: the strongest logit within `radius` frames
/// of its timestamp, squashed to 0.0–1.0. The model emits logits at 50 fps.
fn peak_confidence(logits: &[f32], time: f32, radius: isize) -> f32 {
    if logits.is_empty() || !time.is_finite() || time < 0.0 {
        return 0.0;
    }
    let center = (time * MODEL_FPS).round();
    if !center.is_finite() {
        return 0.0;
    }
    let center = center as isize;
    let start = (center - radius).max(0);
    let end = (center + radius).min(logits.len() as isize - 1);
    if start > end {
        return 0.0;
    }
    let mut best = f32::NEG_INFINITY;
    for index in start..=end {
        let value = logits[index as usize];
        if value.is_finite() && value > best {
            best = value;
        }
    }
    if best == f32::NEG_INFINITY {
        0.0
    } else {
        sigmoid(best)
    }
}

/// Expected PCM size for a known duration; `None` when the duration is unusable.
fn expected_analysis_bytes(duration_secs: Option<f64>) -> Option<f64> {
    duration_secs
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value * SAMPLE_RATE as f64 * 4.0)
}

/// Maps decoded bytes to the 0.08–0.90 slice of the overall analysis progress.
fn analysis_progress(decoded_bytes: u64, expected: f64) -> f32 {
    let ratio = (decoded_bytes as f64 / expected).clamp(0.0, 1.0) as f32;
    0.08 + ratio * 0.82
}

/// Decodes complete little-endian f32 frames and keeps the incomplete tail.
fn decode_f32le(carry: &mut Vec<u8>) -> Vec<f32> {
    let complete = carry.len() / 4;
    if complete == 0 {
        return Vec::new();
    }
    let mut samples = Vec::with_capacity(complete);
    for chunk in carry[..complete * 4].as_chunks::<4>().0 {
        samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    carry.drain(..complete * 4);
    samples
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

    let expected_bytes = expected_analysis_bytes(duration_secs);

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
            let samples = decode_f32le(&mut carry);
            stream
                .push(&samples)
                .map_err(|error| format!("beat grid inference failed: {error}"))?;
        }

        if let Some(expected) = expected_bytes {
            let next = analysis_progress(decoded_bytes, expected);
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

    let beat_confidence = analysis
        .beats
        .iter()
        .map(|time| peak_confidence(&analysis.beat_logits, *time, 1))
        .collect();
    let downbeat_confidence = analysis
        .downbeats
        .iter()
        .map(|time| peak_confidence(&analysis.downbeat_logits, *time, 8))
        .collect();

    Ok(BeatRun {
        beats: analysis.beats,
        downbeats: analysis.downbeats,
        beat_confidence,
        downbeat_confidence,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;

    use super::*;

    fn models() -> ModelPaths {
        ModelPaths {
            mel: PathBuf::from("missing-mel.onnx"),
            beat: PathBuf::from("missing-beat.onnx"),
        }
    }

    #[test]
    fn expected_size_requires_a_usable_duration() {
        assert_eq!(expected_analysis_bytes(None), None);
        assert_eq!(expected_analysis_bytes(Some(0.0)), None);
        assert_eq!(expected_analysis_bytes(Some(-3.0)), None);
        assert_eq!(expected_analysis_bytes(Some(f64::NAN)), None);
        assert_eq!(expected_analysis_bytes(Some(f64::INFINITY)), None);
        assert_eq!(
            expected_analysis_bytes(Some(10.0)),
            Some(10.0 * 22_050.0 * 4.0)
        );
    }

    #[test]
    fn progress_stays_inside_the_decode_slice() {
        let expected = 1_000.0;
        assert!((analysis_progress(0, expected) - 0.08).abs() < 1e-6);
        assert!((analysis_progress(500, expected) - 0.49).abs() < 1e-6);
        assert!((analysis_progress(1_000, expected) - 0.90).abs() < 1e-6);
        assert!((analysis_progress(5_000, expected) - 0.90).abs() < 1e-6);
    }

    #[test]
    fn decoding_keeps_an_incomplete_tail_for_the_next_read() {
        let bytes = (-2.5f32).to_le_bytes();
        let mut carry = Vec::new();
        carry.extend_from_slice(&1.0f32.to_le_bytes());
        carry.extend_from_slice(&bytes[..2]);

        assert_eq!(decode_f32le(&mut carry), vec![1.0]);
        assert_eq!(carry, bytes[..2].to_vec());

        carry.extend_from_slice(&bytes[2..]);
        assert_eq!(decode_f32le(&mut carry), vec![-2.5]);
        assert!(carry.is_empty());
    }

    #[test]
    fn decoding_multiple_frames_at_once() {
        let mut carry = Vec::new();
        for sample in [0.5f32, -0.5, 2.0] {
            carry.extend_from_slice(&sample.to_le_bytes());
        }
        assert_eq!(decode_f32le(&mut carry), vec![0.5, -0.5, 2.0]);
        assert!(carry.is_empty());
    }

    #[test]
    fn decoding_without_complete_frames_is_a_no_op() {
        let mut carry = vec![1, 2, 3];
        assert!(decode_f32le(&mut carry).is_empty());
        assert_eq!(carry, vec![1, 2, 3]);

        let mut empty = Vec::new();
        assert!(decode_f32le(&mut empty).is_empty());
        assert!(empty.is_empty());
    }

    #[test]
    fn peak_confidence_reads_the_strongest_logit_near_the_timestamp() {
        let mut logits = vec![-2.0f32; 200];
        logits[100] = 4.0;
        let confidence = peak_confidence(&logits, 2.0, 1);
        assert!(confidence > 0.98);
        assert!((confidence - 1.0 / (1.0 + (-4.0f32).exp())).abs() < 1e-6);

        // One frame of jitter still finds the peak.
        let jittered = peak_confidence(&logits, 2.01, 1);
        assert!(jittered > 0.98);
    }

    #[test]
    fn downbeat_confidence_tolerates_snapping_to_a_beat() {
        let mut logits = vec![-2.0f32; 200];
        logits[100] = 4.0;
        // Downbeats are snapped to the nearest beat, up to ~0.16 s away.
        assert!(peak_confidence(&logits, 2.15, 8) > 0.98);
    }

    #[test]
    fn peak_confidence_handles_missing_and_invalid_input() {
        assert_eq!(peak_confidence(&[], 1.0, 1), 0.0);
        assert_eq!(peak_confidence(&[1.0], f32::NAN, 1), 0.0);
        assert_eq!(peak_confidence(&[1.0], -1.0, 1), 0.0);
        assert_eq!(peak_confidence(&[1.0], 10.0, 1), 0.0);

        let mut logits = vec![0.0f32; 3];
        logits[1] = f32::NAN;
        assert_eq!(peak_confidence(&logits, 0.02, 1), sigmoid(0.0));
    }

    #[test]
    fn peak_confidence_keeps_negative_logits_below_half() {
        let logits = vec![-1.0f32; 10];
        let confidence = peak_confidence(&logits, 0.1, 1);
        assert!(confidence < 0.5);
        assert!(confidence > 0.0);
    }

    #[test]
    fn a_pre_cancelled_analysis_fails_before_starting_ffmpeg() {
        let cancel = AtomicBool::new(true);
        let error = analyze(
            Path::new("definitely-not-ffmpeg"),
            &models(),
            Path::new("missing.wav"),
            Some(10.0),
            &cancel,
            |_| {},
        )
        .unwrap_err();

        assert_eq!(error, "analysis cancelled");
    }
}
