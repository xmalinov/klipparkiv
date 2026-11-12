use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::analysis::types::{ANALYSIS_VERSION, PracticeAnalysis};

pub const MODEL_ID: &str = "beat-this-small-1";

pub fn cache_key(path: &Path, mtime: i64, size: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(mtime.to_le_bytes());
    hasher.update(b"|");
    hasher.update(size.to_le_bytes());
    hasher.update(b"|");
    hasher.update(MODEL_ID.as_bytes());
    hasher.update(b"|");
    hasher.update(ANALYSIS_VERSION.to_le_bytes());
    hex::encode(hasher.finalize())
}

pub fn cache_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.json"))
}

pub fn load(dir: &Path, key: &str) -> Option<PracticeAnalysis> {
    let text = fs::read_to_string(cache_path(dir, key)).ok()?;
    let analysis = serde_json::from_str::<PracticeAnalysis>(&text).ok()?;
    (analysis.version == ANALYSIS_VERSION).then_some(analysis)
}

pub fn save(dir: &Path, key: &str, analysis: &PracticeAnalysis) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let text = serde_json::to_string(analysis).map_err(|error| error.to_string())?;
    fs::write(cache_path(dir, key), text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis() -> PracticeAnalysis {
        PracticeAnalysis {
            version: ANALYSIS_VERSION,
            path: "a.mp3".into(),
            duration_secs: 10.0,
            bpm_raw: 120.0,
            bpm: 120.0,
            octave: 1.0,
            confidence: 0.9,
            beats: vec![0.0, 0.5],
            downbeats: vec![0.0],
            beat_confidence: vec![1.0, 1.0],
            downbeat_confidence: vec![1.0],
            beats_per_bar: 4,
            bar_offset: 0,
            model: MODEL_ID.into(),
            analyzed_at: 1,
        }
    }

    #[test]
    fn roundtrips_analysis() {
        let dir = tempfile::tempdir().unwrap();
        let analysis = analysis();
        save(dir.path(), "key", &analysis).unwrap();
        let loaded = load(dir.path(), "key").unwrap();
        assert_eq!(loaded.bpm, 120.0);
        assert_eq!(loaded.beats.len(), 2);
        assert_eq!(loaded.beat_confidence, vec![1.0, 1.0]);
        assert_eq!(loaded.downbeat_confidence, vec![1.0]);
    }

    #[test]
    fn serializes_confidence_fields_in_camel_case() {
        let text = serde_json::to_string(&analysis()).unwrap();
        assert!(text.contains("\"beatConfidence\""));
        assert!(text.contains("\"downbeatConfidence\""));
    }

    #[test]
    fn rejects_an_old_analysis_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut analysis = analysis();
        analysis.version = ANALYSIS_VERSION - 1;
        save(dir.path(), "key", &analysis).unwrap();
        assert!(load(dir.path(), "key").is_none());
    }

    #[test]
    fn key_changes_with_file_identity_and_version() {
        let a = cache_key(Path::new("a.mp3"), 10, 100);
        let b = cache_key(Path::new("a.mp3"), 11, 100);
        let c = cache_key(Path::new("a.mp3"), 10, 101);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, cache_key(Path::new("a.mp3"), 10, 100));
    }
}
