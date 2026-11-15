use std::path::{Path, PathBuf};

use minisign_verify::{PublicKey, Signature};

use crate::i18n;
use crate::paths::AppPaths;
use crate::update::manifest::staged_file_name;

pub(crate) const PUBLIC_KEY: &str = include_str!("../../update.pub");

pub(crate) fn staged_path(paths: &AppPaths, version: &str) -> PathBuf {
    paths.update().join(version).join(staged_file_name())
}

pub(crate) fn signed_version(trusted_comment: &str) -> Option<&str> {
    trusted_comment
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"))
}

pub(crate) fn decode_signature(signature: &str) -> Result<Signature, String> {
    use base64::Engine;

    let trimmed = signature.trim();
    // Tauri writes the signature as base64 of the minisign text. Accept the
    // raw minisign text too, which keeps hand-written manifests testable.
    let text = match base64::engine::general_purpose::STANDARD.decode(trimmed) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|error| {
            i18n::tf(
                || format!("подпись обновления не в UTF-8: {error}"),
                || format!("the update signature is not UTF-8: {error}"),
            )
        })?,
        Err(_) => trimmed.to_string(),
    };
    Signature::decode(&text).map_err(|error| {
        i18n::tf(
            || format!("некорректная подпись обновления: {error}"),
            || format!("invalid update signature: {error}"),
        )
    })
}

pub(crate) fn verify_signature_only(data: &[u8], signature: &str) -> Result<Signature, String> {
    let key = PublicKey::decode(PUBLIC_KEY.trim()).map_err(|error| {
        i18n::tf(
            || format!("некорректный публичный ключ обновлений: {error}"),
            || format!("invalid update public key: {error}"),
        )
    })?;
    let signature = decode_signature(signature)?;
    key.verify(data, &signature, true).map_err(|error| {
        i18n::tf(
            || format!("подпись обновления не прошла проверку: {error}"),
            || format!("the update signature is invalid: {error}"),
        )
    })?;
    Ok(signature)
}

pub(crate) fn verify_file(path: &Path, signature: &str, version: &str) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|error| {
        i18n::tf(
            || format!("не удалось прочитать обновление: {error}"),
            || format!("could not read the update: {error}"),
        )
    })?;
    let signature = verify_signature_only(&data, signature)?;
    match signed_version(signature.trusted_comment()) {
        Some(signed) if signed == version => Ok(()),
        Some(signed) => Err(i18n::tf(
            || format!("обновление подписано для версии {signed}, а не {version}"),
            || format!("the update is signed for version {signed}, not {version}"),
        )),
        None => Err(i18n::t(
            "подпись обновления не содержит версию",
            "the update signature does not contain a version",
        )),
    }
}
