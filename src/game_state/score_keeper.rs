use belly::build::{Elements, eml};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Component, Entity, Event, EventReader, in_state, IntoSystemConfigs, Resource};
use crate::game_state::GameState;
use bevy::prelude::*;
use belly::prelude::*;


#[derive(Debug, Component)]
pub struct Shooter(Entity);

#[derive(Debug, Component, Default)]
pub struct Score {
    pub kills: u32,
    pub shots_fired: u32,
    pub shots_hit: u32,
}


#[derive(Debug)]
pub enum LevelState {
    NotStarted,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Resource)]
pub struct LevelTracker {
    pub level_name: String,
    pub aliens_to_spawn: u32,
    pub aliens_left_to_spawn: u32,
    pub aliens_killed: u32,
    pub spawn_rate_per_minute: f32,
    pub level_state: LevelState,
}

impl Default for LevelTracker {
    fn default() -> Self {
        LevelTracker {
            level_name: "Level 1".to_string(),
            aliens_to_spawn: 10,
            aliens_left_to_spawn: 10,
            aliens_killed: 0,
            spawn_rate_per_minute: 10.0,
            level_state: LevelState::NotStarted,
        }
    }
}

impl LevelTracker {
    pub fn update(level_name: String, aliens_to_spawn: u32, spawn_rate_per_minute: f32) -> Self {
        LevelTracker {
            level_name,
            aliens_to_spawn,
            aliens_left_to_spawn: aliens_to_spawn,
            aliens_killed: 0,
            spawn_rate_per_minute,
            level_state: LevelState::NotStarted,
        }
    }
}

#[derive(Debug, Event)]
pub enum GameTrackingEvent {
    PlayerAdded(Entity),
    PlayerRemoved(Entity),
    AlienKilled(Entity),
    ShotFired(Entity),
    ShotHit(Entity),
    AlienSpawned,
}
//
// impl GameTrackingEvent {
//     pub fn new(related_entity: Entity, event_type: GameEvent) -> Self {
//         Self {
//             related_entity,
//             event_type,
//         }
//     }
// }

#[derive(Debug, Event)]
pub struct ShotHit;

pub struct ScoreKeeperPlugin;

impl Plugin for ScoreKeeperPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<GameTrackingEvent>()
            .insert_resource(LevelTracker::default())
            .add_systems(Update, (game_tracking_event_system).run_if(in_state(GameState::InGame)))
        ;
    }
}

pub fn game_tracking_event_system(
    mut game_tracking_events: EventReader<GameTrackingEvent>,
    mut level_tracker: ResMut<LevelTracker>,
    mut score_query: Query<&mut Score>,
    mut elements: Elements,
) {
    for event in game_tracking_events.read() {
        match event {
            GameTrackingEvent::PlayerAdded(player) => {
                let p = *player;
                elements.select("#ui-footer")
                    .add_child(eml! {
                        <span c:cell>
                            <label bind:value=from!(p, Score:kills | fmt.c("Kills: {c}") )/>
                            <label bind:value=from!(p, Score:shots_fired | fmt.c("Shots: {c}") )/>
                            <label bind:value=from!(p, Score:shots_hit | fmt.c("Hits: {c}") )/>
                        </span>
                    });
            }
            GameTrackingEvent::PlayerRemoved(_) => {}
            GameTrackingEvent::AlienKilled(player) => {
                if let Ok(mut score) = score_query.get_mut(*player) {
                    score.kills += 1;
                }
            }
            GameTrackingEvent::ShotFired(player) => {
                if let Ok(mut score) = score_query.get_mut(*player) {
                    score.shots_fired += 1;
                }
            }
            GameTrackingEvent::ShotHit(player) => {
                if let Ok(mut score) = score_query.get_mut(*player) {
                    score.shots_hit += 1;
                }
            }
            GameTrackingEvent::AlienSpawned => {}
        }
    }
}