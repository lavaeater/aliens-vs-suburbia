use bevy::prelude::{Commands, Entity, Query, Without};
use crate::general::components::{Health, Indestructible};
use crate::player::components::Player;

#[allow(clippy::type_complexity)]
pub fn health_monitor_system(
    mut commands: Commands,
    // Players stay alive as downed entities — handled by detect_player_death.
    // Indestructible entities are never despawned by health loss.
    query: Query<(Entity, &Health), (Without<Player>, Without<Indestructible>)>,
) {
    for (entity, health) in query.iter() {
        if health.health <= 0 {
            commands.entity(entity).despawn();
        }
    }
}
