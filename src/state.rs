use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    #[default]
    Blue,
    Green,
}

impl Slot {
    pub fn other(self) -> Self {
        match self {
            Slot::Blue => Slot::Green,
            Slot::Green => Slot::Blue,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Blue => "blue",
            Slot::Green => "green",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedState {
    pub active_slot: Slot,
}

const STATE_FILE: &str = ".rollploy-state.json";

pub fn load(dir: &Path) -> Result<PersistedState> {
    let path = dir.join(STATE_FILE);
    if !path.exists() {
        return Ok(PersistedState::default());
    }
    let content = std::fs::read_to_string(&path).context("failed to read state file")?;
    serde_json::from_str(&content).context("failed to parse state file")
}

pub fn save(dir: &Path, state: &PersistedState) -> Result<()> {
    let path = dir.join(STATE_FILE);
    let content = serde_json::to_string_pretty(state).context("failed to serialize state")?;
    std::fs::write(&path, content).context("failed to write state file")
}
