/// Machine-readable line: only these are parsed, human output is ignored.
pub const PROGRESS_MARKER: &str = "__PROGRESS__";

pub const PROGRESS_TEMPLATE: &str = concat!(
    "download:__PROGRESS__",
    "%(progress._percent_str)s|",
    "%(progress.downloaded_bytes)s|",
    "%(progress.total_bytes_estimate)s|",
    "%(progress.speed)s|",
    "%(progress.eta)s|",
    "%(info.playlist_index)s|",
    "%(info.n_entries)s"
);

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub percent: f32,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub playlist_index: Option<u32>,
    pub playlist_count: Option<u32>,
}

/// Parses `__PROGRESS__45.0%|123|456|2.5|12|2|10`.
/// Returns `None` for any other line, so stderr noise can be ignored safely.
pub fn parse_line(line: &str) -> Option<Progress> {
    let payload = line.trim().strip_prefix(PROGRESS_MARKER)?;
    let mut parts = payload.split('|');

    let percent = parse_percent(parts.next()?)?;
    let downloaded_bytes = parse_u64(parts.next()?);
    let total_bytes = parse_u64(parts.next()?);
    let speed_bps = parse_f64(parts.next()?);
    let eta_secs = parse_u64(parts.next()?);
    // Older yt-dlp builds print nothing after the ETA, so both fields are optional.
    let playlist_index = parts.next().and_then(parse_u32);
    let playlist_count = parts.next().and_then(parse_u32);

    Some(Progress {
        percent,
        downloaded_bytes,
        total_bytes,
        speed_bps,
        eta_secs,
        playlist_index,
        playlist_count,
    })
}

fn parse_percent(raw: &str) -> Option<f32> {
    let cleaned = raw.trim().trim_end_matches('%').trim();
    let value: f32 = cleaned.parse().ok()?;
    (value.is_finite() && (0.0..=100.0).contains(&value)).then_some(value)
}

fn parse_u64(raw: &str) -> Option<u64> {
    let cleaned = raw.trim();
    if cleaned.is_empty() || cleaned.eq_ignore_ascii_case("NA") {
        return None;
    }
    cleaned.parse().ok()
}

fn parse_u32(raw: &str) -> Option<u32> {
    let cleaned = raw.trim();
    if cleaned.is_empty() || cleaned.eq_ignore_ascii_case("NA") {
        return None;
    }
    cleaned.parse().ok()
}

fn parse_f64(raw: &str) -> Option<f64> {
    let cleaned = raw.trim();
    if cleaned.is_empty() || cleaned.eq_ignore_ascii_case("NA") {
        return None;
    }
    let value: f64 = cleaned.parse().ok()?;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_progress_line() {
        let progress = parse_line("__PROGRESS__ 45.2%|1048576|4194304|2.5|12|2|10").unwrap();

        assert_eq!(
            progress,
            Progress {
                percent: 45.2,
                downloaded_bytes: Some(1_048_576),
                total_bytes: Some(4_194_304),
                speed_bps: Some(2.5),
                eta_secs: Some(12),
                playlist_index: Some(2),
                playlist_count: Some(10),
            }
        );
    }

    #[test]
    fn tolerates_unknown_values_and_missing_playlist_fields() {
        let progress = parse_line("__PROGRESS__  3.0%|NA|NA|NA|NA").unwrap();

        assert_eq!(progress.percent, 3.0);
        assert_eq!(progress.downloaded_bytes, None);
        assert_eq!(progress.total_bytes, None);
        assert_eq!(progress.speed_bps, None);
        assert_eq!(progress.eta_secs, None);
        assert_eq!(progress.playlist_index, None);
        assert_eq!(progress.playlist_count, None);
    }

    #[test]
    fn ignores_regular_ytdlp_output() {
        assert!(parse_line("[download]  45.2% of 4.00MiB").is_none());
        assert!(parse_line("ERROR: video unavailable").is_none());
        assert!(parse_line("").is_none());
    }

    #[test]
    fn rejects_broken_percent() {
        assert!(parse_line("__PROGRESS__abc|1|2|3|4").is_none());
        assert!(parse_line("__PROGRESS__120%|1|2|3|4").is_none());
    }

    #[test]
    fn template_contains_every_placeholder() {
        for field in [
            "%(progress._percent_str)s",
            "%(progress.downloaded_bytes)s",
            "%(progress.total_bytes_estimate)s",
            "%(progress.speed)s",
            "%(progress.eta)s",
            "%(info.playlist_index)s",
            "%(info.n_entries)s",
        ] {
            assert!(PROGRESS_TEMPLATE.contains(field), "missing {field}");
        }
    }
}
