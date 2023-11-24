use bevy::animation::{AnimationClip, AnimationPlayer};
use bevy::asset::{AssetServer, Handle};
use bevy::core::Name;
use bevy::math::Vec3;
use bevy::prelude::{Added, Commands, EventReader, EventWriter, Query, Res, Resource, Transform, With};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::control::components::{Controller, DynamicMovement, KeyboardController};
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::CurrentTile;
use crate::general::events::map_events::SpawnPlayer;
use crate::player::components::Player;
use crate::ui::spawn_ui::AddHealthBar;
pub fn spawn_players(
    mut spawn_player_event_reader: EventReader<SpawnPlayer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut add_health_bar_ew: EventWriter<AddHealthBar>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        let player = commands.spawn((
            Name::from("Player"),
            Player {},
            KeyboardController {},
            Controller::new(3.0, 3.0, 60.0),
            DynamicMovement {},
            SceneBundle {
                scene: asset_server.load("quaternius/astronaut_rotated.glb#Scene0"),
                transform: Transform::from_xyz(spawn_player.position.x, spawn_player.position.y, spawn_player.position.z).with_scale(Vec3::new(0.5, 0.5, 0.5)),
                ..Default::default()
            },
            Friction::from(0.0),
            AngularDamping(1.0),
            LinearDamping(0.9),
            RigidBody::Dynamic,
            Collider::capsule(0.25, 0.25),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            CollisionLayers::new(
                [CollisionLayer::Player],
                [
                    CollisionLayer::Ball,
                    CollisionLayer::Impassable,
                    CollisionLayer::Floor,
                    CollisionLayer::Alien,
                    CollisionLayer::Player,
                    CollisionLayer::AlienSpawnPoint,
                    CollisionLayer::AlienGoal
                ]),
            Health::default(),
            CurrentTile::default(),
        )).id();

        add_health_bar_ew.send(AddHealthBar {
            entity: player,
            name: "PLAYER",
        });
    }
}


#[derive(Resource)]
pub struct PlayerAnimations(Vec<Handle<AnimationClip>>);

pub fn load_player_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(PlayerAnimations(vec![
        asset_server.load("quaternius/astronaut_rotated.glb#Animation0"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation1"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation2"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation3"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation4"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation5"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation6"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation7"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation8"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation9"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation10"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation11"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation12"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation13"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation14"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation15"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation16"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation17"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation18"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation19"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation20"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation21"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation22"),
        asset_server.load("quaternius/astronaut_rotated.glb#Animation23"),
    ]));
}

pub fn animate_players(
    animations: Res<PlayerAnimations>,
    mut players: Query<&mut AnimationPlayer, With<Player>>,
) {
    for mut player in  players.iter_mut() {
        if player.is_paused() {
            player.play(animations.0[22].clone_weak()).repeat();
        }
    }
}