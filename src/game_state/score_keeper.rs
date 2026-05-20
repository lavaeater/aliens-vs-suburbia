use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Component, Entity, Message, MessageReader, MessageWriter, Res, ResMut,
                    Resource, With, in_state, IntoScheduleConfigs, Query};
use bevy::time::Time;
use crate::alien::wave_manager::WaveManager;
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::player::components::Player;
use crate::ui::spawn_ui::GotoState;

#[allow(dead_code)]
#[derive(Debug, Component)]
pub struct Shooter(Entity);

#[derive(Debug, Component, Default)]
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
#[allow(dead_code)]
pub struct LevelTracker {
    pub level_name: String,
    pub aliens_to_spawn: i32,
    pub aliens_left_to_spawn: i32,
    pub aliens_killed: i32,
    pub spawn_rate_per_minute: f32,
    pub level_state: LevelState,
    pub aliens_reached_goal: i32,
    /// How many aliens reaching the goal triggers a loss.
    pub aliens_win_cut_off: i32,
    /// Cooldown before the end-screen transition fires, to let animations settle.
    pub end_delay: f32,
}

impl Default for LevelTracker {
    fn default() -> Self {
        LevelTracker {
            level_name: "Level 1".to_string(),
            aliens_to_spawn: 30,
            aliens_left_to_spawn: 30,
            aliens_killed: 0,
            spawn_rate_per_minute: 10.0,
            level_state: LevelState::NotStarted,
            aliens_reached_goal: 0,
            aliens_win_cut_off: 10,
            end_delay: 0.0,
        }
    }
}

impl LevelTracker {
    #[allow(dead_code)]
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
            end_delay: 0.0,
        }
    }
}

#[derive(Debug, Message, Clone)]
pub enum GameTrackingEvent {
    PlayerAdded(Entity),
    #[allow(dead_code)]
    PlayerRemoved(Entity),
    AlienKilled(Entity),
    ShotFired(Entity),
    ShotHit(Entity),
    AlienSpawned,
    AlienReachedGoal,
}

pub struct ScoreKeeperPlugin;

impl Plugin for ScoreKeeperPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<GameTrackingEvent>()
            .insert_resource(LevelTracker::default())
            .add_systems(Update, (
                game_tracking_event_system,
                level_state_system,
            )
                .run_if(in_state(GameState::InGame)),
            )
        ;
    }
}

pub fn game_tracking_event_system(
    mut game_tracking_events: MessageReader<GameTrackingEvent>,
    mut level_tracker: ResMut<LevelTracker>,
    mut score_query: Query<&mut Score>,
) {
    for event in game_tracking_events.read() {
        match event {
            GameTrackingEvent::PlayerAdded(_player) => {}
            GameTrackingEvent::PlayerRemoved(_) => {}
            GameTrackingEvent::AlienKilled(player) => {
                if let Ok(mut score) = score_query.get_mut(*player) {
                    score.kills += 1;
                }
                level_tracker.aliens_killed += 1;
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
            GameTrackingEvent::AlienSpawned => {
                level_tracker.aliens_left_to_spawn -= 1;
            }
            GameTrackingEvent::AlienReachedGoal => {
                level_tracker.aliens_reached_goal += 1;
            }
        }
    }
}

pub fn level_state_system(
    mut level_tracker: ResMut<LevelTracker>,
    mut goto_state_mw: MessageWriter<GotoState>,
    time: Res<Time>,
    player_query: Query<&Health, With<Player>>,
    wave_manager: Option<Res<WaveManager>>,
) {
    if matches!(level_tracker.level_state, LevelState::NotStarted) {
        level_tracker.level_state = LevelState::InProgress;
    }

    if matches!(level_tracker.level_state, LevelState::InProgress) {
        // Win: all waves done and all spawned aliens killed.
        let all_waves_done = wave_manager.as_ref().is_none_or(|wm| !wm.waves_remaining());
        let all_killed = level_tracker.aliens_killed >= level_tracker.aliens_to_spawn
            && level_tracker.aliens_to_spawn > 0;
        if all_waves_done && all_killed {
            level_tracker.level_state = LevelState::Completed;
        }

        // Lose: too many reached the goal.
        if level_tracker.aliens_reached_goal >= level_tracker.aliens_win_cut_off {
            level_tracker.level_state = LevelState::Failed;
        }

        // Lose: every player is out of health simultaneously.
        let players: Vec<&Health> = player_query.iter().collect();
        if !players.is_empty() && players.iter().all(|h| h.health <= 0) {
            level_tracker.level_state = LevelState::Failed;
        }
    }

    // Brief delay before transitioning so the game doesn't snap away instantly.
    if matches!(level_tracker.level_state, LevelState::Completed | LevelState::Failed) {
        level_tracker.end_delay += time.delta_secs();
        if level_tracker.end_delay >= 2.0 {
            goto_state_mw.write(GotoState { state: GameState::Menu });
        }
    }
}
