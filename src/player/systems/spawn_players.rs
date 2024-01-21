use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, Component, EventReader, EventWriter};
use bevy_xpbd_3d::prelude::CollisionLayers;
use space_editor::prelude::PrefabBundle;
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{CollisionLayer};
use crate::general::events::map_events::SpawnPlayer;
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
    for _spawn_player in spawn_player_event_reader.read() {
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