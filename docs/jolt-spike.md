# Jolt spike: oxijolt vs Avian

Spike of 2026-10-30. Question: is [oxijolt](https://github.com/pockerhead/oxijolt) (Rust bindings
for Jolt Physics 5.6, v1.4.0 on crates.io) faster than Avian 0.7 *for this game*, and what would a
port cost? Code: `spikes/jolt-bench/` (its own workspace; not part of the game build).

## The bench

`spikes/jolt-bench/src/scene.rs` is one engine-neutral scene, built and driven identically by
`avian_engine.rs` and `jolt_engine.rs`, both inside the same headless Bevy 0.19 app
(`MinimalPlugins` + `TransformPlugin`), one engine per process:

- a 48 x 48 level of 2 m tiles, walls greedy-merged per row (291 static boxes) on a floor box;
- N aliens: dynamic capsules, rotation locked to Y, sleeping off, steered by setting the
  horizontal velocity every tick, teleported back to a spawn on reaching the goal;
- three wall rays per alien per tick (like `avoid_walls_systems.rs`), four players shooting a
  40 m ray each per tick;
- a kill every 0.5 s bursting 12 convex-hull gibs, at most 240 alive (oldest removed);
- layers as the game has them (gibs only hit floor and walls, aliens and players hit everything).

Fixed step 0.05 s (the game's), 600 ticks, timed from tick 61. Each engine as shipped: Avian's
6 substeps, Jolt's 10 velocity / 2 position steps. "Update" is the whole `App::update` (game
systems, physics, transform propagation); "physics" is Avian's `PhysicsTotalDiagnostics` step
time, or the time of `PhysicsWorld::step`.

```sh
cd spikes/jolt-bench
cargo run --release -- avian 200
cargo run --release -- jolt 200        # Jolt workers = cores - 1
cargo run --release -- jolt 200 1      # one Jolt worker
```

## Results

16-core Linux machine, release, mean per tick (two repeats agree within a few %):

| aliens | Avian step | Jolt step, 15 workers | Jolt step, 1 worker | Avian update | Jolt update (15w) |
|---|---|---|---|---|---|
| 50 | 0.87 ms | 0.14 ms | 0.15 ms | 1.29 ms | 0.39 ms |
| 200 | 1.41 ms | 0.24 ms | 0.42 ms | 2.05 ms | 0.80 ms |
| 500 | 3.16 ms | 0.55 ms | 1.03 ms | 4.28 ms | 1.75 ms |
| 1000 | 5.98 ms | 1.15 ms | 2.17 ms | 8.06 ms | 3.52 ms |
| 2000 | 12.8 ms | 2.98 ms | 5.36 ms | 17.2 ms | 8.24 ms |

- **The step is 4-6 times faster on Jolt**, and single-threaded Jolt still beats multi-threaded
  Avian 2-3 times. The whole update is 2-2.5 times faster.
- Both engines did the same work: wall-ray hits within 3 %, arrivals within 10 %.
- **The rest of the update got slightly slower** (update minus step at 1000 aliens: Jolt 2.4 ms,
  Avian 2.1 ms). The spike reads positions with `world.body(id)`, which takes a body lock per
  call; the bridge should read the synced `Transform` instead, and write velocities in one pass.
- **Jolt's buffers are fixed-size.** At 1000 crowding aliens the default
  `max_contact_constraints` (10240) ran out and the step dropped contacts (it says so in its
  `StepReport`; the spike asserts on it). Sizing these, and logging an incomplete step, is part
  of any integration.
- Scale check: the game ticks physics 20 times a second. At the alien counts the waves have
  today (tens), Avian costs about 1 ms per tick, which is small; the gain matters for hordes of
  hundreds or a lot of gibs and debris, and for the fixed-step hitch when a frame has to run two
  ticks to catch up. **Before porting, measure the real game's physics share at a realistic wave**
  (Avian's `PhysicsDiagnosticsUiPlugin` shows it live).

oxijolt's own [comparison](https://github.com/pockerhead/oxijolt/blob/main/docs/comparison.md) on
Rapier's stress scenes agrees: Avian is the slowest of the three on almost every scene. Rapier
beats Jolt on contact-heavy piles, which this game doesn't have; Jolt wins on joints.

## What a port would take

No Bevy plugin exists yet ("Bevy plugin, in a separate repository" is an open roadmap item), so
we would write the bridge. `jolt_engine.rs` is the smallest version: the world as a resource, a
`JoltBody(BodyId)` component and a `BodyId -> Entity` map, the step in `FixedPostUpdate` (where
Avian steps), and the awake bodies' poses copied into `Transform` with
`active_body_poses_into`. oxijolt's math types convert to Bevy's through its `glam032` feature
(Bevy 0.19 is on glam 0.32).

What the game uses from Avian today (about 40 files), and where each goes:

| Avian | oxijolt | Notes |
|---|---|---|
| `RigidBody::{Static,Dynamic,Kinematic}` (22 sites) | `BodySettings::new_static/dynamic/kinematic` | bodies made by a component hook, removed by an `on_remove` hook |
| `Collider::{cuboid,capsule,sphere,cylinder,cone,convex_hull}` | `Shape::new_box/capsule/sphere/cylinder`, tapered cylinder, `new_convex_hull` | box and capsule take *half* sizes; boxes and cylinders have a 0.05 m convex radius by default |
| `CollisionLayers` (10-layer `CollisionLayer` enum, 36 sites) | one `ObjectLayer` per distinct (memberships, filters) pair, pairs enabled by Avian's own rule | every combination is fixed at spawn today; the layer table is built once with the world, so a body cannot change layer without being recreated |
| `LockedAxes` (10) | `BodySettings::allowed_dofs` | world axes; a body with fewer DOFs cannot join a constraint |
| `LinearVelocity` (24) | `BodyMut::set_linear_velocity` / `linear_velocity` | keep a component and sync it both ways, or write through the world |
| `SpatialQuery::cast_ray` / `cast_shape` (6 sites) | `cast_ray`, `cast_shape`, `collide_shape` with `QueryFilter` | a `SystemParam` wrapping `Res<Jolt>` keeps the call sites; layer masks become lists of object layers |
| `CollisionStart` + `CollisionEventsEnabled` (projectiles, collision handling) | `EventSettings::contacts`, `ContactEvent::Added` from `take_events` | re-emit as a Bevy message after the step |
| `CollidingEntities` (touch damage) | a set kept from `ContactEvent::Added` / `Removed` | |
| `Sensor` layers (spawn point, goal, aim sensor) | `BodySettings::sensor(true)` | a *static* sensor only sees *awake* bodies: keep sleeping off on whatever it must detect |
| `Friction`, `GravityScale`, damping | `BodySettings::friction`, `gravity_factor`, `linear_damping` | creation-time settings |
| `TransformInterpolation` (camera, players) | none | keep the previous and current pose per body and blend in `PostUpdate` |
| `PhysicsDebugPlugin` | feature `debug-renderer` -> `debug_lines` | draw the lines with gizmos |
| kinematic build indicator | `BodyMut` kinematic move | |

Also on the list: Jolt's `CharacterVirtual` (stairs, slopes, floor sticking, a humanoid preset)
could replace the hand-rolled dynamic-capsule players; and ragdolls could be an alternative to
the dismember gibs.

## Costs and risks

- **Not pure Rust.** A prebuilt Jolt is downloaded (sha256-checked) on x86_64 Linux and Windows
  and Apple-silicon macOS; elsewhere it needs CMake and a C++ toolchain. No WASM. The spike's
  first build downloaded it without trouble here (oxijolt measures its own clean build at about
  4 s with the prebuilt library).
- **Shape and rigid-body API not complete** (their own words), and one maintainer; on the other
  hand the crate is at 1.4 with a semver promise, CI with Jolt's assertions on, typed errors
  before anything reaches Jolt, and thorough docs.
- **Global state**: oxijolt owns joltc's process-global listener, filter and assert procs.
- A port touches about 40 files and every physics-feel tuning (friction, damping, throw arcs,
  gib bounce). Running both backends side by side would mean a physics facade over the whole game;
  cheaper is a branch that ports outright, gated on a playtest.

## Suggested next steps

1. Measure the real game: Avian's diagnostics at a busy wave plus gib bursts. If physics is a
   few percent of the frame, stop here and keep this note.
2. If it matters: grow `jolt_engine.rs` into `src/physics/` (hooks, events, query param,
   interpolation, layer table from the `CollisionLayer` enum), port one slice (static walls +
   aliens + rays) on a branch, then the rest.
3. Watch for oxijolt's own Bevy plugin; it may save step 2.
