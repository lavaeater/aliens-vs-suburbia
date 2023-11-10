use bevy::core::Name;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Camera3dBundle, Commands, Transform};
use bevy::prelude::Projection::Perspective;
use bevy::utils::default;
use bevy_xpbd_3d::math::PI;
use crate::camera::components::camera::{CameraOffset, GameCamera};
pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::from("Camera"),
        CameraOffset(Vec3::new(0.0, 10.0, 10.0)),
        Camera3dBundle {
            projection: Perspective(Default::default()),
            transform: Transform {
                rotation: Quat::from_rotation_x(-PI / 4.),
                ..default()
            },
            ..default()
        },
        GameCamera {},
    ));
}