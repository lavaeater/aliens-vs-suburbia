use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::{SceneBundle};
use bevy_third_person_camera::ThirdPersonCameraTarget;
use bevy_xpbd_3d::components::LockedAxes;
use bevy_xpbd_3d::prelude::{Collider, RigidBody};
use crate::player::components::general::{Controller, KeyboardController, Player};

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Name::from("Player"),
        Player {},
        KeyboardController {},
        Controller::default(),
        ThirdPersonCameraTarget {},
        SceneBundle {
            scene: asset_server.load("player.glb#Scene0"),
            transform: Transform::from_xyz(2.0, 0.0, -5.0),
            ..Default::default()
        },
        RigidBody::Dynamic,
        Collider::cuboid(0.45, 0.45, 0.3),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
    ));
}