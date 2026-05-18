use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use bevy::window::PrimaryWindow;
use crate::map_editor::state::{MapEditorState, TILE_ALIEN_GOAL, TILE_ALIEN_SPAWN, TILE_PLAYER_SPAWN};
use crate::ui::spawn_ui::StateMarker;

pub const CELL_SIZE: f32 = 24.0; // pixels per tile in the grid view

#[derive(Component)]
pub struct GridCamera;

#[derive(Component)]
pub struct GridCellMarker { pub x: usize, pub y: usize }

pub fn spawn_grid_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera { order: 0, ..Default::default() },
        GridCamera,
        StateMarker,
        Transform::default(),
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

fn tile_color(tile: u8) -> Color {
    match tile {
        0                   => Color::srgb(0.08, 0.08, 0.08),
        1                   => Color::srgb(0.28, 0.38, 0.28),
        TILE_ALIEN_SPAWN    => Color::srgb(0.8, 0.2, 0.2),
        TILE_ALIEN_GOAL     => Color::srgb(0.8, 0.6, 0.1),
        TILE_PLAYER_SPAWN   => Color::srgb(0.2, 0.5, 0.9),
        _                   => Color::srgb(0.4, 0.4, 0.5),
    }
}

/// Map screen cursor position → tile (col, row) using the same formula as rebuild_grid.
pub fn handle_grid_click(
    mut state: ResMut<MapEditorState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let (left, right) = (mouse.just_pressed(MouseButton::Left), mouse.just_pressed(MouseButton::Right));
    if !left && !right { return; }

    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };

    let w = state.width;
    let h = state.height;

    let grid_left = (window.width()  * 0.5 - w as f32 * CELL_SIZE * 0.5).round();
    let grid_top  = (window.height() * 0.5 - h as f32 * CELL_SIZE * 0.5).round();

    let col = ((cursor.x - grid_left) / CELL_SIZE) as i32;
    let row = ((cursor.y - grid_top)  / CELL_SIZE) as i32;

    if col < 0 || row < 0 || col >= w as i32 || row >= h as i32 { return; }

    if left {
        state.place_at(col, row);
    } else {
        state.erase_placement_at(col, row);
    }
}
