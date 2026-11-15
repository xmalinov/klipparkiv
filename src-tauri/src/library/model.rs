use serde::{Deserialize, Serialize};

/// Audio or video. Stored as `"audio"`/`"video"` in SQLite and on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Audio,
    Video,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Audio => "audio",
            MediaKind::Video => "video",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "audio" => Some(MediaKind::Audio),
            "video" => Some(MediaKind::Video),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub path: String,
    pub kind: MediaKind,
    pub title: String,
    pub channel: Option<String>,
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: u64,
    pub mtime: i64,
    pub thumb_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    pub total: u32,
}

/// Sort order accepted from the UI. Unknown values are a decode error instead
/// of silently falling back to the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    #[default]
    DateDesc,
    DateAsc,
    Title,
    Duration,
    Size,
    Channel,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQuery {
    pub search: Option<String>,
    pub kind: Option<MediaKind>,
    #[serde(default)]
    pub sort: SortKey,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub scanned: u32,
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
}

pub struct MediaRow {
    pub path: String,
    pub kind: MediaKind,
    pub title: String,
    pub channel: Option<String>,
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: u64,
    pub mtime: i64,
    pub thumb_path: Option<String>,
    pub origin: String,
    pub indexed_at: i64,
}

impl MediaRow {
    pub(crate) fn into_item(self) -> LibraryItem {
        LibraryItem {
            id: self.path.clone(),
            path: self.path,
            kind: self.kind,
            title: self.title,
            channel: self.channel,
            duration_secs: self.duration_secs,
            width: self.width,
            height: self.height,
            size_bytes: self.size_bytes,
            mtime: self.mtime,
            thumb_path: self.thumb_path,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub job_id: u64,
    pub index: u32,
    pub total: u32,
    pub name: String,
    pub copied_bytes: u64,
    pub total_bytes: u64,
    pub phase: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<LibraryItem>,
    pub existing: Vec<LibraryItem>,
    pub skipped: Vec<ImportSkip>,
    pub failed: Vec<ImportFailure>,
    pub cancelled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkip {
    pub path: String,
    pub reason: ImportSkipReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSkipReason {
    Unsupported,
    Folder,
    DuplicateInBatch,
    AlreadyInLibrary,
    Cancelled,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub path: String,
    pub error: String,
}
