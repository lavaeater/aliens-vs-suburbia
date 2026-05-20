use bevy::prelude::Resource;
use crate::assets::asset_definition::{AssetDefinition, ModelType};

pub const MAX_PLAYERS: usize = 4;

#[derive(Clone, PartialEq)]
pub enum SlotState {
    Empty,
    Selecting { def_index: usize },
    Confirmed { def_path: String },
}

#[derive(Resource)]
pub struct PlayerSetupState {
    pub slots: [SlotState; MAX_PLAYERS],
    /// All Player-typed def paths found in assets/defs/.
    pub player_defs: Vec<String>,
    pub dirty: bool,
}

impl Default for PlayerSetupState {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| SlotState::Empty),
            player_defs: scan_player_defs(),
            dirty: true,
        }
    }
}

impl PlayerSetupState {
    pub fn join(&mut self, slot: usize) {
        if slot >= MAX_PLAYERS { return; }
        if self.slots[slot] == SlotState::Empty {
            self.slots[slot] = SlotState::Selecting { def_index: 0 };
            self.dirty = true;
        }
    }

    pub fn confirm(&mut self, slot: usize) {
        if slot >= MAX_PLAYERS { return; }
        if let SlotState::Selecting { def_index } = self.slots[slot] {
            let path = self.player_defs.get(def_index).cloned().unwrap_or_default();
            self.slots[slot] = SlotState::Confirmed { def_path: path };
            self.dirty = true;
        }
    }

    pub fn cycle_next(&mut self, slot: usize) {
        if self.player_defs.is_empty() { return; }
        if let SlotState::Selecting { ref mut def_index } = self.slots[slot] {
            *def_index = (*def_index + 1) % self.player_defs.len();
            self.dirty = true;
        }
    }

    pub fn cycle_prev(&mut self, slot: usize) {
        if self.player_defs.is_empty() { return; }
        if let SlotState::Selecting { ref mut def_index } = self.slots[slot] {
            let len = self.player_defs.len();
            *def_index = (*def_index + len - 1) % len;
            self.dirty = true;
        }
    }

    pub fn any_confirmed(&self) -> bool {
        self.slots.iter().any(|s| matches!(s, SlotState::Confirmed { .. }))
    }

    pub fn confirmed_paths(&self) -> Vec<String> {
        self.slots.iter().filter_map(|s| {
            if let SlotState::Confirmed { def_path } = s { Some(def_path.clone()) } else { None }
        }).collect()
    }

    pub fn display_name(&self, slot: usize) -> String {
        match &self.slots[slot] {
            SlotState::Empty => format!("Player {}  --  press Enter to join", slot + 1),
            SlotState::Selecting { def_index } => {
                let name = self.player_defs.get(*def_index)
                    .map(|p| def_stem(p))
                    .unwrap_or("(no models)");
                format!("Player {}  ◀  {}  ▶  [Enter] confirm", slot + 1, name)
            }
            SlotState::Confirmed { def_path } => {
                format!("Player {}  [OK]  {}", slot + 1, def_stem(def_path))
            }
        }
    }
}

fn def_stem(path: &str) -> &str {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

/// Returns paths of all defs in assets/defs/ whose ModelType is Player.
fn scan_player_defs() -> Vec<String> {
    let dir = std::path::Path::new("assets/defs");
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut paths: Vec<String> = entries.flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? != "ron" { return None; }
            let text = std::fs::read_to_string(&p).ok()?;
            let def: AssetDefinition = ron::from_str(&text).ok()?;
            if matches!(def.model_type, ModelType::Player(_)) { Some(p.to_string_lossy().replace('\\', "/")) } else { None }
        })
        .collect();
    paths.sort();
    paths
}

/// Persisted roster written by PlayerSetupState once confirmed, read by spawn_players.
#[derive(Resource, Default)]
pub struct PlayerRoster {
    /// def file paths for each active player slot, in order.
    pub def_paths: Vec<String>,
}
