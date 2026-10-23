//! The House Editor's drawing surface, in world space on its own `Camera2d`.
//!
//! Polygon points are whole-number *corner* coordinates (`procgen::Polygon`): cell
//! `(x, y)` is the square from corner `(x, y)` to `(x + 1, y + 1)`. Grid y grows
//! downwards like the map's rows; [`grid_to_world`] flips it into Bevy's y-up 2D space.
//! Outline, nodes, grid dots and the cursor preview are gizmos (redrawn every frame for
//! free); the resolved cells are sprites, rebuilt only when the state changes.
//!
//! The cursor goes through the camera ([`Camera::viewport_to_world_2d`]), so what is
//! drawn and what is clicked are the same space whatever the window size. The camera is
//! shifted so the origin sits in the middle of the area right of the side panel, and
//! clicks over the panel are ignored.

use crate::house_editor::state::HouseEditorState;
use crate::house_editor::ui::PANEL_WIDTH;
use crate::map::procgen::WallCellKind;
use crate::ui::spawn_ui::StateMarker;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// World units (pixels at the camera's default scale) per grid unit.
pub const UNIT_PX: f32 = 32.0;

/// How many grid units around the origin get a dot.
const GRID_REACH: i32 = 40;

#[derive(Component)]
pub struct HouseEditorCamera;

/// A sprite showing one resolved cell.
#[derive(Component)]
pub struct HouseCellSprite;

pub fn spawn_house_editor_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        HouseEditorCamera,
        StateMarker,
        // Half the panel to the left, so the origin is centred in the free area.
        Transform::from_xyz(-PANEL_WIDTH * 0.5, 0.0, 0.0),
    ));
}

/// The world position of grid corner `(x, y)`.
pub fn grid_to_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x * UNIT_PX, -y * UNIT_PX)
}

/// The grid corner nearest a world position.
pub fn world_to_grid(world: Vec2) -> (i32, i32) {
    (
        (world.x / UNIT_PX).round() as i32,
        (-world.y / UNIT_PX).round() as i32,
    )
}

/// `point` moved onto the horizontal or vertical line through `from`, whichever is
/// closer: the next edge comes out square.
pub fn snap_square(from: (i32, i32), point: (i32, i32)) -> (i32, i32) {
    if (point.0 - from.0).abs() >= (point.1 - from.1).abs() {
        (point.0, from.1)
    } else {
        (from.0, point.1)
    }
}

/// The grid corner the next click would place, if the cursor is over the canvas.
pub fn hovered_corner(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    keys: &ButtonInput<KeyCode>,
    state: &HouseEditorState,
) -> Option<(i32, i32)> {
    let cursor = window.cursor_position()?;
    if cursor.x < PANEL_WIDTH {
        return None;
    }
    let world = camera.viewport_to_world_2d(camera_transform, cursor).ok()?;
    let corner = world_to_grid(world);
    let square = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    Some(match state.points.last() {
        Some(&last) if square => snap_square(last, corner),
        _ => corner,
    })
}

pub fn handle_canvas_click(
    mut state: ResMut<HouseEditorState>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<HouseEditorCamera>>,
) {
    let (Ok(window), Ok((camera, camera_transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let Some(corner) = hovered_corner(window, camera, camera_transform, &keys, &state) else {
        return;
    };
    if mouse.just_pressed(MouseButton::Right) {
        if state.points.is_empty() {
            state.clear_all();
        } else {
            state.cancel_points();
        }
    } else if mouse.just_pressed(MouseButton::Left) {
        state.add_point(corner.0, corner.1);
    }
}

const fn wall_kind_color(kind: WallCellKind) -> Color {
    match kind {
        WallCellKind::Wall => Color::srgb(0.55, 0.42, 0.28),
        WallCellKind::Door => Color::srgb(0.25, 0.65, 0.30),
        WallCellKind::Window => Color::srgb(0.30, 0.55, 0.80),
    }
}

const GRID_DOT: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);
const ORIGIN: Color = Color::srgb(1.0, 0.3, 0.3);
const EDGE: Color = Color::srgba(1.0, 1.0, 1.0, 0.9);
const PREVIEW: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);
const FIRST_NODE: Color = Color::srgb(1.0, 0.9, 0.2);
const NODE: Color = Color::WHITE;
const HOVER: Color = Color::srgb(0.4, 1.0, 0.6);

/// Grid dots, the polygon, its nodes and where the next click lands.
pub fn draw_canvas(
    mut gizmos: Gizmos,
    state: Res<HouseEditorState>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<HouseEditorCamera>>,
) {
    for y in -GRID_REACH..=GRID_REACH {
        for x in -GRID_REACH..=GRID_REACH {
            gizmos.circle_2d(grid_to_world(x as f32, y as f32), 1.0, GRID_DOT);
        }
    }
    gizmos.circle_2d(Vec2::ZERO, 4.0, ORIGIN);

    let world: Vec<Vec2> = state
        .points
        .iter()
        .map(|&(x, y)| grid_to_world(x as f32, y as f32))
        .collect();
    for pair in world.windows(2) {
        if let &[from, to] = pair {
            gizmos.line_2d(from, to, EDGE);
        }
    }
    // The edge back to the first node: solid once resolved, a hint while drawing.
    if let (Some(&first), Some(&last)) = (world.first(), world.last())
        && world.len() >= 3
    {
        let color = if state.resolved.is_some() {
            EDGE
        } else {
            PREVIEW
        };
        gizmos.line_2d(last, first, color);
    }
    for (i, &at) in world.iter().enumerate() {
        gizmos.circle_2d(at, 6.0, if i == 0 { FIRST_NODE } else { NODE });
    }

    let (Ok(window), Ok((camera, camera_transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    if let Some((x, y)) = hovered_corner(window, camera, camera_transform, &keys, &state) {
        let at = grid_to_world(x as f32, y as f32);
        if state.resolved.is_none()
            && let Some(&last) = world.last()
        {
            gizmos.line_2d(last, at, PREVIEW);
        }
        gizmos.circle_2d(at, 8.0, HOVER);
    }
}

/// Rebuild the resolved house's cell sprites when the state changes.
pub fn redraw_cells(
    state: Res<HouseEditorState>,
    mut commands: Commands,
    existing: Query<Entity, With<HouseCellSprite>>,
) {
    if !state.is_changed() {
        return;
    }
    for e in existing.iter() {
        commands.entity(e).despawn();
    }
    let Some(result) = &state.resolved else {
        return;
    };
    let floor = Color::srgb(0.20, 0.22, 0.20);
    let cells = result
        .floor_cells
        .iter()
        .map(|&(x, y)| (x, y, floor))
        .chain(
            result
                .wall_cells
                .iter()
                .map(|c| (c.x, c.y, wall_kind_color(c.kind))),
        );
    for (x, y, color) in cells {
        // Cell (x, y) spans corners (x, y)..(x + 1, y + 1): centred half a unit in.
        let centre = grid_to_world(x as f32 + 0.5, y as f32 + 0.5);
        commands.spawn((
            HouseCellSprite,
            StateMarker,
            Sprite::from_color(color, Vec2::splat(UNIT_PX - 2.0)),
            // Behind the gizmos.
            Transform::from_translation(centre.extend(-1.0)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_round_trips_through_the_world() {
        for corner in [(0, 0), (3, -2), (-5, 7)] {
            let world = grid_to_world(corner.0 as f32, corner.1 as f32);
            assert_eq!(world_to_grid(world), corner);
        }
        // Grid y grows downwards, world y upwards.
        assert!(grid_to_world(0.0, 1.0).y < 0.0);
        // A click near a corner lands on it.
        assert_eq!(
            world_to_grid(Vec2::new(UNIT_PX * 0.4, -UNIT_PX * 1.6)),
            (0, 2)
        );
    }

    #[test]
    fn snapping_keeps_the_longer_direction() {
        assert_eq!(snap_square((0, 0), (5, 1)), (5, 0));
        assert_eq!(snap_square((0, 0), (1, -4)), (0, -4));
        assert_eq!(snap_square((2, 2), (2, 2)), (2, 2));
    }
}
