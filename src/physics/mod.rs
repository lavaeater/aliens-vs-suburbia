//! The physics backend, behind one prelude. Game code imports `crate::physics::prelude` and never
//! `avian3d` directly, so the engine underneath can change without touching the call sites:
//! Avian by default, Jolt (oxijolt) with `--features physics-jolt` (`docs/jolt-port.md`).
//!
//! The prelude lists every name the game uses, one by one: it is the contract a backend has to
//! meet. A name missing here is a name the game doesn't use yet; add it to every backend.

#[cfg(not(feature = "physics-jolt"))]
mod avian;
#[cfg(not(feature = "physics-jolt"))]
use avian as backend;

#[cfg(feature = "physics-jolt")]
pub mod jolt;
#[cfg(feature = "physics-jolt")]
use jolt as backend;

pub mod prelude {
    pub use super::backend::{
        AngularDamping, AngularVelocity, Collider, CollidingEntities, CollisionEventsEnabled,
        CollisionLayers, CollisionStart, Friction, GravityScale, LayerMask, LinearDamping,
        LinearVelocity, LockedAxes, PhysicsGizmos, PhysicsLayer, Position, RigidBody, Rotation,
        Sensor, ShapeCastConfig, SpatialQuery, SpatialQueryFilter, TransformInterpolation,
    };
    pub use super::GamePhysicsPlugin;
}

/// The backend's plugins, its debug drawing (off until F3 / the editor turns it on) and its
/// gizmo config.
pub struct GamePhysicsPlugin;

impl bevy::prelude::Plugin for GamePhysicsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        backend::build(app);
    }
}
