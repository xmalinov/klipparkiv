use std::path::{Path, PathBuf};

pub const SMALL_BEAT_MODEL: &str = "beat_this_small.onnx";
pub const MEL_MODEL: &str = "mel_spectrogram.onnx";

pub struct ModelPaths {
    pub mel: PathBuf,
    pub beat: PathBuf,
}

pub fn resolve(models_dir: &Path) -> Result<ModelPaths, String> {
    let mel = models_dir.join(MEL_MODEL);
    let beat = models_dir.join(SMALL_BEAT_MODEL);
    if !mel.is_file() {
        return Err(format!(
            "beat grid model is missing: {}",
            mel.to_string_lossy()
        ));
    }
    if !beat.is_file() {
        return Err(format!(
            "beat grid model is missing: {}",
            beat.to_string_lossy()
        ));
    }
    Ok(ModelPaths { mel, beat })
}
