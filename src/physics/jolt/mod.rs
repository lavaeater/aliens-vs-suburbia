//! The Jolt backend (oxijolt), behind feature `physics-jolt`: Avian's names over a Jolt world.
//! Plan and open ends: `docs/jolt-port.md`.
//!
//! The types carry Avian's API whole (constructors, constants, builder methods) even where the
//! game doesn't call a part yet, so new game code written against Avian keeps compiling here.
#![allow(dead_code)]

mod components;
mod debug;
mod interpolation;
mod layers;
mod query;
mod sync;
mod world;

#[cfg(test)]
mod tests;

use bevy::app::RunFixedMainLoopSystems;
use bevy::gizmos::AppGizmoBuilder;
use bevy::gizmos::config::GizmoConfig;
use bevy::prelude::*;

pub use components::{
    AngularDamping, AngularVelocity, Collider, CollidingEntities,
    CollisionEventsEnabled, CollisionStart, Friction, GravityScale, LinearDamping, LinearVelocity,
    LockedAxes, Position, RigidBody, Rotation, Sensor, TransformInterpolation,
};
pub use debug::PhysicsGizmos;
pub use layers::{CollisionLayers, LayerMask, PhysicsLayer};
pub use query::{ShapeCastConfig, SpatialQuery, SpatialQueryFilter};
pub use world::JoltWorld;

/// The step and its sync, in order, in `FixedPostUpdate`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct JoltSystems;

pub(super) fn build(app: &mut App) {
    build_core(app);
    app.add_systems(PostUpdate, debug::draw_colliders).insert_gizmo_config(
        PhysicsGizmos,
        GizmoConfig {
            enabled: false,
            ..Default::default()
        },
    );
}

/// Everything but the debug drawing, which needs Bevy's gizmos (the tests run without them).
fn build_core(app: &mut App) {
    app.init_resource::<JoltWorld>()
        .init_resource::<sync::Removed>()
        .init_resource::<sync::Touching>()
        .add_message::<CollisionStart>()
        .add_observer(sync::remove_body)
        .add_systems(
            FixedPostUpdate,
            (
                sync::create_bodies,
                sync::push_changes,
                sync::step,
                sync::contacts,
                sync::pull_poses,
            )
                .chain()
                .in_set(JoltSystems),
        )
        .add_systems(FixedFirst, interpolation::restore)
        .add_systems(FixedLast, interpolation::capture)
        .add_systems(
            RunFixedMainLoop,
            interpolation::ease.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        );
}
