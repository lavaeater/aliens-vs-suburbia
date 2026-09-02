use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, UIBuilder};
use crate::game_state::GameState;
use crate::house_editor::state::HouseEditorState;
use crate::ui::spawn_ui::StateMarker;

#[derive(Component)] pub struct HouseInfoLabel;

const PANEL_BG: Color = Color::srgba(0.04, 0.08, 0.05, 0.95);

pub fn spawn_house_editor_ui(commands: Commands, theme: Res<LavaTheme>, mut state: ResMut<HouseEditorState>) {
    *state = HouseEditorState::default();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row()
        .bg_color(Color::NONE);

    ui.side_panel(220.0, PANEL_BG, |left| {
        left.themed_header("House Editor");
        left.label("Click to place polygon nodes.", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
        left.label("Click the first (yellow) node,", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
        left.label("or press Enter, to close.", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
        left.label("RClick: cancel / clear.  C: clear all", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
        left.label("Up/Down: door count", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
        left.label("Left/Right: window spacing", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
        left.label("No grid, no snapping - whole numbers only.", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));

        left.with_child(|c| {
            c.with_text("", Some(lava_ui_builder::TextStyle::size_color(12.0, Color::srgb(0.85, 0.95, 0.85))))
             .insert(HouseInfoLabel);
        });

        left.add_button_observe("Clear", |b| { b.width(percent(100.0)).height(px(28.0)).font_size(13.0); },
            |_: On<Activate>, mut s: ResMut<HouseEditorState>| { s.clear_all(); });
        left.add_button_observe("<- Back to Menu", |b| { b.width(percent(100.0)).height(px(28.0)).font_size(13.0); },
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| { next.set(GameState::Menu); });
    });

    ui.with_child(|c| { c.with_flex_grow(1.0).height_percent(100.0); });

    ui.build();
}

pub fn rebuild_info_label(
    mut state: ResMut<HouseEditorState>,
    mut label_q: Query<&mut Text, With<HouseInfoLabel>>,
) {
    if !state.info_dirty { return; }
    state.info_dirty = false;
    let Ok(mut t) = label_q.single_mut() else { return };

    let shape_line = if state.points.is_empty() {
        "No polygon yet.".to_string()
    } else {
        format!("{} node(s) placed", state.points.len())
    };
    let result_line = match &state.resolved {
        Some(r) => format!(
            "Floor {}  Wall {}  Door {}  Window {}",
            r.floor_cells.len(), r.wall_count(), r.door_count(), r.window_count(),
        ),
        None => "Not resolved yet.".to_string(),
    };
    **t = format!(
        "{shape_line}\n{result_line}\n\nDoors: {}\nWindow: 1 per {:.0}u (min {:.0}u)",
        state.spec.door_count, state.spec.window_rule.max_per_wall_len, state.spec.window_rule.min_len_for_window,
    );
}

pub fn handle_house_editor_keys(
    mut state: ResMut<HouseEditorState>,
    mut next: ResMut<NextState<GameState>>,
    mut keyboard: MessageReader<KeyboardInput>,
) {
    for event in keyboard.read() {
        if event.state != ButtonState::Pressed { continue; }
        match &event.logical_key {
            Key::Character(c) if c == "c" || c == "C" => state.clear_all(),
            Key::Enter => state.close(),
            Key::ArrowUp => state.adjust_door_count(1),
            Key::ArrowDown => state.adjust_door_count(-1),
            Key::ArrowRight => state.adjust_window_spacing(1.0),
            Key::ArrowLeft => state.adjust_window_spacing(-1.0),
            Key::Escape => next.set(GameState::Menu),
            _ => {}
        }
    }
}
