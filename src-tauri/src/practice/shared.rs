use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// State shared between the UI commands, the audio callback and the event thread.
/// The callback only touches atomics; the mutex is used on error paths.
pub struct Shared {
    speed_bits: AtomicU32,
    pitch_bits: AtomicU32,
    volume_bits: AtomicU32,
    consumed_frames: AtomicU64,
    underruns: AtomicU64,
    position_override: AtomicU64,
    playing: AtomicBool,
    ended: AtomicBool,
    sample_rate: u32,
    channels: usize,
    base_secs: f64,
    latency_secs: f64,
    error: Mutex<Option<String>>,
}

impl Shared {
    pub fn new(
        sample_rate: u32,
        channels: usize,
        speed: f32,
        pitch: f32,
        volume: f32,
        base_secs: f64,
        latency_secs: f64,
    ) -> Self {
        Self {
            speed_bits: AtomicU32::new(speed.to_bits()),
            pitch_bits: AtomicU32::new(pitch.to_bits()),
            volume_bits: AtomicU32::new(volume.to_bits()),
            consumed_frames: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            position_override: AtomicU64::new(u64::MAX),
            playing: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            sample_rate,
            channels,
            base_secs,
            latency_secs,
            error: Mutex::new(None),
        }
    }

    pub fn speed(&self) -> f32 {
        f32::from_bits(self.speed_bits.load(Ordering::Relaxed))
    }

    pub fn set_speed(&self, speed: f32) {
        self.speed_bits.store(speed.to_bits(), Ordering::Relaxed);
    }

    pub fn pitch(&self) -> f32 {
        f32::from_bits(self.pitch_bits.load(Ordering::Relaxed))
    }

    pub fn set_pitch(&self, semitones: f32) {
        self.pitch_bits
            .store(semitones.to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume_bits.store(volume.to_bits(), Ordering::Relaxed);
    }

    pub fn add_consumed(&self, frames: u64) {
        self.consumed_frames.fetch_add(frames, Ordering::Relaxed);
    }

    pub fn position_secs(&self) -> f64 {
        let override_bits = self.position_override.load(Ordering::Relaxed);
        if override_bits != u64::MAX {
            return f64::from_bits(override_bits);
        }
        let raw = self.base_secs
            + self.consumed_frames.load(Ordering::Relaxed) as f64 / self.sample_rate as f64
            - self.latency_secs;
        raw.max(self.base_secs)
    }

    pub fn set_position_override(&self, position: Option<f64>) {
        let bits = position.map(f64::to_bits).unwrap_or(u64::MAX);
        self.position_override.store(bits, Ordering::Relaxed);
    }

    pub fn add_underrun(&self) {
        self.underruns.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
    }

    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub fn set_ended(&self, ended: bool) {
        self.ended.store(ended, Ordering::Relaxed);
    }

    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::Relaxed)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn set_error(&self, message: String) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(message);
        }
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut slot| slot.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_is_latency_compensated_and_clamped() {
        let shared = Shared::new(48_000, 2, 1.0, 0.0, 1.0, 10.0, 0.08);
        assert_eq!(shared.position_secs(), 10.0);
        shared.add_consumed(4_800);
        assert!((shared.position_secs() - 10.02).abs() < 1e-6);
    }

    #[test]
    fn position_never_goes_before_the_session_start() {
        let shared = Shared::new(48_000, 2, 1.0, 0.0, 1.0, 0.0, 0.08);
        shared.add_consumed(1_920);
        assert_eq!(shared.position_secs(), 0.0);
        shared.add_consumed(3_840);
        assert!((shared.position_secs() - 0.04).abs() < 1e-6);
    }

    #[test]
    fn position_override_wins_until_cleared() {
        let shared = Shared::new(48_000, 2, 1.0, 0.0, 1.0, 0.0, 0.08);
        shared.set_position_override(Some(12.5));
        assert_eq!(shared.position_secs(), 12.5);
        shared.set_position_override(None);
        assert_eq!(shared.position_secs(), 0.0);
    }
}
