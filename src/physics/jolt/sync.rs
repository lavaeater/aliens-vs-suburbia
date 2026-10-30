//! The step and everything around it, in `FixedPostUpdate` (where Avian steps):
//! make new bodies -> push the game's writes -> step -> contacts -> pull the poses back.

use bevy::platform::collections::HashSet;
use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use oxijolt::{Activation, BodyPose, ContactEvent};

use super::components::*;
use super::layers::CollisionLayers;
use super::world::{BodyParts, JoltBody, JoltWorld, create_body};

/// Bodies removed since the last step, cleared from every `CollidingEntities` after it.
#[derive(Resource, Default)]
pub(super) struct Removed(Vec<Entity>);

#[derive(QueryData)]
#[query_data(mutable)]
pub(super) struct BodyData {
    entity: Entity,
    collider: &'static Collider,
    rigid_body: Option<&'static RigidBody>,
    position: &'static mut Position,
    rotation: &'static mut Rotation,
    layers: Option<&'static CollisionLayers>,
    sensor: Has<Sensor>,
    linear_velocity: Option<&'static LinearVelocity>,
    angular_velocity: Option<&'static AngularVelocity>,
    locked: Option<&'static LockedAxes>,
    friction: Option<&'static Friction>,
    linear_damping: Option<&'static LinearDamping>,
    angular_damping: Option<&'static AngularDamping>,
    gravity: Option<&'static GravityScale>,
}

fn parts<'a>(b: &'a BodyDataItem<'_, '_>) -> BodyParts<'a> {
    BodyParts {
        collider: b.collider,
        rigid_body: b.rigid_body.copied(),
        position: b.position.0,
        rotation: b.rotation.0,
        layers: b.layers.copied().unwrap_or_default(),
        sensor: b.sensor,
        linear_velocity: b.linear_velocity.map_or(Vec3::ZERO, |v| v.0),
        angular_velocity: b.angular_velocity.map_or(Vec3::ZERO, |v| v.0),
        locked: b.locked.copied(),
        friction: b.friction.copied(),
        linear_damping: b.linear_damping.copied(),
        angular_damping: b.angular_damping.copied(),
        gravity: b.gravity.copied(),
    }
}

/// Bodies for new colliders. A `Position` still at its default is taken from the entity's
/// transform (Avian's rule), so spawning with only a `Transform` works. A layer combination the
/// world doesn't know yet rebuilds it, every body included.
pub(super) fn create_bodies(
    mut commands: Commands,
    mut jolt: ResMut<JoltWorld>,
    mut touching: ResMut<Touching>,
    fresh: Query<Entity, (With<Collider>, Without<JoltBody>)>,
    mut bodies: Query<BodyData>,
    existing: Query<Entity, With<JoltBody>>,
    helper: TransformHelper,
) {
    if fresh.is_empty() {
        return;
    }
    let mut rebuild = false;
    for entity in &fresh {
        let Ok(mut body) = bodies.get_mut(entity) else { continue };
        if body.position.0 == Vec3::ZERO
            && body.rotation.0 == Quat::IDENTITY
            && let Ok(global) = helper.compute_global_transform(entity)
        {
            let (_, rotation, translation) = global.to_scale_rotation_translation();
            body.position.0 = translation;
            body.rotation.0 = rotation;
        }
        let parts = parts(&body);
        rebuild |= jolt.layers.learn(parts.layers, parts.moving());
    }
    if rebuild {
        debug!("Jolt: new collision layer combination, rebuilding the world");
        jolt.rebuild();
        touching.after_rebuild();
        for entity in &existing {
            let Ok(body) = bodies.get_mut(entity) else { continue };
            if let Some(id) = create_body(&mut jolt, entity, &parts(&body)) {
                commands.entity(entity).insert(JoltBody(id));
            }
        }
    }
    for entity in &fresh {
        let Ok(body) = bodies.get_mut(entity) else { continue };
        if let Some(id) = create_body(&mut jolt, entity, &parts(&body)) {
            commands.entity(entity).insert(JoltBody(id));
        }
    }
    if rebuild || fresh.iter().len() > 32 {
        jolt.world.optimize_broad_phase();
    }
}

/// A collider removed (or its entity despawned) takes its body with it.
pub(super) fn remove_body(
    remove: On<Remove, Collider>,
    bodies: Query<&JoltBody>,
    mut jolt: ResMut<JoltWorld>,
    mut removed: ResMut<Removed>,
    mut commands: Commands,
) {
    let Ok(&JoltBody(id)) = bodies.get(remove.entity) else { return };
    if jolt.entities.remove(&id).is_some() {
        let _ = jolt.world.remove_body(id);
    }
    removed.0.push(remove.entity);
    commands.entity(remove.entity).try_remove::<JoltBody>();
}

type Moved = (
    &'static JoltBody,
    Ref<'static, Position>,
    Ref<'static, Rotation>,
    Option<&'static mut Transform>,
    Has<ChildOf>,
);
type Pushed = (
    &'static JoltBody,
    Option<Ref<'static, LinearVelocity>>,
    Option<Ref<'static, AngularVelocity>>,
);
type MovedFilter = Or<(Changed<Position>, Changed<Rotation>)>;
type PushedFilter = Or<(Changed<LinearVelocity>, Changed<AngularVelocity>)>;
type Pulled = (
    &'static mut Position,
    &'static mut Rotation,
    Option<&'static mut LinearVelocity>,
    Option<&'static mut AngularVelocity>,
    Option<&'static mut Transform>,
    Has<ChildOf>,
);

/// What the game wrote since the last step: a moved `Position`/`Rotation` teleports the body
/// (and its `Transform`), a changed velocity is set. The pull writes without change detection,
/// so anything that shows up as changed here came from the game.
pub(super) fn push_changes(
    mut jolt: ResMut<JoltWorld>,
    mut moved: Query<Moved, MovedFilter>,
    pushed: Query<Pushed, PushedFilter>,
) {
    for (&JoltBody(id), position, rotation, transform, parented) in &mut moved {
        if position.is_added() {
            continue;
        }
        let Ok(mut body) = jolt.world.body_mut(id) else { continue };
        let _ = body.set_position_and_rotation(
            position.0.into(),
            rotation.0.normalize().into(),
            Activation::Activate,
        );
        if let (Some(mut t), false) = (transform, parented) {
            t.translation = position.0;
            t.rotation = rotation.0;
        }
    }
    for (&JoltBody(id), linear, angular) in &pushed {
        let Ok(mut body) = jolt.world.body_mut(id) else { continue };
        if body.motion_type() == oxijolt::MotionType::Static {
            continue;
        }
        if let Some(v) = linear.filter(|v| v.is_changed() && !v.is_added()) {
            let _ = body.set_linear_velocity(v.0.into());
        }
        if let Some(w) = angular.filter(|w| w.is_changed() && !w.is_added()) {
            let _ = body.set_angular_velocity(w.0.into());
        }
    }
}

pub(super) fn step(mut jolt: ResMut<JoltWorld>, time: Res<Time>, mut warned: Local<bool>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    match jolt.world.step(dt) {
        Ok(report) if !report.is_complete() && !*warned => {
            warn!("Jolt dropped contacts this step, raise the limit it names: {report:?}");
            *warned = true;
        }
        Ok(_) => {}
        Err(e) => error!("Jolt refused the step: {e}"),
    }
}

/// Which colliders touch, kept across steps. Jolt reports a contact as removed when its bodies
/// fall asleep, and Avian doesn't, so such a pair is kept as `unsure` until one of its bodies
/// wakes and Jolt can say whether they still touch (a rebuilt world makes every pair unsure).
#[derive(Resource, Default)]
pub(super) struct Touching {
    pairs: HashSet<(Entity, Entity)>,
    unsure: HashSet<(Entity, Entity)>,
}

impl Touching {
    pub(super) fn after_rebuild(&mut self) {
        self.unsure.extend(self.pairs.iter().copied());
    }
}

fn key(a: Entity, b: Entity) -> (Entity, Entity) {
    if a < b { (a, b) } else { (b, a) }
}

fn touch(colliding: &mut Query<&mut CollidingEntities>, (a, b): (Entity, Entity), on: bool) {
    for (this, other) in [(a, b), (b, a)] {
        if let Ok(mut set) = colliding.get_mut(this) {
            if on {
                set.0.insert(other);
            } else {
                set.0.remove(&other);
            }
        }
    }
}

/// New and ended contacts -> `CollidingEntities` and `CollisionStart` (once per pair, when it
/// starts touching).
#[allow(clippy::too_many_arguments)]
pub(super) fn contacts(
    mut jolt: ResMut<JoltWorld>,
    mut removed: ResMut<Removed>,
    mut touching: ResMut<Touching>,
    mut colliding: Query<&mut CollidingEntities>,
    bodies: Query<&JoltBody>,
    events_enabled: Query<(), With<CollisionEventsEnabled>>,
    rigid_bodies: Query<(), With<RigidBody>>,
    mut starts: MessageWriter<CollisionStart>,
) {
    let events = jolt.world.take_events();
    for event in events.contacts {
        let pair = event.pair();
        let (Some(a), Some(b)) = (jolt.entity(pair.body1), jolt.entity(pair.body2)) else { continue };
        let k = key(a, b);
        match event {
            ContactEvent::Added { .. } => {
                touching.unsure.remove(&k);
                if !touching.pairs.insert(k) {
                    continue;
                }
                touch(&mut colliding, k, true);
                if events_enabled.contains(a) || events_enabled.contains(b) {
                    let body = |e: Entity| rigid_bodies.contains(e).then_some(e);
                    starts.write(CollisionStart {
                        collider1: a,
                        collider2: b,
                        body1: body(a),
                        body2: body(b),
                    });
                }
            }
            ContactEvent::Removed(_) => {
                let asleep = |id| jolt.world.body(id).is_ok_and(|b| !b.is_active());
                if asleep(pair.body1) && asleep(pair.body2) {
                    touching.unsure.insert(k);
                } else if touching.pairs.remove(&k) {
                    touch(&mut colliding, k, false);
                }
            }
            ContactEvent::Persisted { .. } => {}
        }
    }

    let unsure: Vec<_> = touching.unsure.iter().copied().collect();
    for k in unsure {
        let still = match bodies.get(k.0).ok().zip(bodies.get(k.1).ok()) {
            None => Some(false),
            Some((a, b)) => {
                let awake = |id| jolt.world.body(id).is_ok_and(|b| b.is_active());
                (awake(a.0) || awake(b.0))
                    .then(|| jolt.world.were_bodies_in_contact(a.0, b.0).unwrap_or(false))
            }
        };
        match still {
            None => {}
            Some(true) => {
                touching.unsure.remove(&k);
            }
            Some(false) => {
                touching.unsure.remove(&k);
                if touching.pairs.remove(&k) {
                    touch(&mut colliding, k, false);
                }
            }
        }
    }

    if !removed.0.is_empty() {
        let gone: HashSet<Entity> = removed.0.drain(..).collect();
        touching.pairs.retain(|(a, b)| !gone.contains(a) && !gone.contains(b));
        touching.unsure.retain(|(a, b)| !gone.contains(a) && !gone.contains(b));
        for mut set in &mut colliding {
            set.0.retain(|e| !gone.contains(e));
        }
    }
}

/// The awake bodies' poses and velocities into `Position`, `Rotation`, the velocities and (for
/// unparented entities) `Transform`, without change detection.
pub(super) fn pull_poses(
    jolt: Res<JoltWorld>,
    mut poses: Local<Vec<BodyPose>>,
    mut bodies: Query<Pulled>,
) {
    jolt.world.active_body_poses_into(&mut poses);
    for pose in &*poses {
        let Some(entity) = jolt.entity(pose.id) else { continue };
        let Ok((mut position, mut rotation, linear, angular, transform, parented)) =
            bodies.get_mut(entity)
        else {
            continue;
        };
        let p: Vec3 = pose.position.into();
        let q: Quat = pose.rotation.into();
        position.bypass_change_detection().0 = p;
        rotation.bypass_change_detection().0 = q;
        if let Ok(body) = jolt.world.body(pose.id) {
            if let Some(mut v) = linear {
                v.bypass_change_detection().0 = body.linear_velocity().into();
            }
            if let Some(mut w) = angular {
                w.bypass_change_detection().0 = body.angular_velocity().into();
            }
        }
        if let (Some(mut t), false) = (transform, parented) {
            t.translation = p;
            t.rotation = q;
        }
    }
}
