use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use cpal::SampleFormat;
use cpal::traits::StreamTrait;
use serde::Serialize;

use crate::practice::decode::{self, DecoderMsg};
use crate::practice::looping::{self, LoopRuntime, LoopSettings};
use crate::practice::metronome::MetronomeGrid;
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
    fn set_grid(&self, grid: Option<Arc<MetronomeGrid>>) {
        self.shared.set_grid(grid);
    }

    fn set_metronome(&self, enabled: bool, volume: f32) {
        self.shared.set_metronome(enabled, volume);
    }

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
        grid: Option<Arc<MetronomeGrid>>,
        metronome_enabled: bool,
        metronome_volume: f32,
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
        shared.set_grid(grid);
        shared.set_metronome(metronome_enabled, metronome_volume);
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
    grid: Option<Arc<MetronomeGrid>>,
    metronome_enabled: bool,
    metronome_volume: f32,
}

impl Player {
    pub fn new(ffmpeg: PathBuf) -> Self {
        Self {
            ffmpeg,
            track: None,
            session: None,
            grid: None,
            metronome_enabled: false,
            metronome_volume: 0.5,
        }
    }

    pub fn open(&mut self, path: PathBuf, duration_secs: Option<f64>) -> Result<Opened, String> {
        self.session = None;
        self.grid = None;
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
                pre_roll_secs: state.pre_roll_secs,
            })
        } else {
            None
        };
        self.metronome_enabled = state.metronome_enabled;
        self.metronome_volume = state.metronome_volume;
        if let Some(settings) = track.loop_settings
            && (track.position < settings.start || track.position > settings.end)
        {
            track.position = settings.start;
        }
    }

    pub fn play(&mut self) -> Result<(), String> {
        let (
            path,
            mut position,
            speed,
            pitch,
            volume,
            duration,
            loop_settings,
            grid,
            metronome_enabled,
            metronome_volume,
        ) = {
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
                self.grid.clone(),
                self.metronome_enabled,
                self.metronome_volume,
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
            let pre_roll_start = (settings.start - settings.pre_roll_secs).max(0.0);
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
            grid,
            metronome_enabled,
            metronome_volume,
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
            let grid = self.grid.clone();
            let metronome_enabled = self.metronome_enabled;
            let metronome_volume = self.metronome_volume;
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
                grid,
                metronome_enabled,
                metronome_volume,
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
        pre_roll_secs: Option<f64>,
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
            let pre_roll_secs = pre_roll_secs.unwrap_or(2.0).clamp(0.0, 30.0);
            Some(LoopSettings {
                start,
                end,
                pre_roll,
                pre_roll_secs,
            })
        } else {
            None
        };

        let position = match settings {
            Some(settings) => {
                // Clicking inside an active region must keep the click position;
                // moving to the region start is only for positions outside it.
                if current >= settings.start && current <= settings.end {
                    current
                } else if settings.pre_roll {
                    (settings.start - settings.pre_roll_secs).max(0.0)
                } else {
                    settings.start
                }
            }
            None => current,
        };
        let was_playing = self.session.as_ref().map(Session::playing).unwrap_or(false);

        if let Some(track) = self.track.as_mut() {
            track.loop_settings = settings;
            track.position = position;
        }

        if self.session.is_some() {
            self.session = None;
            let grid = self.grid.clone();
            let metronome_enabled = self.metronome_enabled;
            let metronome_volume = self.metronome_volume;
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
                grid,
                metronome_enabled,
                metronome_volume,
            )?;
            self.session = Some(session);
        }

        Ok(())
    }

    pub fn set_grid(&mut self, grid: Option<Arc<MetronomeGrid>>) {
        self.grid = grid.clone();
        if let Some(session) = self.session.as_ref() {
            session.set_grid(grid);
        }
    }

    pub fn set_metronome(&mut self, enabled: bool, volume: f32) {
        self.metronome_enabled = enabled;
        self.metronome_volume = volume.clamp(0.0, 1.0);
        if let Some(session) = self.session.as_ref() {
            session.set_metronome(enabled, self.metronome_volume);
        }
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
    use crate::practice::state::Region;

    fn player() -> Player {
        Player::new(PathBuf::from("ffmpeg"))
    }

    fn opened(duration: Option<f64>) -> Player {
        let mut player = player();
        player
            .open(PathBuf::from("song.mp3"), duration)
            .expect("open");
        player
    }

    fn region(start: f64, end: f64) -> Region {
        Region {
            id: "r1".into(),
            name: "Verse".into(),
            kind: "verse".into(),
            color: "#fff".into(),
            start,
            end,
        }
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

    #[test]
    fn opening_a_track_resets_playback_metadata() {
        let player = opened(Some(10.0));
        let snapshot = player.snapshot().expect("snapshot");

        assert!(player.has_track());
        assert_eq!(snapshot.position_secs, 0.0);
        assert_eq!(snapshot.duration_secs, Some(10.0));
        assert!(!snapshot.playing);
        assert!(!snapshot.ended);
        assert_eq!(snapshot.underruns, 0);
        assert_eq!(player.sample_rate(), None);
        assert_eq!(player.channels(), None);
        assert_eq!(player.take_error(), None);
    }

    #[test]
    fn closing_resets_the_track() {
        let mut player = opened(Some(10.0));
        player.close();
        assert!(!player.has_track());
        assert!(player.snapshot().is_none());
        assert_eq!(player.position(), None);
    }

    #[test]
    fn playback_requires_an_open_track() {
        let mut player = player();
        assert_eq!(player.play().unwrap_err(), "no track is open");
        assert_eq!(player.seek(1.0).unwrap_err(), "no track is open");
        assert_eq!(player.pause().unwrap(), ());
        player.apply_state(&PracticeState::default());
        assert!(!player.has_track());
    }

    #[test]
    fn applying_state_clamps_speed_pitch_volume_and_position() {
        let mut player = opened(Some(10.0));
        let state = PracticeState {
            speed: 5.0,
            pitch_semitones: 20.0,
            cents: 25.0,
            volume: 2.0,
            position_secs: -4.0,
            ..PracticeState::default()
        };
        player.apply_state(&state);

        let track = player.track.as_ref().expect("track");
        assert_eq!(track.speed, 2.0);
        assert_eq!(track.pitch, 12.0);
        assert_eq!(track.volume, 1.0);
        assert_eq!(track.position, 0.0);

        let past_end = PracticeState {
            position_secs: 99.0,
            ..PracticeState::default()
        };
        player.apply_state(&past_end);
        assert_eq!(player.track.as_ref().unwrap().position, 10.0);
    }

    #[test]
    fn applying_state_installs_the_active_loop_and_snaps_into_it() {
        let mut player = opened(Some(20.0));
        let state = PracticeState {
            loop_enabled: true,
            active_region_id: Some("r1".into()),
            regions: vec![region(5.0, 8.0)],
            position_secs: 2.0,
            ..PracticeState::default()
        };
        player.apply_state(&state);

        let track = player.track.as_ref().unwrap();
        assert_eq!(track.position, 5.0);
        assert_eq!(
            track.loop_settings,
            Some(LoopSettings {
                start: 5.0,
                end: 8.0,
                pre_roll: true,
                pre_roll_secs: 2.0,
            })
        );
    }

    #[test]
    fn applying_state_without_a_region_clears_the_loop() {
        let mut player = opened(Some(20.0));
        let state = PracticeState {
            loop_enabled: true,
            active_region_id: Some("missing".into()),
            regions: vec![region(5.0, 8.0)],
            position_secs: 4.0,
            ..PracticeState::default()
        };
        player.apply_state(&state);

        let track = player.track.as_ref().unwrap();
        assert_eq!(track.loop_settings, None);
        assert_eq!(track.position, 4.0);
    }

    #[test]
    fn seeking_pulls_the_position_into_the_loop() {
        let mut player = opened(Some(10.0));
        player
            .set_loop(true, Some(5.0), Some(8.0), false, None)
            .unwrap();

        player.seek(2.0).unwrap();
        assert_eq!(player.position(), Some(5.0));
        player.seek(7.0).unwrap();
        assert_eq!(player.position(), Some(7.0));
        player.seek(20.0).unwrap();
        assert_eq!(player.position(), Some(5.0));
    }

    #[test]
    fn loop_settings_are_validated() {
        let mut with_track = opened(Some(10.0));
        assert_eq!(
            with_track
                .set_loop(true, None, Some(4.0), false, None)
                .unwrap_err(),
            "loop start is required"
        );
        assert_eq!(
            with_track
                .set_loop(true, Some(1.0), None, false, None)
                .unwrap_err(),
            "loop end is required"
        );
        assert_eq!(
            with_track
                .set_loop(true, Some(4.0), Some(4.0), false, None)
                .unwrap_err(),
            "loop region must have start < end"
        );
        assert_eq!(
            with_track
                .set_loop(true, Some(-1.0), Some(4.0), false, None)
                .unwrap_err(),
            "loop region must have start < end"
        );
        assert_eq!(
            with_track
                .set_loop(true, Some(f64::NAN), Some(4.0), false, None)
                .unwrap_err(),
            "loop region must have start < end"
        );
        assert_eq!(
            with_track
                .set_loop(true, Some(1.0), Some(30.0), false, None)
                .unwrap_err(),
            "loop region is outside the track"
        );

        let mut no_track = player();
        assert_eq!(
            no_track
                .set_loop(true, Some(1.0), Some(2.0), false, None)
                .unwrap_err(),
            "no track is open"
        );
    }

    #[test]
    fn loop_pre_roll_rewinds_and_is_clamped() {
        let mut player = opened(Some(100.0));
        player.seek(50.0).unwrap();
        player
            .set_loop(true, Some(10.0), Some(20.0), true, None)
            .unwrap();
        assert_eq!(player.position(), Some(8.0));

        player
            .set_loop(true, Some(10.0), Some(20.0), true, Some(100.0))
            .unwrap();
        assert_eq!(player.position(), Some(0.0));

        player
            .set_loop(true, Some(10.0), Some(20.0), true, Some(-5.0))
            .unwrap();
        assert_eq!(player.position(), Some(10.0));
    }

    #[test]
    fn setting_loop_keeps_a_position_inside_the_loop() {
        let mut player = opened(Some(10.0));
        player.seek(6.0).unwrap();
        player
            .set_loop(true, Some(5.0), Some(8.0), true, Some(2.0))
            .unwrap();
        assert_eq!(player.position(), Some(6.0));
    }

    #[test]
    fn disabling_the_loop_keeps_the_current_position() {
        let mut player = opened(Some(10.0));
        player
            .set_loop(true, Some(2.0), Some(4.0), false, None)
            .unwrap();
        assert_eq!(player.position(), Some(2.0));

        player.set_loop(false, None, None, false, None).unwrap();
        assert_eq!(player.position(), Some(2.0));
        assert_eq!(player.track.as_ref().unwrap().loop_settings, None);
    }

    #[test]
    fn validation_runs_even_without_a_track() {
        let mut player = player();
        assert!(player.set_speed(3.0).is_err());
        assert!(player.set_pitch(20.0, 0.0).is_err());
        assert!(player.set_volume(-1.0).is_err());
        assert!(player.set_speed(1.5).is_ok());
        assert!(player.set_pitch(0.0, 0.0).is_ok());
        assert!(player.set_volume(0.5).is_ok());
    }

    #[test]
    fn metronome_state_is_clamped_and_kept_without_a_session() {
        let mut player = player();
        player.set_metronome(true, 4.0);
        assert!(player.metronome_enabled);
        assert_eq!(player.metronome_volume, 1.0);

        player.set_metronome(false, -2.0);
        assert!(!player.metronome_enabled);
        assert_eq!(player.metronome_volume, 0.0);

        player.set_grid(None);
        assert!(player.grid.is_none());
    }

    #[test]
    fn snapshot_clamps_to_the_track_duration() {
        let mut player = opened(Some(10.0));
        player.track.as_mut().unwrap().position = 99.0;
        assert_eq!(player.snapshot().unwrap().position_secs, 10.0);
    }
}
