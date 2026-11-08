//! Physics: Jolt (oxijolt) behind one prelude, `crate::physics::prelude`, which is all game code
//! imports. The names are Avian's, the engine the game started on (`docs/jolt-port.md`), so the
//! game's physics code reads as it always has.
//!
//! The prelude lists every name the game uses, one by one. A name missing here is a name the game
//! doesn't use yet; add it to `jolt` and here.

pub mod jolt;
use jolt as backend;

pub mod prelude {
    pub use super::backend::{
        AngularDamping, AngularVelocity, Collider, CollidingEntities, CollisionEventsEnabled,
        CollisionLayers, CollisionStart, Friction, GravityScale, LayerMask, LinearDamping,
        LinearVelocity, LockedAxes, PhysicsGizmos, PhysicsLayer, Position, RigidBody, Rotation,
        Sensor, ShapeCastConfig, SpatialQuery, SpatialQueryFilter, TransformInterpolation,
        local_size,
    };
    pub use super::{GamePhysicsPlugin, PhysicsStep};
}

/// Everything the backend runs for one fixed step, in `FixedPostUpdate`: what `--perf` times.
/// Interpolation and the game's own spatial queries are outside it.
#[derive(bevy::prelude::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhysicsStep;

/// The backend's plugins, its debug drawing (off until F3 / the editor turns it on) and its
/// gizmo config.
pub struct GamePhysicsPlugin;

impl bevy::prelude::Plugin for GamePhysicsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        backend::build(app);
    }
}
