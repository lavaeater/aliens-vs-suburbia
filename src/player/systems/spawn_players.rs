use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::{SceneBundle};
use bevy::utils::default;
use bevy_third_person_camera::ThirdPersonCameraTarget;
use bevy_xpbd_3d::components::LockedAxes;
use bevy_xpbd_3d::prelude::{AngularDamping, Collider, ExternalForce, ExternalTorque, Friction, LinearDamping, RigidBody};
use crate::player::components::general::{Controller, DynamicMovement, KeyboardController, KinematicMovement, Player};

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Name::from("Player"),
        Player {},
        KeyboardController {},
        Controller::default(),
        DynamicMovement {},
        ThirdPersonCameraTarget {},
        SceneBundle {
            scene: asset_server.load("player.glb#Scene0"),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..Default::default()
        },
        Friction::from(0.0),
        AngularDamping(1.0),
        LinearDamping(0.9),
        RigidBody::Dynamic,
        Collider::cuboid(0.5, 0.5, 0.45),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
    ));
}