//! The Jolt world as a resource, and bodies made from an entity's components.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use oxijolt::{BodyId, BodySettings, EventSettings, PhysicsWorld, WorldSettings};

use super::components::*;
use super::layers::{CollisionLayers, LayerTable};

/// Avian's default gravity.
const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);
/// Sized well past a busy wave (the spike ran out of the default 10240 contact constraints at
/// about 1000 crowding aliens).
const MAX_BODIES: u32 = 16_384;
const MAX_BODY_PAIRS: u32 = 65_536;
const MAX_CONTACT_CONSTRAINTS: u32 = 32_768;
const TEMP_ALLOCATOR: u32 = 32 << 20;

/// The entity's Jolt body.
#[derive(Component, Clone, Copy, Debug)]
pub struct JoltBody(pub BodyId);

#[derive(Resource)]
pub struct JoltWorld {
    pub(super) world: PhysicsWorld,
    pub(super) layers: LayerTable,
    pub(super) entities: HashMap<BodyId, Entity>,
}

impl JoltWorld {
    pub fn new() -> Self {
        let layers = LayerTable::default();
        Self { world: make_world(&layers), layers, entities: HashMap::default() }
    }

    pub fn entity(&self, body: BodyId) -> Option<Entity> {
        self.entities.get(&body).copied()
    }

    /// A fresh world with every combination learnt so far; the caller makes the bodies again.
    pub(super) fn rebuild(&mut self) {
        self.world = make_world(&self.layers);
        self.entities.clear();
    }
}

impl Default for JoltWorld {
    fn default() -> Self {
        Self::new()
    }
}

fn make_world(layers: &LayerTable) -> PhysicsWorld {
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get() as u32);
    // Leave the main thread and one more to Bevy.
    let workers = workers.saturating_sub(2).clamp(1, 8);
    let settings = WorldSettings::default()
        .layers(layers.to_jolt())
        .gravity(GRAVITY.into())
        .worker_threads(workers)
        .max_bodies(MAX_BODIES)
        .max_body_pairs(MAX_BODY_PAIRS)
        .max_contact_constraints(MAX_CONTACT_CONSTRAINTS)
        .temp_allocator_size(TEMP_ALLOCATOR);
    // The settings are constants inside Jolt's limits; a refusal is a bug to see at once.
    #[allow(clippy::expect_used)]
    let mut world = PhysicsWorld::new(settings).expect("a Jolt world with sane settings");
    world.set_event_settings(EventSettings::default().contacts(true));
    world
}

/// Everything a body is made from.
pub(super) struct BodyParts<'a> {
    pub collider: &'a Collider,
    pub rigid_body: Option<RigidBody>,
    pub position: Vec3,
    pub rotation: Quat,
    pub layers: CollisionLayers,
    pub sensor: bool,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub locked: Option<LockedAxes>,
    pub friction: Option<Friction>,
    pub linear_damping: Option<LinearDamping>,
    pub angular_damping: Option<AngularDamping>,
    pub gravity: Option<GravityScale>,
}

impl BodyParts<'_> {
    /// The motion Jolt gets. A sensor that wouldn't move is made kinematic: a static Jolt sensor
    /// only sees awake bodies, a kinematic one sees sleeping ones too, which is what Avian does.
    pub fn motion(&self) -> RigidBody {
        match (self.rigid_body.unwrap_or(RigidBody::Static), self.sensor) {
            (RigidBody::Static, true) => RigidBody::Kinematic,
            (motion, _) => motion,
        }
    }

    pub fn moving(&self) -> bool {
        !self.motion().is_static()
    }
}

/// Makes the body; the layer combination must already be in the table.
pub(super) fn create_body(jolt: &mut JoltWorld, entity: Entity, parts: &BodyParts) -> Option<BodyId> {
    let layer = jolt.layers.get(parts.layers, parts.moving())?;
    let mut settings = match parts.motion() {
        RigidBody::Static => BodySettings::new_static(),
        RigidBody::Kinematic => BodySettings::new_kinematic(),
        RigidBody::Dynamic => BodySettings::new_dynamic(),
    }
    .position(parts.position.into())
    .rotation(parts.rotation.normalize().into())
    .object_layer(layer)
    .sensor(parts.sensor)
    .user_data(entity.to_bits())
    .friction(parts.friction.unwrap_or_default().dynamic_coefficient)
    .linear_damping(parts.linear_damping.map_or(0.0, |d| d.0))
    .angular_damping(parts.angular_damping.map_or(0.0, |d| d.0));
    if !parts.motion().is_static() {
        settings = settings
            .linear_velocity(parts.linear_velocity.into())
            .angular_velocity(parts.angular_velocity.into());
    }
    if parts.motion().is_dynamic() {
        settings = settings.gravity_factor(parts.gravity.map_or(1.0, |g| g.0));
        if let Some(locked) = parts.locked {
            settings = settings.allowed_dofs(locked.allowed());
        }
    }
    match jolt.world.create_body(parts.collider.shape(), &settings) {
        Ok(id) => {
            jolt.entities.insert(id, entity);
            Some(id)
        }
        Err(e) => {
            warn!("Jolt refused the body of {entity}: {e}");
            None
        }
    }
}
