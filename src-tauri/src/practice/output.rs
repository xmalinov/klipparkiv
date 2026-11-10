use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

use crate::practice::decode::{ControlEvent, DecoderMsg};
use crate::practice::looping::LoopRuntime;
use crate::practice::metronome::click_events;
use crate::practice::shared::Shared;
use crate::practice::stretch::Stretch;

/// Fixed -3 dB headroom: the stretcher can raise peaks above the input level.
const HEADROOM: f32 = 0.707;
const CLICK_SECS: f32 = 0.02;

fn apply_headroom(samples: &mut [f32]) {
    for sample in samples {
        *sample *= HEADROOM;
    }
}

fn make_click(sample_rate: u32, channels: usize, frequency: f32, gain: f32) -> Vec<f32> {
    let frames = (sample_rate as f32 * CLICK_SECS).max(1.0) as usize;
    let mut output = vec![0.0f32; frames * channels];
    for frame in 0..frames {
        let time = frame as f32 / sample_rate as f32;
        let envelope = (-time * 90.0).exp();
        let sample = (std::f32::consts::TAU * frequency * time).sin() * envelope * gain;
        for channel in 0..channels {
            output[frame * channels + channel] = sample;
        }
    }
    output
}

fn mix_click(output: &mut [f32], channels: usize, click: &[f32], start_frame: usize, volume: f32) {
    let start = start_frame * channels;
    if start >= output.len() {
        return;
    }
    for (index, sample) in click.iter().enumerate() {
        let target = start + index;
        if target >= output.len() {
            break;
        }
        output[target] = (output[target] + sample * volume).clamp(-1.0, 1.0);
    }
}

pub struct OutputDevice {
    pub device: cpal::Device,
    pub config: cpal::SupportedStreamConfig,
}

pub fn default_output() -> Result<OutputDevice, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|error| format!("no audio output config: {error}"))?;
    Ok(OutputDevice { device, config })
}

fn fill_from_receiver(
    receiver: &Receiver<DecoderMsg>,
    leftover: &mut VecDeque<f32>,
    eof_seen: &mut bool,
    waiting_restart: &mut bool,
    shared: &Shared,
    input: &mut [f32],
) -> usize {
    let needed = input.len();
    let mut received = 0usize;

    while received < needed {
        if *waiting_restart {
            break;
        }
        if leftover.is_empty() {
            match receiver.try_recv() {
                Ok(DecoderMsg::Data(chunk)) => leftover.extend(chunk),
                Ok(DecoderMsg::Reset) => {
                    leftover.clear();
                    *waiting_restart = false;
                    *eof_seen = false;
                }
                Ok(DecoderMsg::Eof) => {
                    *eof_seen = true;
                }
                Ok(DecoderMsg::Error(message)) => {
                    shared.set_error(message);
                    shared.set_ended(true);
                    *eof_seen = true;
                }
                Err(_) => break,
            }
        }
        if leftover.is_empty() {
            break;
        }
        let take = (needed - received).min(leftover.len());
        for _ in 0..take {
            input[received] = leftover.pop_front().unwrap();
            received += 1;
        }
    }

    received
}

pub fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    shared: Arc<Shared>,
    receiver: Receiver<DecoderMsg>,
    loop_runtime: Option<LoopRuntime>,
    control: Sender<ControlEvent>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = shared.channels();
    let sample_rate = shared.sample_rate();
    let mut stretch = Stretch::new(channels as u32, sample_rate);
    let mut last_pitch = shared.pitch();
    stretch.set_pitch_semitones(last_pitch);
    let click = make_click(sample_rate, channels, 1_000.0, 0.28);
    let accent_click = make_click(sample_rate, channels, 1_500.0, 0.38);

    let error_shared = shared.clone();
    let mut leftover: VecDeque<f32> = VecDeque::new();
    let mut input_buffer: Vec<f32> = Vec::new();
    let mut output_buffer: Vec<f32> = Vec::new();
    let mut debt = 0.0f32;
    let mut eof_seen = false;
    let mut in_loop = loop_runtime
        .as_ref()
        .map(|runtime| runtime.pre_roll_frames == 0)
        .unwrap_or(false);
    let mut loop_frame = loop_runtime
        .as_ref()
        .map(|runtime| runtime.initial_frame)
        .unwrap_or(0);
    let mut pre_roll_consumed = 0usize;
    let mut waiting_restart = false;
    let mut decoder_stopped = false;

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
                let output_frames = data.len() / channels;
                if output_frames == 0 {
                    return;
                }
                let block_position = shared.position_secs();

                let pitch = shared.pitch();
                if pitch != last_pitch {
                    stretch.set_pitch_semitones(pitch);
                    last_pitch = pitch;
                }

                let speed = shared.speed();
                let volume = shared.volume();
                debt += output_frames as f32 * speed;
                let input_frames = (debt.floor() as usize).max(1);
                debt -= input_frames as f32;
                let needed = input_frames * channels;

                input_buffer.clear();
                input_buffer.resize(needed, 0.0);

                if waiting_restart {
                    while let Ok(message) = receiver.try_recv() {
                        if matches!(message, DecoderMsg::Reset) {
                            waiting_restart = false;
                            leftover.clear();
                            eof_seen = false;
                            break;
                        }
                    }
                }

                let received = if let Some(runtime) = loop_runtime.as_ref() {
                    if in_loop {
                        if let Some(cache) = runtime.cache.as_ref() {
                            if !decoder_stopped {
                                let _ = control.send(ControlEvent::Stop);
                                decoder_stopped = true;
                                while receiver.try_recv().is_ok() {}
                            }
                            for index in 0..input_frames {
                                let frame = (loop_frame + index) % cache.frames;
                                let source = frame * channels;
                                let target = index * channels;
                                input_buffer[target..target + channels]
                                    .copy_from_slice(&cache.pcm[source..source + channels]);
                            }
                            loop_frame = (loop_frame + input_frames) % cache.frames;
                            shared.set_position_override(Some(
                                runtime.start + loop_frame as f64 / sample_rate as f64,
                            ));
                            needed
                        } else if waiting_restart {
                            0
                        } else {
                            let received = fill_from_receiver(
                                &receiver,
                                &mut leftover,
                                &mut eof_seen,
                                &mut waiting_restart,
                                &shared,
                                &mut input_buffer[..needed],
                            );
                            loop_frame += input_frames;
                            shared.set_position_override(Some(
                                runtime.start + loop_frame as f64 / sample_rate as f64,
                            ));
                            if loop_frame >= runtime.loop_frames {
                                let _ = control.send(ControlEvent::Restart(runtime.start));
                                waiting_restart = true;
                                loop_frame = 0;
                                shared.set_position_override(Some(runtime.start));
                            }
                            received
                        }
                    } else {
                        let received = fill_from_receiver(
                            &receiver,
                            &mut leftover,
                            &mut eof_seen,
                            &mut waiting_restart,
                            &shared,
                            &mut input_buffer[..needed],
                        );
                        pre_roll_consumed += input_frames;
                        shared.set_position_override(None);
                        if pre_roll_consumed >= runtime.pre_roll_frames {
                            in_loop = true;
                            loop_frame = 0;
                            shared.set_position_override(Some(runtime.start));
                        }
                        received
                    }
                } else {
                    fill_from_receiver(
                        &receiver,
                        &mut leftover,
                        &mut eof_seen,
                        &mut waiting_restart,
                        &shared,
                        &mut input_buffer[..needed],
                    )
                };

                if received < needed {
                    shared.add_underrun();
                }

                output_buffer.clear();
                output_buffer.resize(data.len(), 0.0);
                stretch.process(
                    &mut input_buffer[..needed],
                    &mut output_buffer[..data.len()],
                );
                apply_headroom(&mut output_buffer[..data.len()]);
                if volume != 1.0 {
                    for sample in &mut output_buffer[..data.len()] {
                        *sample *= volume;
                    }
                }

                if shared.metronome_enabled()
                    && let Some(grid) = shared.grid()
                {
                    let source_span = input_frames as f64 / sample_rate as f64;
                    let metronome_volume = shared.metronome_volume();
                    for event in click_events(
                        &grid,
                        block_position,
                        block_position + source_span,
                        output_frames,
                    ) {
                        let buffer = if event.accent { &accent_click } else { &click };
                        mix_click(
                            &mut output_buffer[..data.len()],
                            channels,
                            buffer,
                            event.frame,
                            metronome_volume,
                        );
                    }
                }

                for (frame, source) in data
                    .chunks_mut(channels)
                    .zip(output_buffer.chunks_exact(channels))
                {
                    for channel in 0..channels {
                        frame[channel] = T::from_sample(source[channel]);
                    }
                }

                shared.add_consumed(input_frames as u64);
                if eof_seen && leftover.is_empty() && loop_runtime.is_none() {
                    shared.set_ended(true);
                }
            },
            move |error| {
                error_shared.set_error(format!("audio stream: {error}"));
                error_shared.set_ended(true);
            },
            None,
        )
        .map_err(|error| format!("cannot build audio stream: {error}"))
}

pub fn sample_format_supported(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headroom_is_minus_three_db() {
        let mut samples = [0.5, -0.5, 0.0];
        apply_headroom(&mut samples);
        assert!((samples[0] - 0.3535).abs() < 0.001);
        assert!((samples[1] + 0.3535).abs() < 0.001);
        assert_eq!(samples[2], 0.0);
    }
}
