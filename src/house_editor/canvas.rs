use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use bevy::window::PrimaryWindow;
use crate::house_editor::state::HouseEditorState;
use crate::map::procgen::WallCellKind;
use crate::ui::spawn_ui::StateMarker;

/// Pixels per whole-number world unit. No grid is drawn — this is only used to place
/// nodes and result cells on screen; the underlying coordinates are just integers.
pub const UNIT_PX: f32 = 32.0;

#[derive(Component)]
pub struct HouseEditorCamera;

#[derive(Component)]
pub struct HouseCanvasMarker;

pub fn spawn_house_editor_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera { order: 0, ..default() },
        HouseEditorCamera,
        StateMarker,
        Transform::default(),
    ));
}

/// Screen-space cursor position -> whole-number world coordinate, centered on the window.
/// No snapping beyond rounding to the nearest integer unit — free-angle polygons are fine,
/// the resolver rasterizes any closed shape.
fn cursor_to_world(window: &Window) -> Option<(i32, i32)> {
    let cursor = window.cursor_position()?;
    let cx = window.width() * 0.5;
    let cy = window.height() * 0.5;
    let wx = ((cursor.x - cx) / UNIT_PX).round() as i32;
    let wy = ((cursor.y - cy) / UNIT_PX).round() as i32;
    Some((wx, wy))
}

fn world_to_screen(window: &Window, x: i32, y: i32) -> (f32, f32) {
    (window.width() * 0.5 + x as f32 * UNIT_PX, window.height() * 0.5 + y as f32 * UNIT_PX)
}

pub fn handle_canvas_click(
    mut state: ResMut<HouseEditorState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(window) = windows.single() else { return };

    if mouse.just_pressed(MouseButton::Right) {
        if state.points.is_empty() {
            state.clear_all();
        } else {
            state.cancel_points();
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        let Some((x, y)) = cursor_to_world(window) else { return };
        state.add_point(x, y);
    }
}

fn wall_kind_color(kind: WallCellKind) -> Color {
    match kind {
        WallCellKind::Wall => Color::srgb(0.55, 0.42, 0.28),
        WallCellKind::Door => Color::srgb(0.25, 0.65, 0.30),
        WallCellKind::Window => Color::srgb(0.30, 0.55, 0.80),
    }
}

/// Redraws the whole canvas every frame: the in-progress polygon (nodes + edges) and, if
/// present, the last resolved house's floor/wall/door/window cells. Editor-only UI, not a
/// hot path, so despawn-and-respawn is simplest.
pub fn redraw_canvas(
    state: Res<HouseEditorState>,
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    existing: Query<Entity, With<HouseCanvasMarker>>,
) {
    for e in existing.iter() { commands.entity(e).despawn(); }

    let Ok(window) = windows.single() else { return };

    // Resolved result first, so in-progress polygon markers draw on top.
    if let Some(result) = &state.resolved {
        for &(x, y) in &result.floor_cells {
            spawn_cell(&mut commands, window, x, y, Color::srgb(0.20, 0.22, 0.20), UNIT_PX - 2.0);
        }
        for cell in &result.wall_cells {
            spawn_cell(&mut commands, window, cell.x, cell.y, wall_kind_color(cell.kind), UNIT_PX - 2.0);
        }
    }

    const EDGE_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.85);
    const EDGE_THICKNESS: f32 = 3.0;
    for pair in state.points.windows(2) {
        let (x0, y0) = world_to_screen(window, pair[0].0, pair[0].1);
        let (x1, y1) = world_to_screen(window, pair[1].0, pair[1].1);
        let (left, top, width, height) = if (x1 - x0).abs() >= (y1 - y0).abs() {
            (x0.min(x1), y0 - EDGE_THICKNESS * 0.5, (x1 - x0).abs(), EDGE_THICKNESS)
        } else {
            (x0 - EDGE_THICKNESS * 0.5, y0.min(y1), EDGE_THICKNESS, (y1 - y0).abs())
        };
        commands.spawn((
            HouseCanvasMarker, StateMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left), top: Val::Px(top), width: Val::Px(width), height: Val::Px(height),
                ..default()
            },
            BackgroundColor(EDGE_COLOR),
        ));
    }

    const NODE_SIZE: f32 = 14.0;
    for (i, &(x, y)) in state.points.iter().enumerate() {
        let color = if i == 0 { Color::srgba(1.0, 0.9, 0.2, 1.0) } else { Color::srgba(1.0, 1.0, 1.0, 1.0) };
        let (cx, cy) = world_to_screen(window, x, y);
        commands.spawn((
            HouseCanvasMarker, StateMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cx - NODE_SIZE * 0.5),
                top:  Val::Px(cy - NODE_SIZE * 0.5),
                width:  Val::Px(NODE_SIZE),
                height: Val::Px(NODE_SIZE),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(Color::BLACK),
        ));
    }

    // Origin marker — the only fixed reference point on an otherwise grid-less canvas.
    let (ox, oy) = world_to_screen(window, 0, 0);
    commands.spawn((
        HouseCanvasMarker, StateMarker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(ox - 3.0), top: Val::Px(oy - 3.0), width: Val::Px(6.0), height: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.9)),
    ));
}

fn spawn_cell(commands: &mut Commands, window: &Window, x: i32, y: i32, color: Color, size: f32) {
    let (cx, cy) = world_to_screen(window, x, y);
    commands.spawn((
        HouseCanvasMarker, StateMarker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(cx - size * 0.5),
            top:  Val::Px(cy - size * 0.5),
            width:  Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        BackgroundColor(color),
    ));
}
