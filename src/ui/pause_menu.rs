//! The in-game pause menu, and its Controls page for rebinding `ControlBindings`.
//!
//! The pause binding (Esc / Start by default) opens it in a normal match (not in the
//! character editor, which owns Escape). `PauseState::Paused` freezes `Time<Virtual>`,
//! which stops physics (fixed timestep), animation and wave timers, and
//! `gate_player_input` closes every player's input, so clicking a menu button does not
//! shoot. The same binding, Escape, or Circle / B goes back: Controls -> main page -> resume.
//!
//! Rebinding: click a cell, then press the new key, mouse button or pad button (pad
//! column). Delete / Backspace clears the cell, Escape cancels. Each change is saved to
//! `input-bindings.ron` right away, and `sync_input_maps` rebuilds every player's map
//! from it.

use bevy::input::gamepad::Gamepad;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, TextStyle, UIBuilder};

use crate::character_editor::state::in_normal_game;
use crate::control::bindings::{BindingSlot, ControlBindings, KbmInput, PlayerAction, SlotInput};
use crate::control::components::{CharacterControl, ControlCommand, InputKeyboard};
use crate::control::gamepad_input::InputGamepad;
use crate::game_state::{GameState, PauseState};
use crate::player::components::Player;

pub struct PauseMenuPlugin;

impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<PauseState>()
            .init_resource::<RebindCapture>()
            .add_systems(
                Update,
                request_pause
                    .run_if(in_state(PauseState::Running))
                    .run_if(in_normal_game),
            )
            .add_systems(OnEnter(PauseState::Paused), enter_pause)
            .add_systems(OnExit(PauseState::Paused), exit_pause)
            .add_systems(
                Update,
                (
                    pause_menu_input,
                    show_pause_page.run_if(resource_exists_and_changed::<PausePage>),
                    refresh_binding_cells,
                )
                    .chain()
                    .run_if(in_state(PauseState::Paused)),
            );
    }
}

/// Which page of the pause menu is up. Changing it respawns the menu.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PausePage {
    #[default]
    Main,
    Controls,
}

/// The controls cell waiting for an input, if any.
#[derive(Resource, Debug, Default)]
pub struct RebindCapture {
    target: Option<(PlayerAction, BindingSlot)>,
    /// Set when a capture took a mouse button. The click's release would otherwise land
    /// on the cell as an `Activate` and start a new capture straight away.
    swallow_click: bool,
}

#[derive(Component)]
struct PauseMenuRoot;

/// One rebindable cell on the Controls page.
#[derive(Component, Debug, Clone, Copy)]
struct BindingCell {
    action: PlayerAction,
    slot: BindingSlot,
}

/// Open the pause menu on the pause binding, from the keyboard or any pad.
///
/// Escape is also the default cancel-build key, so a player in build mode leaves build
/// mode first (`player_input`) instead of also pausing. That only applies while the two
/// actions share an input on that device.
#[allow(clippy::type_complexity)]
fn request_pause(
    bindings: Res<ControlBindings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<(Entity, &Gamepad)>,
    players: Query<(&CharacterControl, Has<InputKeyboard>, Option<&InputGamepad>), With<Player>>,
    mut next: ResMut<NextState<PauseState>>,
) {
    let building = |device: Option<Entity>| {
        players.iter().any(|(control, keyboard, pad)| {
            let on_device = match device {
                None => keyboard,
                Some(pad_entity) => pad.is_some_and(|p| p.gamepad == Some(pad_entity)),
            };
            on_device && control.triggers.contains(&ControlCommand::Build)
        })
    };

    let from_keyboard = bindings.kbm_just_pressed(PlayerAction::Pause, &keys, &mouse)
        && !(building(None) && bindings.share_kbm(PlayerAction::Pause, PlayerAction::CancelBuild));
    let from_pad = gamepads.iter().any(|(entity, pad)| {
        bindings.pad_just_pressed(PlayerAction::Pause, pad)
            && !(building(Some(entity))
                && bindings.share_pad(PlayerAction::Pause, PlayerAction::CancelBuild))
    });
    if from_keyboard || from_pad {
        next.set(PauseState::Paused);
    }
}

fn enter_pause(
    mut time: ResMut<Time<Virtual>>,
    mut capture: ResMut<RebindCapture>,
    mut commands: Commands,
) {
    time.pause();
    *capture = RebindCapture::default();
    commands.insert_resource(PausePage::Main);
}

/// Also runs when the match ends while paused (Quit to menu): leaving `InGame` exits
/// the sub-state, so the clock is never left stopped.
fn exit_pause(mut time: ResMut<Time<Virtual>>, mut commands: Commands) {
    time.unpause();
    commands.remove_resource::<PausePage>();
}

/// Back / resume, and the rebind capture.
#[allow(clippy::too_many_arguments)]
fn pause_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    mut bindings: ResMut<ControlBindings>,
    mut capture: ResMut<RebindCapture>,
    page: Option<ResMut<PausePage>>,
    mut next: ResMut<NextState<PauseState>>,
) {
    // The swallowed click's release has been through picking (PreUpdate) by now.
    if capture.swallow_click && mouse.get_just_released().next().is_some() {
        capture.swallow_click = false;
    }

    if let Some((action, slot)) = capture.target {
        if keys.just_pressed(KeyCode::Escape) {
            capture.target = None;
            return;
        }
        if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
            bindings.set(action, slot, None);
            bindings.save();
            capture.target = None;
            return;
        }
        let input = match slot {
            BindingSlot::Key(_) => keys
                .get_just_pressed()
                .next()
                .map(|key| SlotInput::Kbm(KbmInput::Key(*key)))
                .or_else(|| {
                    mouse
                        .get_just_pressed()
                        .next()
                        .map(|button| SlotInput::Kbm(KbmInput::Mouse(*button)))
                }),
            BindingSlot::Pad => gamepads
                .iter()
                .find_map(|pad| pad.get_just_pressed().next().copied())
                .map(SlotInput::Pad),
        };
        if let Some(input) = input {
            capture.swallow_click = matches!(input, SlotInput::Kbm(KbmInput::Mouse(_)));
            bindings.set(action, slot, Some(input));
            bindings.save();
            capture.target = None;
        }
        return;
    }

    let back = keys.just_pressed(KeyCode::Escape)
        || bindings.kbm_just_pressed(PlayerAction::Pause, &keys, &mouse)
        || gamepads.iter().any(|pad| {
            pad.just_pressed(GamepadButton::East)
                || bindings.pad_just_pressed(PlayerAction::Pause, pad)
        });
    if !back {
        return;
    }
    match page.as_deref().copied() {
        Some(PausePage::Controls) => {
            if let Some(mut page) = page {
                *page = PausePage::Main;
            }
        }
        _ => next.set(PauseState::Running),
    }
}

/// (Re)spawn the menu for the current page.
fn show_pause_page(
    page: Res<PausePage>,
    roots: Query<Entity, With<PauseMenuRoot>>,
    theme: Res<LavaTheme>,
    mut commands: Commands,
) {
    for root in roots.iter() {
        commands.entity(root).despawn();
    }
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    // Full-screen dimmer over the game and the HUD.
    ui.insert_bundle((
        PauseMenuRoot,
        DespawnOnExit(PauseState::Paused),
        GlobalZIndex(100),
    ))
    .size_percent(100.0, 100.0)
    .absolute_position()
    .display_flex()
    .flex_column()
    .align_items_center()
    .justify_center()
    .bg_color(Color::srgba(0.0, 0.0, 0.0, 0.6));

    match *page {
        PausePage::Main => main_page(&mut ui, &theme),
        PausePage::Controls => controls_page(&mut ui, &theme),
    }
    ui.build();
}

fn panel(ui: &mut UIBuilder, theme: &LavaTheme, f: impl FnOnce(&mut UIBuilder)) {
    ui.with_child(|panel| {
        panel
            .display_flex()
            .flex_column()
            .align_items_center()
            .gap_px(16.0)
            .padding_all_px(32.0)
            .bg_color(theme.bg_color)
            .border_all_px(2.0, theme.border_color)
            .border_radius_all_px(8.0);
        f(panel);
    });
}

fn main_page(ui: &mut UIBuilder, theme: &LavaTheme) {
    panel(ui, theme, |panel| {
        panel.themed_header("Paused");
        panel.add_themed_button_observe(
            "Resume",
            |_| {},
            |_: On<Activate>, mut next: ResMut<NextState<PauseState>>| {
                next.set(PauseState::Running);
            },
        );
        panel.add_themed_button_observe(
            "Controls",
            |_| {},
            |_: On<Activate>, mut page: ResMut<PausePage>| {
                *page = PausePage::Controls;
            },
        );
        panel.add_themed_button_observe(
            "Quit to menu",
            |_| {},
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::Menu);
            },
        );
    });
}

const ACTION_COLUMN_PX: f32 = 220.0;
const CELL_PX: (f32, f32) = (160.0, 34.0);

fn controls_page(ui: &mut UIBuilder, theme: &LavaTheme) {
    let dim = theme.text.label_color;
    panel(ui, theme, |panel| {
        panel.max_height_px(860.0);
        panel.themed_header("Controls");
        panel.label(
            "Click a binding, then press a key, mouse button or pad button. \
             Delete clears it, Esc cancels.",
            16.0,
            dim,
        );

        // Column titles.
        panel.with_child(|row| {
            row.flex_row().column_gap_px(8.0);
            row.with_child(|c| {
                c.width_px(ACTION_COLUMN_PX)
                    .with_text("Action", Some(TextStyle::size_color(16.0, dim)));
            });
            for title in ["Key / mouse", "Alternate", "Gamepad"] {
                row.with_child(|c| {
                    c.width_px(CELL_PX.0)
                        .with_text(title, Some(TextStyle::size_color(16.0, dim)));
                });
            }
        });

        panel.scrollable_list_bounded(560.0, |list| {
            list.row_gap_px(4.0);
            for action in PlayerAction::REBINDABLE {
                list.with_child(|row| {
                    row.flex_row().align_items_center().column_gap_px(8.0);
                    row.with_child(|c| {
                        c.width_px(ACTION_COLUMN_PX).with_text(
                            action.label(),
                            Some(TextStyle::size_color(18.0, theme.text.header_color)),
                        );
                    });
                    for slot in [BindingSlot::Key(0), BindingSlot::Key(1), BindingSlot::Pad] {
                        row.add_button_observe(
                            "",
                            |btn| {
                                btn.size_px(CELL_PX.0, CELL_PX.1)
                                    .font_size(16.0)
                                    .insert(BindingCell { action, slot });
                            },
                            start_capture,
                        );
                    }
                });
            }
        });

        panel.with_child(|row| {
            row.flex_row().column_gap_px(16.0);
            row.add_themed_button_observe(
                "Reset to defaults",
                |btn| {
                    btn.width_px(260.0);
                },
                |_: On<Activate>, mut bindings: ResMut<ControlBindings>| {
                    bindings.actions = ControlBindings::default().actions;
                    bindings.save();
                },
            );
            row.add_themed_button_observe(
                "Back",
                |_| {},
                |_: On<Activate>, mut page: ResMut<PausePage>| {
                    *page = PausePage::Main;
                },
            );
        });
    });
}

fn start_capture(
    activate: On<Activate>,
    cells: Query<&BindingCell>,
    mut capture: ResMut<RebindCapture>,
    focus: Option<ResMut<InputFocus>>,
) {
    if capture.swallow_click {
        capture.swallow_click = false;
        return;
    }
    let Ok(cell) = cells.get(activate.entity) else {
        return;
    };
    capture.target = Some((cell.action, cell.slot));
    // A focused button activates on Space/Enter; with focus left on the cell, binding
    // Space would restart the capture it just finished.
    if let Some(mut focus) = focus {
        focus.clear();
    }
}

/// Keep every cell's text in step with the bindings and the capture.
fn refresh_binding_cells(
    bindings: Res<ControlBindings>,
    capture: Res<RebindCapture>,
    cells: Query<(&BindingCell, &Children)>,
    mut texts: Query<&mut Text>,
) {
    for (cell, children) in cells.iter() {
        let wanted = if capture.target == Some((cell.action, cell.slot)) {
            "press...".to_string()
        } else {
            bindings.slot_label(cell.action, cell.slot)
        };
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != wanted
            {
                text.0.clone_from(&wanted);
            }
        }
    }
}
