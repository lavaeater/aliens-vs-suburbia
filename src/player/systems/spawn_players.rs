use bevy::asset::{AssetServer};
use bevy::hierarchy::{BuildChildren, Children};
use bevy::math::{EulerRot, Quat, Vec3, vec3};
use bevy::prelude::{Commands, Component, Entity, EventReader, EventWriter, Query, Res, Transform, Visibility, With};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{Collider, CollisionLayers};
use bevy_xpbd_3d::prelude::Sensor;
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{CollisionLayer};
use crate::general::events::map_events::SpawnPlayer;
use crate::player::bundle::PlayerBundle;
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
    game_assets: Res<GameAssets>,
    mut add_health_bar_ew: EventWriter<AddHealthBar>,
    mut player_addedd_ew: EventWriter<GameTrackingEvent>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        let player = commands.spawn((
            FixSceneTransform::new(
                Vec3::new(0.0, -0.37, 0.0),
                Quat::from_euler(
                    EulerRot::YXZ,
                    180.0f32.to_radians(), 0.0, 0.0),
                Vec3::new(0.5, 0.5, 0.5),
            ),
            SceneBundle {
                scene: game_assets.girl_scene.clone(),
                transform: Transform::from_xyz(spawn_player.position.x, spawn_player.position.y, spawn_player.position.z),
                ..Default::default()
            },
            PlayerBundle::new(
                "player",
                "Player One",
                [CollisionLayer::Player].into(),
                [
                    CollisionLayer::Ball,
                    CollisionLayer::Impassable,
                    CollisionLayer::Floor,
                    CollisionLayer::Alien,
                    CollisionLayer::Player,
                    CollisionLayer::AlienSpawnPoint,
                    CollisionLayer::AlienGoal
                ].into(),
            ),
        )).with_children(|children|
            { // Spawn the child colliders positioned relative to the rigid body
                children.spawn(
                    (
                        Collider::capsule(0.4, 0.2),
                        Transform::from_xyz(0.0, 0.0, 0.0)));
                children.spawn((
                    CollisionLayers::new(
                        [CollisionLayer::PlayerAimSensor],
                        [
                            CollisionLayer::Alien,
                        ]),
                    Sensor,
                    Collider::triangle(vec3(0.0, 0.0, 0.0), vec3(2.0, 0.0, 0.0), vec3(0.0, 0.0, 2.0)),
                    Transform::from_xyz(0.0, 0.0, -0.25).with_rotation(Quat::from_euler(
                        EulerRot::YXZ,
                        135.0f32.to_radians(), 0.0, 0.0), )
                ));
            }).id();
        add_health_bar_ew.send(AddHealthBar {
            entity: player,
            name: "PLAYER",
        });
        player_addedd_ew.send(
            GameTrackingEvent::PlayerAdded(player));
    }
}

pub fn fix_scene_transform(
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