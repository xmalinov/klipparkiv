//! Local media library: the SQLite index, the filesystem scan, the import
//! pipeline and the path layout they share.

pub(crate) mod commands;
pub(crate) mod copy;
pub(crate) mod db;
pub(crate) mod guard;
pub(crate) mod import;
pub(crate) mod model;
pub(crate) mod scan;
pub(crate) mod thumbs;
pub(crate) mod util;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crate::paths::{AUDIO_DIR, AppPaths, DATA_DIR, THUMBS_DIR, VIDEO_DIR};

pub(crate) use db::Library;
pub(crate) use guard::resolve_media;
pub(crate) use import::ImportState;
pub(crate) use model::LibraryItem;
pub(crate) use util::display_title;

/// Every folder the library owns. Derived once from the app root, so scan,
/// import and the thumbnail cache can never disagree about `data/thumbs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LibraryPaths {
    root: PathBuf,
    audio: PathBuf,
    video: PathBuf,
    thumbs: PathBuf,
}

impl LibraryPaths {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            audio: root.join(AUDIO_DIR),
            video: root.join(VIDEO_DIR),
            thumbs: root.join(DATA_DIR).join(THUMBS_DIR),
        }
    }

    pub(crate) fn from_app(paths: &AppPaths) -> Self {
        let mut layout = Self::new(paths.root());
        layout.audio = paths.audio();
        layout.video = paths.video();
        layout.thumbs = paths.thumbs();
        layout
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn audio(&self) -> PathBuf {
        self.audio.clone()
    }

    pub(crate) fn video(&self) -> PathBuf {
        self.video.clone()
    }

    pub(crate) fn thumbs(&self) -> PathBuf {
        self.thumbs.clone()
    }
}
