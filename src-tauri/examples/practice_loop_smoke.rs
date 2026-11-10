use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use klipparkiv_lib::practice::engine::Player;

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
        .ok_or_else(|| "usage: practice_loop_smoke <audio> [ffmpeg]".to_string())?;
    let ffmpeg = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_ffmpeg);

    let mut player = Player::new(ffmpeg);
    player.open(PathBuf::from(path), None)?;
    player.set_loop(true, Some(2.0), Some(4.0), true, Some(2.0))?;
    player.play()?;

    for index in 0..16 {
        sleep(Duration::from_millis(500));
        if let Some(snapshot) = player.snapshot() {
            println!(
                "{index:02} position={:.3} playing={} ended={} underruns={}",
                snapshot.position_secs, snapshot.playing, snapshot.ended, snapshot.underruns
            );
        }
    }

    player.seek(3.0)?;
    sleep(Duration::from_millis(500));
    println!("seek_in_loop={:?}", player.snapshot());

    player.set_loop(false, None, None, false, None)?;
    sleep(Duration::from_secs(1));
    println!("loop_off={:?}", player.snapshot());

    player.pause()?;
    println!("paused={:?}", player.snapshot());

    player.close();
    Ok(())
}
