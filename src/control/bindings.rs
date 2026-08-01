//! Remappable gamepad bindings, loaded from `gamepad-bindings.ron` at the project root.
//!
//! Same load/save shape as `GameSettings` — if the file is missing or unparseable the
//! defaults below are used, so deleting it always gets you back to a working layout.
//! Button names are Bevy's `GamepadButton` variants, which are layout-neutral: `South`
//! is Cross on a DualShock and A on an Xbox pad. The doc comments name the DualShock
//! equivalents since that's what this was developed against.
//!
//! Example `gamepad-bindings.ron`:
//! ```ron
//! (
//!     fire: RightTrigger2,
//!     build_mode: West,
//!     execute_build: South,
//!     exit_build: East,
//!     ability: North,
//!     next_build_item: DPadRight,
//!     prev_build_item: DPadLeft,
//!     stick_dead_zone: 0.2,
//!     trigger_threshold: 0.3,
//! )
//! ```

use bevy::prelude::{GamepadButton, Resource};
use serde::{Deserialize, Serialize};

pub const GAMEPAD_BINDINGS_PATH: &str = "gamepad-bindings.ron";

#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct GamepadBindings {
    /// Fire / throw. Analog: counts as held past `trigger_threshold`. Default R2.
    #[serde(default = "default_fire")]
    pub fire: GamepadButton,
    /// Toggle build mode on and off. Default Square.
    #[serde(default = "default_build_mode")]
    pub build_mode: GamepadButton,
    /// Place the previewed tile while in build mode. Default Cross.
    #[serde(default = "default_execute_build")]
    pub execute_build: GamepadButton,
    /// Leave build mode without placing. Default Circle.
    #[serde(default = "default_exit_build")]
    pub exit_build: GamepadButton,
    /// Fire the special ability. Default Triangle.
    #[serde(default = "default_ability")]
    pub ability: GamepadButton,
    /// Cycle the build indicator forward / back. Default D-pad right / left.
    #[serde(default = "default_next_build_item")]
    pub next_build_item: GamepadButton,
    #[serde(default = "default_prev_build_item")]
    pub prev_build_item: GamepadButton,
    /// Sticks below this deflection read as centred (on top of Bevy's own dead zone).
    #[serde(default = "default_stick_dead_zone")]
    pub stick_dead_zone: f32,
    /// Analog buttons past this pull read as pressed.
    #[serde(default = "default_trigger_threshold")]
    pub trigger_threshold: f32,
}

fn default_fire() -> GamepadButton { GamepadButton::RightTrigger2 }
fn default_build_mode() -> GamepadButton { GamepadButton::West }
fn default_execute_build() -> GamepadButton { GamepadButton::South }
fn default_exit_build() -> GamepadButton { GamepadButton::East }
fn default_ability() -> GamepadButton { GamepadButton::North }
fn default_next_build_item() -> GamepadButton { GamepadButton::DPadRight }
fn default_prev_build_item() -> GamepadButton { GamepadButton::DPadLeft }
fn default_stick_dead_zone() -> f32 { 0.2 }
fn default_trigger_threshold() -> f32 { 0.3 }

impl Default for GamepadBindings {
    fn default() -> Self {
        Self {
            fire: default_fire(),
            build_mode: default_build_mode(),
            execute_build: default_execute_build(),
            exit_build: default_exit_build(),
            ability: default_ability(),
            next_build_item: default_next_build_item(),
            prev_build_item: default_prev_build_item(),
            stick_dead_zone: default_stick_dead_zone(),
            trigger_threshold: default_trigger_threshold(),
        }
    }
}

impl GamepadBindings {
    pub fn load() -> Self {
        let path = std::path::Path::new(GAMEPAD_BINDINGS_PATH);
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
            && let Ok(bindings) = ron::from_str::<GamepadBindings>(&text)
        {
            return bindings;
        }
        GamepadBindings::default()
    }

    /// Load, and write the defaults out if there is no file yet, so the bindings are
    /// discoverable (and editable) without having to know the field names up front.
    pub fn load_or_write_default() -> Self {
        let bindings = Self::load();
        if !std::path::Path::new(GAMEPAD_BINDINGS_PATH).exists() {
            bindings.save();
        }
        bindings
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(GAMEPAD_BINDINGS_PATH, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_bindings_file_parses() {
        // Guards against the checked-in file drifting out of sync with the struct —
        // `load` swallows parse errors, so a typo there would silently fall back.
        let text = std::fs::read_to_string(GAMEPAD_BINDINGS_PATH)
            .expect("gamepad-bindings.ron ships with the project");
        ron::from_str::<GamepadBindings>(&text).expect("shipped bindings parse");
    }

    #[test]
    fn a_partial_file_fills_the_rest_from_defaults() {
        // Every field is #[serde(default)], so users can override just one button.
        let bindings: GamepadBindings = ron::from_str("(ability: West)").expect("partial parses");
        assert_eq!(bindings.ability, GamepadButton::West, "the override applies");
        assert_eq!(bindings.fire, default_fire(), "the rest fall back to defaults");
    }

    #[test]
    fn a_saved_file_round_trips() {
        let mut bindings = GamepadBindings::default();
        bindings.fire = GamepadButton::LeftTrigger2;
        let text = ron::ser::to_string_pretty(&bindings, ron::ser::PrettyConfig::default())
            .expect("serializes");
        let back: GamepadBindings = ron::from_str(&text).expect("round trips");
        assert_eq!(back.fire, GamepadButton::LeftTrigger2);
    }
}
