use bevy::core::Name;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Camera3dBundle, Commands, Transform};
use bevy::utils::default;
use bevy_third_person_camera::ThirdPersonCamera;
use bevy_xpbd_3d::math::PI;
use crate::camera::components::camera::GameCamera;
pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::from("Camera"),
        ThirdPersonCamera {
            mouse_orbit_button_enabled: true,
            cursor_lock_active: false,
            zoom_enabled: false,
            // offset_enabled: true,
            // offset: Offset::new(5.0,5.0),
            ..default()
        },
        Camera3dBundle {
            transform: Transform {
                translation: Vec3::new(-2.3, 5.0, 6.0),
                rotation: Quat::from_rotation_x(-PI / 4.),
                ..default()
            },
            ..default()
        },
        GameCamera {},
    ));
}