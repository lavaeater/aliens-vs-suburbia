use bevy::app::{App, Plugin, Update};
use crate::alien::systems::spawn_aliens::{alien_spawner_system, spawn_aliens};

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
