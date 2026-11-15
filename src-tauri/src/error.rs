//! Structured errors for the IPC boundary.
//!
//! A command returns `AppError` so the UI can branch on `code` without parsing
//! a localized sentence. The message is still human-readable and localized;
//! the code is stable across languages.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// Stable identifier: `invalidInput`, `busy`, `notFound`, `access`, `internal`.
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The user asked for something impossible: bad timecode, unknown mode, …
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new("invalidInput", message)
    }

    /// The request conflicts with the current state (a running task, a busy job).
    pub(crate) fn busy(message: impl Into<String>) -> Self {
        Self::new("busy", message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new("notFound", message)
    }

    pub(crate) fn access(message: impl Into<String>) -> Self {
        Self::new("access", message)
    }

    /// Anything the user cannot act on differently: a failed process, an I/O
    /// failure, a malformed backend response.
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }
}

/// Lets `?` keep working on the not-yet-migrated `Result<_, String>` helpers.
impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_code_and_message_for_the_ui() {
        let error = AppError::invalid_input("bad timecode");
        let value = serde_json::to_value(&error).unwrap();

        assert_eq!(value["code"], serde_json::json!("invalidInput"));
        assert_eq!(value["message"], serde_json::json!("bad timecode"));
    }

    #[test]
    fn a_plain_string_becomes_an_internal_error() {
        let error = AppError::from("yt-dlp failed".to_string());

        assert_eq!(error.code, "internal");
        assert_eq!(error.message, "yt-dlp failed");
        assert_eq!(error.to_string(), "yt-dlp failed");
    }

    #[test]
    fn codes_stay_stable_when_the_message_is_localized() {
        let english = AppError::busy("a running download cannot be removed");
        let russian = AppError::busy("незавершённую загрузку нельзя убрать");

        assert_eq!(english.code, russian.code);
        assert_ne!(english.message, russian.message);
    }
}
