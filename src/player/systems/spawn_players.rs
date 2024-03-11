use bevy::core::Name;
use bevy::hierarchy::{Children, Parent};
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Color, Commands, Component, EventReader, EventWriter, Has, Query, With};
use bevy::scene::SceneInstanceReady;
use bevy::utils::default;
use bevy_mod_outline::{OutlineBundle, OutlineVolume};
use bevy_xpbd_3d::prelude::CollisionLayers;
use space_editor::prelude::PrefabBundle;
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{CollisionLayer};
use crate::general::events::map_events::SpawnPlayer;
use crate::player::components::Player;

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

#[derive(Component)]
pub struct CheckModelChildren;

pub fn check_model_children(
    query: Query<(&CheckModelChildren, &Children), With<Player>>,
    name_query: Query<&Name>,
) {
    for (_check_model_children, children) in query.iter() {
        for child in children.iter() {
            let name = name_query.get(*child).unwrap();
            println!("Child: {:?}", child);
        }
    }
}

/*
fn camera_with_parent(
    q_child: Query<(&Parent, &Transform), With<Camera>>,
    q_parent: Query<&GlobalTransform>,
) {
    for (parent, child_transform) in q_child.iter() {
        // `parent` contains the Entity ID we can use
        // to query components from the parent:
        let parent_global_transform = q_parent.get(parent.get());

        // do something with the components
    }
}
 */

pub fn model_is_ready(
    mut scene_ready: EventReader<SceneInstanceReady>,
    p_query: Query<(&Name, &Children, Has<Player>)>,
) {
    for scene_ready in scene_ready.read() {
        if let Ok((name, children, has_player)) = p_query.get(scene_ready.parent) {
            println!("Scene is ready: {:?}", name);
            if has_player {
                println!("Player is READYYYY!");
            }
        }
    }
}

pub fn spawn_players(
    mut spawn_player_event_reader: EventReader<SpawnPlayer>,
    mut commands: Commands,
    mut player_addedd_ew: EventWriter<GameTrackingEvent>,
) {
    for _spawn_player in spawn_player_event_reader.read() {
        let player = commands.spawn((
            PrefabBundle::new("hazmat.scn.ron"),
            CheckModelChildren,
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
        ))
            .id();
        player_addedd_ew.send(
            GameTrackingEvent::PlayerAdded(player));
    }
}