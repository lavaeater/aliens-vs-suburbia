# Porting physics to Jolt

Plan for moving the game from Avian 0.7 to Jolt (oxijolt 1.4), in parallel with the gameplay
work on `playtest`. Why: `docs/jolt-spike.md` (step 4-6 times faster on a game-shaped scene) and,
later, Jolt's wheeled and tracked vehicles and its `CharacterVirtual`.

## How it runs in parallel

- Branch `jolt`, cut from `playtest`. `playtest` is merged *into* `jolt` often (not the other
  way, until the end), so gameplay never waits on the port.
- **Game code never names `avian3d`.** It imports `crate::physics::prelude`, which today
  re-exports Avian (`src/physics/avian.rs`). That was phase 1: a one-line import change in 38
  files and nothing else. Merging `playtest` in can only conflict on those import lines; new
  code on `playtest` that writes `use avian3d::prelude::X` just gets its import rewritten on the
  merge.
- The Jolt backend is new files under `src/physics/jolt/`, behind a cargo feature
  (`physics-jolt`, Avian stays the default), so it can't conflict with anything and the game
  keeps building and playing on Avian the whole time.
- When Jolt plays as well as Avian: make it the default, playtest, then delete the Avian
  backend. Only then does `jolt` go back into `playtest` / `main`.

The habit that keeps this cheap: on `playtest`, import physics from `crate::physics::prelude`
too (it is identical there once phase 1 is merged), and if a new Avian name is needed, add it to
the prelude rather than reaching into `avian3d`.

## The contract

`src/physics/mod.rs`'s prelude lists every name the game uses. The Jolt backend provides the
same names with the same shapes, so call sites don't change:

| Name | Used for | Jolt backend |
|---|---|---|
| `RigidBody` (Static / Dynamic / Kinematic) | 22 spawn sites | `on_add` creates the Jolt body from the entity's components; `on_remove` removes it |
| `Collider::{cuboid, capsule, sphere, cylinder, cone, convex_hull}` | | a shape description, built to a `Shape` when the body is created; Avian takes full sizes and `capsule(radius, length)`, Jolt half sizes and `capsule(half_length, radius)` |
| `Position`, `Rotation` | 25 queries, 6 writers | authoritative copies like Avian's: a write teleports the body before the step, the step writes them back (without change detection), then into `Transform` |
| `LinearVelocity`, `AngularVelocity` | AI steering, throws, gibs, knockback | same: written in before the step, read back after |
| `CollisionLayers`, `LayerMask`, `PhysicsLayer` | 20 `CollisionLayers::new`, 10 `LayerMask::from` | see *Layers* |
| `LockedAxes` | players, aliens, build indicator | `BodySettings::allowed_dofs` |
| `Friction`, `LinearDamping`, `AngularDamping`, `GravityScale` | | body settings at creation; a change recreates the body (rare) |
| `Sensor` | spawn point, goal, aim sensor, towers | `BodySettings::sensor(true)`; a static sensor only sees awake bodies |
| `CollisionStart`, `CollisionEventsEnabled` | projectiles, collision handling | `ContactEvent::Added` from `take_events`, re-sent as a message after the step |
| `CollidingEntities` | touch damage | a set per entity kept from `Added` / `Removed` |
| `SpatialQuery` (`cast_ray` x4, `cast_shape` x1), `SpatialQueryFilter::from_mask`, `ShapeCastConfig` | AI wall rays, shooting, attack sweep, explosions | a `SystemParam` over the world resource with the same methods, returning the same hit data (`entity`, `distance`, `normal`, `point`) |
| `TransformInterpolation` | camera target, players | previous and current pose per body, blended in `PostUpdate` by `Time<Fixed>::overstep_fraction` |
| `PhysicsGizmos` | F3, the editor's debug toggle | our own `GizmoConfigGroup`; lines from oxijolt's `debug-renderer` feature |

The world steps in `FixedPostUpdate`, where Avian steps, so system order elsewhere stays as is
(`camera_plugin.rs` relies on that).

### Layers

Avian lets a body be in several layers and filter on several; Jolt puts each body in exactly one
object layer and takes a fixed table of which layers collide, made with the world. Every
`CollisionLayers` the game makes is a constant combination fixed at spawn, so: one Jolt object
layer per distinct (memberships, filters) pair, and a pair collides by Avian's own rule
(`a.memberships & b.filters != 0 && b.memberships & a.filters != 0`). The table has to be
complete when the world is made, so the backend needs the list of combinations up front. The
choices:

1. **A registry** (preferred): the combinations live as named constants next to
   `CollisionLayer` (they are repeated inline today anyway), the backend builds the table from
   that list, and a body with an unlisted combination is logged and recreated in a rebuilt
   world (slow, but only ever on a bug).
2. Rebuild the world whenever a new combination shows up: works without a list, but hitches
   once per new combination.

Queries filter by object layer list, which is computed from the mask the same way.

## Phases

1. **Facade** (done): `src/physics/`, every import through it, Avian behind it. Game unchanged.
2. **Jolt backend, headless**: `src/physics/jolt/` behind `physics-jolt`, every prelude name, with
   tests that run a small app (as `spikes/jolt-bench` does): bodies made and removed by hooks,
   Position/velocity round trips, layers, ray and shape casts, contact events and
   `CollidingEntities`, sensors, interpolation. Sized buffers, and an incomplete `StepReport`
   logged.
3. **The game on Jolt**: `cargo run --features physics-jolt`; fix what the facade didn't cover,
   then tune feel (friction, damping, throw arcs, gib bounce: Jolt's defaults differ) against
   Avian side by side.
4. **Switch**: Jolt default, playtest, Avian removed, branch merged.
5. **Jolt-only features**: `CharacterVirtual` for players and neighbours (stairs, slopes, no
   hand-rolled capsule); vehicles (`WheeledVehicleSettings::car`, tracked, motorcycles); ragdolls
   for kills as an alternative to `gore::dismember`.

## Risks

- oxijolt has no Bevy plugin of its own yet (on their roadmap); this backend is that plugin.
- Not pure Rust: prebuilt Jolt on x86_64 Linux / Windows and Apple silicon, else CMake and a C++
  toolchain; no WASM.
- Per-body reads take a lock in oxijolt; the backend reads poses in one batch after the step
  (`active_body_poses_into`) and game code reads `Position`, never the world.
