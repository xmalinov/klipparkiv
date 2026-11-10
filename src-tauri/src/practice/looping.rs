use std::path::Path;
use std::sync::Arc;

use crate::practice::decode;

pub const MAX_CACHE_SECS: f64 = 120.0;
pub const FADE_SECS: f64 = 0.005;

#[derive(Debug, Clone, Copy)]
pub struct LoopSettings {
    pub start: f64,
    pub end: f64,
    pub pre_roll: bool,
    pub pre_roll_secs: f64,
}

pub struct LoopCache {
    pub pcm: Vec<f32>,
    pub frames: usize,
}

pub struct LoopRuntime {
    pub start: f64,
    pub pre_roll_frames: usize,
    pub loop_frames: usize,
    pub initial_frame: usize,
    pub cache: Option<Arc<LoopCache>>,
}

pub fn load_cache(
    ffmpeg: &Path,
    path: &Path,
    settings: LoopSettings,
    sample_rate: u32,
    channels: usize,
) -> Result<LoopCache, String> {
    let mut pcm = decode::decode_region(
        ffmpeg,
        path,
        settings.start,
        settings.end,
        sample_rate,
        channels,
    )?;
    let frames = pcm.len() / channels;
    if frames == 0 {
        return Err("loop region is empty".into());
    }
    apply_fade(&mut pcm, channels, sample_rate, FADE_SECS);
    Ok(LoopCache { pcm, frames })
}

pub fn apply_fade(pcm: &mut [f32], channels: usize, sample_rate: u32, seconds: f64) {
    let frames = pcm.len() / channels;
    if frames == 0 || channels == 0 {
        return;
    }
    let fade_frames = ((sample_rate as f64 * seconds) as usize).min(frames / 2);
    if fade_frames == 0 {
        return;
    }
    for index in 0..fade_frames {
        let gain = index as f32 / fade_frames as f32;
        let head = index * channels;
        let tail = (frames - 1 - index) * channels;
        for channel in 0..channels {
            pcm[head + channel] *= gain;
            pcm[tail + channel] *= gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_silences_both_edges_and_keeps_the_middle() {
        let channels = 2;
        let mut pcm = vec![1.0f32; 100 * channels];
        apply_fade(&mut pcm, channels, 1_000, 0.01);
        assert_eq!(pcm[0], 0.0);
        assert_eq!(pcm[99 * channels], 0.0);
        assert_eq!(pcm[50 * channels], 1.0);
    }

    #[test]
    fn fade_is_clamped_for_tiny_buffers() {
        let mut pcm = vec![1.0f32; 4];
        apply_fade(&mut pcm, 1, 48_000, 1.0);
        assert_eq!(pcm[0], 0.0);
        assert_eq!(pcm[3], 0.0);
    }
}
