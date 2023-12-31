use bevy::hierarchy::DespawnRecursiveExt;
use bevy::prelude::{Commands, Entity, Query};
use crate::general::components::Health;

pub fn health_monitor_system(
    mut commands: Commands,
    query: Query<(Entity, &Health)>,
) {
    for (entity, health) in query.iter() {
        if health.health <= 0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}
