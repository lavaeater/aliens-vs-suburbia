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

#[cfg(test)]
mod tests {
    use super::health_monitor_system;
    use bevy::prelude::*;
    use crate::general::components::{Health, Indestructible};
    use crate::gore::components::EntityDied;
    use crate::player::components::IsObstacle;

    /// Collects the deaths the system announced, so we can assert on them.
    #[derive(Resource, Default)]
    struct Caught(Vec<Entity>);

    fn catch(mut reader: MessageReader<EntityDied>, mut caught: ResMut<Caught>) {
        for died in reader.read() {
            caught.0.push(died.entity);
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<EntityDied>();
        app.init_resource::<Caught>();
        // catch runs after the monitor so it sees the message emitted this frame.
        app.add_systems(Update, (health_monitor_system, catch).chain());
        app
    }

    #[test]
    fn dead_nonplayer_despawns_and_announces_its_death() {
        let mut app = test_app();
        let dead = app.world_mut().spawn(Health { health: 0, max_health: 100 }).id();
        let alive = app.world_mut().spawn(Health { health: 40, max_health: 100 }).id();

        app.update();

        assert!(app.world().get::<Health>(dead).is_none(), "a dead entity should despawn");
        assert!(app.world().get::<Health>(alive).is_some(), "a living entity should survive");
        assert_eq!(app.world().resource::<Caught>().0, vec![dead], "exactly the dead one is announced");
    }

    #[test]
    fn indestructible_and_obstacles_are_left_for_other_systems() {
        let mut app = test_app();
        // Indestructible things never despawn from health loss...
        let indestructible = app
            .world_mut()
            .spawn((Health { health: 0, max_health: 100 }, Indestructible))
            .id();
        // ...and obstacles crumble via destroy_damaged_terrain, not here.
        let obstacle = app
            .world_mut()
            .spawn((Health { health: 0, max_health: 100 }, IsObstacle))
            .id();

        app.update();

        assert!(app.world().get::<Health>(indestructible).is_some());
        assert!(app.world().get::<Health>(obstacle).is_some());
        assert!(app.world().resource::<Caught>().0.is_empty(), "no deaths announced for those");
    }
}
