use bevy::hierarchy::{BuildChildren, Children};
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Color, Commands, Component, Entity, EventReader, EventWriter, Name, Query, Res, Transform, Visibility, With};
use bevy::scene::SceneBundle;
use bevy::utils::default;
use bevy_mod_outline::{OutlineBundle, OutlineVolume};
use bevy_xpbd_3d::components::{Collider};
use bevy_xpbd_3d::prelude::CollisionLayers;
use space_editor::prelude::PrefabBundle;
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{CollisionLayer};
use crate::general::events::map_events::SpawnPlayer;
use crate::player::bundle::PlayerBundle;
use crate::player::components::Player;
use crate::player::player_prefab::PlayerPrefab;
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
    mut add_health_bar_ew: EventWriter<AddHealthBar>,
    mut player_addedd_ew: EventWriter<GameTrackingEvent>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        let player = commands.spawn((
            PrefabBundle::new("girl_fini.scn.ron"),
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
                ])
        )).id();
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