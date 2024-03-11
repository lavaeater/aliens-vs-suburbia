use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Component, Entity, Event, in_state, IntoSystemConfigs, Resource};
use crate::game_state::GameState;
use bevy::prelude::*;
use crate::game_state::game_state_plugin::GotoState;


#[derive(Debug, Component)]
pub struct Shooter(Entity);

#[derive(Debug, Component, Default, Reflect, Clone)]
#[reflect(Component)]
pub struct Score {
    pub kills: u32,
    pub shots_fired: u32,
    pub shots_hit: u32,
}

impl Score {
    pub fn new() -> Self {
        Self {
            kills: 0,
            shots_fired: 0,
            shots_hit: 0,
        }
    }
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
    pub aliens_to_spawn: i32,
    pub aliens_left_to_spawn: i32,
    pub aliens_killed: i32,
    pub spawn_rate_per_minute: f32,
    pub level_state: LevelState,
    pub aliens_reached_goal: i32,
    pub aliens_win_cut_off: i32,
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
            aliens_reached_goal: 0,
            aliens_win_cut_off: 600,
        }
    }
}

impl LevelTracker {
    pub fn update(level_name: String, aliens_to_spawn: i32, spawn_rate_per_minute: f32, aliens_win_cutoff: i32) -> Self {
        Self {
            level_name,
            aliens_to_spawn,
            aliens_left_to_spawn: aliens_to_spawn,
            aliens_killed: 0,
            spawn_rate_per_minute,
            level_state: LevelState::NotStarted,
            aliens_reached_goal: 0,
            aliens_win_cut_off: aliens_win_cutoff,
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
    AlienReachedGoal,
}

#[derive(Debug, Event)]
pub struct ShotHit;

pub struct ScoreKeeperPlugin;

impl Plugin for ScoreKeeperPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<GameTrackingEvent>()
            .insert_resource(LevelTracker::default())
            .add_systems(Update, (
                level_state_system,
            )
                .run_if(in_state(GameState::InGame)),
            )
        ;
    }
}

pub fn level_state_system(
    mut level_tracker: ResMut<LevelTracker>,
    mut goto_state_ew: EventWriter<GotoState>,
) {
    if level_tracker.aliens_killed >= level_tracker.aliens_to_spawn {
        level_tracker.level_state = LevelState::Completed;
    }
    if level_tracker.aliens_reached_goal >= level_tracker.aliens_win_cut_off {
        level_tracker.level_state = LevelState::Failed;
    }
    if level_tracker.aliens_left_to_spawn == 0 {
        level_tracker.level_state = LevelState::InProgress;
    }
    match level_tracker.level_state {
        LevelState::NotStarted => {
            level_tracker.level_state = LevelState::InProgress;
        }
        LevelState::InProgress => {}
        LevelState::Completed => {
            goto_state_ew.send(GotoState { state: GameState::Menu });
        }
        LevelState::Failed => {
            goto_state_ew.send(GotoState { state: GameState::Menu });
        }
    }
}