use bevy::asset::{AssetServer};
use bevy::core::Name;
use bevy::hierarchy::{BuildChildren, Children};
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Commands, Component, Entity, EventReader, EventWriter, Query, Res, Transform, Visibility, With};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::alien::systems::spawn_aliens::{AnimationKey, CurrentAnimationKey};
use crate::control::components::{Controller, DynamicMovement, KeyboardController};
use crate::game_state::score_keeper::{GameEvent, GameTrackingEvent};
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::CurrentTile;
use crate::general::events::map_events::SpawnPlayer;
use crate::player::components::Player;
use crate::ui::spawn_ui::AddHealthBar;

#[derive(Component)]
pub struct FixSceneTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl FixSceneTransform {
    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }
}

pub fn spawn_players(
    mut spawn_player_event_reader: EventReader<SpawnPlayer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut add_health_bar_ew: EventWriter<AddHealthBar>,
    mut player_addedd_ew: EventWriter<GameTrackingEvent>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        player_addedd_ew.send(GameTrackingEvent::new("Player One".into(), GameEvent::PlayerAdded));
        let player = commands.spawn((
            Name::from("Player"),
            Player {
              key: "Player One".into()
            },
            FixSceneTransform::new(
                Vec3::new(0.0, -0.35, 0.0),
                Quat::from_euler(
                    EulerRot::YXZ,
                    180.0f32.to_radians(), 0.0, 0.0),
                Vec3::new(0.5, 0.5, 0.5),
            ),
            KeyboardController {},
            Controller::new(3.0, 3.0, 60.0),
            DynamicMovement {},
            SceneBundle {
                scene: asset_server.load("quaternius/worker.glb#Scene0"),
                transform: Transform::from_xyz(spawn_player.position.x, spawn_player.position.y, spawn_player.position.z),
                //     .with_rotation(Quat::from_euler(EulerRot::YXZ, 180.0f32.to_radians(), 0.0, 0.0))
                //     .with_scale(Vec3::new(0.25, 0.25, 0.25)),
                ..Default::default()
            },
            Friction::from(0.0),
            AngularDamping(1.0),
            LinearDamping(0.9),
            RigidBody::Dynamic,
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
        )).insert((
            CurrentAnimationKey::new("players".into(), AnimationKey::Walking),
        )).with_children(|children|
            { // Spawn the child colliders positioned relative to the rigid body
                children.spawn((Collider::capsule(0.4, 0.2), Transform::from_xyz(0.0, 0.0, 0.0)));
            }).id();
        add_health_bar_ew.send(AddHealthBar {
            entity: player,
            name: "PLAYER",
        });
    }
}

pub fn fix_model_transforms(
    mut commands: Commands,
    mut scene_instance_query: Query<(Entity, &FixSceneTransform, &Children)>,
    mut child_query: Query<&mut Transform, With<Visibility>>,
) {
    for (parent, fix_scene_transform, children) in scene_instance_query.iter_mut() {
        for child in children.iter() {
            if let Ok(mut transform) = child_query.get_mut(*child) {
                transform.translation = fix_scene_transform.translation;
                transform.rotation = fix_scene_transform.rotation;
                transform.scale = fix_scene_transform.scale;
                commands.entity(parent).remove::<FixSceneTransform>();
            }
        }
    }
}