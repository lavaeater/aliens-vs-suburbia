use crate::character_editor::ui::row;
use crate::game_state::GameState;
use crate::house_editor::state::HouseEditorState;
use crate::map::house::{self, HOUSE_DIR};
use crate::ui::spawn_ui::StateMarker;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, UIBuilder};

#[derive(Component)]
pub struct HouseInfoLabel;

/// The part of the panel rebuilt when the house changes: properties and files.
#[derive(Component, Default)]
pub struct HousePanelBody;

const PANEL_BG: Color = Color::srgba(0.04, 0.08, 0.05, 0.95);
const HELP: Color = Color::srgba(0.5, 0.7, 0.5, 0.7);
const HEADING: Color = Color::srgb(0.55, 0.8, 0.65);
const LABEL: Color = Color::srgb(0.8, 0.85, 0.9);
const VALUE: Color = Color::srgb(0.6, 0.85, 1.0);
const BUTTON: Color = Color::srgb(0.85, 0.92, 0.95);

/// Width of the side panel, logical pixels. The canvas lies to the right of it.
pub const PANEL_WIDTH: f32 = 260.0;

pub fn spawn_house_editor_ui(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<HouseEditorState>,
) {
    *state = HouseEditorState::default();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row()
        .bg_color(Color::NONE);

    ui.side_panel(PANEL_WIDTH, PANEL_BG, |left| {
        left.themed_header("House Editor");
        for line in [
            "Click: place a node. First node / Enter: close",
            "Shift: square edge   Backspace: undo node",
            "RClick: cancel / clear   C: clear outline",
            "Ctrl+S: save",
        ] {
            left.label(line, 11.0, HELP);
        }

        left.with_child(|c| {
            c.with_text(
                "",
                Some(lava_ui_builder::TextStyle::size_color(
                    12.0,
                    Color::srgb(0.85, 0.95, 0.85),
                )),
            )
            .insert(HouseInfoLabel);
        });

        left.scrollable_list(|list| {
            list.component::<HousePanelBody>();
        });

        left.add_button_observe(
            "<- Back to Menu",
            |b| {
                b.width(percent(100.0)).height(px(28.0)).font_size(13.0);
            },
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::Menu);
            },
        );
    });

    ui.with_child(|c| {
        c.with_flex_grow(1.0).height_percent(100.0);
    });

    ui.build();
}

pub fn rebuild_info_label(
    mut state: ResMut<HouseEditorState>,
    mut label_q: Query<&mut Text, With<HouseInfoLabel>>,
) {
    if !state.info_dirty {
        return;
    }
    state.info_dirty = false;
    let Ok(mut t) = label_q.single_mut() else {
        return;
    };

    let name = format!(
        "{}{}",
        state.name(),
        if state.is_dirty() { " *" } else { "" }
    );
    let shape_line = match (&state.resolved, state.points.len()) {
        (_, 0) => "No outline yet.".to_string(),
        (Some(_), n) => format!("Closed outline, {n} corners"),
        (None, n) => format!("Drawing: {n} node(s)"),
    };
    let result_line = state.resolved.as_ref().map_or_else(String::new, |r| {
        format!(
            "Floor {}  Wall {}  Door {}  Window {}",
            r.floor_cells.len(),
            r.wall_count(),
            r.door_count(),
            r.window_count(),
        )
    });
    **t = format!("{name}\n{shape_line}\n{result_line}\n{}", state.status);
}

fn heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::default().with_font_size(12.0),
        TextColor(HEADING),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        },
    ));
}

fn strip<'a>(parent: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
    parent.spawn(Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        ..default()
    })
}

/// Rebuild the properties and the file list when the house changes.
pub fn rebuild_house_panel(
    state: Res<HouseEditorState>,
    mut commands: Commands,
    body_q: Query<Entity, With<HousePanelBody>>,
    mut built: Local<Option<Entity>>,
) {
    let Ok(body) = body_q.single() else {
        return;
    };
    if !state.is_changed() && *built == Some(body) {
        return;
    }
    *built = Some(body);
    commands.entity(body).despawn_related::<Children>();

    let fields = house::fields(&state.house);
    let files = house::house_files(HOUSE_DIR);
    let open = state.path.clone();

    commands.entity(body).with_children(|parent| {
        heading(parent, "PROPERTIES");
        for field in fields {
            strip(parent).with_children(|line| {
                for (label, forward) in [("<", false), (">", true)] {
                    let id = field.id;
                    row(line, label.to_string(), false, BUTTON).observe(
                        move |_: On<Activate>, mut s: ResMut<HouseEditorState>| {
                            s.step_field(id, forward);
                        },
                    );
                }
                line.spawn((
                    Text::new(format!("{} ", field.label)),
                    TextFont::default().with_font_size(11.0),
                    TextColor(LABEL),
                ));
                line.spawn((
                    Text::new(field.value),
                    TextFont::default().with_font_size(11.0),
                    TextColor(VALUE),
                ));
            });
        }

        heading(parent, "FILE");
        strip(parent).with_children(|line| {
            row(line, "New".to_string(), false, BUTTON)
                .observe(|_: On<Activate>, mut s: ResMut<HouseEditorState>| s.new_house());
            row(line, "Save".to_string(), false, BUTTON)
                .observe(|_: On<Activate>, mut s: ResMut<HouseEditorState>| s.save());
        });
        heading(parent, "OPEN");
        if files.is_empty() {
            parent.spawn((
                Text::new(format!("no houses in {HOUSE_DIR} yet")),
                TextFont::default().with_font_size(10.0),
                TextColor(HELP),
            ));
        }
        for path in files {
            let selected = open.as_deref() == Some(path.as_str());
            let label = house::house_name(&path);
            row(parent, label, selected, BUTTON)
                .observe(move |_: On<Activate>, mut s: ResMut<HouseEditorState>| s.open(&path));
        }
    });
}

pub fn handle_house_editor_keys(
    mut state: ResMut<HouseEditorState>,
    mut next: ResMut<NextState<GameState>>,
    mut keyboard: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    for event in keyboard.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Character(c) if ctrl && (c == "s" || c == "S") => state.save(),
            Key::Character(c) if c == "c" || c == "C" => state.clear_all(),
            Key::Enter => state.close(),
            Key::Backspace => state.remove_last_point(),
            Key::ArrowUp => state.step_field("doors", true),
            Key::ArrowDown => state.step_field("doors", false),
            Key::ArrowRight => state.step_field("window_every", true),
            Key::ArrowLeft => state.step_field("window_every", false),
            Key::Escape => next.set(GameState::Menu),
            _ => {}
        }
    }
}
