use bevy::math::Vec3;
use bevy::prelude::Commands;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody};

pub fn spawn_map(
    mut commands: Commands,
) {
    commands.spawn(
        (
            RigidBody::Static,
            Collider::cuboid(50.0, 0.1, 50.0),
            Position::from(Vec3::new(4.0, -1.0, -4.0)),
        )
    );
}