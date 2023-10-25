use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::{SceneBundle};
use bevy::utils::default;
use bevy_third_person_camera::ThirdPersonCameraTarget;
use bevy_xpbd_3d::components::LockedAxes;
use bevy_xpbd_3d::prelude::{AngularDamping, Collider, ExternalForce, ExternalTorque, Friction, LinearDamping, RigidBody};
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
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..Default::default()
        },
        ExternalForce::default().with_persistence(false),
        ExternalTorque::default().with_persistence(false),
        Friction::default(),
        AngularDamping(1.0),
        LinearDamping(0.9),
        RigidBody::Dynamic,
        Collider::cuboid(0.45, 0.45, 0.3),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
    ));
}