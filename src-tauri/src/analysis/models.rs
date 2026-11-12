use std::path::{Path, PathBuf};

pub const SMALL_BEAT_MODEL: &str = "beat_this_small.onnx";
pub const MEL_MODEL: &str = "mel_spectrogram.onnx";

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn resolve_requires_both_models() {
        let dir = tempfile::tempdir().expect("tempdir");

        let error = resolve(dir.path()).unwrap_err();
        assert!(error.contains(MEL_MODEL));

        fs::write(dir.path().join(MEL_MODEL), b"mel").expect("mel");
        let error = resolve(dir.path()).unwrap_err();
        assert!(error.contains(SMALL_BEAT_MODEL));

        fs::write(dir.path().join(SMALL_BEAT_MODEL), b"beat").expect("beat");
        let models = resolve(dir.path()).expect("both models");
        assert!(models.mel.ends_with(MEL_MODEL));
        assert!(models.beat.ends_with(SMALL_BEAT_MODEL));
    }

    #[test]
    fn resolve_reports_the_missing_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = resolve(dir.path()).unwrap_err();
        let expected = dir.path().join(MEL_MODEL);
        assert!(error.contains(&expected.to_string_lossy().to_string()));
    }
}
