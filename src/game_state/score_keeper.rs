use belly::build::{Elements, eml};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Commands, Component, Entity, Event, EventReader, in_state, IntoSystemConfigs, OnEnter, Resource};
use bevy::utils::HashMap;
use crate::game_state::GameState;

#[derive(Debug, Component)]
pub struct Shooter(Entity);

#[derive(Debug)]
pub struct Score {
    pub kills: u32,
    pub shots_fired: u32,
    pub shots_hit: u32,
}

#[derive(Debug, Resource)]
pub struct ScoreKeeper(HashMap<String, Score>);


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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum GameEvent {
    PlayerAdded,
    PlayerRemoved,
    AlienKilled,
    ShotFired,
    ShotHit,
}

#[derive(Debug, Event)]
pub struct GameTrackingEvent {
    pub player_key: String,
    pub event_type: GameEvent,
}

impl GameTrackingEvent {
    pub fn new(player_key: String, event_type: GameEvent) -> Self {
        Self {
            player_key,
            event_type,
        }
    }
}

#[derive(Debug, Event)]
pub struct ShotHit;

pub struct ScoreKeeperPlugin;

impl Plugin for ScoreKeeperPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<GameTrackingEvent>()
            .insert_resource(ScoreKeeper(HashMap::new()))
            .insert_resource(LevelTracker::default())
            .add_systems(Update, (game_tracking_event_system).run_if(in_state(GameState::InGame)))
        ;
    }
}

pub fn game_tracking_event_system(
    mut score_keeper: bevy::prelude::ResMut<ScoreKeeper>,
    mut game_tracking_events: EventReader<GameTrackingEvent>,
    mut elements: Elements,
    mut commands: Commands,
) {
    for event in game_tracking_events.read() {
        match event.event_type.clone() {
            GameEvent::PlayerAdded => {
                score_keeper.0.insert(event.player_key.clone(), Score {
                    kills: 0,
                    shots_fired: 0,
                    shots_hit: 0,
                });
                // let erp = add_health_bar.entity;
            }
            GameEvent::PlayerRemoved => {
                score_keeper.0.remove(&event.player_key);
            }
            GameEvent::AlienKilled => {
                if let Some(score) = score_keeper.0.get_mut(&event.player_key) {
                    score.kills += 1;
                }
            }
            GameEvent::ShotFired => {
                if let Some(score) = score_keeper.0.get_mut(&event.player_key) {
                    score.shots_fired += 1;
                }
            }
            GameEvent::ShotHit => {
                if let Some(score) = score_keeper.0.get_mut(&event.player_key) {
                    score.shots_hit += 1;
                }
            }
        }
    }
}