use bevy::math::Vec3;
use bevy::prelude::{Commands, Has, MessageReader, MessageWriter, Query, ResMut};
use avian3d::prelude::{CollisionStart, Position};
use crate::alien::components::general::{Alien, AlienCounter};
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{Ball, Health, HittableTarget};
use crate::gore::components::{DamageDealt, DamageKind};

/// Damage a thrown ball deals to what it hits.
const BALL_DAMAGE: i32 = 10;

pub fn collision_handling_system(
    mut alien_counter: ResMut<AlienCounter>,
    mut collision_event_reader: MessageReader<CollisionStart>,
    mut ball_query: Query<&mut Ball>,
    mut hittable_target_query: Query<(&mut Health, &HittableTarget, Has<Alien>)>,
    positions: Query<&Position>,
    mut commands: Commands,
    mut game_mw: MessageWriter<GameTrackingEvent>,
    mut damage_mw: MessageWriter<DamageDealt>,
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

        let (ball_bounces, ball_hit_entity, ball_can_score) = {
            let Ok(mut ball) = ball_query.get_mut(ball_entity) else { continue };
            ball.bounces += 1;
            if ball.bounces >= ball.max_bounces {
                commands.entity(ball_entity).despawn();
            }
            (ball.bounces, ball.entity, ball.can_score)
        };

        let Some(hit_entity) = ball_hit_entity else { continue };

        if let Ok((mut target_health, _, is_alien)) = hittable_target_query.get_mut(hittable_entity) {
            if ball_can_score {
                if let Ok(mut ball) = ball_query.get_mut(ball_entity) {
                    ball.can_score = false;
                }
                game_mw.write(GameTrackingEvent::ShotHit(hit_entity));
            }
            if ball_bounces <= 2 {
                target_health.health -= BALL_DAMAGE;
                let lethal = target_health.health <= 0;

                // Spray gore from the hit, roughly along the ball's travel.
                let target_pos = positions.get(hittable_entity).map(|p| p.0).unwrap_or(Vec3::ZERO);
                let ball_pos = positions.get(ball_entity).map(|p| p.0).unwrap_or(target_pos);
                damage_mw.write(DamageDealt {
                    target: hittable_entity,
                    position: target_pos + Vec3::Y * 0.4,
                    normal: (target_pos - ball_pos).normalize_or(Vec3::Y),
                    amount: BALL_DAMAGE,
                    kind: DamageKind::Ballistic,
                    lethal,
                });

                if lethal && is_alien {
                    game_mw.write(GameTrackingEvent::AlienKilled(hit_entity));
                    alien_counter.count -= 1;
                }
            }
        }
    }
}
