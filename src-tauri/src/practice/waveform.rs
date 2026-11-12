use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const WAVEFORM_VERSION: u32 = 1;
const SAMPLE_RATE: u32 = 8_000;
const BUCKET_MS: f64 = 10.0;
const BUCKET_SAMPLES: usize = (SAMPLE_RATE as f64 * BUCKET_MS / 1000.0) as usize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformLevel {
    pub step_secs: f64,
    pub peaks: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Waveform {
    pub version: u32,
    pub duration_secs: f64,
    pub levels: Vec<WaveformLevel>,
}

pub fn cache_key(path: &Path, mtime: i64, size: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(mtime.to_le_bytes());
    hasher.update(b"|");
    hasher.update(size.to_le_bytes());
    hex::encode(hasher.finalize())
}

pub fn load_or_compute(ffmpeg: &Path, cache_dir: &Path, path: &Path) -> Result<Waveform, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let key = cache_key(path, mtime, metadata.len());
    let cache_path = cache_dir.join(format!("{key}.json"));

    if let Ok(text) = fs::read_to_string(&cache_path)
        && let Ok(waveform) = serde_json::from_str::<Waveform>(&text)
        && waveform.version == WAVEFORM_VERSION
    {
        return Ok(waveform);
    }

    let waveform = compute(ffmpeg, path)?;
    fs::create_dir_all(cache_dir).map_err(|error| error.to_string())?;
    let text = serde_json::to_string(&waveform).map_err(|error| error.to_string())?;
    fs::write(&cache_path, text).map_err(|error| error.to_string())?;
    Ok(waveform)
}

pub fn compute(ffmpeg: &Path, path: &Path) -> Result<Waveform, String> {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(path)
        .args([
            "-map", "0:a:0", "-vn", "-ac", "1", "-ar", "8000", "-f", "f32le", "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::process::hide_console_std(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start ffmpeg for waveform: {error}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg waveform stdout is unavailable".to_string())?;

    let mut base: Vec<i8> = Vec::new();
    let mut carry: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut buckets = BucketBuilder::new();
    let mut total_samples = 0u64;

    loop {
        let read = stdout
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        carry.extend_from_slice(&buffer[..read]);
        let complete = carry.len() / 4;
        for index in 0..complete {
            let start = index * 4;
            let sample = f32::from_le_bytes([
                carry[start],
                carry[start + 1],
                carry[start + 2],
                carry[start + 3],
            ]);
            total_samples += 1;
            buckets.push(sample);
        }
        carry.drain(..complete * 4);
    }

    base.extend(buckets.finish());

    let status = child.wait().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("ffmpeg failed to compute the waveform".into());
    }

    let duration_secs = total_samples as f64 / SAMPLE_RATE as f64;
    let levels = vec![
        WaveformLevel {
            step_secs: 0.01,
            peaks: encode_peaks(&base),
        },
        WaveformLevel {
            step_secs: 0.1,
            peaks: encode_peaks(&derive(&base, 10)),
        },
        WaveformLevel {
            step_secs: 1.0,
            peaks: encode_peaks(&derive(&base, 100)),
        },
    ];

    Ok(Waveform {
        version: WAVEFORM_VERSION,
        duration_secs,
        levels,
    })
}

fn push_bucket(base: &mut Vec<i8>, min: f32, max: f32) {
    base.push(to_i8(min));
    base.push(to_i8(max));
}

/// Groups decoded samples into fixed 10 ms buckets of (min, max) pairs.
struct BucketBuilder {
    base: Vec<i8>,
    min: f32,
    max: f32,
    count: usize,
}

impl BucketBuilder {
    fn new() -> Self {
        Self {
            base: Vec::new(),
            min: 1.0,
            max: -1.0,
            count: 0,
        }
    }

    fn push(&mut self, sample: f32) {
        if sample < self.min {
            self.min = sample;
        }
        if sample > self.max {
            self.max = sample;
        }
        self.count += 1;
        if self.count == BUCKET_SAMPLES {
            push_bucket(&mut self.base, self.min, self.max);
            self.min = 1.0;
            self.max = -1.0;
            self.count = 0;
        }
    }

    fn finish(mut self) -> Vec<i8> {
        if self.count > 0 {
            push_bucket(&mut self.base, self.min, self.max);
        }
        self.base
    }
}

fn encode_peaks(peaks: &[i8]) -> String {
    let bytes: Vec<u8> = peaks.iter().map(|value| *value as u8).collect();
    STANDARD.encode(bytes)
}

fn to_i8(sample: f32) -> i8 {
    (sample.clamp(-1.0, 1.0) * 127.0).round() as i8
}

fn derive(base: &[i8], factor: usize) -> Vec<i8> {
    let buckets = base.len() / 2;
    let groups = buckets.div_ceil(factor);
    let mut output = Vec::with_capacity(groups * 2);
    for group in 0..groups {
        let start = group * factor;
        let end = ((group + 1) * factor).min(buckets);
        let mut min = i8::MAX;
        let mut max = i8::MIN;
        for bucket in start..end {
            min = min.min(base[bucket * 2]);
            max = max.max(base[bucket * 2 + 1]);
        }
        output.push(min);
        output.push(max);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn derives_coarser_levels() {
        let base = vec![-10, 10, -20, 20, -5, 5, -1, 1];
        assert_eq!(derive(&base, 2), vec![-20, 20, -5, 5]);
        assert_eq!(derive(&base, 4), vec![-20, 20]);
    }

    #[test]
    fn derive_handles_an_uneven_last_group() {
        let base = vec![-1, 1, -2, 2, -3, 3, -4, 4, -5, 5];
        assert_eq!(derive(&base, 2), vec![-2, 2, -4, 4, -5, 5]);
        assert_eq!(derive(&base, 20), vec![-5, 5]);
        assert!(derive(&[], 4).is_empty());
    }

    #[test]
    fn cache_key_changes_with_mtime_and_size() {
        let path = Path::new("C:\\media\\song.mp3");
        assert_eq!(cache_key(path, 10, 100), cache_key(path, 10, 100));
        assert_ne!(cache_key(path, 10, 100), cache_key(path, 11, 100));
        assert_ne!(cache_key(path, 10, 100), cache_key(path, 10, 101));
        assert_ne!(
            cache_key(path, 10, 100),
            cache_key(Path::new("C:\\media\\other.mp3"), 10, 100)
        );
    }

    #[test]
    fn buckets_group_samples_and_flush_the_tail() {
        let mut builder = BucketBuilder::new();
        for _ in 0..BUCKET_SAMPLES {
            builder.push(0.5);
        }
        for _ in 0..5 {
            builder.push(-0.5);
        }
        assert_eq!(builder.finish(), vec![64, 64, -64, -64]);

        assert!(BucketBuilder::new().finish().is_empty());
    }

    #[test]
    fn buckets_clamp_out_of_range_samples() {
        let mut builder = BucketBuilder::new();
        builder.push(2.0);
        builder.push(-2.0);
        assert_eq!(builder.finish(), vec![-127, 127]);
    }

    #[test]
    fn peak_values_are_clamped_and_rounded() {
        assert_eq!(to_i8(0.0), 0);
        assert_eq!(to_i8(1.0), 127);
        assert_eq!(to_i8(-1.0), -127);
        assert_eq!(to_i8(2.0), 127);
        assert_eq!(to_i8(-2.0), -127);
        assert_eq!(to_i8(0.5), 64);
        assert_eq!(to_i8(-0.5), -64);
    }

    #[test]
    fn peaks_encode_as_signed_bytes_in_base64() {
        assert_eq!(encode_peaks(&[-10, 10]), "9go=");
        assert_eq!(encode_peaks(&[]), "");
    }

    #[test]
    fn a_cached_waveform_is_returned_without_running_ffmpeg() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        fs::write(&audio, b"not really audio").expect("audio");
        let metadata = fs::metadata(&audio).expect("metadata");
        let mtime = metadata
            .modified()
            .expect("mtime")
            .duration_since(UNIX_EPOCH)
            .expect("epoch")
            .as_secs() as i64;
        let key = cache_key(&audio, mtime, metadata.len());
        let cache_dir = dir.path().join("waveforms");
        fs::create_dir_all(&cache_dir).expect("cache dir");
        let cached = Waveform {
            version: WAVEFORM_VERSION,
            duration_secs: 12.5,
            levels: vec![WaveformLevel {
                step_secs: 0.01,
                peaks: "AA==".into(),
            }],
        };
        fs::write(
            cache_dir.join(format!("{key}.json")),
            serde_json::to_string(&cached).expect("json"),
        )
        .expect("cache write");

        let loaded =
            load_or_compute(Path::new("missing-ffmpeg"), &cache_dir, &audio).expect("cache hit");

        assert_eq!(loaded.duration_secs, 12.5);
        assert_eq!(loaded.levels.len(), 1);
    }

    #[test]
    fn a_stale_cache_version_forces_a_recompute() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        fs::write(&audio, b"not really audio").expect("audio");
        let metadata = fs::metadata(&audio).expect("metadata");
        let mtime = metadata
            .modified()
            .expect("mtime")
            .duration_since(UNIX_EPOCH)
            .expect("epoch")
            .as_secs() as i64;
        let key = cache_key(&audio, mtime, metadata.len());
        let cache_dir = dir.path().join("waveforms");
        fs::create_dir_all(&cache_dir).expect("cache dir");
        let stale = Waveform {
            version: WAVEFORM_VERSION + 1,
            duration_secs: 1.0,
            levels: vec![],
        };
        fs::write(
            cache_dir.join(format!("{key}.json")),
            serde_json::to_string(&stale).expect("json"),
        )
        .expect("cache write");

        let error = load_or_compute(Path::new("missing-ffmpeg"), &cache_dir, &audio)
            .expect_err("must recompute");

        assert!(error.contains("cannot start ffmpeg for waveform"));
    }

    #[test]
    fn a_corrupt_cache_entry_forces_a_recompute() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        fs::write(&audio, b"not really audio").expect("audio");
        let metadata = fs::metadata(&audio).expect("metadata");
        let mtime = metadata
            .modified()
            .expect("mtime")
            .duration_since(UNIX_EPOCH)
            .expect("epoch")
            .as_secs() as i64;
        let key = cache_key(&audio, mtime, metadata.len());
        let cache_dir = dir.path().join("waveforms");
        fs::create_dir_all(&cache_dir).expect("cache dir");
        fs::write(cache_dir.join(format!("{key}.json")), "{not json").expect("cache write");

        let error = load_or_compute(Path::new("missing-ffmpeg"), &cache_dir, &audio)
            .expect_err("must recompute");

        assert!(error.contains("cannot start ffmpeg for waveform"));
    }
}
