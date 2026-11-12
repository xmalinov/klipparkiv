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

const ALLOWED_METERS: [u8; 11] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

pub fn summarize(beats: &[f32], downbeats: &[f32], downbeat_confidence: &[f32]) -> BpmSummary {
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
    let beats_per_bar = suggest_meter(&beats, &downbeats, downbeat_confidence);
    let bar_offset = suggest_bar_offset(&beats, &downbeats, downbeat_confidence, beats_per_bar);

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

fn downbeat_weight(confidence: &[f32], index: usize) -> f64 {
    confidence
        .get(index)
        .copied()
        .filter(|value| value.is_finite())
        .map(|value| f64::from(value).clamp(0.0, 1.0))
        .unwrap_or(1.0)
}

fn suggest_meter(beats: &[f64], downbeats: &[f64], confidence: &[f32]) -> u8 {
    if beats.len() < 2 || downbeats.len() < 3 {
        return 4;
    }
    let indices: Vec<usize> = downbeats
        .iter()
        .map(|time| nearest_index(beats, *time))
        .collect();

    let mut best_meter = 4;
    let mut best_score = 0.0f64;
    for meter in ALLOWED_METERS {
        let unit = f64::from(meter);
        let mut score = 0.0f64;
        for (pair_index, pair) in indices.windows(2).enumerate() {
            let gap = (pair[1] as f64 - pair[0] as f64).abs();
            if !(2.0..=16.0).contains(&gap) {
                // A one-beat gap is usually a false downbeat; it should not
                // vote for a 2/4 meter.
                continue;
            }
            let weight = (downbeat_weight(confidence, pair_index)
                + downbeat_weight(confidence, pair_index + 1))
                / 2.0;
            if weight <= 0.0 {
                continue;
            }
            if (gap - unit).abs() < 0.5 {
                score += weight;
            } else if gap > unit {
                let bars = (gap / unit).round();
                if bars >= 2.0 && (gap - bars * unit).abs() < 0.5 {
                    score += weight / bars;
                }
            }
        }
        // Prefer the simpler meter when two explanations are close.
        score -= 0.01 * unit;
        if score > best_score + 1e-9 {
            best_score = score;
            best_meter = meter;
        }
    }
    best_meter
}

fn suggest_bar_offset(
    beats: &[f64],
    downbeats: &[f64],
    confidence: &[f32],
    beats_per_bar: u8,
) -> u8 {
    if beats.is_empty() || downbeats.is_empty() || beats_per_bar == 0 {
        return 0;
    }
    let meter = beats_per_bar as usize;
    let mut weights = vec![0.0f64; meter];
    for (index, time) in downbeats.iter().enumerate() {
        let weight = downbeat_weight(confidence, index);
        if weight <= 0.0 {
            continue;
        }
        let beat = nearest_index(beats, *time);
        weights[beat % meter] += weight;
    }

    let mut best = 0usize;
    for offset in 1..meter {
        if weights[offset] > weights[best] + 1e-9 {
            best = offset;
        }
    }
    best as u8
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
        let summary = summarize(&beats, &[], &[]);
        assert!((summary.raw - 69.77).abs() < 0.1);
        assert_eq!(summary.octave, 2.0);
        assert!((summary.suggested - 139.53).abs() < 0.2);
    }

    #[test]
    fn normal_tempo_is_kept() {
        let beats: Vec<f32> = (0..32).map(|index| index as f32 * 0.5).collect();
        let summary = summarize(&beats, &[], &[]);
        assert_eq!(summary.octave, 1.0);
        assert!((summary.suggested - 120.0).abs() < 0.01);
    }

    #[test]
    fn meter_comes_from_downbeat_spacing() {
        let beats: Vec<f32> = (0..24).map(|index| index as f32 * 0.5).collect();
        let downbeats = vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0];
        let summary = summarize(&beats, &downbeats, &[]);
        assert_eq!(summary.beats_per_bar, 4);
        assert_eq!(summary.bar_offset, 0);
    }

    #[test]
    fn meter_detects_three_and_seven() {
        let beats: Vec<f32> = (0..35).map(|index| index as f32 * 0.5).collect();
        let waltz = vec![0.0, 1.5, 3.0, 4.5, 6.0, 7.5, 9.0];
        assert_eq!(summarize(&beats, &waltz, &[]).beats_per_bar, 3);

        let seven = vec![0.0, 3.5, 7.0, 10.5, 14.0];
        assert_eq!(summarize(&beats, &seven, &[]).beats_per_bar, 7);
    }

    #[test]
    fn meter_ignores_one_beat_false_downbeats() {
        let beats: Vec<f32> = (0..32).map(|index| index as f32 * 0.5).collect();
        // Mostly 4-beat bars with a few spurious downbeats one beat later.
        let downbeats = vec![0.0, 0.5, 2.0, 2.5, 4.0, 6.0, 8.0, 10.0];
        let summary = summarize(&beats, &downbeats, &[]);
        assert_eq!(summary.beats_per_bar, 4);
        assert_eq!(summary.bar_offset, 0);
    }

    #[test]
    fn phase_vote_ignores_a_weak_first_downbeat() {
        let beats: Vec<f32> = (0..20).map(|index| index as f32 * 0.5).collect();
        let downbeats = vec![0.5, 2.0, 4.0, 6.0, 8.0];
        let confidence = vec![0.05, 0.9, 0.9, 0.9, 0.9];
        let summary = summarize(&beats, &downbeats, &confidence);
        // Beat indices are 1, 4, 8, 12, 16; the strong votes are all mod 4 == 0.
        assert_eq!(summary.beats_per_bar, 4);
        assert_eq!(summary.bar_offset, 0);
    }

    #[test]
    fn confidence_can_change_the_detected_meter() {
        let beats: Vec<f32> = (0..32).map(|index| index as f32 * 0.5).collect();
        let downbeats = vec![0.0, 2.0, 4.0, 5.5, 7.0, 8.5, 10.0, 11.5];
        let confidence = vec![0.1, 0.1, 0.1, 0.9, 0.9, 0.9, 0.9, 0.9];
        assert_eq!(summarize(&beats, &downbeats, &confidence).beats_per_bar, 3);
    }

    #[test]
    fn random_inputs_keep_the_summary_invariants() {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 11) as f64) / ((1u64 << 53) as f64)
        };

        for _ in 0..100 {
            let count = 40 + (next() * 200.0) as usize;
            let mut beats: Vec<f32> = Vec::with_capacity(count);
            let mut time = 0.0f64;
            for _ in 0..count {
                beats.push(time as f32);
                time += 0.25 + next() * 0.6;
            }

            let mut downbeats: Vec<f32> = Vec::new();
            let mut confidence: Vec<f32> = Vec::new();
            let mut index = 0usize;
            while index < beats.len() {
                downbeats.push(beats[index]);
                confidence.push(next() as f32);
                index += if next() < 0.15 {
                    1
                } else {
                    2 + (next() * 6.0) as usize
                };
                if next() < 0.1 && index < beats.len() {
                    downbeats.push(beats[index]);
                    confidence.push((next() * 0.4) as f32);
                    index += 1;
                }
            }

            let summary = summarize(&beats, &downbeats, &confidence);
            assert!(ALLOWED_METERS.contains(&summary.beats_per_bar));
            assert!(summary.bar_offset < summary.beats_per_bar);
            assert!(summary.raw.is_finite());
            assert!(summary.suggested.is_finite());
            assert!(summary.octave.is_finite());
            assert!((0.0..=1.0).contains(&summary.confidence));
        }
    }

    #[test]
    fn empty_input_is_safe() {
        let summary = summarize(&[], &[], &[]);
        assert_eq!(summary.raw, 0.0);
        assert_eq!(summary.suggested, 0.0);
        assert_eq!(summary.beats_per_bar, 4);
    }
}
