//! Tauri command handlers. They stay thin: parse the payload, call the domain
//! module, map the result to a `Result` the UI can display.

pub(crate) mod settings;
pub(crate) mod tasks;
