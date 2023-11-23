use bevy::prelude::{Commands, DespawnRecursiveExt, EventReader, Has, Query, ResMut};
use bevy_xpbd_3d::prelude::Collision;
use crate::alien::components::general::{Alien, AlienCounter};
use crate::general::components::{Ball, Health, HittableTarget};

pub fn collision_handling_system(
    mut alien_counter: ResMut<AlienCounter>,
    mut collision_event_reader: EventReader<Collision>,
    mut ball_query: Query<&mut Ball>,
    mut hittable_target_query: Query<(&mut Health, &HittableTarget, Has<Alien>)>,
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
            if let Ok((mut target_health, _, is_alien)) = hittable_target_query.get_mut(hittable_entity) {
                let ball = (if ball_is_first { ball_query.get( contacts.entity1) } else { ball_query.get(contacts.entity2) }).unwrap();
                if ball.bounces <= 2 {
                    target_health.health -= 10;
                    if target_health.health <= 0 && is_alien {
                        //NO need to despawn here, it is done in health_monitor_system
                        alien_counter.count -= 1;
                    }
                }
            }
        }
    }
}