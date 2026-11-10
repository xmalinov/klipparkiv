#![allow(dead_code)]

use std::os::raw::c_int;

#[repr(C)]
pub struct SignalsmithStretch {
    _private: [u8; 0],
}

unsafe extern "C" {
    pub fn signalsmith_stretch_create(
        channel_count: c_int,
        block_length: usize,
        interval: usize,
    ) -> *mut SignalsmithStretch;

    pub fn signalsmith_stretch_destroy(handle: *mut SignalsmithStretch);

    pub fn signalsmith_stretch_reset(handle: *mut SignalsmithStretch);

    pub fn signalsmith_stretch_input_latency(handle: *mut SignalsmithStretch) -> usize;

    pub fn signalsmith_stretch_output_latency(handle: *mut SignalsmithStretch) -> usize;

    pub fn signalsmith_stretch_set_transpose_factor_semitones(
        handle: *mut SignalsmithStretch,
        semitones: f32,
        tonality_limit: f32,
    );

    pub fn signalsmith_stretch_process(
        handle: *mut SignalsmithStretch,
        input: *mut f32,
        input_length: usize,
        output: *mut f32,
        output_length: usize,
    );

    pub fn signalsmith_stretch_flush(
        handle: *mut SignalsmithStretch,
        output: *mut f32,
        output_length: usize,
    );
}
