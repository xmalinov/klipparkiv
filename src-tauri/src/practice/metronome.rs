use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPayload {
    pub bpm: f64,
    pub beats_per_bar: u8,
    pub bar_offset: u8,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct MetronomeGrid {
    pub bpm: f64,
    pub beats_per_bar: u8,
    pub bar_offset: u8,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
}

impl TryFrom<GridPayload> for MetronomeGrid {
    type Error = String;

    fn try_from(payload: GridPayload) -> Result<Self, Self::Error> {
        if !payload.bpm.is_finite() || !(20.0..=400.0).contains(&payload.bpm) {
            return Err("invalid metronome bpm".into());
        }
        if !(1..=16).contains(&payload.beats_per_bar) {
            return Err("invalid beats per bar".into());
        }
        if payload.bar_offset >= payload.beats_per_bar {
            return Err("bar offset is outside the bar".into());
        }
        if payload.beats.len() < 2 {
            return Err("not enough beats for a metronome".into());
        }
        if payload
            .beats
            .windows(2)
            .any(|pair| !pair[0].is_finite() || pair[1] <= pair[0])
        {
            return Err("beats must be finite and sorted".into());
        }
        if payload
            .downbeats
            .windows(2)
            .any(|pair| !pair[0].is_finite() || pair[1] < pair[0])
        {
            return Err("downbeats must be finite and sorted".into());
        }
        Ok(Self {
            bpm: payload.bpm,
            beats_per_bar: payload.beats_per_bar,
            bar_offset: payload.bar_offset,
            beats: payload.beats,
            downbeats: payload.downbeats,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickEvent {
    pub frame: usize,
    pub accent: bool,
}

/// Map beat crossings in a source-time block to output frames.
///
/// `source_start..source_end` is the source range represented by one audio
/// callback block, and `output_frames` is its length after stretching. The
/// mapping is linear inside one block, which is enough for a metronome click.
pub fn click_events(
    grid: &MetronomeGrid,
    source_start: f64,
    source_end: f64,
    output_frames: usize,
) -> Vec<ClickEvent> {
    let span = source_end - source_start;
    if output_frames == 0 || !span.is_finite() || span <= 0.0 {
        return Vec::new();
    }

    let first = grid.beats.partition_point(|beat| *beat < source_start);
    let mut events: Vec<ClickEvent> = Vec::new();
    for &beat in grid.beats[first..]
        .iter()
        .take_while(|beat| **beat < source_end)
    {
        let ratio = (beat - source_start) / span;
        let frame = (ratio * output_frames as f64).floor();
        if !(0.0..output_frames as f64).contains(&frame) {
            continue;
        }
        let accent = grid
            .downbeats
            .binary_search_by(|candidate| candidate.total_cmp(&beat))
            .is_ok();
        let event = ClickEvent {
            frame: frame as usize,
            accent,
        };
        if let Some(last) = events.last_mut()
            && last.frame == event.frame
        {
            last.accent |= event.accent;
        } else {
            events.push(event);
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> MetronomeGrid {
        MetronomeGrid {
            bpm: 120.0,
            beats_per_bar: 4,
            bar_offset: 0,
            beats: vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
            downbeats: vec![0.0, 2.0],
        }
    }

    #[test]
    fn clicks_map_to_block_frames() {
        let events = click_events(&grid(), 0.0, 1.0, 100);
        assert_eq!(
            events,
            vec![
                ClickEvent {
                    frame: 0,
                    accent: true
                },
                ClickEvent {
                    frame: 50,
                    accent: false
                }
            ]
        );
    }

    #[test]
    fn a_beat_at_the_end_belongs_to_the_next_block() {
        let events = click_events(&grid(), 0.0, 0.5, 100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].frame, 0);
    }

    #[test]
    fn downbeats_are_accented() {
        let events = click_events(&grid(), 2.0, 2.5, 100);
        assert_eq!(events.len(), 1);
        assert!(events[0].accent);
    }

    #[test]
    fn invalid_blocks_are_ignored() {
        assert!(click_events(&grid(), 1.0, 1.0, 100).is_empty());
        assert!(click_events(&grid(), 0.0, 1.0, 0).is_empty());
    }

    #[test]
    fn payload_validation_rejects_bad_grids() {
        let payload = GridPayload {
            bpm: 120.0,
            beats_per_bar: 4,
            bar_offset: 0,
            beats: vec![0.0, 0.5, 1.0],
            downbeats: vec![0.0],
        };
        assert!(MetronomeGrid::try_from(payload).is_ok());
    }
}
