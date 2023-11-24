use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs, OnEnter};
use crate::alien::systems::spawn_aliens::{alien_spawner_system, start_some_animations, load_animations, spawn_aliens};
use crate::game_state::GameState;

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
            .add_systems(
                OnEnter(GameState::InGame),
                load_animations,
            )
            .add_systems(
                Update,
                (
                    alien_spawner_system,
                    spawn_aliens,
                    start_some_animations
                ).run_if(in_state(GameState::InGame)),
            );
    }
}
