use serde::{Deserialize, Serialize};

const STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub color: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub position_secs: f64,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub pitch_semitones: f32,
    #[serde(default)]
    pub cents: f32,
    #[serde(default)]
    pub loop_enabled: bool,
    #[serde(default = "default_true")]
    pub pre_roll: bool,
    #[serde(default)]
    pub active_region_id: Option<String>,
    #[serde(default)]
    pub regions: Vec<Region>,
}

fn default_version() -> u32 {
    STATE_VERSION
}

fn default_speed() -> f32 {
    1.0
}

fn default_volume() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

impl Default for PracticeState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            position_secs: 0.0,
            speed: 1.0,
            volume: 1.0,
            pitch_semitones: 0.0,
            cents: 0.0,
            loop_enabled: false,
            pre_roll: true,
            active_region_id: None,
            regions: Vec::new(),
        }
    }
}

impl PracticeState {
    pub fn sanitize(&mut self) {
        self.version = STATE_VERSION;
        if !self.position_secs.is_finite() {
            self.position_secs = 0.0;
        }
        self.position_secs = self.position_secs.max(0.0);
        if !self.speed.is_finite() {
            self.speed = 1.0;
        }
        self.speed = self.speed.clamp(0.25, 2.0);
        if !self.volume.is_finite() {
            self.volume = 1.0;
        }
        self.volume = self.volume.clamp(0.0, 1.0);
        if !self.pitch_semitones.is_finite() {
            self.pitch_semitones = 0.0;
        }
        self.pitch_semitones = self.pitch_semitones.clamp(-12.0, 12.0);
        if !self.cents.is_finite() {
            self.cents = 0.0;
        }
        self.cents = self.cents.clamp(-50.0, 50.0);

        self.regions.retain(|region| {
            !region.id.is_empty()
                && !region.name.trim().is_empty()
                && region.start.is_finite()
                && region.end.is_finite()
                && region.start >= 0.0
                && region.end > region.start
        });
        for region in &mut self.regions {
            if ![
                "intro", "verse", "chorus", "solo", "bridge", "outro", "custom",
            ]
            .contains(&region.kind.as_str())
            {
                region.kind = "custom".into();
            }
        }
        self.regions.sort_by(|left, right| {
            left.start
                .partial_cmp(&right.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(active) = self.active_region_id.as_deref()
            && !self.regions.iter().any(|region| region.id == active)
        {
            self.active_region_id = None;
        }
    }

    pub fn active_region(&self) -> Option<&Region> {
        let active = self.active_region_id.as_deref()?;
        self.regions.iter().find(|region| region.id == active)
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|error| error.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut state: Self = serde_json::from_str(text)
            .map_err(|error| format!("invalid practice state: {error}"))?;
        state.sanitize();
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(id: &str, start: f64, end: f64) -> Region {
        Region {
            id: id.into(),
            name: format!("Region {id}"),
            kind: "custom".into(),
            color: "#ffffff".into(),
            start,
            end,
        }
    }

    #[test]
    fn roundtrips_state() {
        let state = PracticeState {
            position_secs: 12.5,
            speed: 0.75,
            pitch_semitones: -2.0,
            regions: vec![region("a", 1.0, 2.0)],
            active_region_id: Some("a".into()),
            ..Default::default()
        };
        let json = state.to_json().unwrap();
        let parsed = PracticeState::from_json(&json).unwrap();
        assert_eq!(parsed.position_secs, 12.5);
        assert_eq!(parsed.speed, 0.75);
        assert_eq!(parsed.active_region().unwrap().name, "Region a");
    }

    #[test]
    fn sanitizes_out_of_range_values_and_bad_regions() {
        let mut state = PracticeState {
            speed: 9.0,
            pitch_semitones: -99.0,
            cents: 900.0,
            regions: vec![region("a", 2.0, 1.0), region("b", 0.0, 5.0)],
            active_region_id: Some("missing".into()),
            ..Default::default()
        };
        state.sanitize();
        assert_eq!(state.speed, 2.0);
        assert_eq!(state.pitch_semitones, -12.0);
        assert_eq!(state.cents, 50.0);
        assert_eq!(state.regions.len(), 1);
        assert_eq!(state.regions[0].id, "b");
        assert!(state.active_region_id.is_none());
    }
}
