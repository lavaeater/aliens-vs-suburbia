use bevy::prelude::{Commands, Entity, Query, Without};
use crate::general::components::Health;
use crate::player::components::Player;

pub fn health_monitor_system(
    mut commands: Commands,
    // Players stay alive as downed entities — handled by detect_player_death.
    query: Query<(Entity, &Health), Without<Player>>,
) {
    for (entity, health) in query.iter() {
        if health.health <= 0 {
            commands.entity(entity).despawn();
        }
    }
}
