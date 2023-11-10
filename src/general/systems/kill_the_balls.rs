use bevy::prelude::{Commands, DespawnRecursiveExt, EventReader, Query};
use bevy_xpbd_3d::prelude::Collision;
use crate::general::components::{Ball, Floor, HittableTarget, Wall};

pub fn kill_the_balls(
    mut collision_event_reader: EventReader<Collision>,
    ball_query: Query<&Ball>,
    hittable_target_query: Query<&HittableTarget>,
    mut commands: Commands
) {
    for Collision(contacts) in collision_event_reader.read() {
        if ball_query.contains(contacts.entity1) && (hittable_target_query.contains(contacts.entity2)) {
            commands.entity(contacts.entity1).despawn_recursive();
        }
        if ball_query.contains(contacts.entity2) && (hittable_target_query.contains(contacts.entity1)) {
            commands.entity(contacts.entity2).despawn_recursive();
        }
    }
}