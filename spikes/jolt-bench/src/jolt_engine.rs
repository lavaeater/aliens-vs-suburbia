//! The scene on Jolt through oxijolt, with the smallest Bevy bridge that does the job: the world
//! is a resource, each entity holds its `BodyId`, the world steps in `FixedPostUpdate` (where
//! Avian steps) and awake bodies' poses are copied into `Transform` after it.
//!
//! What a real plugin would add on top (see `docs/jolt-spike.md`): spawning bodies from
//! components with hooks instead of by hand, removing a body on despawn, contact events as
//! messages, a `SystemParam` for queries, interpolation.

use std::collections::HashMap;
use std::time::Instant;

use bevy::prelude::*;
use oxijolt::prelude::*;
use oxijolt::{AllowedDofs, RayCast};

use crate::scene::*;

#[derive(Resource)]
pub struct Jolt {
    pub world: PhysicsWorld,
    poses: Vec<BodyPose>,
    layers: Layers,
}

#[derive(Clone, Copy)]
struct Layers {
    solid: ObjectLayer,
    alien: ObjectLayer,
    player: ObjectLayer,
    gib: ObjectLayer,
}

#[derive(Component, Clone, Copy)]
pub struct JoltBody(pub BodyId);

#[derive(Resource, Default)]
struct BodyEntities(HashMap<BodyId, Entity>);

#[derive(Component)]
struct Shooter(usize);

fn v(v: Vec3) -> oxijolt::Vec3 {
    v.into()
}

fn r(v: Vec3) -> oxijolt::RVec3 {
    v.into()
}

/// The game's `CollisionLayers` boil down to one Jolt object layer per (memberships, filters)
/// pair; this scene has four.
fn layers() -> (CollisionLayers, Layers) {
    let mut table = CollisionLayers::new(2);
    let solid = table.add_object_layer(BroadPhaseLayer::NON_MOVING);
    let alien = table.add_object_layer(BroadPhaseLayer::MOVING);
    let player = table.add_object_layer(BroadPhaseLayer::MOVING);
    let gib = table.add_object_layer(BroadPhaseLayer::MOVING);
    table
        .enable_collision(alien, solid)
        .enable_collision(alien, alien)
        .enable_collision(alien, player)
        .enable_collision(player, solid)
        .enable_collision(player, player)
        .enable_collision(gib, solid);
    (table, Layers { solid, alien, player, gib })
}

pub fn build(app: &mut App, workers: u32) {
    let (table, layers) = layers();
    let settings = WorldSettings::default()
        .layers(table)
        .worker_threads(workers)
        // The default 10240 runs out at about 1000 crowding aliens (`contact_constraints_full`).
        .max_contact_constraints(65536)
        .gravity(oxijolt::Vec3::new(0.0, -9.81, 0.0));
    let world = PhysicsWorld::new(settings).expect("the world");
    app.insert_resource(Jolt { world, poses: Vec::new(), layers })
        .init_resource::<BodyEntities>()
        .add_systems(Startup, spawn)
        .add_systems(FixedUpdate, (wall_rays_system, steer, shoot, gibs).chain())
        .add_systems(FixedPostUpdate, (step, sync_transforms).chain());
}

fn spawn(
    mut commands: Commands,
    scene: Res<Scene>,
    mut rng: ResMut<Rng>,
    mut jolt: ResMut<Jolt>,
    mut entities: ResMut<BodyEntities>,
) {
    let jolt = &mut *jolt;
    let l = jolt.layers;
    let floor = Shape::new_box(v(scene.floor.half)).expect("floor");
    let settings = BodySettings::new_static().position(r(scene.floor.center)).object_layer(l.solid);
    jolt.world.create_body(&floor, &settings).expect("floor body");
    for wall in &scene.walls {
        let shape = Shape::new_box(v(wall.half)).expect("wall");
        let settings = BodySettings::new_static().position(r(wall.center)).object_layer(l.solid);
        jolt.world.create_body(&shape, &settings).expect("wall body");
    }
    jolt.world.optimize_broad_phase();

    // Avian's `capsule(radius, length)` is Jolt's `capsule(length / 2, radius)`.
    let capsule = Shape::new_capsule(ALIEN_LENGTH / 2.0, ALIEN_RADIUS).expect("capsule");
    let upright = AllowedDofs::TRANSLATION_X
        | AllowedDofs::TRANSLATION_Y
        | AllowedDofs::TRANSLATION_Z
        | AllowedDofs::ROTATION_Y;
    for index in 0..scene.aliens {
        let at = scene.spawn_of(index, &mut rng);
        let settings = BodySettings::new_dynamic()
            .position(r(at))
            .allowed_dofs(upright)
            .allow_sleeping(false)
            .object_layer(l.alien);
        let id = jolt.world.create_body(&capsule, &settings).expect("alien");
        let e = commands
            .spawn((Alien { index, forward: Vec3::Z, wall_ahead: None }, JoltBody(id), Transform::from_translation(at)))
            .id();
        entities.0.insert(id, e);
    }
    for (p, &at) in scene.players.iter().enumerate() {
        let settings = BodySettings::new_dynamic()
            .position(r(at))
            .allowed_dofs(upright)
            .object_layer(l.player);
        let id = jolt.world.create_body(&capsule, &settings).expect("player");
        let e = commands.spawn((Shooter(p), JoltBody(id), Transform::from_translation(at))).id();
        entities.0.insert(id, e);
    }
}

fn position(jolt: &Jolt, body: JoltBody) -> Vec3 {
    jolt.world.body(body.0).map_or(Vec3::ZERO, |b| b.position().into())
}

fn wall_rays_system(jolt: Res<Jolt>, mut aliens: Query<(&mut Alien, &JoltBody)>, mut stats: ResMut<Stats>) {
    let only_walls = [jolt.layers.solid];
    let filter = QueryFilter::new().object_layers(&only_walls);
    for (mut alien, &body) in &mut aliens {
        let at = position(&jolt, body);
        let mut nearest = None;
        for (k, dir) in wall_rays(alien.forward).into_iter().enumerate() {
            let ray = RayCast::new(r(at), v(dir * WALL_RAY_LENGTH));
            if let Ok(Some(hit)) = jolt.world.cast_ray(&ray, &filter) {
                stats.wall_hits += 1;
                if k == 0 {
                    nearest = Some(hit.fraction * WALL_RAY_LENGTH);
                }
            }
        }
        alien.wall_ahead = nearest;
    }
}

fn steer(
    scene: Res<Scene>,
    mut rng: ResMut<Rng>,
    mut stats: ResMut<Stats>,
    mut jolt: ResMut<Jolt>,
    mut aliens: Query<(&mut Alien, &JoltBody)>,
) {
    stats.tick += 1;
    for (mut alien, &body) in &mut aliens {
        let Ok(mut b) = jolt.world.body_mut(body.0) else { continue };
        let at: Vec3 = b.position().into();
        if scene.reached_goal(at) {
            let back = scene.spawn_of(alien.index, &mut rng);
            let _ = b.set_position(r(back), Activation::Activate);
            let _ = b.set_linear_velocity(oxijolt::Vec3::new(0.0, 0.0, 0.0));
            stats.arrivals += 1;
            continue;
        }
        let want = scene.steer(alien.index, stats.tick, at, alien.wall_ahead);
        alien.forward = want;
        let now: Vec3 = b.linear_velocity().into();
        let _ = b.set_linear_velocity(v(want.with_y(now.y)));
    }
}

fn shoot(jolt: Res<Jolt>, players: Query<(&Shooter, &JoltBody)>, mut stats: ResMut<Stats>) {
    let targets = [jolt.layers.solid, jolt.layers.alien];
    let filter = QueryFilter::new().object_layers(&targets);
    for (shooter, &body) in &players {
        let dir = shot_dir(shooter.0, stats.tick);
        let origin = position(&jolt, body) + dir * (ALIEN_RADIUS + 0.05);
        let ray = RayCast::new(r(origin), v(dir * SHOT_LENGTH));
        if let Ok(Some(_)) = jolt.world.cast_ray(&ray, &filter) {
            stats.shot_hits += 1;
        }
    }
}

fn gibs(
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    mut stats: ResMut<Stats>,
    mut jolt: ResMut<Jolt>,
    mut entities: ResMut<BodyEntities>,
    aliens: Query<(&Alien, &JoltBody)>,
    bodies: Query<&JoltBody>,
) {
    if !stats.tick.is_multiple_of(GIB_EVERY) {
        return;
    }
    let count = aliens.iter().len().max(1);
    let victim = (stats.tick / GIB_EVERY) as usize % count;
    let Some((_, &body)) = aliens.iter().find(|(a, _)| a.index == victim) else { return };
    let at = position(&jolt, body) + Vec3::Y;
    let layer = jolt.layers.gib;
    for _ in 0..GIBS_PER_KILL {
        let points: Vec<oxijolt::Vec3> = gib_points(&mut rng).into_iter().map(v).collect();
        let Ok(hull) = Shape::new_convex_hull(&points) else { continue };
        let settings = BodySettings::new_dynamic()
            .position(r(at))
            .linear_velocity(v(gib_velocity(&mut rng)))
            .object_layer(layer);
        let Ok(id) = jolt.world.create_body(&hull, &settings) else { continue };
        let e = commands.spawn((Gib, JoltBody(id), Transform::from_translation(at))).id();
        entities.0.insert(id, e);
        stats.gibs.push_back(e);
    }
    while stats.gibs.len() > GIB_BUDGET {
        let Some(old) = stats.gibs.pop_front() else { break };
        // A plugin would do this in an `on_remove` hook of `JoltBody`.
        if let Ok(&JoltBody(id)) = bodies.get(old) {
            let _ = jolt.world.remove_body(id);
            entities.0.remove(&id);
        }
        commands.entity(old).despawn();
    }
}

fn step(mut jolt: ResMut<Jolt>, time: Res<Time<Fixed>>, mut stats: ResMut<Stats>) {
    let started = Instant::now();
    let report = jolt.world.step(time.delta_secs()).expect("step");
    stats.physics_time = started.elapsed();
    assert!(report.is_complete(), "{report:?}");
}

fn sync_transforms(mut jolt: ResMut<Jolt>, entities: Res<BodyEntities>, mut transforms: Query<&mut Transform>) {
    let jolt = &mut *jolt;
    jolt.world.active_body_poses_into(&mut jolt.poses);
    for pose in &jolt.poses {
        let Some(&e) = entities.0.get(&pose.id) else { continue };
        if let Ok(mut t) = transforms.get_mut(e) {
            t.translation = pose.position.into();
            t.rotation = pose.rotation.into();
        }
    }
}

pub fn body_count(world: &mut World) -> usize {
    world.resource::<BodyEntities>().0.len()
}
