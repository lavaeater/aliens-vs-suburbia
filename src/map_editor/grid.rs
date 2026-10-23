use crate::general::components::map_components::HousePlacement;
use crate::map::MapFeatures;
use crate::map::procgen::{HouseResolveResult, WallCellKind};
use crate::map_editor::modal::Mode;
use crate::map_editor::state::MapEditorState;
use crate::ui::spawn_ui::StateMarker;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use enumflags2::BitFlags;

pub const CELL_SIZE: f32 = 24.0; // pixels per tile in the grid view

#[derive(Component)]
pub struct GridCamera;

#[derive(Component)]
pub struct GridCellMarker {
    #[allow(dead_code)]
    pub x: usize,
    #[allow(dead_code)]
    pub y: usize,
}

/// The editor's cursor: the keyboard's, which the mouse also moves by hovering.
#[derive(Component)]
pub struct HoverHighlight;

/// The rectangle being selected in Select mode.
#[derive(Component)]
pub struct SelectionOverlay;

pub fn spawn_grid_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..Default::default()
        },
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
        BorderColor::all(Color::WHITE),
        Visibility::Hidden,
        // Drawn over the tiles, which are respawned (and so re-ordered) on every edit.
        GlobalZIndex(10),
    ));

    commands.spawn((
        SelectionOverlay,
        StateMarker,
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(Val::Px(2.0)),
            ..Default::default()
        },
        BackgroundColor(Color::srgba(0.4, 0.7, 1.0, 0.2)),
        BorderColor::all(Color::srgba(0.5, 0.8, 1.0, 0.9)),
        Visibility::Hidden,
        GlobalZIndex(9),
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
    if !state.grid_dirty {
        return;
    }
    state.grid_dirty = false;

    for e in cells.iter() {
        commands.entity(e).despawn();
    }

    let Ok(window) = windows.single() else { return };
    let win_w = window.width();
    let win_h = window.height();

    let w = state.width;
    let h = state.height;

    // Top-left corner of the grid in screen space, centred on the window.
    let grid_left = (win_w * 0.5 - w as f32 * CELL_SIZE * 0.5).round();
    let grid_top = (win_h * 0.5 - h as f32 * CELL_SIZE * 0.5).round();

    for (row, cols) in state.tiles.iter().enumerate().take(h) {
        for (col, tile) in cols.iter().enumerate().take(w) {
            let color = tile_color(*tile);
            commands.spawn((
                GridCellMarker { x: col, y: row },
                StateMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(grid_left + col as f32 * CELL_SIZE),
                    top: Val::Px(grid_top + row as f32 * CELL_SIZE),
                    width: Val::Px(CELL_SIZE - 1.0),
                    height: Val::Px(CELL_SIZE - 1.0),
                    ..Default::default()
                },
                BackgroundColor(color),
            ));
        }
    }

    // Overlay: first-letter of each placement's def name.
    for p in &state.placements {
        if p.x < 0 || p.y < 0 || p.x >= w as i32 || p.y >= h as i32 {
            continue;
        }
        let label = std::path::Path::new(&p.def_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?");
        let initial = label
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        commands
            .spawn((
                StateMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(grid_left + p.x as f32 * CELL_SIZE),
                    top: Val::Px(grid_top + p.y as f32 * CELL_SIZE),
                    width: Val::Px(CELL_SIZE - 1.0),
                    height: Val::Px(CELL_SIZE - 1.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..Default::default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                Text::new(initial),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::WHITE),
            ));
    }

    // Houses over the tiles: what the game will stamp there when the level loads.
    for placement in &state.houses {
        let Some(resolved) = state.resolve_placed(placement) else {
            continue;
        };
        for (x, y, color) in house_cell_colors(&resolved, 0.9) {
            if !state.in_bounds(x, y) {
                continue;
            }
            commands.spawn((
                GridCellMarker {
                    x: x as usize,
                    y: y as usize,
                },
                StateMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(grid_left + x as f32 * CELL_SIZE),
                    top: Val::Px(grid_top + y as f32 * CELL_SIZE),
                    width: Val::Px(CELL_SIZE - 1.0),
                    height: Val::Px(CELL_SIZE - 1.0),
                    ..Default::default()
                },
                BackgroundColor(color),
            ));
        }
    }
}

/// A house's tiles coloured like the house editor draws them: floor, wall, door, window.
fn house_cell_colors(
    resolved: &HouseResolveResult,
    alpha: f32,
) -> impl Iterator<Item = (i32, i32, Color)> + '_ {
    let floor = Color::srgba(0.20, 0.22, 0.20, alpha);
    resolved
        .floor_cells
        .iter()
        .map(move |&(x, y)| (x, y, floor))
        .chain(resolved.wall_cells.iter().map(move |c| {
            let color = match c.kind {
                WallCellKind::Wall => Color::srgba(0.55, 0.42, 0.28, alpha),
                WallCellKind::Door => Color::srgba(0.25, 0.65, 0.30, alpha),
                WallCellKind::Window => Color::srgba(0.30, 0.55, 0.80, alpha),
            };
            (c.x, c.y, color)
        }))
}

/// One tile of the house brush's footprint under the cursor.
#[derive(Component)]
pub struct HouseGhost;

/// Show where the house brush would put its house: its footprint at the cursor, rebuilt
/// only when the cursor, the brush, its turn or the window changes.
pub fn update_house_ghost(
    state: Res<MapEditorState>,
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    ghosts: Query<Entity, With<HouseGhost>>,
    mut shown: Local<Option<(HousePlacement, Vec2)>>,
) {
    let Ok(window) = windows.single() else { return };
    let brush = match state.mode {
        Mode::Normal | Mode::Paint => state.house_brush_at(state.cursor.0, state.cursor.1),
        _ => None,
    };
    let wanted = brush.map(|b| (b, window.size()));
    if *shown == wanted {
        return;
    }
    for e in ghosts.iter() {
        commands.entity(e).despawn();
    }
    *shown = wanted;
    let Some((placement, _)) = shown.as_ref() else {
        return;
    };
    let Some(resolved) = state.resolve_placed(placement) else {
        return;
    };
    let grid_left = (window.width() * 0.5 - state.width as f32 * CELL_SIZE * 0.5).round();
    let grid_top = (window.height() * 0.5 - state.height as f32 * CELL_SIZE * 0.5).round();
    for (x, y, color) in house_cell_colors(&resolved, 0.5) {
        if !state.in_bounds(x, y) {
            continue;
        }
        commands.spawn((
            HouseGhost,
            StateMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(grid_left + x as f32 * CELL_SIZE),
                top: Val::Px(grid_top + y as f32 * CELL_SIZE),
                width: Val::Px(CELL_SIZE - 1.0),
                height: Val::Px(CELL_SIZE - 1.0),
                ..Default::default()
            },
            BackgroundColor(color),
            GlobalZIndex(8),
        ));
    }
}

fn tile_color(raw: u64) -> Color {
    if raw == 0 {
        return Color::srgb(0.08, 0.08, 0.08);
    }
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(raw);

    // Functional addors take display priority over terrain.
    if flags.contains(MapFeatures::PlayerSpawn) {
        return Color::srgb(0.2, 0.5, 0.9);
    }
    if flags.contains(MapFeatures::EnemySpawn) {
        return Color::srgb(0.8, 0.2, 0.2);
    }
    if flags.contains(MapFeatures::EnemyExit) {
        return Color::srgb(0.8, 0.6, 0.1);
    }
    if flags.contains(MapFeatures::ImpassableForPlayers)
        && flags.contains(MapFeatures::ImpassableForEnemies)
    {
        return Color::srgb(0.25, 0.22, 0.18); // solid wall — dark brown
    }
    if flags.contains(MapFeatures::ImpassableForPlayers) {
        return Color::srgb(0.55, 0.30, 0.12); // player-only wall — orange-brown
    }
    if flags.contains(MapFeatures::ImpassableForEnemies) {
        return Color::srgb(0.15, 0.30, 0.50); // alien-only wall — steel blue
    }

    // Terrain type.
    if flags.contains(MapFeatures::Water) {
        return Color::srgb(0.15, 0.35, 0.7);
    }
    if flags.contains(MapFeatures::Grass) {
        return Color::srgb(0.22, 0.55, 0.22);
    }
    if flags.contains(MapFeatures::Floor) {
        return Color::srgb(0.28, 0.38, 0.28);
    }
    if flags.contains(MapFeatures::Mud) {
        return Color::srgb(0.45, 0.32, 0.18);
    }
    if flags.contains(MapFeatures::Snow) {
        return Color::srgb(0.82, 0.88, 0.92);
    }
    if flags.contains(MapFeatures::Rock) {
        return Color::srgb(0.38, 0.35, 0.30);
    }

    Color::srgb(0.4, 0.4, 0.5)
}

fn cursor_to_tile(window: &Window, w: usize, h: usize) -> Option<(i32, i32)> {
    let cursor = window.cursor_position()?;
    let grid_left = (window.width() * 0.5 - w as f32 * CELL_SIZE * 0.5).round();
    let grid_top = (window.height() * 0.5 - h as f32 * CELL_SIZE * 0.5).round();
    let col = ((cursor.x - grid_left) / CELL_SIZE).floor() as i32;
    let row = ((cursor.y - grid_top) / CELL_SIZE).floor() as i32;
    if col < 0 || row < 0 || col >= w as i32 || row >= h as i32 {
        return None;
    }
    Some((col, row))
}

/// Hovering a new tile moves the cursor there. Only a *change* of hovered tile does, so
/// a mouse resting on the grid does not pull the cursor back after every arrow key.
pub fn follow_mouse(
    mut state: ResMut<MapEditorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut last_hovered: Local<Option<(i32, i32)>>,
) {
    let Ok(window) = windows.single() else { return };
    let hovered = cursor_to_tile(window, state.width, state.height);
    if hovered != *last_hovered {
        *last_hovered = hovered;
        if let Some(tile) = hovered
            && state.cursor != tile
        {
            state.cursor = tile;
        }
    }
}

/// Draw the cursor, tinted by mode, and the Select mode rectangle.
#[allow(clippy::type_complexity)]
pub fn update_hover_highlight(
    state: Res<MapEditorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut highlight_q: Query<
        (&mut Node, &mut Visibility, &mut BackgroundColor),
        (With<HoverHighlight>, Without<SelectionOverlay>),
    >,
    mut selection_q: Query<(&mut Node, &mut Visibility), With<SelectionOverlay>>,
) {
    let Ok(window) = windows.single() else { return };
    let grid_left = (window.width() * 0.5 - state.width as f32 * CELL_SIZE * 0.5).round();
    let grid_top = (window.height() * 0.5 - state.height as f32 * CELL_SIZE * 0.5).round();

    if let Ok((mut node, mut vis, mut bg)) = highlight_q.single_mut() {
        let (col, row) = state.cursor;
        node.left = Val::Px(grid_left + col as f32 * CELL_SIZE);
        node.top = Val::Px(grid_top + row as f32 * CELL_SIZE);
        node.border = UiRect::all(Val::Px(2.0));
        bg.0 = match state.mode {
            Mode::Paint => Color::srgba(0.4, 1.0, 0.4, 0.45),
            Mode::Erase => Color::srgba(1.0, 0.3, 0.3, 0.45),
            Mode::House => Color::srgba(1.0, 0.9, 0.2, 0.45),
            _ => Color::srgba(1.0, 1.0, 1.0, 0.25),
        };
        *vis = if state.in_bounds(col, row) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if let Ok((mut node, mut vis)) = selection_q.single_mut() {
        if let Mode::Select { anchor } = state.mode {
            let ((x0, y0), (x1, y1)) = MapEditorState::rect(anchor, state.cursor);
            node.left = Val::Px(grid_left + x0 as f32 * CELL_SIZE);
            node.top = Val::Px(grid_top + y0 as f32 * CELL_SIZE);
            node.width = Val::Px((x1 - x0 + 1) as f32 * CELL_SIZE - 1.0);
            node.height = Val::Px((y1 - y0 + 1) as f32 * CELL_SIZE - 1.0);
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

/// Map screen cursor position -> tile (col, row) using the same formula as rebuild_grid.
/// Uses `pressed` so drag-painting works.
pub fn handle_grid_click(
    mut state: ResMut<MapEditorState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(window) = windows.single() else { return };

    match state.mode {
        Mode::House => {
            handle_house_polygon_input(&mut state, &mouse, window);
            return;
        }
        // The mouse sizes a selection by hovering; clicks would fight the keys.
        Mode::Select { .. } | Mode::Search => return,
        _ => {}
    }
    // A new press is a new undo step.
    if mouse.any_just_pressed([MouseButton::Left, MouseButton::Right]) {
        state.stroke_open = false;
    }

    let (left, right) = (
        mouse.pressed(MouseButton::Left),
        mouse.pressed(MouseButton::Right),
    );
    if !left && !right {
        return;
    }

    let Some((col, row)) = cursor_to_tile(window, state.width, state.height) else {
        return;
    };

    state.begin_stroke();
    let erasing = right || (left && state.mode == Mode::Erase);
    if erasing {
        state.erase_at(col, row);
    } else if left {
        state.place_at(col, row);
    }
}

/// Click-to-place nodes for the House tool: each click adds a (perpendicular-snapped)
/// polygon node, clicking back on the first node closes and resolves the shape, and
/// right-click cancels the in-progress polygon.
fn handle_house_polygon_input(
    state: &mut MapEditorState,
    mouse: &ButtonInput<MouseButton>,
    window: &Window,
) {
    if mouse.just_pressed(MouseButton::Right) {
        state.cancel_house_polygon();
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        let Some((col, row)) = cursor_to_tile(window, state.width, state.height) else {
            return;
        };
        state.house_add_point(col, row);
    }
}

#[derive(Component)]
pub struct HousePreviewMarker;

/// Draws a marker at each node of the in-progress House polygon, plus a thin bar along
/// each edge so far, so the shape being drawn is actually legible rather than a scatter
/// of dots. Rebuilt fresh every frame while the tool is active — editor-only UI, not a
/// hot path.
pub fn update_house_preview(
    state: Res<MapEditorState>,
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    markers: Query<Entity, With<HousePreviewMarker>>,
) {
    for e in markers.iter() {
        commands.entity(e).despawn();
    }
    if state.house_points.is_empty() {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let grid_left = (window.width() * 0.5 - state.width as f32 * CELL_SIZE * 0.5).round();
    let grid_top = (window.height() * 0.5 - state.height as f32 * CELL_SIZE * 0.5).round();
    let cell_center = |x: i32, y: i32| {
        (
            grid_left + x as f32 * CELL_SIZE + CELL_SIZE * 0.5,
            grid_top + y as f32 * CELL_SIZE + CELL_SIZE * 0.5,
        )
    };

    const EDGE_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.8);
    const NODE_SIZE: f32 = 12.0;
    const EDGE_THICKNESS: f32 = 3.0;

    // Edges between consecutive placed nodes (the closing edge is only implied once the
    // polygon is actually resolved, so it isn't drawn here).
    for pair in state.house_points.windows(2) {
        let [p0, p1] = pair else { continue };
        let (x0, y0) = cell_center(p0.0, p0.1);
        let (x1, y1) = cell_center(p1.0, p1.1);
        let (left, top, width, height) = if (x1 - x0).abs() >= (y1 - y0).abs() {
            (
                x0.min(x1),
                y0 - EDGE_THICKNESS * 0.5,
                (x1 - x0).abs(),
                EDGE_THICKNESS,
            )
        } else {
            (
                x0 - EDGE_THICKNESS * 0.5,
                y0.min(y1),
                EDGE_THICKNESS,
                (y1 - y0).abs(),
            )
        };
        commands.spawn((
            HousePreviewMarker,
            StateMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(width),
                height: Val::Px(height),
                ..Default::default()
            },
            BackgroundColor(EDGE_COLOR),
        ));
    }

    for (i, &(x, y)) in state.house_points.iter().enumerate() {
        let color = if i == 0 {
            Color::srgba(1.0, 0.9, 0.2, 1.0)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 1.0)
        };
        let (cx, cy) = cell_center(x, y);
        commands.spawn((
            HousePreviewMarker,
            StateMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cx - NODE_SIZE * 0.5),
                top: Val::Px(cy - NODE_SIZE * 0.5),
                width: Val::Px(NODE_SIZE),
                height: Val::Px(NODE_SIZE),
                border: UiRect::all(Val::Px(1.5)),
                ..Default::default()
            },
            BackgroundColor(color),
            BorderColor::all(Color::BLACK),
        ));
    }
}
