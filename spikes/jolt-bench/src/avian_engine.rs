//! The scene on Avian, written the way the game uses it: components, `SpatialQuery`, layers.

use avian3d::diagnostics::PhysicsTotalDiagnostics;
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::scene::*;

#[derive(PhysicsLayer, Default, Clone, Copy)]
enum Layer {
    #[default]
    Floor,
    Wall,
    Alien,
    Player,
    Gib,
}

#[derive(Component)]
struct Shooter(usize);

pub fn build(app: &mut App) {
    app.add_plugins((PhysicsPlugins::default(), avian3d::diagnostics::PhysicsTotalDiagnosticsPlugin))
        .add_systems(Startup, spawn)
        .add_systems(
            FixedUpdate,
            (wall_rays_system, steer, shoot, gibs, read_physics_time).chain(),
        );
}

fn capsule() -> Collider {
    Collider::capsule(ALIEN_RADIUS, ALIEN_LENGTH)
}

fn spawn(mut commands: Commands, scene: Res<Scene>, mut rng: ResMut<Rng>) {
    let solid = CollisionLayers::new([Layer::Wall], [Layer::Alien, Layer::Player, Layer::Gib]);
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(scene.floor.half.x * 2.0, scene.floor.half.y * 2.0, scene.floor.half.z * 2.0),
        Transform::from_translation(scene.floor.center),
        CollisionLayers::new([Layer::Floor], [Layer::Alien, Layer::Player, Layer::Gib]),
    ));
    for wall in &scene.walls {
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(wall.half.x * 2.0, wall.half.y * 2.0, wall.half.z * 2.0),
            Transform::from_translation(wall.center),
            solid,
        ));
    }
    for index in 0..scene.aliens {
        commands.spawn((
            Alien { index, forward: Vec3::Z, wall_ahead: None },
            RigidBody::Dynamic,
            capsule(),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            SleepingDisabled,
            Transform::from_translation(scene.spawn_of(index, &mut rng)),
            CollisionLayers::new(
                [Layer::Alien],
                [Layer::Floor, Layer::Wall, Layer::Alien, Layer::Player],
            ),
        ));
    }
    for (p, &at) in scene.players.iter().enumerate() {
        commands.spawn((
            Shooter(p),
            RigidBody::Dynamic,
            capsule(),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            Transform::from_translation(at),
            CollisionLayers::new(
                [Layer::Player],
                [Layer::Floor, Layer::Wall, Layer::Alien, Layer::Player],
            ),
        ));
    }
}

fn wall_rays_system(spatial: SpatialQuery, mut aliens: Query<(&mut Alien, &Position)>, mut stats: ResMut<Stats>) {
    let filter = SpatialQueryFilter::from_mask([Layer::Wall]);
    for (mut alien, position) in &mut aliens {
        let mut nearest: Option<f32> = None;
        for (k, dir) in wall_rays(alien.forward).into_iter().enumerate() {
            let Ok(dir) = Dir3::new(dir) else { continue };
            if let Some(hit) = spatial.cast_ray(position.0, dir, WALL_RAY_LENGTH, true, &filter) {
                stats.wall_hits += 1;
                if k == 0 {
                    nearest = Some(hit.distance);
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
    mut aliens: Query<(&mut Alien, &mut Position, &mut LinearVelocity)>,
) {
    stats.tick += 1;
    for (mut alien, mut position, mut velocity) in &mut aliens {
        if scene.reached_goal(position.0) {
            position.0 = scene.spawn_of(alien.index, &mut rng);
            velocity.0 = Vec3::ZERO;
            stats.arrivals += 1;
            continue;
        }
        let v = scene.steer(alien.index, stats.tick, position.0, alien.wall_ahead);
        alien.forward = v;
        velocity.0 = v.with_y(velocity.0.y);
    }
}

fn shoot(spatial: SpatialQuery, players: Query<(&Shooter, &Position)>, mut stats: ResMut<Stats>) {
    let filter = SpatialQueryFilter::from_mask([Layer::Wall, Layer::Alien]);
    for (shooter, position) in &players {
        let Ok(dir) = Dir3::new(shot_dir(shooter.0, stats.tick)) else { continue };
        let origin = position.0 + dir * (ALIEN_RADIUS + 0.05);
        if spatial.cast_ray(origin, dir, SHOT_LENGTH, true, &filter).is_some() {
            stats.shot_hits += 1;
        }
    }
}

fn gibs(
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    mut stats: ResMut<Stats>,
    aliens: Query<(&Alien, &Position)>,
) {
    if !stats.tick.is_multiple_of(GIB_EVERY) {
        return;
    }
    let count = aliens.iter().len().max(1);
    let victim = (stats.tick / GIB_EVERY) as usize % count;
    let Some((_, at)) = aliens.iter().find(|(a, _)| a.index == victim) else { return };
    let at = at.0 + Vec3::Y;
    for _ in 0..GIBS_PER_KILL {
        let Some(hull) = Collider::convex_hull(gib_points(&mut rng)) else { continue };
        let gib = commands
            .spawn((
                Gib,
                RigidBody::Dynamic,
                hull,
                Transform::from_translation(at),
                LinearVelocity(gib_velocity(&mut rng)),
                CollisionLayers::new([Layer::Gib], [Layer::Floor, Layer::Wall]),
            ))
            .id();
        stats.gibs.push_back(gib);
    }
    while stats.gibs.len() > GIB_BUDGET {
        if let Some(old) = stats.gibs.pop_front() {
            commands.entity(old).despawn();
        }
    }
}

/// Avian's step of the previous tick (it runs in `FixedPostUpdate`, after these systems).
fn read_physics_time(diagnostics: Option<Res<PhysicsTotalDiagnostics>>, mut stats: ResMut<Stats>) {
    if let Some(d) = diagnostics {
        stats.physics_time = d.step_time;
    }
}

pub fn body_count(world: &mut World) -> usize {
    world.query::<&RigidBody>().iter(world).filter(|b| !b.is_static()).count()
}
