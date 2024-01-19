use bevy::core::Name;
use bevy::math::{Mat3, Quat, Rect, Vec2, Vec3};
use bevy::prelude::{Camera3dBundle, Commands, OrthographicProjection, PerspectiveProjection, Projection, Query, Transform, With};
use bevy::prelude::Projection::{Orthographic, Perspective};
use bevy::render::camera::ScalingMode;
use bevy::utils::default;
use bevy_atmosphere::plugin::AtmosphereCamera;
use bevy_video_glitch::VideoGlitchSettings;
use bevy_xpbd_3d::math::PI;
use bevy_xpbd_3d::components::Position;
use crate::camera::camera_components::{CameraOffset, GameCamera};
use crate::player::components::Player;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::from("Camera"),
        CameraOffset(Vec3::new(0.0, 6.0, 6.0)),
        VideoGlitchSettings {
            intensity: 0.05,
            color_aberration: Mat3::IDENTITY
        },
        Camera3dBundle {
            projection: Projection::from(PerspectiveProjection {
                fov: 57.0,
                near: 0.1,
                far: 1000.0,
                aspect_ratio: 1.0,
            }),
            transform: Transform {
                rotation: Quat::from_rotation_x(-PI / 4.),
                ..default()
            },
            ..default()
        },
        GameCamera {},
        AtmosphereCamera::default(),
    ));
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
