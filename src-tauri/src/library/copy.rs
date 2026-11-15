use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::i18n;

const STEM_LIMIT: usize = 120;
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn split_name(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(index) if index > 0 && index + 1 < name.len() => {
            (name[..index].to_string(), name[index..].to_string())
        }
        _ => (name.to_string(), String::new()),
    }
}

/// Keeps only the base name and makes it safe for the destination folder.
pub(crate) fn sanitize_file_name(raw: &str, fallback: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .filter(|character| !character.is_control())
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect();
    let mut name = cleaned.trim_end_matches([' ', '.']).to_string();
    if name.is_empty() || name == "." || name == ".." {
        name = fallback.to_string();
    }
    let (mut stem, extension) = split_name(&name);
    if stem.chars().count() > STEM_LIMIT {
        stem = stem.chars().take(STEM_LIMIT).collect();
    }
    if stem.is_empty() {
        stem = fallback.to_string();
    }
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        stem.insert(0, '_');
    }
    format!("{stem}{extension}")
}

pub(crate) fn unique_target(
    dir: &Path,
    name: &str,
    taken: &mut HashSet<String>,
) -> Result<PathBuf, String> {
    let (stem, extension) = split_name(name);
    let mut counter = 1u32;
    loop {
        let candidate = if counter == 1 {
            format!("{stem}{extension}")
        } else {
            format!("{stem} ({counter}){extension}")
        };
        let key = candidate.to_lowercase();
        if !taken.contains(&key) && !dir.join(&candidate).exists() {
            taken.insert(key);
            return Ok(dir.join(candidate));
        }
        counter += 1;
        if counter > 999 {
            return Err(i18n::t(
                "слишком много файлов с таким именем",
                "too many files with this name",
            ));
        }
    }
}

pub(crate) fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub(crate) fn hash_cached(cache: &mut HashMap<String, String>, path: &Path) -> Option<String> {
    let key = path.to_string_lossy().into_owned();
    if let Some(hash) = cache.get(&key) {
        return Some(hash.clone());
    }
    let hash = file_sha256(path).ok()?;
    cache.insert(key, hash.clone());
    Some(hash)
}

pub(crate) fn set_modified_now(path: &Path) -> std::io::Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_modified(SystemTime::now())
}

pub(crate) enum CopyFailure {
    Cancelled,
    Failed(String),
}

pub(crate) fn copy_with_progress(
    source: &Path,
    part: &Path,
    total: u64,
    cancel: &AtomicBool,
    mut on_bytes: impl FnMut(u64),
) -> Result<u64, CopyFailure> {
    if cancel.load(Ordering::SeqCst) {
        return Err(CopyFailure::Cancelled);
    }
    let mut input =
        fs::File::open(source).map_err(|error| CopyFailure::Failed(error.to_string()))?;
    let mut output =
        fs::File::create(part).map_err(|error| CopyFailure::Failed(error.to_string()))?;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut copied = 0u64;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(CopyFailure::Cancelled);
        }
        let read = input
            .read(&mut buffer)
            .map_err(|error| CopyFailure::Failed(error.to_string()))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| CopyFailure::Failed(error.to_string()))?;
        copied += read as u64;
        on_bytes(copied);
    }
    output
        .flush()
        .map_err(|error| CopyFailure::Failed(error.to_string()))?;
    let _ = total;
    Ok(copied)
}
