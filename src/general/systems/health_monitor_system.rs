use bevy::math::Vec3;
use bevy::prelude::{Commands, Entity, MessageWriter, Query, Without};
use avian3d::prelude::Position;
use crate::general::components::{Health, Indestructible};
use crate::gore::components::{EntityDied, LastHit};
use crate::player::components::{IsObstacle, Player};

#[allow(clippy::type_complexity)]
pub fn health_monitor_system(
    mut commands: Commands,
    // Players stay alive as downed entities — handled by detect_player_death.
    // Indestructible entities are never despawned by health loss.
    // Obstacles (walls/towers) crumble into rubble via destroy_damaged_terrain instead.
    query: Query<(Entity, &Health, Option<&Position>, Option<&LastHit>), (Without<Player>, Without<Indestructible>, Without<IsObstacle>)>,
    mut died_mw: MessageWriter<EntityDied>,
) {
    for (entity, health, position, last_hit) in query.iter() {
        if health.health <= 0 {
            // Announce the death (gibs, death SFX subscribe) before despawning it.
            let last = last_hit.copied().unwrap_or_default();
            died_mw.write(EntityDied {
                entity,
                position: position.map(|p| p.0).unwrap_or(Vec3::ZERO),
                normal: last.normal,
                kind: last.kind,
            });
            commands.entity(entity).despawn();
        }
    }
}
