//! Mouse aiming for the keyboard player. The cursor is projected onto the ground plane
//! and the player aims toward that point, so guns and throws fire where you point.
//! Gamepad players aim with the right stick (or auto-aim) instead.
//!
//! `mouse_aim` writes `AutoAim` (the fire direction, consumed by shooting and throwing).
//! The body does *not* turn to face it: the hips follow the direction of travel and the
//! spine twists toward the aim instead -- see `player::systems::torso_twist`.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::components::GameCamera;
use crate::control::components::InputKeyboard;
use crate::player::components::{AutoAim, Player, PlayerDead};

/// Project the cursor onto the ground plane and point the keyboard player's `AutoAim`
/// from the player toward it.
#[allow(clippy::type_complexity)]
pub fn mouse_aim(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    mut players: Query<(&GlobalTransform, &mut AutoAim), (With<Player>, With<InputKeyboard>, Without<PlayerDead>)>,
) {
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    // The active window-rendering game camera (the pixelated render-texture cam is dead code).
    let Some((camera, cam_tf)) = cameras.iter().find(|(c, _)| c.is_active) else { return };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else { return };

    for (player_tf, mut aim) in players.iter_mut() {
        if let Some(dir) = ground_aim_from_ray(ray, player_tf.translation()) {
            aim.0 = dir;
        }
    }
}

/// Where on the ground (the horizontal plane through `player_pos`) does `ray` land, as a
/// normalized horizontal direction from the player? `None` if the ray is parallel to the
/// ground or lands on the player. Pure, so it's unit-testable without a window/camera.
pub fn ground_aim_from_ray(ray: Ray3d, player_pos: Vec3) -> Option<Vec3> {
    let dist = ray.intersect_plane(player_pos, InfinitePlane3d::new(Vec3::Y))?;
    let point = ray.get_point(dist);
    let dir = Vec3::new(point.x - player_pos.x, 0.0, point.z - player_pos.z);
    (dir.length_squared() > 1e-4).then(|| dir.normalize())
}

#[cfg(test)]
mod tests {
    use super::ground_aim_from_ray;
    use bevy::prelude::*;

    #[test]
    fn ray_straight_down_aims_from_player_to_the_hit_point() {
        // Cursor ray drops straight down onto (2, 0, 0); player sits at the origin.
        let ray = Ray3d { origin: Vec3::new(2.0, 10.0, 0.0), direction: Dir3::NEG_Y };
        let dir = ground_aim_from_ray(ray, Vec3::ZERO).expect("ray meets the ground");
        assert!((dir - Vec3::X).length() < 1e-5, "aim points +X toward the hit, got {dir:?}");
    }

    #[test]
    fn aim_is_flattened_onto_the_horizontal_plane() {
        // A slanted ray still yields a purely horizontal aim (no y component).
        let ray = Ray3d {
            origin: Vec3::new(0.0, 5.0, 0.0),
            direction: Dir3::new(Vec3::new(1.0, -1.0, 1.0)).unwrap(),
        };
        let dir = ground_aim_from_ray(ray, Vec3::ZERO).expect("ray meets the ground");
        assert!(dir.y.abs() < 1e-6, "aim must be flat, got {dir:?}");
        assert!((dir.length() - 1.0).abs() < 1e-5, "aim is normalized");
    }

    #[test]
    fn ray_parallel_to_the_ground_has_no_aim() {
        let ray = Ray3d { origin: Vec3::new(0.0, 5.0, 0.0), direction: Dir3::X };
        assert!(ground_aim_from_ray(ray, Vec3::ZERO).is_none());
    }

    #[test]
    fn ray_landing_on_the_player_has_no_aim() {
        let ray = Ray3d { origin: Vec3::new(0.0, 10.0, 0.0), direction: Dir3::NEG_Y };
        assert!(ground_aim_from_ray(ray, Vec3::ZERO).is_none(), "zero-length direction is rejected");
    }
}
