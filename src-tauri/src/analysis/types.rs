use serde::{Deserialize, Serialize};

/// Version of the analysis result schema. Bump when the cached JSON shape or
/// the beat model changes in a way that should invalidate old results.
pub const ANALYSIS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeAnalysis {
    pub version: u32,
    pub path: String,
    pub duration_secs: f64,
    pub bpm_raw: f64,
    pub bpm: f64,
    /// 0.5, 1.0 or 2.0: how the raw BPM was scaled by the octave heuristic.
    pub octave: f64,
    pub confidence: f64,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub beats_per_bar: u8,
    pub bar_offset: u8,
    pub model: String,
    pub analyzed_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub path: String,
    pub stage: &'static str,
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisError {
    pub path: String,
    pub message: String,
}
