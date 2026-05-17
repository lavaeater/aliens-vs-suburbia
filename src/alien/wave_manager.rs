use bevy::prelude::*;
use crate::alien::components::general::AlienCounter;
use crate::game_state::score_keeper::{LevelState, LevelTracker};
use crate::general::components::map_components::AlienSpawnPoint;

/// Definition for one wave of alien spawns.
#[derive(Clone)]
pub struct WaveDef {
    pub alien_count: i32,
    pub spawn_rate_per_minute: f32,
    /// Seconds to wait after the previous wave (or level start) before this wave begins.
    pub delay_before: f32,
}

#[derive(Resource)]
pub struct WaveManager {
    pub waves: Vec<WaveDef>,
    pub current_wave: usize,
    /// Counts down to the start of the next wave.
    pub wave_timer: f32,
    /// True while the current wave is actively spawning.
    pub spawning: bool,
    /// How many aliens from the current wave have been spawned so far.
    pub spawned_this_wave: i32,
}

impl Default for WaveManager {
    fn default() -> Self {
        Self {
            waves: vec![
                WaveDef { alien_count: 10, spawn_rate_per_minute:  6.0, delay_before:  5.0 },
                WaveDef { alien_count: 15, spawn_rate_per_minute:  9.0, delay_before: 20.0 },
                WaveDef { alien_count: 20, spawn_rate_per_minute: 12.0, delay_before: 20.0 },
            ],
            current_wave: 0,
            wave_timer: 5.0, // initial countdown before wave 1
            spawning: false,
            spawned_this_wave: 0,
        }
    }
}

impl WaveManager {
    pub fn total_aliens(&self) -> i32 {
        self.waves.iter().map(|w| w.alien_count).sum()
    }

    pub fn waves_remaining(&self) -> bool {
        self.current_wave < self.waves.len()
    }

    pub fn label(&self) -> String {
        if !self.waves_remaining() {
            return "All waves done".to_string();
        }
        if self.spawning {
            format!("Wave {} / {}", self.current_wave + 1, self.waves.len())
        } else {
            format!("Wave {} / {} in {:.0}s", self.current_wave + 1, self.waves.len(), self.wave_timer)
        }
    }
}

/// Drives wave progression and gates spawning.
pub fn wave_system(
    time: Res<Time>,
    mut manager: ResMut<WaveManager>,
    mut tracker: ResMut<LevelTracker>,
    mut spawn_points: Query<&mut AlienSpawnPoint>,
    alien_counter: Res<AlienCounter>,
) {
    if !matches!(tracker.level_state, LevelState::InProgress) { return; }
    if !manager.waves_remaining() { return; }

    let dt = time.delta_secs();

    if manager.spawning {
        let wave = &manager.waves[manager.current_wave];
        let finished = manager.spawned_this_wave >= wave.alien_count
            && alien_counter.count == 0;

        if finished {
            manager.current_wave += 1;
            manager.spawning = false;
            manager.spawned_this_wave = 0;
            if manager.waves_remaining() {
                manager.wave_timer = manager.waves[manager.current_wave].delay_before;
            }
        }
    } else {
        manager.wave_timer -= dt;
        if manager.wave_timer <= 0.0 {
            let wave = &manager.waves[manager.current_wave];
            let rate = wave.spawn_rate_per_minute;
            for mut sp in spawn_points.iter_mut() {
                sp.spawn_rate_per_minute = rate;
                sp.spawn_cool_down = 0.0;
            }
            tracker.aliens_to_spawn = manager.total_aliens();
            tracker.aliens_left_to_spawn = manager.total_aliens() - tracker.aliens_killed;
            manager.spawning = true;
        }
    }
}

/// Called by alien_spawner_system to record that a spawn happened this wave.
pub fn count_wave_spawn(
    mut manager: ResMut<WaveManager>,
    tracker: Res<LevelTracker>,
) {
    // Sync spawned_this_wave from the global spawned count.
    let wave_offset: i32 = manager.waves[..manager.current_wave]
        .iter().map(|w| w.alien_count).sum();
    manager.spawned_this_wave = (tracker.aliens_to_spawn - tracker.aliens_left_to_spawn - wave_offset).max(0);
}
