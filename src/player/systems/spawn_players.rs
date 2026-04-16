use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Children, Commands, Component, Entity, MessageReader, MessageWriter, Query,
                    Res, Transform, Visibility, With};
use bevy::scene::SceneRoot;
use bevy_mod_outline::OutlineVolume;
use avian3d::prelude::Collider;
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::CollisionLayer;
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
    mut spawn_player_event_reader: MessageReader<SpawnPlayer>,
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    mut player_added_mw: MessageWriter<GameTrackingEvent>,
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
            SceneRoot(game_assets.girl_scene.clone()),
            Transform::from_xyz(spawn_player.position.x, spawn_player.position.y, spawn_player.position.z),
            PlayerBundle::new(
                "player",
                "Player One",
                [CollisionLayer::Player],
                [
                    CollisionLayer::Ball,
                    CollisionLayer::Impassable,
                    CollisionLayer::Floor,
                    CollisionLayer::Alien,
                    CollisionLayer::Player,
                    CollisionLayer::AlienSpawnPoint,
                    CollisionLayer::AlienGoal
                ],
            ),
            OutlineVolume {
                visible: true,
                width: 1.0,
                colour: bevy::prelude::Color::BLACK,
            },
        )).with_children(|children| {
            children.spawn((
                Collider::capsule(0.4, 0.2),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        }).id();
        add_health_bar_mw.write(AddHealthBar {
            entity: player,
            name: "PLAYER",
        });
        player_added_mw.write(GameTrackingEvent::PlayerAdded(player));
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
