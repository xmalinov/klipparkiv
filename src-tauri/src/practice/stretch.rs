#![allow(dead_code)]

use crate::practice::ffi;

/// 80 ms block / 20 ms interval: 80 ms total latency, see spike-report.md.
const BLOCK_MS: f32 = 80.0;
const INTERVAL_MS: f32 = 20.0;

pub struct Stretch {
    handle: *mut ffi::SignalsmithStretch,
    channels: usize,
}

unsafe impl Send for Stretch {}

impl Stretch {
    pub fn new(channels: u32, sample_rate: u32) -> Self {
        let block = (sample_rate as f32 * BLOCK_MS / 1000.0) as usize;
        let interval = (sample_rate as f32 * INTERVAL_MS / 1000.0) as usize;
        let handle = unsafe { ffi::signalsmith_stretch_create(channels as i32, block, interval) };
        assert!(!handle.is_null(), "signalsmith stretch creation failed");
        Self {
            handle,
            channels: channels as usize,
        }
    }

    pub fn reset(&mut self) {
        unsafe { ffi::signalsmith_stretch_reset(self.handle) }
    }

    pub fn input_latency(&self) -> usize {
        unsafe { ffi::signalsmith_stretch_input_latency(self.handle) }
    }

    pub fn output_latency(&self) -> usize {
        unsafe { ffi::signalsmith_stretch_output_latency(self.handle) }
    }

    pub fn set_pitch_semitones(&mut self, semitones: f32) {
        unsafe {
            ffi::signalsmith_stretch_set_transpose_factor_semitones(self.handle, semitones, 0.0)
        }
    }

    pub fn process(&mut self, input: &mut [f32], output: &mut [f32]) {
        assert_eq!(input.len() % self.channels, 0);
        assert_eq!(output.len() % self.channels, 0);
        unsafe {
            ffi::signalsmith_stretch_process(
                self.handle,
                input.as_mut_ptr(),
                input.len() / self.channels,
                output.as_mut_ptr(),
                output.len() / self.channels,
            )
        }
    }

    pub fn flush(&mut self, output: &mut [f32]) {
        assert_eq!(output.len() % self.channels, 0);
        unsafe {
            ffi::signalsmith_stretch_flush(
                self.handle,
                output.as_mut_ptr(),
                output.len() / self.channels,
            )
        }
    }
}

impl Drop for Stretch {
    fn drop(&mut self) {
        unsafe { ffi::signalsmith_stretch_destroy(self.handle) }
    }
}

/// Total algorithmic latency of the configured stretcher, in seconds.
pub fn latency_secs(channels: u32, sample_rate: u32) -> f64 {
    let stretch = Stretch::new(channels, sample_rate);
    (stretch.input_latency() + stretch.output_latency()) as f64 / sample_rate as f64
}
