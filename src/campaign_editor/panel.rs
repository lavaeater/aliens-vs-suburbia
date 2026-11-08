//! The campaign editor's side panel and keys: the levels in order, the selected one's
//! title, region, markers, path check and rules, and the campaign's own texts.
//!
//! Text fields: click one to type into it; Enter or Esc ends. Plain ASCII only.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, UIBuilder};

use super::analysis::Analysis;
use super::doc::CampaignDoc;
use super::view::{CampaignCamera, TILE, Tool, tile_to_world};
use crate::character_editor::text_field::edit_value;
use crate::character_editor::ui::row;
use crate::game_state::GameState;
use crate::general::components::map_components::{MarkerKind, Region};
use crate::ui::spawn_ui::StateMarker;

/// Width of the side panel, in UI pixels (`UiScale` multiplies them). The world view is
/// right of it.
pub const PANEL_WIDTH: f32 = 330.0;

/// The panel's width in window pixels.
pub fn panel_px(ui_scale: &UiScale) -> f32 {
    PANEL_WIDTH * ui_scale.0
}

const PANEL_BG: Color = Color::srgba(0.04, 0.06, 0.08, 0.96);
const HELP: Color = Color::srgba(0.6, 0.7, 0.75, 0.75);
const HEADING: Color = Color::srgb(0.95, 0.8, 0.4);
const LABEL: Color = Color::srgb(0.8, 0.85, 0.9);
const VALUE: Color = Color::srgb(0.6, 0.85, 1.0);
const BUTTON: Color = Color::srgb(0.88, 0.92, 0.95);
const OK: Color = Color::srgb(0.45, 0.9, 0.5);
const PROBLEM: Color = Color::srgb(1.0, 0.45, 0.4);

/// A text the keyboard types into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title(usize),
    Tagline(usize),
    VictoryTitle,
    Name(usize),
}

/// The field being typed into, if any.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextFocus(pub Option<Field>);

/// Ask the camera to frame a region.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct FrameOn(pub Option<Region>);

/// The panel's body, rebuilt on a change.
#[derive(Component, Default)]
pub struct PanelBody;

/// Set when Esc was pressed once over unsaved changes, or Delete once on a level.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct Confirm {
    pub leave: bool,
    pub remove: Option<usize>,
}

pub fn spawn_panel(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row()
        .bg_color(Color::NONE);
    ui.side_panel(PANEL_WIDTH, PANEL_BG, |left| {
        left.themed_header("Campaign Editor");
        for line in [
            "Wheel/+-: zoom  RMB/MMB drag, arrows: pan",
            "Click: select  drag: move  edges: resize",
            "Shift+drag (or drag on empty ground): new level",
            "1 player  2 spawn  3 goal  Esc: select tool",
            "S: suggest spawns  F: frame  PgUp/PgDn: order",
            "Del: remove level  Ctrl+S: save  Esc: leave",
            "F5: save and play the selected level",
        ] {
            left.label(line, 11.0, HELP);
        }
        left.scrollable_list(|list| {
            list.component::<PanelBody>();
        });
    });
    ui.with_child(|c| {
        c.with_flex_grow(1.0).height_percent(100.0);
    });
    ui.build();
}

fn text(parent: &mut ChildSpawnerCommands, s: impl Into<String>, size: f32, color: Color) {
    parent.spawn((
        Text::new(s.into()),
        TextFont::default().with_font_size(size),
        TextColor(color),
    ));
}

fn heading(parent: &mut ChildSpawnerCommands, s: &str) {
    parent.spawn((
        Text::new(s),
        TextFont::default().with_font_size(12.0),
        TextColor(HEADING),
        Node {
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        },
    ));
}

fn strip<'a>(parent: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
    parent.spawn(Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(3.0),
        ..default()
    })
}

/// A field row: its label and its value, which takes the keyboard when clicked.
fn field(parent: &mut ChildSpawnerCommands, label: &str, value: &str, which: Field, focus: TextFocus) {
    let typing = focus.0 == Some(which);
    strip(parent).with_children(|line| {
        text(line, format!("{label}:"), 11.0, LABEL);
        let shown = if typing {
            format!("{value}_")
        } else if value.is_empty() {
            "(click to type)".to_string()
        } else {
            value.to_string()
        };
        row(line, shown, typing, VALUE).observe(
            move |_: On<Activate>, mut focus: ResMut<TextFocus>| {
                focus.0 = Some(which);
            },
        );
    });
}

fn button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    selected: bool,
    action: impl Fn(&mut CampaignDoc, &mut Tool, &mut FrameOn) + Send + Sync + 'static,
) {
    row(parent, label.to_string(), selected, BUTTON).observe(
        move |_: On<Activate>,
              mut doc: ResMut<CampaignDoc>,
              mut tool: ResMut<Tool>,
              mut frame: ResMut<FrameOn>| {
            action(&mut doc, &mut tool, &mut frame);
        },
    );
}

type Map = crate::general::components::map_components::MapFile;

/// A number rule: `<` and `>` step it by `step`; at 0 it is the game's default.
fn stepper(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    index: usize,
    step: u32,
    get: fn(&Map) -> Option<u32>,
    set: fn(&mut Map, Option<u32>),
    value: Option<u32>,
) {
    strip(parent).with_children(|line| {
        text(line, format!("{label}:"), 11.0, LABEL);
        for (glyph, up) in [("<", false), (">", true)] {
            button(line, glyph, false, move |doc, _, _| {
                doc.edit_map(index, |map| {
                    let now = get(map).unwrap_or(0);
                    let next = if up {
                        now.saturating_add(step)
                    } else {
                        now.saturating_sub(step)
                    };
                    set(map, Some(next).filter(|&n| n > 0));
                });
            });
        }
        text(
            line,
            value.map_or_else(|| "default".to_string(), |v| v.to_string()),
            11.0,
            VALUE,
        );
    });
}

/// The level's waves: count and rate steppers each, add (a copy of the last) and remove.
fn waves(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    waves: &[crate::general::components::map_components::WaveDef],
) {
    use crate::general::components::map_components::WaveDef;
    if waves.is_empty() {
        text(parent, "Waves: the game's defaults", 11.0, LABEL);
    }
    for (n, wave) in waves.iter().enumerate() {
        let enemy = if wave.enemy_def.is_empty() {
            "alien".to_string()
        } else {
            std::path::Path::new(&wave.enemy_def)
                .file_stem()
                .map_or_else(|| wave.enemy_def.clone(), |s| s.to_string_lossy().into_owned())
        };
        strip(parent).with_children(|line| {
            text(line, format!("{}. {enemy}", n + 1), 11.0, LABEL);
            for (label, count, rate) in [("-", -2i32, 0i32), ("+", 2, 0), ("slower", 0, -2), ("faster", 0, 2)] {
                button(line, label, false, move |doc, _, _| {
                    doc.edit_map(index, |map| {
                        if let Some(w) = map.waves.get_mut(n) {
                            w.count = w.count.saturating_add_signed(count).max(1);
                            w.spawn_rate_per_minute = (w.spawn_rate_per_minute + rate as f32).max(1.0);
                        }
                    });
                });
            }
            text(
                line,
                format!("{} at {}/min", wave.count, wave.spawn_rate_per_minute),
                11.0,
                VALUE,
            );
        });
    }
    strip(parent).with_children(|line| {
        button(line, "Add wave", false, move |doc, _, _| {
            doc.edit_map(index, |map| {
                let next = map.waves.last().cloned().unwrap_or_default();
                map.waves.push(WaveDef {
                    count: next.count.saturating_add(4),
                    ..next
                });
            });
        });
        if !waves.is_empty() {
            button(line, "Remove last", false, move |doc, _, _| {
                doc.edit_map(index, |map| {
                    map.waves.pop();
                });
            });
        }
    });
}

/// What the panel shows; it rebuilds when this changes.
#[derive(PartialEq, Default)]
pub struct PanelKey {
    revision: u64,
    analysis: Analysis,
    tool: Option<Tool>,
    focus: TextFocus,
    remove: Option<usize>,
}

#[allow(clippy::too_many_arguments)]
pub fn rebuild_panel(
    mut commands: Commands,
    doc: Res<CampaignDoc>,
    analysis: Res<Analysis>,
    tool: Res<Tool>,
    focus: Res<TextFocus>,
    confirm: Res<Confirm>,
    body: Query<Entity, With<PanelBody>>,
    mut key: Local<PanelKey>,
) {
    let Ok(body) = body.single() else {
        return;
    };
    let now = PanelKey {
        revision: doc.revision,
        analysis: analysis.clone(),
        tool: Some(*tool),
        focus: *focus,
        remove: confirm.remove,
    };
    if *key == now {
        return;
    }
    *key = now;
    commands.entity(body).despawn_related::<Children>();
    let focus = *focus;
    let tool = *tool;
    let (bw, bh) = doc.base_size();

    commands.entity(body).with_children(|p| {
        text(
            p,
            format!(
                "{}{}",
                doc.name,
                if doc.is_dirty() { "  (unsaved)" } else { "" }
            ),
            14.0,
            LABEL,
        );
        if doc.base.is_some() {
            text(p, format!("base {} ({bw} x {bh})", doc.base_ref), 10.0, HELP);
        }
        if !doc.status.is_empty() {
            text(p, doc.status.clone(), 11.0, VALUE);
        }

        heading(p, "LEVELS");
        for (i, level) in doc.file.levels.iter().enumerate() {
            let doc_level = doc.levels.get(i);
            let size = doc
                .region_of(i)
                .map_or_else(|| "not on the base".into(), |r| format!("{} x {}", r.width, r.height));
            let dirty = doc_level.is_some_and(super::doc::LevelDoc::is_dirty);
            let label = format!(
                "{}. {}  [{size}]{}",
                i + 1,
                level.title,
                if dirty { " *" } else { "" }
            );
            row(p, label, doc.selected == Some(i), BUTTON).observe(
                move |_: On<Activate>, mut doc: ResMut<CampaignDoc>, mut frame: ResMut<FrameOn>| {
                    doc.select(Some(i));
                    frame.0 = doc.region_of(i);
                },
            );
        }
        if doc.levels.is_empty() {
            text(p, "No levels yet: Shift+drag on the map.", 11.0, HELP);
        }

        if let (Some(i), Some(level)) = (doc.selected, doc.selected_level()) {
            heading(p, "SELECTED LEVEL");
            let entry = doc.file.levels.get(i);
            field(p, "Title", entry.map_or("", |l| l.title.as_str()), Field::Title(i), focus);
            field(p, "Tagline", entry.map_or("", |l| l.tagline.as_str()), Field::Tagline(i), focus);
            field(p, "Map name", &level.map.name, Field::Name(i), focus);
            text(
                p,
                format!("{}{}", level.path, if level.is_dirty() { " *" } else { "" }),
                10.0,
                HELP,
            );
            if let Some(r) = doc.region_of(i) {
                text(
                    p,
                    format!(
                        "Region {}, {}  {} x {}  ({} open tiles)",
                        r.x, r.y, r.width, r.height, analysis.open
                    ),
                    11.0,
                    LABEL,
                );
                if r.width * r.height > 250_000 {
                    text(p, "Big: expect long loads and slow waves.", 11.0, PROBLEM);
                }
            }
            text(
                p,
                format!(
                    "Markers: {} player  {} spawn  {} goal",
                    level.count(MarkerKind::Player),
                    level.count(MarkerKind::Spawn),
                    level.count(MarkerKind::Goal)
                ),
                11.0,
                LABEL,
            );
            strip(p).with_children(|line| {
                row(line, "Play (F5)".into(), false, OK).observe(
                    |_: On<Activate>,
                     mut commands: Commands,
                     mut doc: ResMut<CampaignDoc>,
                     mut roster: ResMut<crate::player_setup::state::PlayerRoster>,
                     mut next: ResMut<NextState<GameState>>| {
                        super::play::play_selected(&mut commands, &mut doc, &mut roster, &mut next);
                    },
                );
                button(line, "Up", false, move |doc, _, _| doc.move_level(i, true));
                button(line, "Down", false, move |doc, _, _| doc.move_level(i, false));
                button(line, "Frame", false, move |doc, _, frame| {
                    frame.0 = doc.region_of(i);
                });
                let removing = confirm.remove == Some(i);
                row(line, if removing { "Remove? Del again".into() } else { "Remove".into() }, removing, PROBLEM)
                    .observe(move |_: On<Activate>, mut confirm: ResMut<Confirm>, mut doc: ResMut<CampaignDoc>| {
                        if confirm.remove == Some(i) {
                            doc.remove_level(i);
                            confirm.remove = None;
                        } else {
                            confirm.remove = Some(i);
                        }
                    });
            });

            heading(p, &format!("TOOLS (now: {})", tool.name()));
            strip(p).with_children(|line| {
                for (label, t) in [
                    ("Select", Tool::Select),
                    ("1 Player", Tool::Marker(MarkerKind::Player)),
                    ("2 Spawn", Tool::Marker(MarkerKind::Spawn)),
                    ("3 Goal", Tool::Marker(MarkerKind::Goal)),
                ] {
                    button(line, label, tool == t, move |_, tool, _| *tool = t);
                }
            });
            if !analysis.suggested_spawns.is_empty() {
                let spawns = analysis.suggested_spawns.clone();
                strip(p).with_children(|line| {
                    button(line, "S: spawns where roads leave", false, move |doc, _, _| {
                        doc.set_spawns(i, &spawns);
                    });
                    text(line, format!("({})", analysis.suggested_spawns.len()), 11.0, HELP);
                });
            }

            heading(p, "CHECK");
            if analysis.problems.is_empty() {
                text(p, "OK: every spawn reaches the goal.", 11.0, OK);
            }
            for problem in &analysis.problems {
                text(p, problem.clone(), 11.0, PROBLEM);
            }
            if analysis.cut_off > 0 {
                text(
                    p,
                    format!("{} open tiles can't reach the goal (fine if fenced off)", analysis.cut_off),
                    10.0,
                    HELP,
                );
            }

            heading(p, "RULES");
            stepper(
                p,
                "Escape limit",
                i,
                1,
                |m| m.escape_limit,
                |m, v| m.escape_limit = v,
                level.map.escape_limit,
            );
            stepper(p, "Coins", i, 50, |m| m.coins, |m, v| m.coins = v, level.map.coins);
            waves(p, i, &level.map.waves);

            heading(p, "STORY");
            if level.map.stories.is_empty() {
                strip(p).with_children(|line| {
                    button(line, "Create story + intro crawl", false, move |doc, _, _| {
                        if let Err(e) = doc.create_story(i) {
                            doc.status = format!("No story: {e}");
                        }
                    });
                });
            } else {
                text(p, level.map.stories.clone(), 10.0, HELP);
                text(p, "Edit it (and its crawls) in a text editor.", 10.0, HELP);
            }
        }

        heading(p, "CAMPAIGN");
        field(p, "Victory title", &doc.file.victory_title, Field::VictoryTitle, focus);
        strip(p).with_children(|line| {
            button(line, "Save (Ctrl+S)", false, |doc, _, _| {
                if let Err(e) = doc.save() {
                    doc.status = format!("Save failed: {e}");
                }
            });
            row(line, "Back to menu".into(), false, BUTTON).observe(
                |_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                    next.set(GameState::Menu);
                },
            );
        });
    });
}

fn field_value(doc: &CampaignDoc, field: Field) -> String {
    match field {
        Field::Title(i) => doc.file.levels.get(i).map(|l| l.title.clone()),
        Field::Tagline(i) => doc.file.levels.get(i).map(|l| l.tagline.clone()),
        Field::VictoryTitle => Some(doc.file.victory_title.clone()),
        Field::Name(i) => doc.levels.get(i).map(|l| l.map.name.clone()),
    }
    .unwrap_or_default()
}

fn set_field(doc: &mut CampaignDoc, field: Field, value: String) {
    match field {
        Field::Title(i) => doc.set_title(i, value),
        Field::Tagline(i) => doc.set_tagline(i, value),
        Field::VictoryTitle => doc.set_victory_title(value),
        Field::Name(i) => doc.edit_map(i, |map| map.name = value),
    }
}

/// Typing into a field, or the editor's keys.
#[allow(clippy::too_many_arguments)]
pub fn handle_keys(
    mut keyboard: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<TextFocus>,
    mut doc: ResMut<CampaignDoc>,
    mut tool: ResMut<Tool>,
    mut frame: ResMut<FrameOn>,
    mut confirm: ResMut<Confirm>,
    analysis: Res<Analysis>,
    mut next: ResMut<NextState<GameState>>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    for event in keyboard.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if let Some(field) = focus.0 {
            match &event.logical_key {
                Key::Enter | Key::Escape | Key::Tab => focus.0 = None,
                key => {
                    let mut value = field_value(&doc, field);
                    if edit_value(&mut value, key) {
                        set_field(&mut doc, field, value);
                    }
                }
            }
            continue;
        }
        let selected = doc.selected;
        if event.key_code != KeyCode::Delete {
            confirm.remove = None;
        }
        if event.key_code != KeyCode::Escape {
            confirm.leave = false;
        }
        match event.key_code {
            KeyCode::KeyS if ctrl => {
                if let Err(e) = doc.save() {
                    doc.status = format!("Save failed: {e}");
                }
            }
            KeyCode::Digit1 => *tool = Tool::Marker(MarkerKind::Player),
            KeyCode::Digit2 => *tool = Tool::Marker(MarkerKind::Spawn),
            KeyCode::Digit3 => *tool = Tool::Marker(MarkerKind::Goal),
            KeyCode::KeyS => {
                if let Some(i) = selected
                    && !analysis.suggested_spawns.is_empty()
                {
                    let spawns = analysis.suggested_spawns.clone();
                    doc.set_spawns(i, &spawns);
                }
            }
            KeyCode::KeyF => frame.0 = selected.and_then(|i| doc.region_of(i)),
            KeyCode::PageUp => {
                if let Some(i) = selected {
                    doc.move_level(i, true);
                }
            }
            KeyCode::PageDown => {
                if let Some(i) = selected {
                    doc.move_level(i, false);
                }
            }
            KeyCode::Delete => {
                if let Some(i) = selected {
                    if confirm.remove == Some(i) {
                        doc.remove_level(i);
                        confirm.remove = None;
                    } else {
                        confirm.remove = Some(i);
                        doc.status = "Delete again to remove the level from the campaign".into();
                        doc.revision = doc.revision.wrapping_add(1);
                    }
                }
            }
            KeyCode::Escape => {
                if *tool != Tool::Select {
                    *tool = Tool::Select;
                } else if doc.selected.is_some() {
                    doc.select(None);
                } else if doc.is_dirty() && !confirm.leave {
                    confirm.leave = true;
                    doc.status = "Unsaved changes: Esc again to leave without saving, Ctrl+S to save".into();
                    doc.revision = doc.revision.wrapping_add(1);
                } else {
                    next.set(GameState::Menu);
                }
            }
            _ => {}
        }
    }
}

/// Move and zoom the camera to show a region, when asked.
pub fn frame_region(
    mut frame: ResMut<FrameOn>,
    ui_scale: Res<UiScale>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<CampaignCamera>>,
) {
    let Some(region) = frame.0.take() else {
        return;
    };
    let (Ok(window), Ok((mut transform, mut projection))) = (windows.single(), cameras.single_mut())
    else {
        return;
    };
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    let panel = panel_px(&ui_scale);
    let view = Vec2::new(window.width() - panel, window.height()).max(Vec2::splat(100.0));
    let size = Vec2::new(region.width as f32, region.height as f32) * TILE;
    ortho.scale = ((size / view).max_element() * 1.15).clamp(0.05, 8.0);
    let centre = tile_to_world(
        region.x as f32 + region.width as f32 / 2.0,
        region.y as f32 + region.height as f32 / 2.0,
    );
    transform.translation = (centre - Vec2::new(panel * ortho.scale / 2.0, 0.0)).extend(0.0);
}
