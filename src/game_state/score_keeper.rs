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
    pub player_entity: Entity,
    pub event_type: GameEvent,
}

impl GameTrackingEvent {
    pub fn new(player_entity: Entity, event_type: GameEvent) -> Self {
        Self {
            player_entity,
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
            .insert_resource(LevelTracker::default())
            .add_systems(Update, (game_tracking_event_system).run_if(in_state(GameState::InGame)))
        ;
    }
}

pub fn game_tracking_event_system(
    mut game_tracking_events: EventReader<GameTrackingEvent>,
    mut score_query: Query<&mut Score>,
    mut elements: Elements,
) {
    for event in game_tracking_events.read() {
        match event.event_type.clone() {
            GameEvent::PlayerAdded => {
                let p_entity = event.player_entity;
                elements.select("#ui-footer")
                    .add_child(eml! {
                        <span c:cell>
                            <label bind:value=from!(p_entity, Score:kills | fmt.c("Kills: {c}") )/>
                        </span>
                    });
            }
            GameEvent::PlayerRemoved => {
                //remove player from score keeper
            }
            GameEvent::AlienKilled => {
                if let Ok(mut score) = score_query.get_mut(event.player_entity) {
                    score.kills += 1;
                }
            }
            GameEvent::ShotFired => {
                if let Ok(mut score) = score_query.get_mut(event.player_entity) {
                    score.shots_fired += 1;
                }
            }
            GameEvent::ShotHit => {
                if let Ok(mut score) = score_query.get_mut(event.player_entity) {
                    score.shots_hit += 1;
                }
            }
        }
    }
}