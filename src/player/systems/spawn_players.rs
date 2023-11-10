use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::general::components::{HittableTarget, Layer};
use crate::player::components::general::{Controller, DynamicMovement, KeyboardController, Player};

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Name::from("Player"),
        Player {},
        HittableTarget {},
        KeyboardController {},
        Controller::default(),
        DynamicMovement {},
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
        CollisionLayers::new([Layer::Player], [Layer::Ball, Layer::Wall, Layer::Floor, Layer::Alien, Layer::Player]),
    ));
}