use std::time::{Duration, Instant};

/// yt-dlp can print a progress line on every chunk; 10 events per second is
/// enough for a human eye and keeps the IPC bus and React quiet.
pub(crate) const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) struct ProgressThrottle {
    interval: Duration,
    last_emit: Option<Instant>,
}

impl ProgressThrottle {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_emit: None,
        }
    }

    /// `true` for the first call and then at most once per interval.
    pub(crate) fn allow(&mut self, now: Instant) -> bool {
        let allowed = match self.last_emit {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= self.interval,
        };
        if allowed {
            self.last_emit = Some(now);
        }
        allowed
    }
}
