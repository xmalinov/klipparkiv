use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BpmSummary {
    pub raw: f64,
    pub suggested: f64,
    pub octave: f64,
    pub confidence: f64,
    pub beats_per_bar: u8,
    pub bar_offset: u8,
}

const ALLOWED_METERS: [u8; 5] = [2, 3, 4, 6, 8];

pub fn summarize(beats: &[f32], downbeats: &[f32]) -> BpmSummary {
    let beats: Vec<f64> = beats
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(f64::from)
        .collect();
    let downbeats: Vec<f64> = downbeats
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(f64::from)
        .collect();

    let intervals = beat_intervals(&beats);
    let median = median(&intervals).unwrap_or(0.0);
    let raw = if median > 0.0 { 60.0 / median } else { 0.0 };
    let (octave, suggested) = octave_correction(raw);
    let confidence = confidence(&intervals);
    let beats_per_bar = suggest_meter(&beats, &downbeats);
    let bar_offset = suggest_bar_offset(&beats, &downbeats, beats_per_bar);

    BpmSummary {
        raw,
        suggested,
        octave,
        confidence,
        beats_per_bar,
        bar_offset,
    }
}

pub fn beat_intervals(beats: &[f64]) -> Vec<f64> {
    beats
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|interval| *interval > 0.2 && *interval < 2.0)
        .collect()
}

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        Some((sorted[middle - 1] + sorted[middle]) / 2.0)
    } else {
        Some(sorted[middle])
    }
}

fn octave_correction(raw: f64) -> (f64, f64) {
    if !raw.is_finite() || raw <= 0.0 {
        return (1.0, 0.0);
    }
    // Beat trackers commonly halve or double the perceived tempo. Prefer a
    // value inside a musician-friendly range, but let the UI override it.
    if raw < 78.0 && raw * 2.0 <= 190.0 {
        (2.0, raw * 2.0)
    } else if raw > 185.0 && raw / 2.0 >= 55.0 {
        (0.5, raw / 2.0)
    } else {
        (1.0, raw)
    }
}

fn confidence(intervals: &[f64]) -> f64 {
    if intervals.len() < 3 {
        return 0.0;
    }
    let Some(center) = median(intervals) else {
        return 0.0;
    };
    if center <= 0.0 {
        return 0.0;
    }
    let deviations: Vec<f64> = intervals
        .iter()
        .map(|interval| (interval - center).abs())
        .collect();
    let mad = median(&deviations).unwrap_or(0.0);
    (1.0 - 3.0 * mad / center).clamp(0.0, 1.0)
}

fn suggest_meter(beats: &[f64], downbeats: &[f64]) -> u8 {
    if beats.len() < 2 || downbeats.len() < 2 {
        return 4;
    }
    let indices: Vec<usize> = downbeats
        .iter()
        .map(|time| nearest_index(beats, *time))
        .collect();
    let gaps: Vec<f64> = indices
        .windows(2)
        .map(|pair| (pair[1] as f64 - pair[0] as f64).abs())
        .filter(|gap| *gap >= 1.0 && *gap <= 16.0)
        .collect();
    let Some(gap) = median(&gaps) else {
        return 4;
    };
    ALLOWED_METERS
        .into_iter()
        .min_by(|left, right| {
            (*left as f64 - gap)
                .abs()
                .total_cmp(&(*right as f64 - gap).abs())
        })
        .unwrap_or(4)
}

fn suggest_bar_offset(beats: &[f64], downbeats: &[f64], beats_per_bar: u8) -> u8 {
    if beats.is_empty() || downbeats.is_empty() || beats_per_bar == 0 {
        return 0;
    }
    let first_beat = nearest_index(beats, beats[0]);
    let first_downbeat = nearest_index(beats, downbeats[0]);
    let offset = first_downbeat.saturating_sub(first_beat);
    (offset % beats_per_bar as usize) as u8
}

fn nearest_index(beats: &[f64], time: f64) -> usize {
    let index = beats.partition_point(|beat| *beat < time);
    if index == 0 {
        return 0;
    }
    if index >= beats.len() {
        return beats.len().saturating_sub(1);
    }
    let before = (time - beats[index - 1]).abs();
    let after = (beats[index] - time).abs();
    if before <= after { index - 1 } else { index }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_handles_odd_and_even_lengths() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn half_time_is_doubled() {
        let beats: Vec<f32> = (0..32).map(|index| index as f32 * 0.86).collect();
        let summary = summarize(&beats, &[]);
        assert!((summary.raw - 69.77).abs() < 0.1);
        assert_eq!(summary.octave, 2.0);
        assert!((summary.suggested - 139.53).abs() < 0.2);
    }

    #[test]
    fn normal_tempo_is_kept() {
        let beats: Vec<f32> = (0..32).map(|index| index as f32 * 0.5).collect();
        let summary = summarize(&beats, &[]);
        assert_eq!(summary.octave, 1.0);
        assert!((summary.suggested - 120.0).abs() < 0.01);
    }

    #[test]
    fn meter_comes_from_downbeat_spacing() {
        let beats: Vec<f32> = (0..24).map(|index| index as f32 * 0.5).collect();
        let downbeats = vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0];
        let summary = summarize(&beats, &downbeats);
        assert_eq!(summary.beats_per_bar, 4);
        assert_eq!(summary.bar_offset, 0);
    }

    #[test]
    fn empty_input_is_safe() {
        let summary = summarize(&[], &[]);
        assert_eq!(summary.raw, 0.0);
        assert_eq!(summary.suggested, 0.0);
        assert_eq!(summary.beats_per_bar, 4);
    }
}
