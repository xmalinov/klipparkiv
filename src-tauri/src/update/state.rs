use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::update::manifest::current_version;
pub(crate) const STATE_IDLE: &str = "idle";
pub(crate) const STATE_CHECKING: &str = "checking";
pub(crate) const STATE_AVAILABLE: &str = "available";
pub(crate) const STATE_DOWNLOADING: &str = "downloading";
pub(crate) const STATE_READY: &str = "ready";
pub(crate) const STATE_DEFERRED: &str = "deferred";
pub(crate) const STATE_INSTALLING: &str = "installing";
#[cfg(target_os = "macos")]
pub(crate) const STATE_MANUAL: &str = "manual";
pub(crate) const STATE_ERROR: &str = "error";
pub(crate) const STATE_CURRENT: &str = "current";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub state: String,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub size: Option<u64>,
    pub downloaded: u64,
    pub percent: f32,
    pub error: Option<String>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            state: STATE_IDLE.to_string(),
            current_version: current_version().to_string(),
            version: None,
            notes: None,
            size: None,
            downloaded: 0,
            percent: 0.0,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StagedAsset {
    pub(crate) version: String,
    pub(crate) notes: Option<String>,
    pub(crate) size: u64,
    pub(crate) url: Url,
    pub(crate) sha256: String,
    pub(crate) signature: String,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) manual: Option<ManualAsset>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ManualAsset {
    pub(crate) size: u64,
    pub(crate) url: Url,
    pub(crate) sha256: String,
    pub(crate) signature: String,
}

#[derive(Default)]
pub struct UpdateStore {
    status: Mutex<UpdateStatus>,
    asset: Mutex<Option<StagedAsset>>,
}

impl UpdateStore {
    pub(crate) fn status(&self) -> UpdateStatus {
        self.status.lock().unwrap().clone()
    }

    pub(crate) fn update_status<F>(&self, change: F) -> UpdateStatus
    where
        F: FnOnce(&mut UpdateStatus),
    {
        let mut status = self.status.lock().unwrap();
        change(&mut status);
        status.clone()
    }

    pub(crate) fn asset(&self) -> Option<StagedAsset> {
        self.asset.lock().unwrap().clone()
    }

    pub(crate) fn set_asset(&self, asset: Option<StagedAsset>) {
        *self.asset.lock().unwrap() = asset;
    }
}

pub(crate) fn emit_status(app: &AppHandle, status: &UpdateStatus) {
    let _ = app.emit("update://state", status);
}

pub(crate) fn set_status(app: &AppHandle, status: UpdateStatus) -> UpdateStatus {
    let status = app.state::<UpdateStore>().update_status(|current| {
        *current = status;
    });
    emit_status(app, &status);
    status
}

pub(crate) fn error_status(app: &AppHandle, message: impl Into<String>) -> UpdateStatus {
    let message = message.into();
    set_status(
        app,
        app.state::<UpdateStore>().update_status(|status| {
            status.state = STATE_ERROR.to_string();
            status.error = Some(message.clone());
        }),
    )
}

pub(crate) fn installing_status(app: &AppHandle) -> UpdateStatus {
    let store = app.state::<UpdateStore>();
    store.update_status(|status| {
        status.state = STATE_INSTALLING.to_string();
        status.error = None;
    });
    set_status(app, store.status())
}

#[cfg(target_os = "macos")]
pub(crate) fn manual_status(app: &AppHandle, asset: &StagedAsset) -> UpdateStatus {
    set_status(
        app,
        app.state::<UpdateStore>().update_status(|status| {
            status.state = STATE_MANUAL.to_string();
            status.version = Some(asset.version.clone());
            status.size = asset.manual.as_ref().map(|manual| manual.size);
            status.downloaded = 0;
            status.percent = 0.0;
            status.error = None;
        }),
    )
}
