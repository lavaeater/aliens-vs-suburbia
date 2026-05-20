use bevy::input::ButtonState;
use bevy::input::gamepad::Gamepad;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use lava_ui_builder::{LavaTheme, TextTheme, UIBuilder};
use crate::game_state::GameState;
use crate::player_setup::state::{MAX_PLAYERS, PlayerRoster, PlayerSetupState, SlotState};
use crate::ui::spawn_ui::StateMarker;

#[derive(Component)]
pub struct SlotLabel(pub usize);

pub fn spawn_player_setup_ui(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<PlayerSetupState>,
) {
    // Reset state on each entry.
    *state = PlayerSetupState::default();
    state.dirty = true;

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    let t = theme.text.clone();
    let dim = TextTheme { label_size: 14.0, label_color: Color::srgba(0.5, 0.7, 0.5, 0.7), ..t.clone() };

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_column()
        .align_items_center()
        .justify_center()
        .gap_px(20.0)
        .bg_color(Color::srgba(0.04, 0.08, 0.05, 0.97));

    ui.with_child(|h| { h.insert_bundle(lava_ui_builder::header("Select Players", &t)); });
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("Arrow keys <> to pick model  |  Enter to join/confirm", &dim)); });

    for slot in 0..MAX_PLAYERS {
        ui.with_child(|row| {
            row.modify_node(|mut n| {
                n.width = Val::Percent(60.0);
                n.padding = UiRect::all(Val::Px(12.0));
                n.border_radius = BorderRadius::all(Val::Px(6.0));
            })
            .bg_color(Color::srgba(0.08, 0.14, 0.10, 0.85));

            row.with_child(|c| {
                c.insert_bundle(lava_ui_builder::label("", &TextTheme {
                    label_size: 16.0,
                    label_color: Color::srgb(0.75, 1.0, 0.80),
                    ..t.clone()
                })).insert(SlotLabel(slot));
            });
        });
    }

    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("At least one player must confirm before starting.", &dim)); });

    ui.build();
}

pub fn rebuild_slot_labels(
    mut state: ResMut<PlayerSetupState>,
    mut labels: Query<(&SlotLabel, &mut Text)>,
) {
    if !state.dirty { return; }
    state.dirty = false;

    for (label, mut text) in labels.iter_mut() {
        **text = state.display_name(label.0);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_setup_input(
    mut state: ResMut<PlayerSetupState>,
    mut roster: ResMut<PlayerRoster>,
    mut next_state: ResMut<NextState<GameState>>,
    mut keyboard: MessageReader<KeyboardInput>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut prev_gamepad_south: Local<[bool; MAX_PLAYERS]>,
    mut prev_gamepad_left: Local<[bool; MAX_PLAYERS]>,
    mut prev_gamepad_right: Local<[bool; MAX_PLAYERS]>,
) {
    // ── Keyboard: slot 0 ────────────────────────────────────────────────────
    for event in keyboard.read() {
        if event.state != ButtonState::Pressed { continue; }
        match &event.logical_key {
            Key::Enter => {
                match state.slots[0] {
                    SlotState::Empty => state.join(0),
                    SlotState::Selecting { .. } => state.confirm(0),
                    SlotState::Confirmed { .. } => {
                        if state.any_confirmed() {
                            roster.def_paths = state.confirmed_paths();
                            next_state.set(GameState::InGame);
                        }
                    }
                }
            }
            Key::ArrowRight => state.cycle_next(0),
            Key::ArrowLeft  => state.cycle_prev(0),
            Key::Escape => next_state.set(GameState::Menu),
            _ => {}
        }
    }

    // ── Gamepads: slots 1..MAX_PLAYERS ──────────────────────────────────────
    for (gp_idx, (_, gamepad)) in gamepads.iter().enumerate() {
        let slot = gp_idx + 1;
        if slot >= MAX_PLAYERS { break; }

        let south = gamepad.pressed(GamepadButton::South);
        let left  = gamepad.left_stick().x < -0.5 || gamepad.pressed(GamepadButton::DPadLeft);
        let right = gamepad.left_stick().x >  0.5 || gamepad.pressed(GamepadButton::DPadRight);

        // South: join / confirm / start (rising edge only)
        if south && !prev_gamepad_south[slot] {
            match state.slots[slot] {
                SlotState::Empty => state.join(slot),
                SlotState::Selecting { .. } => state.confirm(slot),
                SlotState::Confirmed { .. } => {
                    if state.any_confirmed() {
                        roster.def_paths = state.confirmed_paths();
                        next_state.set(GameState::InGame);
                    }
                }
            }
        }
        if left  && !prev_gamepad_left[slot]  { state.cycle_prev(slot); }
        if right && !prev_gamepad_right[slot] { state.cycle_next(slot); }

        prev_gamepad_south[slot] = south;
        prev_gamepad_left[slot]  = left;
        prev_gamepad_right[slot] = right;
    }
}
