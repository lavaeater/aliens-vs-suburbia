use bevy::math::Vec3;
use bevy::prelude::Commands;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody};
use crate::METERS_PER_PIXEL;

pub fn spawn_map(
    mut commands: Commands,
) {
    commands.spawn(
        (
            RigidBody::Static,
            Collider::cuboid(5.0, 1.0, 5.0),
            Position::from(Vec3::new(0.0, -1.0, 0.0)),
        )
    );
}