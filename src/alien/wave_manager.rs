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
                WaveDef { alien_count: 1, spawn_rate_per_minute:  6.0, delay_before:  60.0 },
                WaveDef { alien_count: 15, spawn_rate_per_minute:  9.0, delay_before: 30.0 },
                WaveDef { alien_count: 20, spawn_rate_per_minute: 12.0, delay_before: 20.0 },
            ],
            current_wave: 0,
            wave_timer: 60.0, // initial countdown before wave 1
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

#[allow(dead_code)]
pub fn count_wave_spawn(
    mut manager: ResMut<WaveManager>,
    tracker: Res<LevelTracker>,
) {
    // Sync spawned_this_wave from the global spawned count.
    let wave_offset: i32 = manager.waves[..manager.current_wave]
        .iter().map(|w| w.alien_count).sum();
    manager.spawned_this_wave = (tracker.aliens_to_spawn - tracker.aliens_left_to_spawn - wave_offset).max(0);
}

#[cfg(test)]
mod tests {
    use super::WaveManager;

    #[test]
    fn default_waves_sum_to_expected_total() {
        // 1 + 15 + 20 in the hardcoded default schedule.
        assert_eq!(WaveManager::default().total_aliens(), 36);
    }

    #[test]
    fn waves_remaining_flips_when_the_last_wave_is_consumed() {
        let mut wm = WaveManager::default();
        assert!(wm.waves_remaining());
        wm.current_wave = wm.waves.len();
        assert!(!wm.waves_remaining());
    }
}

#[cfg(test)]
mod wave_system_tests {
    use super::{wave_system, WaveManager};
    use bevy::prelude::*;
    use std::time::Duration;
    use crate::alien::components::general::AlienCounter;
    use crate::game_state::score_keeper::{LevelState, LevelTracker};
    use crate::general::components::map_components::AlienSpawnPoint;

    fn test_app(manager: WaveManager) -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        let mut tracker = LevelTracker::default();
        tracker.level_state = LevelState::InProgress;
        app.insert_resource(tracker);
        app.insert_resource(manager);
        app.insert_resource(AlienCounter { count: 0, max_count: 100 });
        app.add_systems(Update, wave_system);
        app
    }

    #[test]
    fn the_countdown_starts_the_wave_and_arms_the_spawn_points() {
        let mut manager = WaveManager::default();
        manager.wave_timer = 0.5; // about to start wave 0 (rate 6/min)
        let mut app = test_app(manager);
        let sp = app.world_mut().spawn(AlienSpawnPoint::new(0.0)).id();

        // Advance past the remaining countdown.
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(600));
        app.update();

        assert!(app.world().resource::<WaveManager>().spawning, "the wave should be spawning");
        let spawn_point = app.world().get::<AlienSpawnPoint>(sp).unwrap();
        assert_eq!(spawn_point.spawn_rate_per_minute, 6.0, "spawn point armed with wave 0's rate");
    }

    #[test]
    fn a_cleared_wave_advances_to_the_next_one() {
        let mut manager = WaveManager::default();
        manager.spawning = true;
        manager.current_wave = 0;
        manager.spawned_this_wave = manager.waves[0].alien_count; // all of wave 0 out
        let mut app = test_app(manager);
        // AlienCounter.count is 0 -> the field is clear.

        app.update();

        let m = app.world().resource::<WaveManager>();
        assert_eq!(m.current_wave, 1, "advanced to wave 1");
        assert!(!m.spawning, "between waves now");
        assert_eq!(m.spawned_this_wave, 0, "reset for the next wave");
    }

    #[test]
    fn a_wave_with_aliens_still_alive_does_not_advance() {
        let mut manager = WaveManager::default();
        manager.spawning = true;
        manager.spawned_this_wave = manager.waves[0].alien_count;
        let mut app = test_app(manager);
        app.world_mut().resource_mut::<AlienCounter>().count = 3; // still fighting

        app.update();

        assert_eq!(app.world().resource::<WaveManager>().current_wave, 0, "wave not cleared yet");
    }
}
