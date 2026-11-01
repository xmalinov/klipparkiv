use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// Sets the UI language used for backend-generated messages.
pub fn set_language(language: &str) {
    ENGLISH.store(language == "en", Ordering::Relaxed);
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Picks a message for an explicit language.
pub fn pick<'a>(language: &str, ru: &'a str, en: &'a str) -> &'a str {
    if language == "en" { en } else { ru }
}

/// Picks a static message in the current language.
pub fn t(ru: &str, en: &str) -> String {
    pick(if is_english() { "en" } else { "ru" }, ru, en).to_string()
}

/// Picks an owned, formatted message in the current language.
pub fn tf(ru: impl FnOnce() -> String, en: impl FnOnce() -> String) -> String {
    if is_english() { en() } else { ru() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_language_statically() {
        assert_eq!(pick("ru", "Привет", "Hello"), "Привет");
        assert_eq!(pick("en", "Привет", "Hello"), "Hello");
        assert_eq!(pick("other", "Привет", "Hello"), "Привет");
    }
}
