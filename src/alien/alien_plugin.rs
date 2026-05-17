use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs};
use crate::alien::systems::spawn_aliens::{alien_spawner_system, spawn_aliens};
use crate::alien::wave_manager::{WaveManager, wave_system};
use crate::game_state::GameState;

#[allow(dead_code)]
pub struct AlienPlugin;

impl Plugin for AlienPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Update,
                (
                    alien_spawner_system,
                    spawn_aliens,
                ),
            );
    }
}

pub struct StatefulAlienPlugin;

impl Plugin for StatefulAlienPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(WaveManager::default())
            .add_systems(
                Update,
                (
                    wave_system,
                    alien_spawner_system,
                    spawn_aliens,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}
