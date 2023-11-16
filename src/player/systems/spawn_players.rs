use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::prelude::{Commands, EventReader, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::general::components::{Health, Layer};
use crate::general::components::map_components::CurrentTile;
use crate::general::events::map_events::SpawnPlayer;
use crate::player::components::general::{Controller, DynamicMovement, KeyboardController, Player};

pub fn spawn_players(
    mut spawn_player_event_reader: EventReader<SpawnPlayer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        commands.spawn((
            Name::from("Player"),
            Player {},
            KeyboardController {},
            Controller::new (3.0, 3.0, 120.0),
            DynamicMovement {},
            SceneBundle {
                scene: asset_server.load("player.glb#Scene0"),
                transform: Transform::from_xyz(spawn_player.position.x, spawn_player.position.y, spawn_player.position.z),
                ..Default::default()
            },
            Friction::from(0.0),
            AngularDamping(1.0),
            LinearDamping(0.9),
            RigidBody::Dynamic,
            Collider::capsule(0.25, 0.25),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            CollisionLayers::new([Layer::Player], [Layer::Ball, Layer::Wall, Layer::Floor, Layer::Alien, Layer::Player, Layer::AlienSpawnPoint, Layer::AlienGoal]),
            Health::default(),
            CurrentTile::default(),
        ));
    }
}