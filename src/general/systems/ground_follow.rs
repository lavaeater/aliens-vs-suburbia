//! Walking on hills. Characters don't stand on the terrain collider: they would climb it
//! corner by corner at 20 Hz and leave the ground over every crest. Instead they ignore it,
//! have no gravity, and each physics step their vertical speed is set to carry them onto the
//! ground where that step will end. Sideways, physics is as on a flat map: colliders are tall
//! enough to always meet, so walls and other characters block as before.

use crate::physics::prelude::{
    Collider, CollisionLayers, GravityScale, LinearVelocity, Position, local_size,
};
use bevy::prelude::*;

use crate::alien::components::general::Alien;
use crate::general::components::CollisionLayer;
use crate::general::systems::map_systems::TileDefinitions;
use crate::map::ground::GroundHeight;
use crate::player::components::Player;

/// How tall a character's collider is on hills, either way around its origin. Shots are level
/// rays, so this is how far up or down a slope they can still hit it.
const TALL_COLLIDER: f32 = 4.0;

/// Walks on the ground, `standing` above it.
#[derive(Component, Debug, Clone, Copy)]
pub struct FollowsGround {
    standing: f32,
}

/// The vertical speed that puts a body at `target` after `dt`.
fn climb(y: f32, target: f32, dt: f32) -> f32 {
    (target - y) / dt
}

/// Set characters up for hills as they spawn, and put them on the ground.
#[allow(clippy::type_complexity)]
pub fn adopt_walkers(
    mut commands: Commands,
    ground: Res<GroundHeight>,
    tiles: Res<TileDefinitions>,
    mut walkers: Query<
        (
            Entity,
            &Collider,
            &CollisionLayers,
            &Transform,
            Option<&mut Position>,
        ),
        (Or<(With<Player>, With<Alien>)>, Without<FollowsGround>),
    >,
) {
    for (entity, collider, layers, transform, position) in &mut walkers {
        // The unscaled shape: a transform's scale applies to the new collider as well.
        let size = local_size(collider);
        let scale = transform.scale;
        let standing = size.y * scale.y / 2.0;
        let (x, z) = (transform.translation.x, transform.translation.z);
        let y = tiles.floor_level + ground.height(x, z) + standing;

        let mut layers = *layers;
        layers.filters.remove(CollisionLayer::Floor);
        commands.entity(entity).insert((
            FollowsGround { standing },
            GravityScale(0.0),
            layers,
            Collider::cuboid(size.x, TALL_COLLIDER * 2.0 / scale.y, size.z),
            Transform::from_translation(transform.translation.with_y(y))
                .with_rotation(transform.rotation)
                .with_scale(scale),
        ));
        if let Some(mut position) = position {
            position.0.y = y;
        }
    }
}

/// Carry characters along the ground, in the fixed step just before physics moves them.
pub fn follow_ground(
    time: Res<Time>,
    ground: Res<GroundHeight>,
    tiles: Res<TileDefinitions>,
    mut walkers: Query<(&Position, &mut LinearVelocity, &FollowsGround)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (position, mut velocity, walker) in &mut walkers {
        let next = position.0 + velocity.0 * dt;
        let target = tiles.floor_level + ground.height(next.x, next.z) + walker.standing;
        velocity.y = climb(position.0.y, target, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_ends_on_the_target() {
        let dt = 0.05;
        for (y, target) in [(1.0, 1.25), (2.0, 1.5), (0.5, 0.5)] {
            let end: f32 = y + climb(y, target, dt) * dt;
            assert!((end - target).abs() < 1e-5, "{y} to {target}: {end}");
        }
    }
}
