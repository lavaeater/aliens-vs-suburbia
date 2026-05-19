use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{InteractionPalette, LavaTheme, TextTheme, UIBuilder};
use crate::game_state::GameState;
use crate::map_editor::state::{MapEditorState, PaletteTab};
use crate::ui::spawn_ui::StateMarker;

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)] pub struct PaletteContainer;
#[derive(Component)] pub struct WaveContainer;
#[derive(Component)] pub struct MapInfoLabel;
#[derive(Component)] pub struct ActiveBrushLabel;

// ── Spawn ─────────────────────────────────────────────────────────────────────

pub fn spawn_map_editor_ui(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<MapEditorState>,
) {
    *state = MapEditorState::default();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    let t = theme.text.clone();
    let dim = TextTheme { label_size: 11.0, label_color: Color::srgba(0.5, 0.7, 0.5, 0.7), ..t.clone() };
    let small = TextTheme { label_size: 10.0, label_color: Color::srgb(0.7, 0.8, 0.7), ..t.clone() };

    // Root: full-screen flex row
    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row()
        .bg_color(Color::srgba(0.0, 0.0, 0.0, 0.0)); // transparent — grid is drawn in 2D world

    // ── Left panel: palette ──────────────────────────────────────────────────
    ui.with_child(|left| {
        left.modify_node(|mut n| {
            n.width = Val::Px(200.0);
            n.height = Val::Percent(100.0);
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(6.0));
            n.row_gap = Val::Px(3.0);
        }).bg_color(Color::srgba(0.04, 0.08, 0.05, 0.95));

        left.with_child(|c| { c.insert_bundle(lava_ui_builder::header("Map Editor", &t)); });
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("[R] rotate  [S] save  [Esc] back", &dim)); });
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("LClick place  RClick erase", &dim)); });

        // Map name / info
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("new_map  20x24", &small)).insert(MapInfoLabel);
        });

        // Tab chips
        left.with_child(|tabs| {
            tabs.display_flex().flex_wrap().gap_px(3.0)
                .modify_node(|mut n| n.align_self = AlignSelf::Stretch);
            for tab in PaletteTab::all() {
                let label = tab.label();
                tabs.add_button_observe(label, |b| { b.font_size(10.0).width(px(56.0)).height(px(22.0)); },
                    move |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                        s.set_tab(match label {
                            "Terrain" => PaletteTab::Terrain,
                            "Tower"   => PaletteTab::Tower,
                            "Item"    => PaletteTab::Item,
                            "Enemy"   => PaletteTab::Enemy,
                            _         => PaletteTab::Special,
                        });
                    });
            }
        });

        // Active brush indicator
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("Brush: —", &TextTheme {
                label_size: 11.0,
                label_color: Color::srgb(0.9, 1.0, 0.6),
                ..t.clone()
            })).insert(ActiveBrushLabel);
        });

        // Palette item list
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .with_flex_grow(1.0).width_percent(100.0)
             .overflow_scroll_y()
             .insert(PaletteContainer);
        });

        // Save button
        left.add_button_observe("Save Map", |b| { b.width(percent(100.0)).height(px(28.0)).font_size(13.0); },
            |_: On<Activate>, s: Res<MapEditorState>| { s.save(); });

        left.add_button_observe("<- Back to Menu", |b| { b.width(percent(100.0)).height(px(28.0)).font_size(13.0); },
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| { next.set(GameState::Menu); });
    });

    // ── Centre: spacer (grid drawn by grid.rs in 2D world space) ────────────
    ui.with_child(|c| { c.with_flex_grow(1.0).height_percent(100.0); });

    // ── Right panel: wave editor ─────────────────────────────────────────────
    ui.with_child(|right| {
        right.modify_node(|mut n| {
            n.width = Val::Px(200.0);
            n.height = Val::Percent(100.0);
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(6.0));
            n.row_gap = Val::Px(3.0);
        }).bg_color(Color::srgba(0.04, 0.08, 0.05, 0.95));

        right.with_child(|c| { c.insert_bundle(lava_ui_builder::label("-- Waves --", &TextTheme {
            label_size: 13.0, label_color: Color::srgb(0.5, 0.8, 0.6), ..t.clone()
        })); });

        right.with_child(|c| {
            c.display_flex().flex_column().gap_px(4.0)
             .with_flex_grow(1.0).width_percent(100.0)
             .overflow_scroll_y()
             .insert(WaveContainer);
        });

        right.add_button_observe("+ Add Wave", |b| { b.width(percent(100.0)).height(px(28.0)).font_size(13.0); },
            |_: On<Activate>, mut s: ResMut<MapEditorState>| { s.add_wave(); s.waves_dirty = true; });
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
    if !state.palette_dirty { return; }
    state.palette_dirty = false;

    // Update the active brush label.
    if let Ok(mut t) = brush_label_q.single_mut() {
        let name = state.selected_item().map(|i| i.display_name()).unwrap_or("—");
        let rot = state.rotation_steps;
        **t = if rot == 0 {
            format!("Brush: {name}")
        } else {
            format!("Brush: {name}  r{}deg", rot * 45)
        };
    }

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let items: Vec<(usize, String, bool)> = state.palette_items.iter().enumerate()
        .map(|(i, item)| (i, item.display_name().to_string(), i == state.selected_palette))
        .collect();

    commands.entity(container).with_children(|parent| {
        for (idx, name, selected) in items {
            let bg = if selected { Color::srgba(0.15, 0.40, 0.20, 0.95) } else { Color::srgba(0.07, 0.12, 0.09, 0.85) };
            let tc = if selected { Color::srgb(0.9, 1.0, 0.9) } else { Color::srgb(0.65, 0.80, 0.65) };
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..Default::default()
                },
                BackgroundColor(bg),
                InteractionPalette { none: bg, hovered: Color::srgba(0.20, 0.50, 0.28, 0.95), pressed: Color::srgba(0.12, 0.32, 0.18, 1.0) },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
            ))
            .with_child((Text::new(name), TextFont::default().with_font_size(11.0), TextColor(tc)))
            .observe(move |_: On<Activate>, mut s: ResMut<MapEditorState>| {
                s.selected_palette = idx;
                s.palette_dirty = true;
            });
        }
    });
}

pub fn rebuild_wave_list(
    mut state: ResMut<MapEditorState>,
    mut commands: Commands,
    container_q: Query<Entity, With<WaveContainer>>,
) {
    if !state.waves_dirty { return; }
    state.waves_dirty = false;

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let waves: Vec<(usize, String, u32, f32)> = state.waves.iter().enumerate()
        .map(|(i, w)| {
            let name = std::path::Path::new(&w.enemy_def)
                .file_stem().and_then(|s| s.to_str())
                .unwrap_or("(no enemy)").to_string();
            (i, name, w.count, w.spawn_rate_per_minute)
        }).collect();

    commands.entity(container).with_children(|parent| {
        if waves.is_empty() {
            parent.spawn((
                Text::new("No waves yet. Click '+ Add Wave'."),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgba(0.5, 0.6, 0.5, 0.6)),
                Node { padding: UiRect::all(Val::Px(4.0)), ..Default::default() },
            ));
            return;
        }
        for (i, name, count, rate) in waves {
            let bg = Color::srgba(0.07, 0.12, 0.09, 0.85);
            parent.spawn((
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
            )).with_children(|row| {
                row.spawn((
                    Text::new(format!("W{}: {} x{} @{:.0}/m", i + 1, name, count, rate)),
                    TextFont::default().with_font_size(10.0),
                    TextColor(Color::srgb(0.75, 0.90, 0.75)),
                    Node { flex_grow: 1.0, overflow: Overflow::clip(), ..Default::default() },
                ));
                row.spawn((
                    Node { width: Val::Px(16.0), height: Val::Px(16.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..Default::default() },
                    BackgroundColor(Color::srgba(0.4, 0.1, 0.1, 0.8)),
                    bevy::picking::hover::Hovered::default(),
                    bevy::ui_widgets::Button,
                    InteractionPalette { none: Color::srgba(0.4, 0.1, 0.1, 0.8), hovered: Color::srgba(0.6, 0.15, 0.15, 0.9), pressed: Color::srgba(0.3, 0.08, 0.08, 1.0) },
                ))
                .with_child((Text::new("x"), TextFont::default().with_font_size(10.0), TextColor(Color::srgb(1.0, 0.7, 0.7))))
                .observe(move |_: On<Activate>, mut s: ResMut<MapEditorState>| { s.remove_wave(i); });
            });
        }
    });
}

// ── Key input ─────────────────────────────────────────────────────────────────

pub fn handle_editor_keys(
    mut state: ResMut<MapEditorState>,
    mut next: ResMut<NextState<GameState>>,
    mut keyboard: MessageReader<KeyboardInput>,
) {
    for event in keyboard.read() {
        if event.state != ButtonState::Pressed { continue; }
        match &event.logical_key {
            Key::Character(c) if c == "r" || c == "R" => state.rotate_brush(),
            Key::Character(c) if c == "s" || c == "S" => state.save(),
            Key::Escape => next.set(GameState::Menu),
            _ => {}
        }
    }
}
