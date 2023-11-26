use bevy::app::{App, Plugin};
use bevy::prelude::{Component, Entity, Event, Resource};

#[derive(Debug, Component)]
pub struct Shooter(Entity);

#[derive(Debug, Resource)]
pub struct ScoreKeeper {
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
    pub level_state: LevelState
}

impl Default for LevelTracker {
    fn default() -> Self {
        LevelTracker {
            level_name: "Level 1".to_string(),
            aliens_to_spawn: 10,
            aliens_left_to_spawn: 10,
            aliens_killed: 0,
            spawn_rate_per_minute: 10.0,
            level_state: LevelState::NotStarted
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
            level_state: LevelState::NotStarted
        }
    }
}

#[derive(Debug, Event)]
pub struct ShotFired;

#[derive(Debug, Event)]
pub struct ShotHit;

pub struct ScoreKeeperPlugin;

impl Plugin for ScoreKeeperPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(ScoreKeeper {
                kills: 0,
                shots_fired: 0,
                shots_hit: 0,
            })
            .insert_resource(LevelTracker::default());
    }
}