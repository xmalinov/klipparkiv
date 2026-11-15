use std::path::{Path, PathBuf};

use crate::i18n;

#[cfg(target_os = "macos")]
use crate::paths::AppPaths;
#[cfg(target_os = "macos")]
use crate::queue::TaskStore;
#[cfg(target_os = "macos")]
use crate::update::download::download_asset_file;
#[cfg(target_os = "macos")]
use crate::update::manifest::current_version;
#[cfg(target_os = "macos")]
use crate::update::state::{
    STATE_ERROR, StagedAsset, UpdateStatus, UpdateStore, error_status, installing_status,
    manual_status, set_status,
};
#[cfg(target_os = "macos")]
use crate::update::verify::{staged_path, verify_file};
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
#[cfg(target_os = "macos")]
pub(crate) fn start_install_macos(
    app: &AppHandle,
    paths: &AppPaths,
    asset: &StagedAsset,
    staged: &Path,
) -> UpdateStatus {
    let target = match macos_bundle_path() {
        Ok(target) => target,
        Err(message) => return error_status(app, message),
    };

    let needs_manual =
        target.to_string_lossy().contains("/AppTranslocation/") || !can_write_next_to(&target);
    if needs_manual {
        if asset.manual.is_some() {
            return manual_status(app, asset);
        }
        return error_status(
            app,
            i18n::t(
                "автоматическое обновление недоступно, а DMG нет в манифесте",
                "automatic updates are unavailable and the manifest has no DMG",
            ),
        );
    }

    let extract_dir = paths.update().join(&asset.version).join("extract");
    let source = match extract_app_tarball(staged, &extract_dir) {
        Ok(source) => source,
        Err(message) => return error_status(app, message),
    };
    if let Err(message) = verify_macos_bundle(&source, &asset.version) {
        let _ = std::fs::remove_dir_all(&extract_dir);
        return error_status(app, message);
    }
    let helper = match prepare_macos_helper(paths, &source, &asset.version) {
        Ok(helper) => helper,
        Err(message) => return error_status(app, message),
    };

    let backup_dir = paths.update_backups();
    if let Err(error) = std::fs::create_dir_all(&backup_dir) {
        return error_status(
            app,
            i18n::tf(
                || format!("не удалось создать папку резервной копии: {error}"),
                || format!("could not create the backup folder: {error}"),
            ),
        );
    }
    let backup = backup_dir.join(format!("Klipparkiv-{}.app", current_version()));
    let log = paths.updater_log();
    if let Some(parent) = log.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_status(
            app,
            i18n::tf(
                || format!("не удалось создать папку логов: {error}"),
                || format!("could not create the log folder: {error}"),
            ),
        );
    }

    let status = installing_status(app);
    app.state::<std::sync::Arc<TaskStore>>().set_paused(true);

    let mut command = std::process::Command::new(&helper);
    command
        .arg("--wait-pid")
        .arg(std::process::id().to_string())
        .arg("--source")
        .arg(&source)
        .arg("--target")
        .arg(&target)
        .arg("--backup")
        .arg(&backup)
        .arg("--log")
        .arg(&log)
        .arg("--support")
        .arg(paths.support())
        .arg("--relaunch")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    match command.spawn() {
        Ok(_) => {
            app.exit(0);
            status
        }
        Err(error) => {
            app.state::<std::sync::Arc<TaskStore>>().set_paused(false);
            error_status(
                app,
                i18n::tf(
                    || format!("не удалось запустить установщик обновления: {error}"),
                    || format!("could not start the update installer: {error}"),
                ),
            )
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_bundle_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| {
        i18n::tf(
            || format!("не удалось определить путь к приложению: {error}"),
            || format!("could not determine the app path: {error}"),
        )
    })?;
    let macos = exe.parent().ok_or_else(|| {
        i18n::t(
            "не удалось определить папку приложения",
            "could not determine the app folder",
        )
    })?;
    let contents = macos.parent().ok_or_else(|| {
        i18n::t(
            "приложение запущено не из .app-бандла",
            "the app is not running from an .app bundle",
        )
    })?;
    let bundle = contents.parent().ok_or_else(|| {
        i18n::t(
            "приложение запущено не из .app-бандла",
            "the app is not running from an .app bundle",
        )
    })?;
    if bundle.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return Err(i18n::t(
            "приложение запущено не из .app-бандла",
            "the app is not running from an .app bundle",
        ));
    }
    Ok(bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
pub(crate) fn can_write_next_to(bundle: &Path) -> bool {
    let Some(parent) = bundle.parent() else {
        return false;
    };
    let probe = parent.join(format!(".klipparkiv-write-probe-{}", std::process::id()));
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn extract_app_tarball(archive: &Path, dest: &Path) -> Result<PathBuf, String> {
    use std::path::Component;

    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest).map_err(|error| {
        i18n::tf(
            || format!("не удалось создать папку распаковки: {error}"),
            || format!("could not create the extraction folder: {error}"),
        )
    })?;

    let file = std::fs::File::open(archive).map_err(|error| {
        i18n::tf(
            || format!("не удалось открыть архив обновления: {error}"),
            || format!("could not open the update archive: {error}"),
        )
    })?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    let mut top: Option<PathBuf> = None;

    let entries = tar.entries().map_err(|error| {
        i18n::tf(
            || format!("не удалось прочитать архив обновления: {error}"),
            || format!("could not read the update archive: {error}"),
        )
    })?;
    for entry in entries {
        let mut entry = entry.map_err(|error| {
            i18n::tf(
                || format!("некорректная запись архива: {error}"),
                || format!("invalid archive entry: {error}"),
            )
        })?;
        let path = entry.path().map_err(|error| {
            i18n::tf(
                || format!("некорректный путь в архиве: {error}"),
                || format!("invalid archive path: {error}"),
            )
        })?;
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(i18n::t(
                "архив обновления содержит недопустимый путь",
                "the update archive contains an unsafe path",
            ));
        }
        let first = path
            .components()
            .find(|component| !matches!(component, Component::CurDir))
            .map(|component| PathBuf::from(component.as_os_str()))
            .ok_or_else(|| i18n::t("архив обновления пуст", "the update archive is empty"))?;
        match &top {
            Some(existing) if existing != &first => {
                return Err(i18n::t(
                    "архив обновления содержит несколько приложений",
                    "the update archive contains more than one app",
                ));
            }
            None => top = Some(first),
            _ => {}
        }
        if let Some(link) = entry.link_name().map_err(|error| {
            i18n::tf(
                || format!("некорректная ссылка в архиве: {error}"),
                || format!("invalid archive link: {error}"),
            )
        })? && (link.is_absolute()
            || link
                .components()
                .any(|component| matches!(component, Component::ParentDir)))
        {
            return Err(i18n::t(
                "архив обновления содержит недопустимую ссылку",
                "the update archive contains an unsafe link",
            ));
        }
        let unpacked = entry.unpack_in(dest).map_err(|error| {
            i18n::tf(
                || format!("не удалось распаковать обновление: {error}"),
                || format!("could not extract the update: {error}"),
            )
        })?;
        if !unpacked {
            return Err(i18n::t(
                "архив обновления содержит недопустимый путь",
                "the update archive contains an unsafe path",
            ));
        }
    }

    if top.as_deref() != Some(Path::new("Klipparkiv.app")) {
        return Err(i18n::t(
            "в архиве нет Klipparkiv.app",
            "the archive does not contain Klipparkiv.app",
        ));
    }
    let bundle = dest.join("Klipparkiv.app");
    if !bundle.is_dir() {
        return Err(i18n::t(
            "Klipparkiv.app в архиве повреждён",
            "Klipparkiv.app in the archive is damaged",
        ));
    }
    Ok(bundle)
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_macos_bundle(bundle: &Path, version: &str) -> Result<(), String> {
    let plist_path = bundle.join("Contents").join("Info.plist");
    let value = plist::Value::from_file(&plist_path).map_err(|error| {
        i18n::tf(
            || format!("не удалось прочитать Info.plist: {error}"),
            || format!("could not read Info.plist: {error}"),
        )
    })?;
    let dict = value.as_dictionary().ok_or_else(|| {
        i18n::t(
            "Info.plist обновления повреждён",
            "the update Info.plist is damaged",
        )
    })?;
    let identifier = dict
        .get("CFBundleIdentifier")
        .and_then(|value| value.as_string())
        .unwrap_or_default();
    if identifier != "com.klipparkiv.app" {
        return Err(i18n::tf(
            || format!("неожиданный bundle id обновления: {identifier}"),
            || format!("unexpected update bundle id: {identifier}"),
        ));
    }
    let bundle_version = dict
        .get("CFBundleShortVersionString")
        .and_then(|value| value.as_string())
        .unwrap_or_default();
    if bundle_version != version {
        return Err(i18n::tf(
            || format!("обновление содержит версию {bundle_version}, а не {version}"),
            || format!("the update contains version {bundle_version}, not {version}"),
        ));
    }

    let status = std::process::Command::new("codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(bundle)
        .status()
        .map_err(|error| {
            i18n::tf(
                || format!("не удалось запустить codesign: {error}"),
                || format!("could not run codesign: {error}"),
            )
        })?;
    if !status.success() {
        return Err(i18n::t(
            "подпись обновления не прошла проверку",
            "the update signature is invalid",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn prepare_macos_helper(
    paths: &AppPaths,
    source: &Path,
    version: &str,
) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let helper = source
        .join("Contents")
        .join("MacOS")
        .join("klipparkiv-updater");
    if !helper.is_file() {
        return Err(i18n::t(
            "в обновлении нет klipparkiv-updater",
            "the update does not contain klipparkiv-updater",
        ));
    }
    let dest = paths.update().join(version).join("klipparkiv-updater");
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            i18n::tf(
                || format!("не удалось создать папку установщика: {error}"),
                || format!("could not create the updater folder: {error}"),
            )
        })?;
    }
    std::fs::copy(&helper, &dest).map_err(|error| {
        i18n::tf(
            || format!("не удалось скопировать установщик: {error}"),
            || format!("could not copy the updater: {error}"),
        )
    })?;
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)).map_err(|error| {
        i18n::tf(
            || format!("не удалось выставить права установщика: {error}"),
            || format!("could not set updater permissions: {error}"),
        )
    })?;
    let status = std::process::Command::new("codesign")
        .args(["--verify", "--strict", "--verbose=2"])
        .arg(&dest)
        .status()
        .map_err(|error| {
            i18n::tf(
                || format!("не удалось проверить установщик: {error}"),
                || format!("could not verify the updater: {error}"),
            )
        })?;
    if !status.success() {
        return Err(i18n::t(
            "подпись установщика обновления недействительна",
            "the update installer signature is invalid",
        ));
    }
    Ok(dest)
}
