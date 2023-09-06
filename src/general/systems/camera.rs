use bevy::math::{Quat, Vec3};
use bevy::prelude::{Camera3dBundle, Commands, Transform};
use bevy::utils::default;
use bevy_xpbd_3d::math::PI;
use crate::general::components::camera::GameCamera;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform {
                translation: Vec3::new(0.0, 5.0, 0.0),
                rotation: Quat::from_rotation_x(-PI / 4.),
                ..default()
            },
            ..default()
        },
        GameCamera {},
    ));
}