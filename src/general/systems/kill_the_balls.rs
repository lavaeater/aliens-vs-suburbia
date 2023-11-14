use bevy::prelude::{Commands, DespawnRecursiveExt, EventReader, Query};
use bevy_xpbd_3d::prelude::Collision;
use crate::general::components::{Ball, Health, HittableTarget};

pub fn collision_handling_system(
    mut collision_event_reader: EventReader<Collision>,
    mut ball_query: Query<&mut Ball>,
    mut hittable_target_query: Query<(&mut Health, &HittableTarget)>,
    mut commands: Commands,
) {
    for Collision(contacts) in collision_event_reader.read() {
        if ball_query.contains(contacts.entity1) || ball_query.contains(contacts.entity2) {
            // we have a ball up in this!
            let mut ball_is_first = true;
            if let Ok(mut ball) = ball_query.get_mut(contacts.entity1) {
                ball.bounces += 1;
                if ball.bounces >= ball.bounce_max {
                    commands.entity(contacts.entity1).despawn_recursive();
                }
            }

            if let Ok(mut ball) = ball_query.get_mut(contacts.entity2) {
                ball_is_first = false;
                ball.bounces += 1;
                if ball.bounces >= ball.bounce_max {
                    commands.entity(contacts.entity2).despawn_recursive();
                }
            }

            let hittable_entity = if ball_is_first { contacts.entity2 } else { contacts.entity1 };
            if let Ok((mut alien_health, _)) = hittable_target_query.get_mut(hittable_entity) {
                let despawn_the_ball = if ball_is_first { contacts.entity1 } else { contacts.entity2 };
                commands.entity(despawn_the_ball).despawn_recursive();
                alien_health.health -= 10;
                if alien_health.health <= 0 {
                    commands.entity(hittable_entity).despawn_recursive();
                }
            }
        }
    }
}