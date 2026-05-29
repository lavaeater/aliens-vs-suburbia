use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use bevy::window::PrimaryWindow;
use enumflags2::BitFlags;
use crate::map::MapFeatures;
use crate::map_editor::state::MapEditorState;
use crate::ui::spawn_ui::StateMarker;

pub const CELL_SIZE: f32 = 24.0; // pixels per tile in the grid view

#[derive(Component)]
pub struct GridCamera;

#[derive(Component)]
pub struct GridCellMarker { #[allow(dead_code)] pub x: usize, #[allow(dead_code)] pub y: usize }

#[derive(Component)]
pub struct HoverHighlight;

pub fn spawn_grid_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera { order: 0, ..Default::default() },
        GridCamera,
        StateMarker,
        Transform::default(),
    ));

    // Hover highlight — a semi-transparent overlay that follows the cursor cell.
    commands.spawn((
        HoverHighlight,
        StateMarker,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(CELL_SIZE - 1.0),
            height: Val::Px(CELL_SIZE - 1.0),
            ..Default::default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25)),
        Visibility::Hidden,
    ));
}

/// Rebuild the visual tile grid whenever grid_dirty is set.
/// Tiles are absolute-positioned in screen space, centered in the area between
/// the two 200-px side panels (i.e. centred on the window).
pub fn rebuild_grid(
    mut state: ResMut<MapEditorState>,
    mut commands: Commands,
    cells: Query<Entity, With<GridCellMarker>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if !state.grid_dirty { return; }
    state.grid_dirty = false;

    for e in cells.iter() { commands.entity(e).despawn(); }

    let Ok(window) = windows.single() else { return };
    let win_w = window.width();
    let win_h = window.height();

    let w = state.width;
    let h = state.height;

    // Top-left corner of the grid in screen space, centred on the window.
    let grid_left = (win_w * 0.5 - w as f32 * CELL_SIZE * 0.5).round();
    let grid_top  = (win_h * 0.5 - h as f32 * CELL_SIZE * 0.5).round();

    for row in 0..h {
        for col in 0..w {
            let tile = state.tiles[row][col];
            let color = tile_color(tile);
            commands.spawn((
                GridCellMarker { x: col, y: row },
                StateMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(grid_left + col as f32 * CELL_SIZE),
                    top:  Val::Px(grid_top  + row as f32 * CELL_SIZE),
                    width:  Val::Px(CELL_SIZE - 1.0),
                    height: Val::Px(CELL_SIZE - 1.0),
                    ..Default::default()
                },
                BackgroundColor(color),
            ));
        }
    }

    // Overlay: first-letter of each placement's def name.
    for p in &state.placements {
        if p.x < 0 || p.y < 0 || p.x >= w as i32 || p.y >= h as i32 { continue; }
        let label = std::path::Path::new(&p.def_path)
            .file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let initial = label.chars().next().unwrap_or('?').to_uppercase().to_string();
        commands.spawn((
            StateMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(grid_left + p.x as f32 * CELL_SIZE),
                top:  Val::Px(grid_top  + p.y as f32 * CELL_SIZE),
                width:  Val::Px(CELL_SIZE - 1.0),
                height: Val::Px(CELL_SIZE - 1.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::NONE),
        )).with_child((
            Text::new(initial),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::WHITE),
        ));
    }
}

fn tile_color(raw: u64) -> Color {
    if raw == 0 { return Color::srgb(0.08, 0.08, 0.08); }
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(raw);

    // Functional addors take display priority over terrain.
    if flags.contains(MapFeatures::PlayerSpawn)  { return Color::srgb(0.2, 0.5, 0.9); }
    if flags.contains(MapFeatures::EnemySpawn)   { return Color::srgb(0.8, 0.2, 0.2); }
    if flags.contains(MapFeatures::EnemyExit)    { return Color::srgb(0.8, 0.6, 0.1); }
    if flags.contains(MapFeatures::ImpassableForPlayers) && flags.contains(MapFeatures::ImpassableForEnemies) {
        return Color::srgb(0.25, 0.22, 0.18); // solid wall — dark brown
    }
    if flags.contains(MapFeatures::ImpassableForPlayers) {
        return Color::srgb(0.55, 0.30, 0.12); // player-only wall — orange-brown
    }
    if flags.contains(MapFeatures::ImpassableForEnemies) {
        return Color::srgb(0.15, 0.30, 0.50); // alien-only wall — steel blue
    }

    // Terrain type.
    if flags.contains(MapFeatures::Water)  { return Color::srgb(0.15, 0.35, 0.7); }
    if flags.contains(MapFeatures::Grass)  { return Color::srgb(0.22, 0.55, 0.22); }
    if flags.contains(MapFeatures::Floor)  { return Color::srgb(0.28, 0.38, 0.28); }
    if flags.contains(MapFeatures::Mud)    { return Color::srgb(0.45, 0.32, 0.18); }
    if flags.contains(MapFeatures::Snow)   { return Color::srgb(0.82, 0.88, 0.92); }
    if flags.contains(MapFeatures::Rock)   { return Color::srgb(0.38, 0.35, 0.30); }

    Color::srgb(0.4, 0.4, 0.5)
}

fn cursor_to_tile(window: &Window, w: usize, h: usize) -> Option<(i32, i32)> {
    let cursor = window.cursor_position()?;
    let grid_left = (window.width()  * 0.5 - w as f32 * CELL_SIZE * 0.5).round();
    let grid_top  = (window.height() * 0.5 - h as f32 * CELL_SIZE * 0.5).round();
    let col = ((cursor.x - grid_left) / CELL_SIZE).floor() as i32;
    let row = ((cursor.y - grid_top)  / CELL_SIZE).floor() as i32;
    if col < 0 || row < 0 || col >= w as i32 || row >= h as i32 { return None; }
    Some((col, row))
}

/// Update the hover highlight position each frame.
pub fn update_hover_highlight(
    state: Res<MapEditorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut highlight_q: Query<(&mut Node, &mut Visibility), With<HoverHighlight>>,
) {
    let Ok((mut node, mut vis)) = highlight_q.single_mut() else { return };
    let Ok(window) = windows.single() else { return };

    match cursor_to_tile(window, state.width, state.height) {
        Some((col, row)) => {
            let grid_left = (window.width()  * 0.5 - state.width  as f32 * CELL_SIZE * 0.5).round();
            let grid_top  = (window.height() * 0.5 - state.height as f32 * CELL_SIZE * 0.5).round();
            node.left = Val::Px(grid_left + col as f32 * CELL_SIZE);
            node.top  = Val::Px(grid_top  + row  as f32 * CELL_SIZE);
            *vis = Visibility::Visible;
        }
        None => { *vis = Visibility::Hidden; }
    }
}

/// Map screen cursor position -> tile (col, row) using the same formula as rebuild_grid.
/// Uses `pressed` so drag-painting works.
pub fn handle_grid_click(
    mut state: ResMut<MapEditorState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let (left, right) = (mouse.pressed(MouseButton::Left), mouse.pressed(MouseButton::Right));
    if !left && !right { return; }

    let Ok(window) = windows.single() else { return };
    let Some((col, row)) = cursor_to_tile(window, state.width, state.height) else { return };

    let erasing = right || (left && state.erase_mode);
    if erasing {
        state.erase_at(col, row);
    } else if left {
        state.place_at(col, row);
    }
}
