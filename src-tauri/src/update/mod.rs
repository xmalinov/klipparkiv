//! Self-update: manifest parsing, signature checks, the verified download and
//! the per-platform installers. Everything here runs only when the app was
//! built with an update key.

pub(crate) mod check;
pub(crate) mod commands;
pub(crate) mod download;
pub(crate) mod http;
pub(crate) mod install;
pub(crate) mod install_macos;
#[cfg(windows)]
pub(crate) mod install_windows;
pub(crate) mod maintenance;
pub(crate) mod manifest;
pub(crate) mod state;
pub(crate) mod verify;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_support;

pub(crate) use commands::maybe_install_when_idle;
pub(crate) use maintenance::{cleanup_stale_state, schedule_startup_check};
pub(crate) use manifest::is_configured;
pub(crate) use state::UpdateStore;
