use crate::alien::components::general::AlienCounter;
use crate::game_state::score_keeper::{LevelState, LevelTracker};
use crate::general::components::map_components::AlienSpawnPoint;
use bevy::prelude::*;

/// Definition for one wave of alien spawns.
#[derive(Clone)]
pub struct WaveDef {
    pub alien_count: i32,
    pub spawn_rate_per_minute: f32,
    /// Seconds to wait after the previous wave (or level start) before this wave begins.
    pub delay_before: f32,
    /// Enemy def path for this wave. Empty = the built-in alien.
    pub enemy_def: String,
}

impl WaveDef {
    pub fn enemy_def(&self) -> Option<String> {
        (!self.enemy_def.is_empty()).then(|| self.enemy_def.clone())
    }
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
                WaveDef {
                    alien_count: 1,
                    spawn_rate_per_minute: 6.0,
                    delay_before: 60.0,
                    enemy_def: String::new(),
                },
                WaveDef {
                    alien_count: 15,
                    spawn_rate_per_minute: 9.0,
                    delay_before: 30.0,
                    enemy_def: String::new(),
                },
                WaveDef {
                    alien_count: 20,
                    spawn_rate_per_minute: 12.0,
                    delay_before: 20.0,
                    enemy_def: String::new(),
                },
            ],
            current_wave: 0,
            wave_timer: 60.0, // initial countdown before wave 1
            spawning: false,
            spawned_this_wave: 0,
        }
    }
}

impl WaveManager {
    /// Waves authored in a map file. The first wave starts after `first_delay` seconds,
    /// later ones `between` seconds after the previous wave is cleared.
    pub fn from_map(waves: &[crate::general::components::map_components::WaveDef]) -> Self {
        const FIRST_DELAY: f32 = 5.0;
        const BETWEEN: f32 = 15.0;
        Self {
            waves: waves
                .iter()
                .enumerate()
                .map(|(i, w)| WaveDef {
                    alien_count: w.count as i32,
                    spawn_rate_per_minute: w.spawn_rate_per_minute,
                    delay_before: if i == 0 { FIRST_DELAY } else { BETWEEN },
                    enemy_def: w.enemy_def.clone(),
                })
                .collect(),
            current_wave: 0,
            wave_timer: FIRST_DELAY,
            spawning: false,
            spawned_this_wave: 0,
        }
    }

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
            format!(
                "Wave {} / {}",
                self.current_wave.saturating_add(1),
                self.waves.len()
            )
        } else {
            format!(
                "Wave {} / {} in {:.0}s",
                self.current_wave.saturating_add(1),
                self.waves.len(),
                self.wave_timer
            )
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
    if !matches!(tracker.level_state, LevelState::InProgress) {
        return;
    }
    if !manager.waves_remaining() {
        return;
    }

    let dt = time.delta_secs();

    if manager.spawning {
        let Some(alien_count) = manager.waves.get(manager.current_wave).map(|w| w.alien_count)
        else {
            return;
        };
        let finished = manager.spawned_this_wave >= alien_count && alien_counter.count == 0;

        if finished {
            manager.current_wave = manager.current_wave.saturating_add(1);
            manager.spawning = false;
            manager.spawned_this_wave = 0;
            if let Some(delay) = manager.waves.get(manager.current_wave).map(|w| w.delay_before) {
                manager.wave_timer = delay;
            }
        }
    } else {
        manager.wave_timer -= dt;
        if manager.wave_timer <= 0.0 {
            let Some(wave) = manager.waves.get(manager.current_wave) else {
                return;
            };
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
pub fn count_wave_spawn(mut manager: ResMut<WaveManager>, tracker: Res<LevelTracker>) {
    // Sync spawned_this_wave from the global spawned count.
    let wave_offset: i32 = manager
        .waves
        .get(..manager.current_wave)
        .unwrap_or_default()
        .iter()
        .map(|w| w.alien_count)
        .sum();
    manager.spawned_this_wave =
        (tracker.aliens_to_spawn - tracker.aliens_left_to_spawn - wave_offset).max(0);
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
    use super::{WaveManager, wave_system};
    use crate::alien::components::general::AlienCounter;
    use crate::game_state::score_keeper::{LevelState, LevelTracker};
    use crate::general::components::map_components::AlienSpawnPoint;
    use bevy::prelude::*;
    use std::time::Duration;

    fn test_app(manager: WaveManager) -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        let tracker = LevelTracker {
            level_state: LevelState::InProgress,
            ..Default::default()
        };
        app.insert_resource(tracker);
        app.insert_resource(manager);
        app.insert_resource(AlienCounter {
            count: 0,
            max_count: 100,
        });
        app.add_systems(Update, wave_system);
        app
    }

    #[test]
    fn the_countdown_starts_the_wave_and_arms_the_spawn_points() {
        let manager = WaveManager {
            wave_timer: 0.5,
            ..Default::default()
        }; // about to start wave 0 (rate 6/min)
        let mut app = test_app(manager);
        let sp = app.world_mut().spawn(AlienSpawnPoint::new(0.0)).id();

        // Advance past the remaining countdown.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(600));
        app.update();

        assert!(
            app.world().resource::<WaveManager>().spawning,
            "the wave should be spawning"
        );
        let spawn_point = app.world().get::<AlienSpawnPoint>(sp).unwrap();
        assert_eq!(
            spawn_point.spawn_rate_per_minute, 6.0,
            "spawn point armed with wave 0's rate"
        );
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

        assert_eq!(
            app.world().resource::<WaveManager>().current_wave,
            0,
            "wave not cleared yet"
        );
    }
}
