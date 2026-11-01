use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const BIN_DIR: &str = "bin";
pub const DATA_DIR: &str = "data";
pub const THUMBS_DIR: &str = "thumbs";
pub const AUDIO_DIR: &str = "audio";
pub const VIDEO_DIR: &str = "video";

/// All app paths are derived from one portable root: the folder that contains the exe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortablePaths {
    root: PathBuf,
}

impl PortablePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Root is the parent folder of the running executable.
    pub fn from_exe(exe: impl AsRef<Path>) -> io::Result<Self> {
        let parent = exe
            .as_ref()
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "executable path has no parent directory",
                )
            })?;
        Ok(Self::new(parent))
    }

    /// In dev builds the root is the project folder, so `bin\` and `data\` are not
    /// looked up inside `target\debug`.
    pub fn current() -> io::Result<Self> {
        #[cfg(debug_assertions)]
        if let Some(root) =
            option_env!("CARGO_MANIFEST_DIR").and_then(|dir| Path::new(dir).parent())
        {
            return Ok(Self::new(root));
        }
        Self::from_exe(std::env::current_exe()?)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn bin(&self) -> PathBuf {
        self.root.join(BIN_DIR)
    }

    pub fn data(&self) -> PathBuf {
        self.root.join(DATA_DIR)
    }

    pub fn thumbs(&self) -> PathBuf {
        self.data().join(THUMBS_DIR)
    }

    pub fn audio(&self) -> PathBuf {
        self.root.join(AUDIO_DIR)
    }

    pub fn video(&self) -> PathBuf {
        self.root.join(VIDEO_DIR)
    }

    pub fn ytdlp(&self) -> PathBuf {
        self.bin().join("yt-dlp.exe")
    }

    pub fn ffmpeg(&self) -> PathBuf {
        self.bin().join("ffmpeg.exe")
    }

    pub fn ffprobe(&self) -> PathBuf {
        self.bin().join("ffprobe.exe")
    }

    /// Creates the folders the app owns and proves the root is writable.
    /// Fails instead of silently falling back to a system profile.
    pub fn ensure_layout(&self) -> io::Result<()> {
        for dir in [self.data(), self.thumbs(), self.audio(), self.video()] {
            fs::create_dir_all(&dir)?;
        }
        let probe = self.data().join(".write-probe");
        fs::write(&probe, b"ok")?;
        fs::remove_file(&probe)?;
        Ok(())
    }

    /// Resolves a user-supplied path and refuses anything outside the portable root.
    ///
    /// Lexical normalization only: symlinks inside the root are not followed.
    /// shortcut: no symlink resolution, revisit when the library starts following links.
    #[allow(dead_code)] // shortcut: used by the folder picker in M1
    pub fn resolve_inside(&self, candidate: impl AsRef<Path>) -> io::Result<PathBuf> {
        let root = normalize(&self.root);
        let joined = if candidate.as_ref().is_absolute() {
            candidate.as_ref().to_path_buf()
        } else {
            root.join(candidate)
        };
        let resolved = normalize(&joined);
        if resolved.starts_with(&root) {
            Ok(resolved)
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "path {} is outside the portable root {}",
                    resolved.display(),
                    root.display()
                ),
            ))
        }
    }
}

#[allow(dead_code)] // shortcut: used by resolve_inside in M1
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_root(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("klipparkiv-paths-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn derives_every_path_from_root() {
        let paths = PortablePaths::new(PathBuf::from(r"C:\apps\klipparkiv"));

        assert_eq!(paths.root(), Path::new(r"C:\apps\klipparkiv"));
        assert_eq!(paths.bin(), PathBuf::from(r"C:\apps\klipparkiv\bin"));
        assert_eq!(paths.data(), PathBuf::from(r"C:\apps\klipparkiv\data"));
        assert_eq!(
            paths.thumbs(),
            PathBuf::from(r"C:\apps\klipparkiv\data\thumbs")
        );
        assert_eq!(paths.audio(), PathBuf::from(r"C:\apps\klipparkiv\audio"));
        assert_eq!(paths.video(), PathBuf::from(r"C:\apps\klipparkiv\video"));
        assert_eq!(
            paths.ytdlp(),
            PathBuf::from(r"C:\apps\klipparkiv\bin\yt-dlp.exe")
        );
        assert_eq!(
            paths.ffmpeg(),
            PathBuf::from(r"C:\apps\klipparkiv\bin\ffmpeg.exe")
        );
        assert_eq!(
            paths.ffprobe(),
            PathBuf::from(r"C:\apps\klipparkiv\bin\ffprobe.exe")
        );
    }

    #[test]
    fn from_exe_uses_parent_folder() {
        let paths = PortablePaths::from_exe(r"C:\games\portable\klipparkiv.exe").unwrap();
        assert_eq!(paths.root(), Path::new(r"C:\games\portable"));
    }

    #[test]
    fn from_exe_rejects_bare_file_name() {
        assert!(PortablePaths::from_exe("klipparkiv.exe").is_err());
    }

    #[test]
    fn ensure_layout_creates_owned_folders() {
        let root = temp_root("layout");
        let paths = PortablePaths::new(&root);

        paths.ensure_layout().unwrap();

        for dir in [paths.data(), paths.thumbs(), paths.audio(), paths.video()] {
            assert!(dir.is_dir(), "{} was not created", dir.display());
        }
        assert!(!paths.data().join(".write-probe").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_inside_accepts_relative_and_absolute_paths() {
        let root = temp_root("inside");
        let paths = PortablePaths::new(&root);

        assert_eq!(
            paths.resolve_inside("audio").unwrap(),
            normalize(&root.join("audio"))
        );
        assert_eq!(
            paths
                .resolve_inside(root.join("video").join("clip.mp4"))
                .unwrap(),
            normalize(&root.join("video").join("clip.mp4"))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_inside_rejects_escape_attempts() {
        let root = temp_root("escape");
        let paths = PortablePaths::new(&root);

        assert!(paths.resolve_inside("../outside.txt").is_err());
        assert!(paths.resolve_inside("audio/../../outside.txt").is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
