//! The physics backend, behind one prelude. Game code imports `crate::physics::prelude` and never
//! `avian3d` directly, so the engine underneath can change (Jolt through oxijolt, see
//! `docs/jolt-spike.md` and `docs/jolt-port.md`) without touching the call sites.
//!
//! The prelude lists every name the game uses, one by one: it is the contract a second backend
//! has to meet. A name missing here is a name the game doesn't use yet; add it to every backend.

mod avian;

pub mod prelude {
    pub use super::avian::{
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
        avian::build(app);
    }
}
