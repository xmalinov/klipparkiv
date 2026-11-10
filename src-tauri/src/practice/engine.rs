use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use cpal::SampleFormat;
use cpal::traits::StreamTrait;
use serde::Serialize;

use crate::practice::decode::{self, DecoderMsg};
use crate::practice::looping::{self, LoopRuntime, LoopSettings};
use crate::practice::output;
use crate::practice::shared::Shared;
use crate::practice::state::PracticeState;
use crate::practice::stretch;

const MIN_SPEED: f32 = 0.25;
const MAX_SPEED: f32 = 2.0;
const MIN_PITCH: f32 = -12.0;
const MAX_PITCH: f32 = 12.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub playing: bool,
    pub ended: bool,
    pub underruns: u64,
}

#[derive(Debug, Clone)]
pub struct Opened {
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<u32>,
}

struct Track {
    path: PathBuf,
    duration: Option<f64>,
    position: f64,
    speed: f32,
    pitch: f32,
    volume: f32,
    loop_settings: Option<LoopSettings>,
}

struct Session {
    stream: Option<cpal::Stream>,
    decoder: Option<decode::DecoderController>,
    shared: Arc<Shared>,
    sample_rate: u32,
    channels: usize,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    fn new(
        ffmpeg: PathBuf,
        path: PathBuf,
        position: f64,
        speed: f32,
        pitch: f32,
        volume: f32,
        autoplay: bool,
        loop_settings: Option<LoopSettings>,
        initial_loop_offset_secs: f64,
    ) -> Result<Self, String> {
        let output = output::default_output()?;
        let sample_format = output.config.sample_format();
        if !output::sample_format_supported(sample_format) {
            return Err(format!(
                "unsupported audio sample format: {sample_format:?}"
            ));
        }

        let sample_rate = output.config.sample_rate();
        let channels = output.config.channels() as usize;
        let loop_runtime = match loop_settings {
            Some(settings) => {
                let cache = if settings.end - settings.start <= looping::MAX_CACHE_SECS {
                    Some(Arc::new(looping::load_cache(
                        &ffmpeg,
                        &path,
                        settings,
                        sample_rate,
                        channels,
                    )?))
                } else {
                    None
                };
                let pre_roll_frames = if settings.pre_roll && initial_loop_offset_secs <= 0.0 {
                    ((settings.start - position).max(0.0) * sample_rate as f64).round() as usize
                } else {
                    0
                };
                let loop_frames =
                    ((settings.end - settings.start) * sample_rate as f64).round() as usize;
                let initial_frame = (initial_loop_offset_secs.max(0.0) * sample_rate as f64).round()
                    as usize
                    % loop_frames.max(1);
                Some(LoopRuntime {
                    start: settings.start,
                    pre_roll_frames,
                    loop_frames,
                    initial_frame,
                    cache,
                })
            }
            None => None,
        };

        let latency_secs = stretch::latency_secs(channels as u32, sample_rate);
        let shared = Arc::new(Shared::new(
            sample_rate,
            channels,
            speed,
            pitch,
            volume,
            position.max(0.0),
            latency_secs,
        ));
        let (sender, receiver) = sync_channel::<DecoderMsg>(64);
        let decoder = decode::DecoderController::new(
            ffmpeg,
            path,
            sample_rate,
            channels,
            position.max(0.0),
            sender,
        );
        let control = decoder.control();
        let config: cpal::StreamConfig = output.config.into();

        let stream = match sample_format {
            SampleFormat::F32 => output::build_stream::<f32>(
                &output.device,
                config,
                shared.clone(),
                receiver,
                loop_runtime,
                control,
            )?,
            SampleFormat::I16 => output::build_stream::<i16>(
                &output.device,
                config,
                shared.clone(),
                receiver,
                loop_runtime,
                control,
            )?,
            SampleFormat::U16 => output::build_stream::<u16>(
                &output.device,
                config,
                shared.clone(),
                receiver,
                loop_runtime,
                control,
            )?,
            other => return Err(format!("unsupported audio sample format: {other:?}")),
        };

        if autoplay {
            stream
                .play()
                .map_err(|error| format!("cannot start audio stream: {error}"))?;
            shared.set_playing(true);
        }

        Ok(Self {
            stream: Some(stream),
            decoder: Some(decoder),
            shared,
            sample_rate,
            channels,
        })
    }

    fn play(&self) -> Result<(), String> {
        self.stream
            .as_ref()
            .ok_or_else(|| "audio stream is not available".to_string())?
            .play()
            .map_err(|error| format!("cannot start audio stream: {error}"))?;
        self.shared.set_playing(true);
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        self.stream
            .as_ref()
            .ok_or_else(|| "audio stream is not available".to_string())?
            .pause()
            .map_err(|error| format!("cannot pause audio stream: {error}"))?;
        self.shared.set_playing(false);
        Ok(())
    }

    fn position(&self) -> f64 {
        self.shared.position_secs()
    }

    fn playing(&self) -> bool {
        self.shared.playing()
    }

    fn ended(&self) -> bool {
        self.shared.ended()
    }

    fn set_speed(&self, speed: f32) {
        self.shared.set_speed(speed);
    }

    fn set_pitch(&self, pitch: f32) {
        self.shared.set_pitch(pitch);
    }

    fn set_volume(&self, volume: f32) {
        self.shared.set_volume(volume);
    }

    fn take_error(&self) -> Option<String> {
        self.shared.take_error()
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> usize {
        self.channels
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.set_playing(false);
        // Drop the stream first: this closes the receiver and unblocks the decoder.
        self.stream.take();
        self.decoder.take();
    }
}

pub struct Player {
    ffmpeg: PathBuf,
    track: Option<Track>,
    session: Option<Session>,
}

impl Player {
    pub fn new(ffmpeg: PathBuf) -> Self {
        Self {
            ffmpeg,
            track: None,
            session: None,
        }
    }

    pub fn open(&mut self, path: PathBuf, duration_secs: Option<f64>) -> Result<Opened, String> {
        self.session = None;
        self.track = Some(Track {
            path,
            duration: duration_secs,
            position: 0.0,
            speed: 1.0,
            pitch: 0.0,
            volume: 1.0,
            loop_settings: None,
        });
        Ok(Opened {
            duration_secs,
            sample_rate: None,
        })
    }

    pub fn apply_state(&mut self, state: &PracticeState) {
        let Some(track) = self.track.as_mut() else {
            return;
        };
        track.speed = state.speed.clamp(MIN_SPEED, MAX_SPEED);
        track.pitch = (state.pitch_semitones + state.cents / 100.0).clamp(MIN_PITCH, MAX_PITCH);
        track.volume = state.volume.clamp(0.0, 1.0);
        track.position = state.position_secs.max(0.0);
        if let Some(duration) = track.duration {
            track.position = track.position.min(duration);
        }
        track.loop_settings = if state.loop_enabled {
            state.active_region().map(|region| LoopSettings {
                start: region.start,
                end: region.end,
                pre_roll: state.pre_roll,
            })
        } else {
            None
        };
        if let Some(settings) = track.loop_settings
            && (track.position < settings.start || track.position > settings.end)
        {
            track.position = settings.start;
        }
    }

    pub fn play(&mut self) -> Result<(), String> {
        let (path, mut position, speed, pitch, volume, duration, loop_settings) = {
            let track = self
                .track
                .as_ref()
                .ok_or_else(|| "no track is open".to_string())?;
            (
                track.path.clone(),
                track.position,
                track.speed,
                track.pitch,
                track.volume,
                track.duration,
                track.loop_settings,
            )
        };

        if let Some(session) = self.session.as_ref()
            && !session.ended()
        {
            return session.play();
        }

        if let Some(duration) = duration
            && position >= duration - 0.05
        {
            position = 0.0;
        }

        let mut initial_loop_offset = 0.0;
        if let Some(settings) = loop_settings {
            let pre_roll_start = (settings.start - 2.0).max(0.0);
            let in_pre_roll =
                settings.pre_roll && position >= pre_roll_start && position <= settings.end;
            if !in_pre_roll && (position < settings.start || position > settings.end) {
                position = settings.start;
            }
            initial_loop_offset = (position - settings.start).max(0.0);
        }

        let session = Session::new(
            self.ffmpeg.clone(),
            path,
            position,
            speed,
            pitch,
            volume,
            true,
            loop_settings,
            initial_loop_offset,
        )?;
        self.session = Some(session);
        if let Some(track) = self.track.as_mut() {
            track.position = position;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        let Some(session) = self.session.as_ref() else {
            return Ok(());
        };
        session.pause()?;
        let position = session.position();
        if let Some(track) = self.track.as_mut() {
            track.position = position;
        }
        Ok(())
    }

    pub fn seek(&mut self, position_secs: f64) -> Result<(), String> {
        let (path, speed, pitch, volume, duration, loop_settings) = {
            let track = self
                .track
                .as_ref()
                .ok_or_else(|| "no track is open".to_string())?;
            (
                track.path.clone(),
                track.speed,
                track.pitch,
                track.volume,
                track.duration,
                track.loop_settings,
            )
        };

        let mut position = position_secs.max(0.0);
        if let Some(duration) = duration {
            position = position.min(duration);
        }

        let mut session_loop = loop_settings;
        let mut initial_loop_offset = 0.0;
        if let Some(settings) = loop_settings {
            if position < settings.start || position > settings.end {
                position = settings.start;
            }
            initial_loop_offset = (position - settings.start).max(0.0);
            session_loop = Some(LoopSettings {
                pre_roll: false,
                ..settings
            });
        }

        if let Some(session) = self.session.as_ref() {
            let was_playing = session.playing();
            self.session = None;
            let session = Session::new(
                self.ffmpeg.clone(),
                path,
                position,
                speed,
                pitch,
                volume,
                was_playing,
                session_loop,
                initial_loop_offset,
            )?;
            self.session = Some(session);
        }

        if let Some(track) = self.track.as_mut() {
            track.position = position;
        }
        Ok(())
    }

    pub fn set_speed(&mut self, speed: f32) -> Result<(), String> {
        if !(MIN_SPEED..=MAX_SPEED).contains(&speed) {
            return Err(format!("speed must be between {MIN_SPEED} and {MAX_SPEED}"));
        }
        if let Some(track) = self.track.as_mut() {
            track.speed = speed;
        }
        if let Some(session) = self.session.as_ref() {
            session.set_speed(speed);
        }
        Ok(())
    }

    pub fn set_pitch(&mut self, semitones: f32, cents: f32) -> Result<(), String> {
        if !(MIN_PITCH..=MAX_PITCH).contains(&semitones) {
            return Err(format!(
                "pitch must be between {MIN_PITCH} and {MAX_PITCH} semitones"
            ));
        }
        if !(-50.0..=50.0).contains(&cents) {
            return Err("cents must be between -50 and 50".to_string());
        }
        let pitch = (semitones + cents / 100.0).clamp(MIN_PITCH, MAX_PITCH);
        if let Some(track) = self.track.as_mut() {
            track.pitch = pitch;
        }
        if let Some(session) = self.session.as_ref() {
            session.set_pitch(pitch);
        }
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<(), String> {
        if !(0.0..=1.0).contains(&volume) {
            return Err("volume must be between 0 and 1".to_string());
        }
        if let Some(track) = self.track.as_mut() {
            track.volume = volume;
        }
        if let Some(session) = self.session.as_ref() {
            session.set_volume(volume);
        }
        Ok(())
    }

    pub fn set_loop(
        &mut self,
        enabled: bool,
        start_secs: Option<f64>,
        end_secs: Option<f64>,
        pre_roll: bool,
    ) -> Result<(), String> {
        let current = self.position().unwrap_or(0.0);
        let (path, speed, pitch, volume, duration) = {
            let track = self
                .track
                .as_ref()
                .ok_or_else(|| "no track is open".to_string())?;
            (
                track.path.clone(),
                track.speed,
                track.pitch,
                track.volume,
                track.duration,
            )
        };

        let settings = if enabled {
            let start = start_secs.ok_or_else(|| "loop start is required".to_string())?;
            let end = end_secs.ok_or_else(|| "loop end is required".to_string())?;
            if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
                return Err("loop region must have start < end".to_string());
            }
            if let Some(duration) = duration
                && end > duration + 0.001
            {
                return Err("loop region is outside the track".to_string());
            }
            Some(LoopSettings {
                start,
                end,
                pre_roll,
            })
        } else {
            None
        };

        let position = match settings {
            Some(settings) if settings.pre_roll => (settings.start - 2.0).max(0.0),
            Some(settings) => settings.start,
            None => current,
        };
        let was_playing = self.session.as_ref().map(Session::playing).unwrap_or(false);

        if let Some(track) = self.track.as_mut() {
            track.loop_settings = settings;
            track.position = position;
        }

        if self.session.is_some() {
            self.session = None;
            let session = Session::new(
                self.ffmpeg.clone(),
                path,
                position,
                speed,
                pitch,
                volume,
                was_playing,
                settings,
                0.0,
            )?;
            self.session = Some(session);
        }

        Ok(())
    }

    pub fn close(&mut self) {
        self.session = None;
        self.track = None;
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        let track = self.track.as_ref()?;
        let (mut position, playing, ended) = match self.session.as_ref() {
            Some(session) => (session.position(), session.playing(), session.ended()),
            None => (track.position, false, false),
        };
        if let Some(duration) = track.duration {
            position = position.min(duration);
        }
        Some(Snapshot {
            position_secs: position.max(0.0),
            duration_secs: track.duration,
            playing,
            ended,
            underruns: self
                .session
                .as_ref()
                .map(|session| session.shared.underruns())
                .unwrap_or(0),
        })
    }

    pub fn position(&self) -> Option<f64> {
        self.snapshot().map(|snapshot| snapshot.position_secs)
    }

    pub fn sample_rate(&self) -> Option<u32> {
        self.session.as_ref().map(Session::sample_rate)
    }

    pub fn channels(&self) -> Option<usize> {
        self.session.as_ref().map(Session::channels)
    }

    pub fn has_track(&self) -> bool {
        self.track.is_some()
    }

    pub fn take_error(&self) -> Option<String> {
        self.session.as_ref().and_then(Session::take_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> Player {
        Player::new(PathBuf::from("ffmpeg"))
    }

    #[test]
    fn speed_is_validated() {
        let mut player = player();
        player.open(PathBuf::from("song.mp3"), Some(120.0)).unwrap();
        assert!(player.set_speed(0.25).is_ok());
        assert!(player.set_speed(2.0).is_ok());
        assert!(player.set_speed(0.24).is_err());
        assert!(player.set_speed(2.01).is_err());
        assert!(player.set_volume(0.5).is_ok());
        assert!(player.set_volume(1.1).is_err());
    }

    #[test]
    fn pitch_is_validated_and_cents_are_folded() {
        let mut player = player();
        player.open(PathBuf::from("song.mp3"), Some(120.0)).unwrap();
        assert!(player.set_pitch(12.0, 0.0).is_ok());
        assert!(player.set_pitch(-12.0, 0.0).is_ok());
        assert!(player.set_pitch(13.0, 0.0).is_err());
        assert!(player.set_pitch(0.0, 51.0).is_err());
        assert!(player.set_pitch(0.0, -50.0).is_ok());
    }

    #[test]
    fn seek_is_clamped_to_duration() {
        let mut player = player();
        player.open(PathBuf::from("song.mp3"), Some(10.0)).unwrap();
        player.seek(99.0).unwrap();
        assert_eq!(player.position(), Some(10.0));
        player.seek(-5.0).unwrap();
        assert_eq!(player.position(), Some(0.0));
    }

    #[test]
    fn snapshot_requires_open_track() {
        let player = player();
        assert!(player.snapshot().is_none());
        assert!(!player.has_track());
    }
}
