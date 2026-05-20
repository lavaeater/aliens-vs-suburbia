use bevy::prelude::{Commands, Has, MessageReader, MessageWriter, Query, ResMut};
use avian3d::prelude::CollisionStart;
use crate::alien::components::general::{Alien, AlienCounter};
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{Ball, Health, HittableTarget};

pub fn collision_handling_system(
    mut alien_counter: ResMut<AlienCounter>,
    mut collision_event_reader: MessageReader<CollisionStart>,
    mut ball_query: Query<&mut Ball>,
    mut hittable_target_query: Query<(&mut Health, &HittableTarget, Has<Alien>)>,
    mut commands: Commands,
    mut game_mw: MessageWriter<GameTrackingEvent>,
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
                target_health.health -= 10;
                if target_health.health <= 0 && is_alien {
                    game_mw.write(GameTrackingEvent::AlienKilled(hit_entity));
                    alien_counter.count -= 1;
                }
            }
        }
    }
}
