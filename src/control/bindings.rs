//! Rebindable controls for every player, loaded from `input-bindings.ron` at the project
//! root and edited in game from the pause menu's Controls page (`ui::pause_menu`).
//!
//! Reading the devices is leafwing-input-manager's job: each player carries an
//! `InputMap<PlayerAction>` built from these bindings (`keyboard_map` for the keyboard
//! and mouse player, `gamepad_map` for a pad player), and `player_input` reads the
//! resulting `ActionState`. This module only owns *what is bound to what*, in a shape
//! the controls screen can show and edit: two keyboard/mouse slots and one pad button
//! per action.
//!
//! Same load/save shape as `GameSettings`: a missing or unparseable file falls back to
//! the defaults, and actions missing from the file are filled in from the defaults, so a
//! file written before an action existed still gets it.
//!
//! Pad buttons are Bevy's `GamepadButton` variants, which are layout neutral: `South` is
//! Cross on a DualShock and A on an Xbox pad. `pad_label` shows both.

use std::collections::BTreeMap;

use bevy::input::gamepad::Gamepad;
use bevy::prelude::{ButtonInput, Entity, GamepadButton, KeyCode, MouseButton, Reflect, Resource};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

pub const INPUT_BINDINGS_PATH: &str = "input-bindings.ron";

/// Everything a player can do. Keyboard and gamepad players share it; only the
/// `InputMap` differs.
#[derive(
    Actionlike,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Reflect,
    Serialize,
    Deserialize,
)]
pub enum PlayerAction {
    // Walking on keys, as four buttons so each one can be rebound on its own. The pad
    // walks with `Move` instead.
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Fire,
    Reload,
    NextWeapon,
    PrevWeapon,
    Weapon1,
    Weapon2,
    Weapon3,
    Weapon4,
    ThrowSpecial,
    Ability,
    /// Hold to revive a downed teammate.
    Interact,
    /// Switch the flashlight on or off.
    Flashlight,
    BuildMode,
    PlaceBuild,
    CancelBuild,
    NextBuildItem,
    PrevBuildItem,
    Pause,
    /// Left stick. Not rebindable.
    #[actionlike(DualAxis)]
    Move,
    /// Right stick. Not rebindable; the mouse aims through `mouse_aim` instead.
    #[actionlike(DualAxis)]
    Aim,
}

impl PlayerAction {
    /// The rows of the controls screen, in display order.
    pub const REBINDABLE: [Self; 22] = [
        Self::MoveUp,
        Self::MoveDown,
        Self::MoveLeft,
        Self::MoveRight,
        Self::Fire,
        Self::Reload,
        Self::NextWeapon,
        Self::PrevWeapon,
        Self::Weapon1,
        Self::Weapon2,
        Self::Weapon3,
        Self::Weapon4,
        Self::ThrowSpecial,
        Self::Ability,
        Self::Interact,
        Self::Flashlight,
        Self::BuildMode,
        Self::PlaceBuild,
        Self::CancelBuild,
        Self::NextBuildItem,
        Self::PrevBuildItem,
        Self::Pause,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::MoveUp => "Move up",
            Self::MoveDown => "Move down",
            Self::MoveLeft => "Move left",
            Self::MoveRight => "Move right",
            Self::Fire => "Fire / throw",
            Self::Reload => "Reload",
            Self::NextWeapon => "Next weapon",
            Self::PrevWeapon => "Previous weapon",
            Self::Weapon1 => "Weapon 1",
            Self::Weapon2 => "Weapon 2",
            Self::Weapon3 => "Weapon 3",
            Self::Weapon4 => "Weapon 4",
            Self::ThrowSpecial => "Grenade / molotov",
            Self::Ability => "Special ability",
            Self::Interact => "Interact / revive",
            Self::Flashlight => "Flashlight",
            Self::BuildMode => "Build mode",
            Self::PlaceBuild => "Place (build)",
            Self::CancelBuild => "Cancel (build)",
            Self::NextBuildItem => "Next build item",
            Self::PrevBuildItem => "Previous build item",
            Self::Pause => "Pause",
            Self::Move => "Move (stick)",
            Self::Aim => "Aim (stick)",
        }
    }
}

/// A keyboard key or a mouse button: the two things a keyboard player's slot can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KbmInput {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl KbmInput {
    pub fn just_pressed(
        self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        match self {
            Self::Key(key) => keys.just_pressed(key),
            Self::Mouse(button) => mouse.just_pressed(button),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Key(key) => key_label(key),
            Self::Mouse(button) => match button {
                MouseButton::Left => "Mouse left".into(),
                MouseButton::Right => "Mouse right".into(),
                MouseButton::Middle => "Mouse middle".into(),
                MouseButton::Back => "Mouse back".into(),
                MouseButton::Forward => "Mouse forward".into(),
                MouseButton::Other(n) => format!("Mouse {n}"),
            },
        }
    }
}

/// What one action is bound to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionBinding {
    /// Primary and secondary keyboard/mouse input.
    #[serde(default)]
    pub keys: [Option<KbmInput>; 2],
    #[serde(default)]
    pub pad: Option<GamepadButton>,
}

/// One cell of the controls screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingSlot {
    /// Index into `ActionBinding::keys`.
    Key(usize),
    Pad,
}

#[derive(Debug, Clone, PartialEq, Resource, Serialize, Deserialize)]
pub struct ControlBindings {
    #[serde(default)]
    pub actions: BTreeMap<PlayerAction, ActionBinding>,
    /// Sticks below this deflection read as centred (on top of Bevy's own dead zone).
    #[serde(default = "default_stick_dead_zone")]
    pub stick_dead_zone: f32,
    /// Analog buttons past this pull count as firing. Applies to `Fire` only; every other
    /// action uses Bevy's own press threshold.
    #[serde(default = "default_trigger_threshold")]
    pub trigger_threshold: f32,
}

const fn default_stick_dead_zone() -> f32 {
    0.2
}
const fn default_trigger_threshold() -> f32 {
    0.3
}

/// The shipped layout. DualShock names in the comments.
fn default_binding(action: PlayerAction) -> ActionBinding {
    use GamepadButton as Pad;
    use KbmInput::{Key, Mouse};
    let (keys, pad) = match action {
        PlayerAction::MoveUp => ([Some(Key(KeyCode::KeyW)), None], None),
        PlayerAction::MoveDown => ([Some(Key(KeyCode::KeyS)), None], None),
        PlayerAction::MoveLeft => ([Some(Key(KeyCode::KeyA)), None], None),
        PlayerAction::MoveRight => ([Some(Key(KeyCode::KeyD)), None], None),
        PlayerAction::Fire => (
            [Some(Mouse(MouseButton::Left)), Some(Key(KeyCode::Space))],
            Some(Pad::RightTrigger2), // R2
        ),
        PlayerAction::Reload => ([Some(Key(KeyCode::KeyR)), None], Some(Pad::RightTrigger)), // R1
        PlayerAction::NextWeapon => ([Some(Key(KeyCode::Tab)), None], Some(Pad::DPadUp)),
        PlayerAction::PrevWeapon => ([None, None], Some(Pad::DPadDown)),
        PlayerAction::Weapon1 => ([Some(Key(KeyCode::Digit1)), None], None),
        PlayerAction::Weapon2 => ([Some(Key(KeyCode::Digit2)), None], None),
        PlayerAction::Weapon3 => ([Some(Key(KeyCode::Digit3)), None], None),
        PlayerAction::Weapon4 => ([Some(Key(KeyCode::Digit4)), None], None),
        PlayerAction::ThrowSpecial => ([Some(Key(KeyCode::KeyG)), None], Some(Pad::LeftTrigger)), // L1
        PlayerAction::Ability => ([Some(Key(KeyCode::KeyQ)), None], Some(Pad::North)), // Triangle
        // Circle, shared with cancel-build, which only applies in build mode.
        PlayerAction::Interact => ([Some(Key(KeyCode::KeyE)), None], Some(Pad::East)),
        PlayerAction::Flashlight => ([Some(Key(KeyCode::KeyF)), None], Some(Pad::LeftThumb)), // L3
        PlayerAction::BuildMode => ([Some(Key(KeyCode::KeyB)), None], Some(Pad::West)), // Square
        PlayerAction::PlaceBuild => (
            [Some(Mouse(MouseButton::Left)), Some(Key(KeyCode::Space))],
            Some(Pad::South), // Cross
        ),
        PlayerAction::CancelBuild => (
            [Some(Key(KeyCode::Escape)), Some(Mouse(MouseButton::Right))],
            Some(Pad::East), // Circle
        ),
        PlayerAction::NextBuildItem => {
            ([Some(Key(KeyCode::ArrowRight)), None], Some(Pad::DPadRight))
        }
        PlayerAction::PrevBuildItem => ([Some(Key(KeyCode::ArrowLeft)), None], Some(Pad::DPadLeft)),
        PlayerAction::Pause => ([Some(Key(KeyCode::Escape)), None], Some(Pad::Start)),
        PlayerAction::Move | PlayerAction::Aim => ([None, None], None),
    };
    ActionBinding { keys, pad }
}

impl Default for ControlBindings {
    fn default() -> Self {
        Self {
            actions: PlayerAction::REBINDABLE
                .into_iter()
                .map(|action| (action, default_binding(action)))
                .collect(),
            stick_dead_zone: default_stick_dead_zone(),
            trigger_threshold: default_trigger_threshold(),
        }
    }
}

impl ControlBindings {
    pub fn load() -> Self {
        let path = std::path::Path::new(INPUT_BINDINGS_PATH);
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
            && let Ok(bindings) = ron::from_str::<Self>(&text)
        {
            return bindings.with_missing_defaults();
        }
        Self::default()
    }

    /// Load, and write the defaults out if there is no file yet, so the bindings are
    /// discoverable (and editable) without having to know the names up front.
    pub fn load_or_write_default() -> Self {
        let bindings = Self::load();
        if !std::path::Path::new(INPUT_BINDINGS_PATH).exists() {
            bindings.save();
        }
        bindings
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(INPUT_BINDINGS_PATH, text);
        }
    }

    /// Fill in every rebindable action the file did not mention. An action that *is*
    /// listed, even with every slot empty, is left alone: that is a deliberate unbind.
    fn with_missing_defaults(mut self) -> Self {
        for action in PlayerAction::REBINDABLE {
            self.actions
                .entry(action)
                .or_insert_with(|| default_binding(action));
        }
        self
    }

    pub fn binding(&self, action: PlayerAction) -> ActionBinding {
        self.actions.get(&action).cloned().unwrap_or_default()
    }

    /// Put `input` in `slot` (`None` clears it). A pad slot ignores a keyboard input and
    /// a key slot a pad button.
    pub fn set(&mut self, action: PlayerAction, slot: BindingSlot, input: Option<SlotInput>) {
        let binding = self.actions.entry(action).or_default();
        match (slot, input) {
            (BindingSlot::Key(i), None) => {
                if let Some(cell) = binding.keys.get_mut(i) {
                    *cell = None;
                }
            }
            (BindingSlot::Key(i), Some(SlotInput::Kbm(kbm))) => {
                if let Some(cell) = binding.keys.get_mut(i) {
                    *cell = Some(kbm);
                }
            }
            (BindingSlot::Pad, None) => binding.pad = None,
            (BindingSlot::Pad, Some(SlotInput::Pad(button))) => binding.pad = Some(button),
            (BindingSlot::Key(_), Some(SlotInput::Pad(_)))
            | (BindingSlot::Pad, Some(SlotInput::Kbm(_))) => {}
        }
    }

    /// What the controls screen shows in a cell.
    pub fn slot_label(&self, action: PlayerAction, slot: BindingSlot) -> String {
        let binding = self.binding(action);
        let label = match slot {
            BindingSlot::Key(i) => binding.keys.get(i).copied().flatten().map(KbmInput::label),
            BindingSlot::Pad => binding.pad.map(|b| pad_label(b).to_string()),
        };
        label.unwrap_or_else(|| "-".into())
    }

    /// The keyboard and mouse player's map. No gamepad inputs, so plugging in a pad does
    /// not drive this player too (leafwing reads *any* pad for a map without one).
    pub fn keyboard_map(&self) -> InputMap<PlayerAction> {
        let mut map = InputMap::default();
        for (action, binding) in &self.actions {
            for input in binding.keys.iter().flatten() {
                match *input {
                    KbmInput::Key(key) => map.insert(*action, key),
                    KbmInput::Mouse(button) => map.insert(*action, button),
                };
            }
        }
        map
    }

    /// A pad player's map, tied to that one pad.
    pub fn gamepad_map(&self, gamepad: Entity) -> InputMap<PlayerAction> {
        let mut map = InputMap::default();
        for (action, binding) in &self.actions {
            if let Some(button) = binding.pad {
                map.insert(*action, button);
            }
        }
        map.insert_dual_axis(
            PlayerAction::Move,
            GamepadStick::LEFT.with_circle_deadzone(self.stick_dead_zone),
        );
        map.insert_dual_axis(
            PlayerAction::Aim,
            GamepadStick::RIGHT.with_circle_deadzone(self.stick_dead_zone),
        );
        map.with_gamepad(gamepad)
    }

    /// For the screens that read devices with no player entity to hang an `ActionState`
    /// on (the pause menu, the respawn picker).
    pub fn kbm_just_pressed(
        &self,
        action: PlayerAction,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        self.binding(action)
            .keys
            .iter()
            .flatten()
            .any(|input| input.just_pressed(keys, mouse))
    }

    pub fn pad_just_pressed(&self, action: PlayerAction, gamepad: &Gamepad) -> bool {
        self.binding(action)
            .pad
            .is_some_and(|button| gamepad.just_pressed(button))
    }

    /// Do two actions share a keyboard/mouse input? Used to let Escape cancel build mode
    /// rather than also opening the pause menu.
    pub fn share_kbm(&self, a: PlayerAction, b: PlayerAction) -> bool {
        let b_keys = self.binding(b).keys;
        self.binding(a)
            .keys
            .iter()
            .flatten()
            .any(|input| b_keys.contains(&Some(*input)))
    }

    pub fn share_pad(&self, a: PlayerAction, b: PlayerAction) -> bool {
        let pad = self.binding(a).pad;
        pad.is_some() && pad == self.binding(b).pad
    }
}

/// An input captured by the controls screen, about to go in a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotInput {
    Kbm(KbmInput),
    Pad(GamepadButton),
}

/// `KeyW` -> `W`, `Digit1` -> `1`, `ArrowLeft` -> `Left arrow`. ASCII only (the default
/// UI font has nothing else).
pub fn key_label(key: KeyCode) -> String {
    let name = format!("{key:?}");
    if let Some(rest) = name.strip_prefix("Key") {
        return rest.to_string();
    }
    if let Some(rest) = name.strip_prefix("Digit") {
        return rest.to_string();
    }
    if let Some(rest) = name.strip_prefix("Arrow") {
        return format!("{rest} arrow");
    }
    match key {
        KeyCode::ShiftLeft => "Left shift".into(),
        KeyCode::ShiftRight => "Right shift".into(),
        KeyCode::ControlLeft => "Left ctrl".into(),
        KeyCode::ControlRight => "Right ctrl".into(),
        KeyCode::AltLeft => "Left alt".into(),
        KeyCode::AltRight => "Right alt".into(),
        KeyCode::Escape => "Esc".into(),
        _ => name,
    }
}

/// DualShock / Xbox names for a pad button.
pub const fn pad_label(button: GamepadButton) -> &'static str {
    match button {
        GamepadButton::South => "Cross / A",
        GamepadButton::East => "Circle / B",
        GamepadButton::North => "Triangle / Y",
        GamepadButton::West => "Square / X",
        GamepadButton::LeftTrigger => "L1 / LB",
        GamepadButton::LeftTrigger2 => "L2 / LT",
        GamepadButton::RightTrigger => "R1 / RB",
        GamepadButton::RightTrigger2 => "R2 / RT",
        GamepadButton::Select => "Select",
        GamepadButton::Start => "Start",
        GamepadButton::Mode => "Home",
        GamepadButton::LeftThumb => "L3",
        GamepadButton::RightThumb => "R3",
        GamepadButton::DPadUp => "D-pad up",
        GamepadButton::DPadDown => "D-pad down",
        GamepadButton::DPadLeft => "D-pad left",
        GamepadButton::DPadRight => "D-pad right",
        GamepadButton::C => "C",
        GamepadButton::Z => "Z",
        GamepadButton::Other(_) => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_bindings_file_parses() {
        // Guards against the checked-in file drifting out of sync with the struct:
        // `load` swallows parse errors, so a typo there would silently fall back.
        let text = std::fs::read_to_string(INPUT_BINDINGS_PATH)
            .expect("input-bindings.ron ships with the project");
        ron::from_str::<ControlBindings>(&text).expect("shipped bindings parse");
    }

    /// The checked-in file is the documented default layout; keep the two in step.
    #[test]
    fn the_shipped_bindings_file_is_the_default_layout() {
        let text = std::fs::read_to_string(INPUT_BINDINGS_PATH).expect("file ships");
        let shipped = ron::from_str::<ControlBindings>(&text).expect("parses");
        assert_eq!(shipped, ControlBindings::default());
    }

    #[test]
    fn the_left_mouse_button_fires_by_default() {
        let bindings = ControlBindings::default();
        assert!(
            bindings
                .binding(PlayerAction::Fire)
                .keys
                .contains(&Some(KbmInput::Mouse(MouseButton::Left)))
        );
    }

    #[test]
    fn every_rebindable_action_has_a_default_row() {
        let bindings = ControlBindings::default();
        for action in PlayerAction::REBINDABLE {
            assert!(bindings.actions.contains_key(&action), "{action:?} missing");
        }
    }

    #[test]
    fn a_partial_file_fills_the_rest_from_defaults() {
        let bindings = ron::from_str::<ControlBindings>(
            "(actions: { Ability: (keys: (Some(Key(KeyF)), None), pad: None) })",
        )
        .expect("partial parses")
        .with_missing_defaults();
        assert_eq!(
            bindings.binding(PlayerAction::Ability).keys,
            [Some(KbmInput::Key(KeyCode::KeyF)), None],
            "the override applies"
        );
        assert_eq!(
            bindings.binding(PlayerAction::Fire),
            default_binding(PlayerAction::Fire),
            "the rest fall back to defaults"
        );
    }

    #[test]
    fn a_listed_but_empty_action_stays_unbound() {
        let bindings = ron::from_str::<ControlBindings>(
            "(actions: { Ability: (keys: (None, None), pad: None) })",
        )
        .expect("parses")
        .with_missing_defaults();
        assert_eq!(
            bindings.binding(PlayerAction::Ability),
            ActionBinding::default()
        );
    }

    #[test]
    fn a_saved_file_round_trips() {
        let mut bindings = ControlBindings::default();
        bindings.set(
            PlayerAction::Fire,
            BindingSlot::Pad,
            Some(SlotInput::Pad(GamepadButton::LeftTrigger2)),
        );
        let text = ron::ser::to_string_pretty(&bindings, ron::ser::PrettyConfig::default())
            .expect("serializes");
        let back: ControlBindings = ron::from_str(&text).expect("round trips");
        assert_eq!(back, bindings);
    }

    #[test]
    fn set_puts_inputs_in_matching_slots_only() {
        let mut bindings = ControlBindings::default();
        let before = bindings.binding(PlayerAction::Reload);
        // A pad button cannot go in a key slot, nor a key in the pad slot.
        bindings.set(
            PlayerAction::Reload,
            BindingSlot::Key(0),
            Some(SlotInput::Pad(GamepadButton::South)),
        );
        bindings.set(
            PlayerAction::Reload,
            BindingSlot::Pad,
            Some(SlotInput::Kbm(KbmInput::Key(KeyCode::KeyT))),
        );
        assert_eq!(bindings.binding(PlayerAction::Reload), before);

        bindings.set(
            PlayerAction::Reload,
            BindingSlot::Key(1),
            Some(SlotInput::Kbm(KbmInput::Key(KeyCode::KeyT))),
        );
        bindings.set(PlayerAction::Reload, BindingSlot::Key(0), None);
        assert_eq!(
            bindings.binding(PlayerAction::Reload).keys,
            [None, Some(KbmInput::Key(KeyCode::KeyT))]
        );
    }

    #[test]
    fn escape_is_shared_by_pause_and_cancel_build() {
        let bindings = ControlBindings::default();
        assert!(bindings.share_kbm(PlayerAction::Pause, PlayerAction::CancelBuild));
        assert!(!bindings.share_pad(PlayerAction::Pause, PlayerAction::CancelBuild));
    }

    #[test]
    fn labels_are_short_and_ascii() {
        assert_eq!(key_label(KeyCode::KeyW), "W");
        assert_eq!(key_label(KeyCode::Digit3), "3");
        assert_eq!(key_label(KeyCode::ArrowLeft), "Left arrow");
        for action in PlayerAction::REBINDABLE {
            for slot in [BindingSlot::Key(0), BindingSlot::Key(1), BindingSlot::Pad] {
                let label = ControlBindings::default().slot_label(action, slot);
                assert!(label.is_ascii(), "{label}");
            }
        }
    }
}
