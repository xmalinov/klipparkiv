use std::path::{Path, PathBuf};

use klipparkiv_lib::practice::waveform;

fn default_ffmpeg() -> PathBuf {
    if let Some(path) = std::env::var_os("FFMPEG") {
        return PathBuf::from(path);
    }
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("bin")
        .join(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        });
    if local.exists() {
        local
    } else {
        PathBuf::from("ffmpeg")
    }
}

fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| "usage: practice_waveform_smoke <audio> [ffmpeg]".to_string())?;
    let ffmpeg = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_ffmpeg);
    let cache = std::env::temp_dir().join("klipparkiv-waveform-smoke");

    let waveform = waveform::load_or_compute(&ffmpeg, &cache, Path::new(&path))?;
    println!("duration={:.3}", waveform.duration_secs);
    for level in &waveform.levels {
        println!("step={} encoded_len={}", level.step_secs, level.peaks.len());
    }
    Ok(())
}
