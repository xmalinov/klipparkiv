use tauri::{AppHandle, Manager};

use crate::i18n;
use crate::paths::AppPaths;
use crate::update::state::{StagedAsset, UpdateStatus, error_status};
use crate::update::verify::staged_path;

#[cfg(target_os = "macos")]
use crate::update::install_macos::start_install_macos;
#[cfg(windows)]
use crate::update::install_windows::start_install_windows;
pub(crate) fn start_install(app: &AppHandle, asset: &StagedAsset) -> UpdateStatus {
    let paths = app.state::<AppPaths>().inner().clone();
    let staged = staged_path(&paths, &asset.version);
    if !staged.is_file() {
        return error_status(
            app,
            i18n::t("файл обновления не найден", "the update file was not found"),
        );
    }

    #[cfg(windows)]
    {
        start_install_windows(app, &paths, &staged)
    }
    #[cfg(target_os = "macos")]
    {
        start_install_macos(app, &paths, asset, &staged)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (app, &paths, asset, &staged);
        error_status(
            app,
            i18n::t(
                "обновления не поддерживаются на этой платформе",
                "updates are not supported on this platform",
            ),
        )
    }
}
