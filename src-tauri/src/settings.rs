use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub audio_subdir: String,
    pub video_subdir: String,
    pub concurrency: usize,
    pub theme: String,
    pub scan_depth: usize,
    pub autoscan: bool,
    pub embed_thumbnail: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio_subdir: "audio".into(),
            video_subdir: "video".into(),
            concurrency: 2,
            theme: "dark".into(),
            scan_depth: 3,
            autoscan: true,
            embed_thumbnail: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        validate_subdir(&self.audio_subdir)?;
        validate_subdir(&self.video_subdir)?;
        if !(1..=4).contains(&self.concurrency) {
            return Err("параллелизм должен быть от 1 до 4".into());
        }
        if !(1..=5).contains(&self.scan_depth) {
            return Err("глубина сканирования должна быть от 1 до 5".into());
        }
        if !["dark", "light", "system"].contains(&self.theme.as_str()) {
            return Err("неизвестная тема".into());
        }
        Ok(())
    }
}

fn validate_subdir(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    let single_component = path.components().count() == 1
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    if value.trim().is_empty() || !single_component {
        return Err(format!(
            "подпапка «{value}» должна быть одним именем внутри корня"
        ));
    }
    Ok(())
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let current = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .filter(|settings| settings.validate().is_ok())
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn save(&self, settings: Settings) -> Result<Settings, String> {
        settings.validate()?;
        let bytes = serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?;
        write_atomic(&self.path, &bytes).map_err(|error| error.to_string())?;
        *self.current.lock().unwrap() = settings.clone();
        Ok(settings)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("klipparkiv-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn defaults_when_file_is_missing() {
        let path = temp_path("missing");
        let store = SettingsStore::load(path);
        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn roundtrips_settings() {
        let path = temp_path("roundtrip");
        let store = SettingsStore::load(path.clone());
        let mut settings = store.get();
        settings.concurrency = 4;
        settings.audio_subdir = "music".into();
        settings.theme = "light".into();

        let saved = store.save(settings.clone()).unwrap();
        assert_eq!(saved, settings);

        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.get(), settings);
    }

    #[test]
    fn rejects_path_traversal_and_out_of_range() {
        let traversal = Settings {
            audio_subdir: "../escape".into(),
            ..Settings::default()
        };
        assert!(traversal.validate().is_err());

        let nested = Settings {
            video_subdir: "video/nested".into(),
            ..Settings::default()
        };
        assert!(nested.validate().is_err());

        let zero = Settings {
            concurrency: 0,
            ..Settings::default()
        };
        assert!(zero.validate().is_err());

        let deep = Settings {
            scan_depth: 9,
            ..Settings::default()
        };
        assert!(deep.validate().is_err());

        let neon = Settings {
            theme: "neon".into(),
            ..Settings::default()
        };
        assert!(neon.validate().is_err());
    }

    #[test]
    fn broken_file_falls_back_to_defaults() {
        let path = temp_path("broken");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{not json").unwrap();

        let store = SettingsStore::load(path);
        assert_eq!(store.get(), Settings::default());
    }
}
