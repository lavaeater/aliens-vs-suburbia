use bevy::math::Vec3;
use bevy::prelude::{Commands, MessageReader, MessageWriter, Query, With};
use avian3d::prelude::{CollisionStart, Position};
use crate::general::components::{Ball, Health, HittableTarget};
use crate::general::damage::ApplyDamage;
use crate::gore::components::DamageKind;

/// Damage a thrown ball deals to what it hits.
const BALL_DAMAGE: i32 = 10;

/// Turns ball-vs-target contacts into damage requests. Bounce accounting and the
/// one-score-per-throw rule live here; whether the hit actually hurts is `apply_damage`'s
/// call.
pub fn collision_handling_system(
    mut collision_event_reader: MessageReader<CollisionStart>,
    mut ball_query: Query<&mut Ball>,
    hittable_target_query: Query<(), (With<Health>, With<HittableTarget>)>,
    positions: Query<&Position>,
    mut commands: Commands,
    mut damage_mw: MessageWriter<ApplyDamage>,
) {
    for collision in collision_event_reader.read() {
        let entity1 = collision.collider1;
        let entity2 = collision.collider2;
        let (ball_entity, hittable_entity) = if ball_query.contains(entity1) {
            (entity1, entity2)
        } else if ball_query.contains(entity2) {
            (entity2, entity1)
        } else {
            continue;
        };

        let (ball_bounces, ball_thrower, ball_can_score) = {
            let Ok(mut ball) = ball_query.get_mut(ball_entity) else { continue };
            ball.bounces += 1;
            if ball.bounces >= ball.max_bounces {
                commands.entity(ball_entity).despawn();
            }
            (ball.bounces, ball.entity, ball.can_score)
        };

        let Some(thrower) = ball_thrower else { continue };

        if hittable_target_query.contains(hittable_entity) {
            if ball_can_score
                && let Ok(mut ball) = ball_query.get_mut(ball_entity)
            {
                ball.can_score = false;
            }
            if ball_bounces <= 2 {
                // Spray gore from the hit, roughly along the ball's travel.
                let target_pos = positions.get(hittable_entity).map_or(Vec3::ZERO, |p| p.0);
                let ball_pos = positions.get(ball_entity).map_or(target_pos, |p| p.0);
                damage_mw.write(
                    ApplyDamage::at(hittable_entity, BALL_DAMAGE, DamageKind::Ballistic, target_pos + Vec3::Y * 0.4)
                        .from(thrower)
                        .along(target_pos - ball_pos),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::collision_handling_system;
    use avian3d::prelude::CollisionStart;
    use bevy::prelude::*;
    use crate::alien::components::general::{Alien, AlienCounter};
    use crate::game_state::score_keeper::GameTrackingEvent;
    use crate::general::components::{Ball, Health, HittableTarget};
    use crate::general::damage::{apply_damage, ApplyDamage, DamageRules};
    use crate::gore::components::DamageDealt;

    #[derive(Resource)]
    struct Collision(Entity, Entity);

    fn fire_collision(pair: Res<Collision>, mut w: MessageWriter<CollisionStart>) {
        w.write(CollisionStart {
            collider1: pair.0,
            collider2: pair.1,
            body1: None,
            body2: None,
        });
    }

    #[derive(Resource, Default)]
    struct Caught(Vec<DamageDealt>);

    fn catch(mut r: MessageReader<DamageDealt>, mut c: ResMut<Caught>) {
        for d in r.read() {
            c.0.push(*d);
        }
    }

    #[test]
    fn ball_hitting_an_alien_damages_it_and_counts_the_kill() {
        let mut app = App::new();
        app.add_message::<CollisionStart>();
        app.add_message::<GameTrackingEvent>();
        app.add_message::<DamageDealt>();
        app.add_message::<ApplyDamage>();
        app.init_resource::<DamageRules>();
        app.insert_resource(AlienCounter { count: 1, max_count: 10 });
        app.init_resource::<Caught>();
        app.add_systems(Update, (fire_collision, collision_handling_system, apply_damage, catch).chain());

        let thrower = app.world_mut().spawn_empty().id();
        let ball = app.world_mut().spawn(Ball::new(thrower)).id();
        let alien = app
            .world_mut()
            .spawn((Health { health: 10, max_health: 10 }, HittableTarget {}, Alien))
            .id();
        app.insert_resource(Collision(ball, alien));

        app.update();

        let health = app.world().get::<Health>(alien).expect("alien still exists");
        assert!(health.health <= 0, "10 hp minus BALL_DAMAGE (10) leaves it dead");

        let caught = &app.world().resource::<Caught>().0;
        assert_eq!(caught.len(), 1, "one DamageDealt emitted");
        assert_eq!(caught[0].target, alien);
        assert!(caught[0].lethal, "the killing blow is flagged lethal");

        assert_eq!(
            app.world().resource::<AlienCounter>().count,
            0,
            "killing the alien decrements the live counter"
        );
    }

    #[test]
    fn a_glancing_ball_only_wounds() {
        let mut app = App::new();
        app.add_message::<CollisionStart>();
        app.add_message::<GameTrackingEvent>();
        app.add_message::<DamageDealt>();
        app.add_message::<ApplyDamage>();
        app.init_resource::<DamageRules>();
        app.insert_resource(AlienCounter { count: 1, max_count: 10 });
        app.init_resource::<Caught>();
        app.add_systems(Update, (fire_collision, collision_handling_system, apply_damage, catch).chain());

        let thrower = app.world_mut().spawn_empty().id();
        let ball = app.world_mut().spawn(Ball::new(thrower)).id();
        let alien = app
            .world_mut()
            .spawn((Health { health: 100, max_health: 100 }, HittableTarget {}, Alien))
            .id();
        app.insert_resource(Collision(ball, alien));

        app.update();

        assert_eq!(app.world().get::<Health>(alien).unwrap().health, 90);
        assert!(!app.world().resource::<Caught>().0[0].lethal);
        assert_eq!(app.world().resource::<AlienCounter>().count, 1, "no kill, no decrement");
    }
}
