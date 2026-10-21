use crate::game_state::GameState;
use crate::map::chunk_loader::stitch_map_from_dir;
use crate::map::chunks::CHUNK_SIZE;
use crate::map::map_generator::generate_suburb_map;
use crate::map_editor::modal::{EditorKey, KeyOutcome, Mode, Mods};
use crate::map_editor::state::{MapEditorState, PaletteTab};
use crate::ui::spawn_ui::StateMarker;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, UIBuilder, spawn_list_item};
use std::fmt::Write as _;

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct PaletteContainer;
#[derive(Component)]
pub struct WaveContainer;
#[derive(Component)]
pub struct EnemyPickerContainer;
#[derive(Component)]
pub struct MapInfoLabel;
#[derive(Component)]
pub struct ActiveBrushLabel;
#[derive(Component)]
pub struct ModeLabel;
#[derive(Component)]
pub struct GenSeedLabel;
#[derive(Component)]
pub struct StatusLabel;

const HELP_COLOR: Color = Color::srgba(0.5, 0.7, 0.5, 0.7);

/// The keymap, as shown in the left panel (`modal.rs` has the full story).
const HELP: &[&str] = &[
    "Arrows move (Shift: 5)  Esc normal",
    "P paint  X erase  V select  H house",
    "Space stamp/corner  Del erase tile",
    "Select: Enter fill, X erase, H house",
    "Tab category  [ ] or , . brush  1-9",
    "/ search  I eyedrop  ' last brush",
    "R/Shift+R rotate  U undo  Ctrl+Y redo",
    "Ctrl+S save  Ctrl+Q quit",
];

const PANEL_BG: Color = Color::srgba(0.04, 0.08, 0.05, 0.95);

// ── Spawn ─────────────────────────────────────────────────────────────────────

pub fn spawn_map_editor_ui(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<MapEditorState>,
) {
    *state = MapEditorState::default();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    // Root: full-screen flex row (transparent — grid drawn in 2D world space)
    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row()
        .bg_color(Color::NONE);

    // ── Left panel: palette ──────────────────────────────────────────────────
    ui.side_panel(200.0, PANEL_BG, |left| {
        left.themed_header("Map Editor");
        for line in HELP {
            left.label(*line, 10.0, HELP_COLOR);
        }
        left.label("Mouse: LMB paint, RMB erase", 10.0, HELP_COLOR);

        left.with_child(|c| {
            c.with_text(
                "-- NORMAL --",
                Some(lava_ui_builder::TextStyle::size_color(
                    12.0,
                    Color::srgb(0.6, 1.0, 0.6),
                )),
            )
            .insert(ModeLabel);
        });
        left.with_child(|c| {
            c.with_text(
                "",
                Some(lava_ui_builder::TextStyle::size_color(
                    10.0,
                    Color::srgb(0.95, 0.85, 0.5),
                )),
            )
            .insert(StatusLabel);
        });

        left.with_child(|c| {
            c.with_text(
                "new_map  20x24",
                Some(lava_ui_builder::TextStyle::size_color(
                    10.0,
                    Color::srgb(0.7, 0.8, 0.7),
                )),
            )
            .insert(MapInfoLabel);
        });

        // Generate section
        left.with_child(|c| {
            c.with_text(
                "seed: -",
                Some(lava_ui_builder::TextStyle::size_color(
                    11.0,
                    Color::srgba(0.5, 0.7, 0.5, 0.7),
                )),
            )
            .insert(GenSeedLabel);
        });
        left.add_button_observe(
            "Generate Map",
            |b| {
                b.width(percent(100.0)).height(px(24.0)).font_size(12.0);
            },
            |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(42, |d| d.as_nanos() as u64);
                let (width, height) = (s.width, s.height);
                let map = generate_suburb_map(seed, width, height);
                s.load_from_map_file(map);
                s.gen_seed = seed;
                s.seed_label_dirty = true;
            },
        );
        left.add_button_observe(
            "Stitch Chunks",
            |b| {
                b.width(percent(100.0)).height(px(24.0)).font_size(12.0);
            },
            |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(42, |d| d.as_nanos() as u64);
                // The editor sizes in tiles; the stitcher works in chunks. Authored
                // chunks under assets/maps/chunks/ augment the built-in library.
                let chunks_wide = (s.width / CHUNK_SIZE).max(2);
                let chunks_high = (s.height / CHUNK_SIZE).max(1);
                let map = stitch_map_from_dir(seed, chunks_wide, chunks_high, "assets/maps/chunks");
                s.load_from_map_file(map);
                s.gen_seed = seed;
                s.seed_label_dirty = true;
            },
        );

        // Tab chips
        left.with_child(|tabs| {
            tabs.display_flex()
                .flex_wrap()
                .gap_px(3.0)
                .modify_node(|mut n| n.align_self = AlignSelf::Stretch);
            for tab in PaletteTab::all() {
                let label = tab.label();
                tabs.add_button_observe(
                    label,
                    |b| {
                        b.font_size(10.0).width(px(56.0)).height(px(22.0));
                    },
                    move |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                        s.set_tab(match label {
                            "Terrain" => PaletteTab::Terrain,
                            "Tower" => PaletteTab::Tower,
                            "Item" => PaletteTab::Item,
                            "Enemy" => PaletteTab::Enemy,
                            _ => PaletteTab::Special,
                        });
                    },
                );
            }
        });

        left.with_child(|c| {
            c.with_text(
                "Brush: -",
                Some(lava_ui_builder::TextStyle::size_color(
                    11.0,
                    Color::srgb(0.9, 1.0, 0.6),
                )),
            )
            .insert(ActiveBrushLabel);
        });

        left.scrollable_list(|list| {
            list.insert(PaletteContainer);
        });

        left.add_button_observe(
            "Save Map",
            |b| {
                b.width(percent(100.0)).height(px(28.0)).font_size(13.0);
            },
            |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                s.save();
            },
        );
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

    // ── Centre: spacer (grid drawn by grid.rs in 2D world space) ────────────
    ui.with_child(|c| {
        c.with_flex_grow(1.0).height_percent(100.0);
    });

    // ── Right panel: wave editor ─────────────────────────────────────────────
    ui.side_panel(200.0, PANEL_BG, |right| {
        right.section_label("-- Enemy --");
        right.scrollable_list_bounded(120.0, |list| {
            list.insert(EnemyPickerContainer);
        });

        right.section_label("-- Waves --");
        right.scrollable_list(|list| {
            list.insert(WaveContainer);
        });

        right.add_button_observe(
            "+ Add Wave",
            |b| {
                b.width(percent(100.0)).height(px(28.0)).font_size(13.0);
            },
            |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                s.add_wave();
            },
        );
    });

    ui.build();
}

// ── Rebuild systems ───────────────────────────────────────────────────────────

pub fn rebuild_palette(
    mut state: ResMut<MapEditorState>,
    mut commands: Commands,
    container_q: Query<Entity, With<PaletteContainer>>,
    mut brush_label_q: Query<&mut Text, With<ActiveBrushLabel>>,
) {
    if !state.palette_dirty {
        return;
    }
    state.palette_dirty = false;

    let searching = state.mode == Mode::Search;
    if let Ok(mut t) = brush_label_q.single_mut() {
        **t = if searching {
            format!("Search: {}_", state.search)
        } else {
            let name = state
                .selected_item()
                .map_or("-", super::state::PaletteItem::display_name);
            let rot = state.rotation_steps;
            let tab = state.active_tab.label();
            if rot == 0 {
                format!("Brush: {tab} / {name}")
            } else {
                format!("Brush: {tab} / {name}  {}deg", u32::from(rot) * 45)
            }
        };
    }

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    // While searching the list shows the hits from every tab, else the active tab
    // numbered for the 1-9 keys.
    let rows: Vec<(usize, String, bool)> = if searching {
        state
            .search_hits
            .iter()
            .enumerate()
            .filter_map(|(i, &(tab, index))| {
                let item = state.catalog_item(tab, index)?;
                Some((
                    i,
                    format!("{}: {}", tab.label(), item.display_name()),
                    i == state.search_selected,
                ))
            })
            .collect()
    } else {
        state
            .palette_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let name = if i < 9 {
                    format!("{}  {}", i + 1, item.display_name())
                } else {
                    format!("   {}", item.display_name())
                };
                (i, name, i == state.selected_palette)
            })
            .collect()
    };

    commands.entity(container).with_children(|parent| {
        for (idx, name, selected) in rows {
            spawn_list_item(
                parent,
                name,
                selected,
                move |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                    if s.mode == Mode::Search {
                        s.pick_search_hit(idx);
                    } else {
                        let tab = s.active_tab;
                        s.select_brush(tab, idx);
                    }
                },
            );
        }
    });
}

pub fn rebuild_wave_list(
    mut state: ResMut<MapEditorState>,
    mut commands: Commands,
    container_q: Query<Entity, With<WaveContainer>>,
) {
    if !state.waves_dirty {
        return;
    }
    state.waves_dirty = false;

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let waves: Vec<(usize, String, u32, f32)> = state
        .waves
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let name = std::path::Path::new(&w.enemy_def)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("(no enemy)")
                .to_string();
            (i, name, w.count, w.spawn_rate_per_minute)
        })
        .collect();

    commands.entity(container).with_children(|parent| {
        if waves.is_empty() {
            parent.spawn((
                Text::new("No waves yet. Click '+ Add Wave'."),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgba(0.5, 0.6, 0.5, 0.6)),
                Node {
                    padding: UiRect::all(Val::Px(4.0)),
                    ..Default::default()
                },
            ));
            return;
        }
        for (i, name, count, rate) in waves {
            let bg = Color::srgba(0.07, 0.12, 0.09, 0.85);
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..Default::default()
                    },
                    BackgroundColor(bg),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(format!("W{}: {} x{} @{:.0}/m", i + 1, name, count, rate)),
                        TextFont::default().with_font_size(10.0),
                        TextColor(Color::srgb(0.75, 0.90, 0.75)),
                        Node {
                            flex_grow: 1.0,
                            overflow: Overflow::clip(),
                            ..Default::default()
                        },
                    ));
                    // Delete button
                    let mut entity_ref = row.spawn((
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..Default::default()
                        },
                        BackgroundColor(Color::srgba(0.4, 0.1, 0.1, 0.8)),
                        bevy::picking::hover::Hovered::default(),
                        bevy::ui_widgets::Button,
                        lava_ui_builder::InteractionPalette {
                            none: Color::srgba(0.4, 0.1, 0.1, 0.8),
                            hovered: Color::srgba(0.6, 0.15, 0.15, 0.9),
                            pressed: Color::srgba(0.3, 0.08, 0.08, 1.0),
                        },
                    ));
                    entity_ref
                        .with_child((
                            Text::new("x"),
                            TextFont::default().with_font_size(10.0),
                            TextColor(Color::srgb(1.0, 0.7, 0.7)),
                        ))
                        .observe(move |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                            s.remove_wave(i);
                        });
                });
        }
    });
}

pub fn rebuild_enemy_picker(
    mut state: ResMut<MapEditorState>,
    mut commands: Commands,
    container_q: Query<Entity, With<EnemyPickerContainer>>,
) {
    if !state.enemy_picker_dirty {
        return;
    }
    state.enemy_picker_dirty = false;

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let items: Vec<(usize, String, bool)> = state
        .enemy_defs
        .iter()
        .enumerate()
        .map(|(i, (_, name))| (i, name.clone(), i == state.selected_enemy))
        .collect();

    if items.is_empty() {
        commands.entity(container).with_children(|p| {
            p.spawn((
                Text::new("No enemy defs found."),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgba(0.5, 0.6, 0.5, 0.6)),
                Node {
                    padding: UiRect::all(Val::Px(4.0)),
                    ..Default::default()
                },
            ));
        });
        return;
    }

    commands.entity(container).with_children(|parent| {
        for (idx, name, selected) in items {
            spawn_list_item(
                parent,
                name,
                selected,
                move |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                    s.selected_enemy = idx;
                    s.enemy_picker_dirty = true;
                },
            );
        }
    });
}

pub fn rebuild_seed_label(
    mut state: ResMut<MapEditorState>,
    mut label_q: Query<&mut Text, With<GenSeedLabel>>,
) {
    if !state.seed_label_dirty {
        return;
    }
    state.seed_label_dirty = false;
    if let Ok(mut t) = label_q.single_mut() {
        **t = format!("seed: {}", state.gen_seed);
    }
}

// ── Key input ─────────────────────────────────────────────────────────────────

/// The mode line and the status line under it.
pub fn rebuild_mode_label(
    state: Res<MapEditorState>,
    mut mode_q: Query<&mut Text, (With<ModeLabel>, Without<StatusLabel>)>,
    mut status_q: Query<&mut Text, (With<StatusLabel>, Without<ModeLabel>)>,
) {
    if !state.is_changed() {
        return;
    }
    if let Ok(mut t) = mode_q.single_mut() {
        let (x, y) = state.cursor;
        let mut line = format!("-- {} --  ({x},{y})", state.mode.label());
        if let Mode::Select { anchor } = state.mode {
            let ((x0, y0), (x1, y1)) = MapEditorState::rect(anchor, state.cursor);
            let _ = write!(line, "  {}x{}", x1 - x0 + 1, y1 - y0 + 1);
        }
        if !state.house_points.is_empty() {
            let _ = write!(line, "  {} corners", state.house_points.len());
        }
        **t = line;
    }
    if let Ok(mut t) = status_q.single_mut()
        && **t != state.status
    {
        t.0.clone_from(&state.status);
    }
}

/// Translate a key press into the editor's own [`EditorKey`]. Ctrl combinations go by
/// the physical key, since with Ctrl held some layouts report a control character.
/// Brackets and the slash also go by position, for layouts (Swedish, German) where they
/// need AltGr or Shift.
fn editor_key(event: &KeyboardInput, ctrl: bool) -> Option<EditorKey> {
    let by_key = match &event.logical_key {
        Key::ArrowUp => Some(EditorKey::Up),
        Key::ArrowDown => Some(EditorKey::Down),
        Key::ArrowLeft => Some(EditorKey::Left),
        Key::ArrowRight => Some(EditorKey::Right),
        Key::Space => Some(EditorKey::Space),
        Key::Enter => Some(EditorKey::Enter),
        Key::Escape => Some(EditorKey::Escape),
        Key::Backspace => Some(EditorKey::Backspace),
        Key::Delete => Some(EditorKey::Delete),
        Key::Tab => Some(EditorKey::Tab),
        Key::Character(c) if !ctrl => {
            let c = c.chars().next()?;
            Some(EditorKey::Char(c.to_ascii_lowercase()))
        }
        _ => None,
    };
    if by_key.is_some() {
        return by_key;
    }
    let c = match event.key_code {
        KeyCode::KeyS => 's',
        KeyCode::KeyZ => 'z',
        KeyCode::KeyY => 'y',
        KeyCode::KeyQ => 'q',
        _ => return None,
    };
    ctrl.then_some(EditorKey::Char(c))
}

/// Keys that should work by position whatever the layout prints on them: the brush
/// steppers and search. Only outside a search, where the printed character is what counts.
fn by_position(key_code: KeyCode) -> Option<EditorKey> {
    match key_code {
        KeyCode::BracketLeft => Some(EditorKey::Char('[')),
        KeyCode::BracketRight => Some(EditorKey::Char(']')),
        KeyCode::Slash => Some(EditorKey::Char('/')),
        _ => None,
    }
}

pub fn handle_editor_keys(
    mut state: ResMut<MapEditorState>,
    mut next: ResMut<NextState<GameState>>,
    mut keyboard: MessageReader<KeyboardInput>,
    held: Res<ButtonInput<KeyCode>>,
) {
    let mods = Mods {
        shift: held.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        ctrl: held.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
    };
    for event in keyboard.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        let positional = (state.mode != Mode::Search && !mods.ctrl)
            .then(|| by_position(event.key_code))
            .flatten();
        let Some(key) = positional.or_else(|| editor_key(event, mods.ctrl)) else {
            continue;
        };
        if event.repeat && !key.repeats() {
            continue;
        }
        if state.handle_key(key, mods) == KeyOutcome::Quit {
            next.set(GameState::Menu);
        }
    }
}
