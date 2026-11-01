use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const BIN_DIR: &str = "bin";
pub const DATA_DIR: &str = "data";
pub const THUMBS_DIR: &str = "thumbs";
pub const AUDIO_DIR: &str = "audio";
pub const VIDEO_DIR: &str = "video";
pub const UPDATE_DIR: &str = "update";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const PORTABLE_MARKER: &str = ".klipparkiv-portable";

/// Whether the app owns a portable folder or is installed as a macOS bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    Portable,
    Installed,
}

/// All app paths are derived from two roots:
///
/// * `root` — the user-visible folder with the library, audio and video;
/// * `support` — app-owned state (updates, backups, logs, managed yt-dlp).
///
/// On Windows both live inside the portable folder, which keeps the existing
/// layout. On macOS the library lives in `~/Movies/Klipparkiv` and the support
/// state in `~/Library/Application Support/Klipparkiv`, so the `.app` bundle is
/// never written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    root: PathBuf,
    support: PathBuf,
    bin: PathBuf,
    kind: InstallKind,
}

impl AppPaths {
    /// Portable layout: every owned folder is below `root`.
    pub fn portable(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let bin = root.join(BIN_DIR);
        Self {
            support: root.join(DATA_DIR),
            bin,
            kind: InstallKind::Portable,
            root,
        }
    }

    /// Portable layout with a bin folder that is not `root/bin`, for example
    /// the resources inside a macOS `.app`.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn portable_with_bin(root: impl Into<PathBuf>, bin: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            support: root.join(DATA_DIR),
            bin: bin.into(),
            kind: InstallKind::Portable,
            root,
        }
    }

    /// Installed layout with explicit roots. Used by macOS and by tests.
    pub fn installed(
        root: impl Into<PathBuf>,
        support: impl Into<PathBuf>,
        bin: impl Into<PathBuf>,
    ) -> Self {
        Self {
            root: root.into(),
            support: support.into(),
            bin: bin.into(),
            kind: InstallKind::Installed,
        }
    }

    /// Resolves the layout from the running executable.
    ///
    /// A macOS `.app` bundle is detected from
    /// `.../Klipparkiv.app/Contents/MacOS/klipparkiv`. A `.klipparkiv-portable`
    /// marker next to the bundle switches to the portable layout.
    pub fn from_exe(exe: impl AsRef<Path>) -> io::Result<Self> {
        let exe = exe.as_ref();
        #[cfg(target_os = "macos")]
        {
            Self::from_exe_with_home(exe, &home_dir()?)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::from_exe_with_home(exe, Path::new(""))
        }
    }

    /// Testable variant of [`from_exe`] with an explicit home directory.
    #[allow(unused_variables)]
    fn from_exe_with_home(exe: &Path, home: &Path) -> io::Result<Self> {
        let parent = exe
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "executable path has no parent directory",
                )
            })?;

        #[cfg(target_os = "macos")]
        if let Some(bundle) = macos_bundle(exe) {
            if let Some(parent) = bundle.parent()
                && parent.join(PORTABLE_MARKER).is_file()
            {
                return Ok(Self::portable_with_bin(
                    parent,
                    bundle.join("Contents").join("Resources").join(BIN_DIR),
                ));
            }
            return Ok(Self::installed(
                home.join("Movies").join("Klipparkiv"),
                home.join("Library")
                    .join("Application Support")
                    .join("Klipparkiv"),
                bundle.join("Contents").join("Resources").join(BIN_DIR),
            ));
        }

        Ok(Self::portable(parent))
    }

    /// In dev builds the root is the project folder, so `bin/` and `data/` are
    /// not looked up inside `target/debug`.
    pub fn current() -> io::Result<Self> {
        #[cfg(debug_assertions)]
        if let Some(root) =
            option_env!("CARGO_MANIFEST_DIR").and_then(|dir| Path::new(dir).parent())
        {
            #[cfg(target_os = "macos")]
            {
                let resources = root.join("src-tauri").join("resources").join(BIN_DIR);
                if resources.is_dir() {
                    return Ok(Self::portable_with_bin(root, resources));
                }
            }
            return Ok(Self::portable(root));
        }
        Self::from_exe(std::env::current_exe()?)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn support(&self) -> &Path {
        &self.support
    }

    pub fn kind(&self) -> InstallKind {
        self.kind
    }

    pub fn bin(&self) -> PathBuf {
        self.bin.clone()
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

    /// Directory for downloaded updates, backups and the updater log.
    pub fn update(&self) -> PathBuf {
        self.support.join(UPDATE_DIR)
    }

    pub fn update_backups(&self) -> PathBuf {
        self.update().join("backup")
    }

    pub fn updater_log(&self) -> PathBuf {
        self.update().join("updater.log")
    }

    /// App-owned binaries that may be replaced without touching the bundle.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn managed_bin(&self) -> PathBuf {
        self.support.join(BIN_DIR)
    }

    /// The bundled yt-dlp is used as the source for the managed copy.
    pub fn bundled_ytdlp(&self) -> PathBuf {
        self.bin.join(exe_name("yt-dlp"))
    }

    /// The executable the app should run. On macOS this is the managed copy so
    /// `yt-dlp --update` does not modify the signed `.app`.
    pub fn ytdlp(&self) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            let managed = self.managed_bin().join(exe_name("yt-dlp"));
            if managed.is_file() {
                return managed;
            }
        }
        self.bundled_ytdlp()
    }

    pub fn ffmpeg(&self) -> PathBuf {
        self.bin.join(exe_name("ffmpeg"))
    }

    pub fn ffprobe(&self) -> PathBuf {
        self.bin.join(exe_name("ffprobe"))
    }

    /// Creates the folders the app owns and proves the support root is writable.
    /// Fails instead of silently falling back to a system profile.
    pub fn ensure_layout(&self) -> io::Result<()> {
        for dir in [
            self.data(),
            self.thumbs(),
            self.audio(),
            self.video(),
            self.support.clone(),
        ] {
            fs::create_dir_all(&dir)?;
        }
        let probe = self.support.join(".write-probe");
        fs::write(&probe, b"ok")?;
        fs::remove_file(&probe)?;
        Ok(())
    }

    /// Resolves a user-supplied path and refuses anything outside the library root.
    #[allow(dead_code)] // shortcut: used by folder pickers and future import flows
    ///
    /// Lexical normalization only: symlinks inside the root are not followed.
    pub fn resolve_inside(&self, candidate: impl AsRef<Path>) -> io::Result<PathBuf> {
        resolve_inside(&self.root, candidate)
    }

    /// Same guard for app-owned state below `support`.
    #[allow(dead_code)] // shortcut: updater paths are constructed internally for now
    pub fn resolve_inside_support(&self, candidate: impl AsRef<Path>) -> io::Result<PathBuf> {
        resolve_inside(&self.support, candidate)
    }
}

fn resolve_inside(root: &Path, candidate: impl AsRef<Path>) -> io::Result<PathBuf> {
    let root = normalize(root);
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
                "path {} is outside the app root {}",
                resolved.display(),
                root.display()
            ),
        ))
    }
}

/// `yt-dlp` on Unix, `yt-dlp.exe` on Windows.
pub fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

#[cfg(target_os = "macos")]
fn macos_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if bundle.extension()? != "app" {
        return None;
    }
    Some(bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
fn home_dir() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cannot resolve the home directory"))
}

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
        let root = temp_root("layout-derive");
        let paths = AppPaths::portable(&root);

        assert_eq!(paths.root(), root);
        assert_eq!(paths.bin(), root.join(BIN_DIR));
        assert_eq!(paths.data(), root.join(DATA_DIR));
        assert_eq!(paths.thumbs(), root.join(DATA_DIR).join(THUMBS_DIR));
        assert_eq!(paths.audio(), root.join(AUDIO_DIR));
        assert_eq!(paths.video(), root.join(VIDEO_DIR));
        assert_eq!(paths.update(), root.join(DATA_DIR).join(UPDATE_DIR));
        assert_eq!(
            paths.bundled_ytdlp(),
            root.join(BIN_DIR).join(exe_name("yt-dlp"))
        );
        assert_eq!(paths.ffmpeg(), root.join(BIN_DIR).join(exe_name("ffmpeg")));
        assert_eq!(
            paths.ffprobe(),
            root.join(BIN_DIR).join(exe_name("ffprobe"))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn exe_names_match_the_platform() {
        if cfg!(windows) {
            assert_eq!(exe_name("yt-dlp"), "yt-dlp.exe");
        } else {
            assert_eq!(exe_name("yt-dlp"), "yt-dlp");
        }
    }

    #[test]
    fn from_exe_uses_parent_folder() {
        let root = temp_root("from-exe");
        let exe = root.join(exe_name("klipparkiv"));
        let paths = AppPaths::from_exe(&exe).unwrap();
        assert_eq!(paths.root(), root);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn from_exe_rejects_bare_file_name() {
        assert!(AppPaths::from_exe(exe_name("klipparkiv")).is_err());
    }

    #[test]
    fn installed_layout_uses_support_and_bundle_bin() {
        let root = temp_root("installed");
        let support = root.join("support");
        let bin = root.join("Klipparkiv.app/Contents/Resources/bin");
        let paths = AppPaths::installed(&root, &support, &bin);

        assert_eq!(paths.kind(), InstallKind::Installed);
        assert_eq!(paths.root(), root);
        assert_eq!(paths.support(), support);
        assert_eq!(paths.bin(), bin);
        assert_eq!(paths.update(), support.join(UPDATE_DIR));
        assert_eq!(paths.ffmpeg(), bin.join(exe_name("ffmpeg")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ensure_layout_creates_owned_folders() {
        let root = temp_root("layout");
        let paths = AppPaths::portable(&root);

        paths.ensure_layout().unwrap();

        for dir in [paths.data(), paths.thumbs(), paths.audio(), paths.video()] {
            assert!(dir.is_dir(), "{} was not created", dir.display());
        }
        assert!(paths.support().is_dir());
        assert!(!paths.support().join(".write-probe").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_inside_accepts_relative_and_absolute_paths() {
        let root = temp_root("inside");
        let paths = AppPaths::portable(&root);

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
        let paths = AppPaths::portable(&root);

        assert!(paths.resolve_inside("../outside.txt").is_err());
        assert!(paths.resolve_inside("audio/../../outside.txt").is_err());
        assert!(paths.resolve_inside_support("../../outside.txt").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bundle_uses_installed_layout() {
        let base = temp_root("macos-installed");
        let home = base.join("home");
        let apps = base.join("Applications");
        let bundle = apps.join("Klipparkiv.app");
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        let exe = bundle.join("Contents/MacOS/klipparkiv");

        let paths = AppPaths::from_exe_with_home(&exe, &home).unwrap();
        assert_eq!(paths.kind(), InstallKind::Installed);
        assert_eq!(paths.root(), home.join("Movies").join("Klipparkiv"));
        assert_eq!(
            paths.support(),
            home.join("Library")
                .join("Application Support")
                .join("Klipparkiv")
        );
        assert_eq!(
            paths.bin(),
            bundle.join("Contents").join("Resources").join(BIN_DIR)
        );

        fs::write(apps.join(PORTABLE_MARKER), b"ok").unwrap();
        let portable = AppPaths::from_exe_with_home(&exe, &home).unwrap();
        assert_eq!(portable.kind(), InstallKind::Portable);
        assert_eq!(portable.root(), apps);
        assert_eq!(portable.support(), apps.join(DATA_DIR));
        assert_eq!(
            portable.bin(),
            bundle.join("Contents").join("Resources").join(BIN_DIR)
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn detects_macos_bundle_layout() {
        let bundle = PathBuf::from("/Applications/Klipparkiv.app");
        let exe = bundle.join("Contents/MacOS/klipparkiv");
        assert_eq!(macos_bundle(&exe), Some(bundle));
        assert_eq!(macos_bundle(Path::new("/tmp/klipparkiv")), None);
    }
}
