//! Headless tests of the Jolt backend: a minimal app stepped tick by tick at the game's 20 Hz.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use super::*;

#[derive(Default, Clone, Copy)]
enum Layer {
    #[default]
    Floor,
    Ball,
    Alien,
    Sensor,
}

impl PhysicsLayer for Layer {
    fn to_bits(&self) -> u32 {
        1 << *self as u32
    }
    fn all_bits() -> u32 {
        0b1111
    }
}

fn app() -> App {
    let step = Duration::from_secs_f32(0.05);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .insert_resource(Time::<Fixed>::from_duration(step))
        .insert_resource(TimeUpdateStrategy::ManualDuration(step));
    build_core(&mut app);
    app.finish();
    app.cleanup();
    app
}

fn run(app: &mut App, ticks: usize) {
    for _ in 0..ticks {
        app.update();
    }
}

fn floor(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(40.0, 1.0, 40.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
            CollisionLayers::new([Layer::Floor], [Layer::Ball, Layer::Alien]),
        ))
        .id()
}

fn ball(app: &mut App, at: Vec3, filters: impl Into<LayerMask>) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.5),
            Transform::from_translation(at),
            CollisionLayers::new([Layer::Ball], filters),
        ))
        .id()
}

fn position(app: &App, e: Entity) -> Vec3 {
    app.world().get::<Position>(e).expect("a body").0
}

#[test]
fn a_dropped_ball_comes_to_rest_on_the_floor() {
    let mut app = app();
    floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 3.0, 0.0), [Layer::Floor]);
    run(&mut app, 60);
    let p = position(&app, ball);
    assert!((p.y - 0.5).abs() < 0.05, "rests on the floor: {p}");
    // `Transform` follows the body, and `Position` was taken from the spawn transform.
    let t = app.world().get::<Transform>(ball).expect("a transform");
    assert!((t.translation - p).length() < 1e-4);
    assert!(p.x.abs() < 1e-3 && p.z.abs() < 1e-3);
}

#[test]
fn layers_that_dont_agree_pass_through_each_other() {
    let mut app = app();
    floor(&mut app);
    // The floor filters on balls, but this ball doesn't filter on the floor.
    let ghost = ball(&mut app, Vec3::new(0.0, 3.0, 0.0), [Layer::Alien]);
    run(&mut app, 30);
    assert!(position(&app, ghost).y < -1.0, "fell through: {}", position(&app, ghost));
}

#[test]
fn a_layer_combination_seen_late_rebuilds_without_losing_bodies() {
    let mut app = app();
    floor(&mut app);
    let first = ball(&mut app, Vec3::new(-2.0, 3.0, 0.0), [Layer::Floor]);
    run(&mut app, 40);
    let resting = position(&app, first);
    // A combination the world hasn't seen: the world is rebuilt with every body in it.
    let second = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.5),
            Transform::from_xyz(2.0, 3.0, 0.0),
            CollisionLayers::new([Layer::Alien], [Layer::Floor, Layer::Alien]),
        ))
        .id();
    run(&mut app, 40);
    assert!((position(&app, first) - resting).length() < 0.02, "the first ball stayed put");
    assert!((position(&app, second).y - 0.5).abs() < 0.05, "the second landed");
}

#[test]
fn a_ray_hits_the_first_collider_its_mask_sees() {
    let mut app = app();
    floor(&mut app);
    let near = ball(&mut app, Vec3::new(0.0, 0.5, 3.0), [Layer::Floor]);
    let far = ball(&mut app, Vec3::new(0.0, 0.5, 6.0), [Layer::Floor]);
    run(&mut app, 2);
    let mut hits = app.world_mut().run_system_cached(|spatial: SpatialQuery| {
        let origin = Vec3::new(0.0, 0.5, 0.0);
        let balls = SpatialQueryFilter::from_mask([Layer::Ball]);
        [
            spatial.cast_ray(origin, Dir3::Z, 20.0, true, &balls),
            spatial.cast_ray(origin, Dir3::Z, 20.0, true, &balls.clone().with_excluded_entities([])),
            spatial.cast_ray(origin, Dir3::Z, 2.0, true, &balls),
            spatial.cast_ray(origin, Dir3::Z, 20.0, true, &SpatialQueryFilter::from_mask([Layer::Sensor])),
        ]
    })
    .expect("the system runs")
    .into_iter();
    let hit = hits.next().flatten().expect("hits the near ball");
    assert_eq!(hit.entity, near);
    assert!((hit.distance - 2.5).abs() < 0.01, "{}", hit.distance);
    assert!((hit.normal - Vec3::NEG_Z).length() < 0.01, "{}", hit.normal);
    assert_eq!(hits.next().flatten().map(|h| h.entity), Some(near));
    assert!(hits.next().flatten().is_none(), "out of range");
    assert!(hits.next().flatten().is_none(), "a mask that sees nothing hits nothing");

    let skipped = app
        .world_mut()
        .run_system_cached_with(
            |near: In<Entity>, spatial: SpatialQuery| {
                let filter = SpatialQueryFilter::from_mask([Layer::Ball]).with_excluded_entities([*near]);
                spatial.cast_ray(Vec3::new(0.0, 0.5, 0.0), Dir3::Z, 20.0, true, &filter)
            },
            near,
        )
        .expect("the system runs");
    assert_eq!(skipped.map(|h| h.entity), Some(far), "an excluded entity is passed over");
}

#[test]
fn a_shape_cast_finds_what_it_would_sweep_into() {
    let mut app = app();
    floor(&mut app);
    let target = ball(&mut app, Vec3::new(4.0, 0.5, 0.0), [Layer::Floor]);
    run(&mut app, 2);
    let hit = app
        .world_mut()
        .run_system_cached(|spatial: SpatialQuery| {
            spatial.cast_shape(
                &Collider::sphere(0.25),
                Vec3::new(0.0, 0.5, 0.0),
                Quat::IDENTITY,
                Dir3::X,
                &ShapeCastConfig { max_distance: 10.0, ..default() },
                &SpatialQueryFilter::from_mask([Layer::Ball]),
            )
        })
        .expect("the system runs")
        .expect("a hit");
    assert_eq!(hit.entity, target);
    assert!((hit.distance - 3.25).abs() < 0.05, "{}", hit.distance);
}

#[derive(Resource, Default)]
struct Starts(Vec<CollisionStart>);

fn record(mut reader: MessageReader<CollisionStart>, mut starts: ResMut<Starts>) {
    starts.0.extend(reader.read().copied());
}

#[test]
fn contacts_fill_colliding_entities_and_send_collision_start_once() {
    let mut app = app();
    app.init_resource::<Starts>().add_systems(FixedLast, record);
    let floor = floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 1.0, 0.0), [Layer::Floor]);
    app.world_mut().entity_mut(ball).insert((CollisionEventsEnabled, CollidingEntities::default()));
    run(&mut app, 30);
    let starts = &app.world().resource::<Starts>().0;
    assert_eq!(starts.len(), 1, "one start for a ball that lands and stays: {starts:?}");
    let s = starts[0];
    assert!([s.collider1, s.collider2].contains(&ball) && [s.collider1, s.collider2].contains(&floor));
    assert!(app.world().get::<CollidingEntities>(ball).expect("kept").contains(&floor));

    app.world_mut().despawn(floor);
    run(&mut app, 2);
    assert!(app.world().get::<CollidingEntities>(ball).expect("kept").is_empty(), "a despawned collider is forgotten");
}

#[test]
fn a_sensor_sees_without_pushing() {
    let mut app = app();
    floor(&mut app);
    let sensor = app
        .world_mut()
        .spawn((
            Collider::cylinder(3.0, 2.0),
            Sensor,
            Position(Vec3::new(0.0, 1.0, 0.0)),
            CollisionLayers::new([Layer::Sensor], [Layer::Alien]),
            CollidingEntities::default(),
        ))
        .id();
    let alien = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::capsule(0.3, 0.8),
            Transform::from_xyz(0.0, 3.0, 0.0),
            CollisionLayers::new([Layer::Alien], [Layer::Floor, Layer::Sensor]),
        ))
        .id();
    run(&mut app, 60);
    assert!(app.world().get::<CollidingEntities>(sensor).expect("kept").contains(&alien));
    assert!(position(&app, alien).y < 1.0, "fell through the sensor onto the floor");
}

#[test]
fn a_written_velocity_moves_the_body_and_locked_axes_keep_it_upright() {
    let mut app = app();
    floor(&mut app);
    let alien = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::capsule(0.3, 0.8),
            Transform::from_xyz(0.0, 0.71, 0.0),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            CollisionLayers::new([Layer::Alien], [Layer::Floor]),
        ))
        .id();
    run(&mut app, 2);
    for _ in 0..20 {
        app.world_mut().get_mut::<LinearVelocity>(alien).expect("a velocity").0 = Vec3::new(3.0, 0.0, 0.0);
        run(&mut app, 1);
    }
    let p = position(&app, alien);
    assert!(p.x > 2.0, "walked: {p}");
    let up = app.world().get::<Rotation>(alien).expect("a rotation").0 * Vec3::Y;
    assert!(up.y > 0.999, "stayed upright: {up}");
}

#[test]
fn a_written_position_teleports_the_body() {
    let mut app = app();
    floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 0.5, 0.0), [Layer::Floor]);
    run(&mut app, 4);
    app.world_mut().get_mut::<Position>(ball).expect("a position").0 = Vec3::new(10.0, 0.5, 0.0);
    run(&mut app, 2);
    assert!((position(&app, ball).x - 10.0).abs() < 0.01, "{}", position(&app, ball));
}

#[test]
fn despawning_removes_the_body() {
    let mut app = app();
    floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 3.0, 0.0), [Layer::Floor]);
    run(&mut app, 2);
    assert_eq!(app.world().resource::<JoltWorld>().entities.len(), 2);
    app.world_mut().despawn(ball);
    run(&mut app, 2);
    assert_eq!(app.world().resource::<JoltWorld>().entities.len(), 1);
}
