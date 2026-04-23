use bevy::math::{Mat3, Quat, Rect, Vec2, Vec3};
use bevy::prelude::{Camera3d, Commands, Name, OrthographicProjection, Query, Transform, Visibility, With, Without, default};
use bevy::camera::{Projection, ScalingMode};
use std::f32::consts::PI;
use avian3d::prelude::Position;
use crate::camera::components::{CameraOffset, GameCamera};
use crate::general::systems::map_systems::WallOccluder;
use crate::player::components::Player;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::from("Camera"),
        CameraOffset(Vec3::new(2.0, 1.5, 2.0)),
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            viewport_origin: Vec2::new(0.5, 0.5),
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 2.0 },
            area: Rect::new(-1.0, -1.0, 1.0, 1.0),
            scale: 2.0,
        }),
        Transform {
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        GameCamera {},
    ));
}

/// Hide wall entities that are between the camera and the player in the XZ plane.
/// Uses a point-to-line-segment distance test with a fixed threshold.
pub fn wall_occlusion_system(
    camera_q: Query<&Transform, With<GameCamera>>,
    player_q: Query<&Position, With<Player>>,
    mut walls: Query<(&Transform, &mut Visibility), (With<WallOccluder>, Without<GameCamera>)>,
) {
    let Ok(cam_transform) = camera_q.single() else { return; };
    let Ok(player_pos) = player_q.single() else { return; };

    let cam = Vec2::new(cam_transform.translation.x, cam_transform.translation.z);
    let player = Vec2::new(player_pos.x, player_pos.z);
    let seg = player - cam;
    let seg_len_sq = seg.length_squared();

    for (wall_transform, mut visibility) in &mut walls {
        let w = Vec2::new(wall_transform.translation.x, wall_transform.translation.z);
        // Project w onto the camera→player segment
        let t = if seg_len_sq > 0.0 {
            ((w - cam).dot(seg) / seg_len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let closest = cam + seg * t;
        let dist = (w - closest).length();
        *visibility = if dist < 0.55 && t > 0.05 && t < 0.95 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

pub fn camera_follow(
    mut camera_query: Query<(&mut Transform, &CameraOffset), With<GameCamera>>,
    player_position: Query<&Position, With<Player>>,
) {
    for (mut camera_transform, offset) in camera_query.iter_mut() {
        for player_position in player_position.iter() {
            camera_transform.translation = camera_transform.translation.lerp(player_position.0 + offset.0, 0.9);
            camera_transform.look_at(player_position.0, Vec3::Y);
        }
    }
}
