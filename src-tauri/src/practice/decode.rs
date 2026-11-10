use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

const BLOCK_FRAMES: usize = 4_800;

pub enum DecoderMsg {
    Data(Vec<f32>),
    Reset,
    Eof,
    Error(String),
}

pub enum ControlEvent {
    Restart(f64),
    Stop,
    Data { generation: u64, pcm: Vec<f32> },
    Eof { generation: u64 },
    Error { generation: u64, message: String },
}

/// Owns the ffmpeg child and can restart it at a new position without
/// rebuilding the cpal stream. Used for loops longer than the in-memory cache.
pub struct DecoderController {
    control: Sender<ControlEvent>,
    join: Option<JoinHandle<()>>,
}

impl DecoderController {
    pub fn new(
        ffmpeg: PathBuf,
        path: PathBuf,
        sample_rate: u32,
        channels: usize,
        position_secs: f64,
        audio: SyncSender<DecoderMsg>,
    ) -> Self {
        let (control, receiver) = channel();
        let control_for_thread = control.clone();
        let join = std::thread::spawn(move || {
            controller_loop(
                ffmpeg,
                path,
                sample_rate,
                channels,
                position_secs,
                audio,
                control_for_thread,
                receiver,
            )
        });
        Self {
            control,
            join: Some(join),
        }
    }

    pub fn control(&self) -> Sender<ControlEvent> {
        self.control.clone()
    }
}

impl Drop for DecoderController {
    fn drop(&mut self) {
        let _ = self.control.send(ControlEvent::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn controller_loop(
    ffmpeg: PathBuf,
    path: PathBuf,
    sample_rate: u32,
    channels: usize,
    position_secs: f64,
    audio: SyncSender<DecoderMsg>,
    control: Sender<ControlEvent>,
    receiver: Receiver<ControlEvent>,
) {
    let mut generation = 0u64;
    let mut child: Option<Child> = None;
    let mut reader: Option<JoinHandle<()>> = None;

    start_child(
        &ffmpeg,
        &path,
        sample_rate,
        channels,
        position_secs,
        generation,
        &control,
        &mut child,
        &mut reader,
    );

    loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(ControlEvent::Restart(position)) => {
                generation += 1;
                stop_child(&mut child, &mut reader);
                let _ = audio.send(DecoderMsg::Reset);
                start_child(
                    &ffmpeg,
                    &path,
                    sample_rate,
                    channels,
                    position,
                    generation,
                    &control,
                    &mut child,
                    &mut reader,
                );
            }
            Ok(ControlEvent::Stop) => {
                stop_child(&mut child, &mut reader);
                break;
            }
            Ok(ControlEvent::Data {
                generation: event_generation,
                pcm,
            }) if event_generation == generation => {
                if audio.send(DecoderMsg::Data(pcm)).is_err() {
                    stop_child(&mut child, &mut reader);
                    break;
                }
            }
            Ok(ControlEvent::Eof {
                generation: event_generation,
            }) if event_generation == generation => {
                let _ = audio.send(DecoderMsg::Eof);
            }
            Ok(ControlEvent::Error {
                generation: event_generation,
                message,
            }) if event_generation == generation => {
                let _ = audio.send(DecoderMsg::Error(message));
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                stop_child(&mut child, &mut reader);
                break;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn start_child(
    ffmpeg: &Path,
    path: &Path,
    sample_rate: u32,
    channels: usize,
    position_secs: f64,
    generation: u64,
    control: &Sender<ControlEvent>,
    child_slot: &mut Option<Child>,
    reader_slot: &mut Option<JoinHandle<()>>,
) {
    let mut child = match Command::new(ffmpeg)
        .args(decoder_args(path, sample_rate, channels, position_secs))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = control.send(ControlEvent::Error {
                generation,
                message: format!("cannot start ffmpeg ({}): {error}", ffmpeg.display()),
            });
            return;
        }
    };

    let Some(mut stdout) = child.stdout.take() else {
        let _ = control.send(ControlEvent::Error {
            generation,
            message: "ffmpeg stdout is unavailable".into(),
        });
        let _ = child.kill();
        let _ = child.wait();
        return;
    };

    let control_for_reader = control.clone();
    let reader = std::thread::spawn(move || {
        let mut buffer = vec![0u8; BLOCK_FRAMES * channels * 4];
        loop {
            let read = match stdout.read(&mut buffer) {
                Ok(read) => read,
                Err(error) => {
                    let _ = control_for_reader.send(ControlEvent::Error {
                        generation,
                        message: format!("ffmpeg read failed: {error}"),
                    });
                    break;
                }
            };
            if read == 0 {
                break;
            }

            let mut pcm = Vec::with_capacity(read / 4);
            let (chunks, _remainder) = buffer[..read].as_chunks::<4>();
            for chunk in chunks {
                pcm.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
            if control_for_reader
                .send(ControlEvent::Data { generation, pcm })
                .is_err()
            {
                break;
            }
        }
        let _ = control_for_reader.send(ControlEvent::Eof { generation });
    });

    *child_slot = Some(child);
    *reader_slot = Some(reader);
}

fn stop_child(child_slot: &mut Option<Child>, reader_slot: &mut Option<JoinHandle<()>>) {
    if let Some(mut child) = child_slot.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some(reader) = reader_slot.take() {
        let _ = reader.join();
    }
}

pub fn decoder_args(
    path: &Path,
    sample_rate: u32,
    channels: usize,
    position_secs: f64,
) -> Vec<String> {
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"]
        .iter()
        .map(|value| value.to_string())
        .collect();

    if position_secs > 0.0 {
        args.push("-ss".into());
        args.push(format!("{position_secs:.3}"));
    }

    args.push("-i".into());
    args.push(path.to_string_lossy().into_owned());
    args.extend(
        ["-map", "0:a:0", "-vn", "-ac"]
            .iter()
            .map(|v| v.to_string()),
    );
    args.push(channels.to_string());
    args.push("-ar".into());
    args.push(sample_rate.to_string());
    args.extend(["-f", "f32le", "pipe:1"].iter().map(|v| v.to_string()));
    args
}

/// Decodes a fixed region into memory for seamless short loops.
pub fn decode_region(
    ffmpeg: &Path,
    path: &Path,
    start_secs: f64,
    end_secs: f64,
    sample_rate: u32,
    channels: usize,
) -> Result<Vec<f32>, String> {
    let duration = (end_secs - start_secs).max(0.0);
    if duration <= 0.0 {
        return Err("loop region is empty".into());
    }

    let output = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-ss",
            &format!("{start_secs:.3}"),
            "-i",
        ])
        .arg(path)
        .arg("-t")
        .arg(format!("{duration:.3}"))
        .args(["-map", "0:a:0", "-vn", "-ac"])
        .arg(channels.to_string())
        .arg("-ar")
        .arg(sample_rate.to_string())
        .args(["-f", "f32le", "pipe:1"])
        .output()
        .map_err(|error| format!("cannot decode loop region: {error}"))?;

    if !output.status.success() {
        return Err("ffmpeg failed to decode the loop region".into());
    }
    if output.stdout.len() % 4 != 0 {
        return Err("loop region is not f32-aligned".into());
    }

    let mut pcm = Vec::with_capacity(output.stdout.len() / 4);
    let (chunks, _remainder) = output.stdout.as_chunks::<4>();
    for chunk in chunks {
        pcm.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_args_seek_before_input() {
        let args = decoder_args(Path::new("C:\\media\\song.mp3"), 48_000, 2, 12.5);
        let seek = args.iter().position(|arg| arg == "-ss").unwrap();
        let input = args.iter().position(|arg| arg == "-i").unwrap();
        assert!(seek < input);
        assert_eq!(args[seek + 1], "12.500");
        assert!(args.contains(&"48000".to_string()));
        assert!(args.contains(&"2".to_string()));
    }

    #[test]
    fn decoder_args_without_seek_for_zero() {
        let args = decoder_args(Path::new("song.mp3"), 44_100, 1, 0.0);
        assert!(!args.iter().any(|arg| arg == "-ss"));
        assert!(args.contains(&"44100".to_string()));
        assert!(args.contains(&"1".to_string()));
    }
}
