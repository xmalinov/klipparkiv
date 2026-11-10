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
        .ok_or_else(|| "usage: practice_smoke <audio> [ffmpeg]".to_string())?;
    let ffmpeg = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_ffmpeg);

    let mut player = Player::new(ffmpeg);
    player.open(PathBuf::from(path), None)?;

    player.play()?;
    sleep(Duration::from_secs(3));
    println!("playing={:?}", player.snapshot());

    player.set_speed(0.5)?;
    sleep(Duration::from_secs(2));
    println!("half_speed={:?}", player.snapshot());

    player.set_pitch(-2.0, 0.0)?;
    sleep(Duration::from_secs(2));
    println!("pitch_down={:?}", player.snapshot());

    player.seek(0.0)?;
    sleep(Duration::from_secs(1));
    println!("after_seek={:?}", player.snapshot());

    player.pause()?;
    println!("paused={:?}", player.snapshot());

    player.close();
    Ok(())
}
