use bevy::core::Name;
use bevy::math::{Quat, Rect, Vec2, Vec3};
use bevy::prelude::{Camera3dBundle, Commands, OrthographicProjection, Transform};
use bevy::prelude::Projection::{Orthographic};
use bevy::render::camera::ScalingMode;
use bevy::utils::default;
use bevy_xpbd_3d::math::PI;
use crate::camera::components::camera::{CameraOffset, GameCamera};
pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::from("Camera"),
        CameraOffset(Vec3::new(1.0, 1.0, 1.0)),
        Camera3dBundle {
            projection: Orthographic(OrthographicProjection {
                scale: 3.0,
                near: -100.0,
                far: 1000.0,
                viewport_origin: Vec2::new(0.5, 0.5),
                scaling_mode: ScalingMode::FixedVertical(2.0),
                area: Rect::new(-1.0, -1.0, 1.0, 1.0),
            }),
            transform: Transform {
                rotation: Quat::from_rotation_x(-PI / 4.),
                ..default()
            },
            ..default()
        },
        GameCamera {},
    ));
}