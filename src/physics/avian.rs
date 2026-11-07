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
use avian3d::prelude::{PhysicsDebugPlugin, PhysicsPlugins};

pub(super) fn build(app: &mut App) {
    app.add_plugins((PhysicsPlugins::default(), PhysicsDebugPlugin))
        .insert_gizmo_config(
            PhysicsGizmos::default(),
            GizmoConfig {
                enabled: false,
                ..Default::default()
            },
        );
}
