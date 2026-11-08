//! The Avian backend: the game's physics as it has always been.

use bevy::gizmos::AppGizmoBuilder;
use bevy::gizmos::config::GizmoConfig;
use bevy::prelude::*;

pub use avian3d::interpolation::TransformInterpolation;
pub use avian3d::prelude::{
    AngularDamping, AngularVelocity, Collider, CollidingEntities, CollisionEventsEnabled,
    CollisionLayers, CollisionStart, Friction, GravityScale, LayerMask, LinearDamping,
    LinearVelocity, LockedAxes, PhysicsGizmos, PhysicsLayer, Position, RigidBody, Rotation,
    Sensor, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
};
use avian3d::prelude::{PhysicsDebugPlugin, PhysicsPlugins, PhysicsSystems};

use super::PhysicsStep;

/// The size of a collider's own bounding box, unscaled and unrotated.
pub fn local_size(collider: &Collider) -> Vec3 {
    collider.shape().compute_local_aabb().half_extents() * 2.0
}

pub(super) fn build(app: &mut App) {
    app.add_plugins((PhysicsPlugins::default(), PhysicsDebugPlugin))
        .configure_sets(
            FixedPostUpdate,
            (
                PhysicsSystems::First,
                PhysicsSystems::Prepare,
                PhysicsSystems::StepSimulation,
                PhysicsSystems::Writeback,
                PhysicsSystems::Last,
            )
                .in_set(PhysicsStep),
        )
        .insert_gizmo_config(
            PhysicsGizmos::default(),
            GizmoConfig {
                enabled: false,
                ..Default::default()
            },
        );
}
