use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs};
use crate::alien::enemy_defs::{build_enemy_anim_graphs, ranged_attacks, EnemyDefCache};
use crate::alien::systems::spawn_aliens::{alien_spawner_system, spawn_aliens};
use crate::alien::wave_manager::{WaveManager, wave_system};
use crate::game_state::GameState;

pub struct StatefulAlienPlugin;

impl Plugin for StatefulAlienPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(WaveManager::default())
            .init_resource::<EnemyDefCache>()
            .add_systems(
                Update,
                (
                    wave_system,
                    alien_spawner_system,
                    spawn_aliens,
                    build_enemy_anim_graphs,
                    ranged_attacks,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}
