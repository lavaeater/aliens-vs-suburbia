# Migrating from Avian to Jolt (via `rolt`)

Status: **DRAFT — planning only.** No code has been changed yet.

## TL;DR

We currently use `avian3d 0.6` as our physics engine. It is a native-Rust,
ECS-first physics engine that plugs directly into Bevy: colliders, rigid bodies,
velocities and transforms are all Bevy components, and its systems run inside the
Bevy schedule automatically.

`rolt 0.3.1+Jolt-5.0.0` is a very different animal. It is a thin, idiomatic Rust
binding over **JoltC** (the C API for the [Jolt Physics] engine). Its only
dependencies are `glam 0.27`, `joltc-sys`, and `paste` — **it has no Bevy
integration at all.** There is no `PhysicsPlugin`, no `Collider` component, no
system that reads Bevy `Transform`s. Jolt owns its own world of bodies, keyed by
its own `BodyID` handles.

**Consequence:** this is not a drop-in dependency swap. Migrating means *building
a Bevy ↔ Jolt bridge*: a plugin that owns the Jolt `PhysicsSystem`, mirrors Bevy
entities into Jolt bodies, steps the simulation on the fixed timestep, and writes
the results back onto Bevy `Transform`s. Every one of the ~27 files that currently
imports `avian3d` has to be re-expressed against that bridge.

This document lays out how to do that, and flags the sharp edges.

[Jolt Physics]: https://github.com/jrouwe/JoltPhysics

## Why do this at all?

Per the commit that added `rolt` ("...for no reason whatsoever"), there is no
pressing gameplay need. The stated motivation is that Jolt is a more mature,
battle-tested engine (it powers Horizon Forbidden West, Deathloop, etc.) than
avian. That maturity buys: better stacking/contact stability, a proper character
controller, deterministic simulation, and good performance on large body counts.
The cost is that we give up avian's seamless ECS ergonomics and take on the
bridge-maintenance burden ourselves.

This is worth doing as an **experiment / spike**, not as a casual refactor.

## Current avian surface area

Avian appears in **27 source files**. The concepts we actually use, grouped by
what the bridge must replace:

| Avian concept | Files (examples) | Bridge replacement |
|---|---|---|
| `RigidBody`, `Collider` | `player/bundle.rs`, `building/systems.rs`, `throwing_system.rs`, `map_systems.rs` | Jolt `Body` created from a `BodyCreationSettings` + `Shape` |
| `LinearVelocity`, `AngularVelocity` | `kinematic_movement_system.rs`, `dynamic_movement_system.rs`, `move_towards_goal_systems.rs`, `death_revive.rs` | `body.set_linear_velocity` / `set_angular_velocity` |
| `Position`, `Rotation` | `camera/systems.rs`, `coin_system.rs`, many AI/ spawn systems | read back from Jolt body transform each step |
| `CollisionLayers`, `LayerMask`, `PhysicsLayer` | `general/components/mod.rs`, `player/bundle.rs`, `map_components.rs` | Jolt object layers + broadphase layers (see below) |
| `LockedAxes` | `player/bundle.rs`, `building/systems.rs` | Jolt `AllowedDOFs` on body creation |
| `Friction`, `LinearDamping`, `AngularDamping` | `player/bundle.rs` | fields on `BodyCreationSettings` / `MotionProperties` |
| `Sensor`, `CollisionEventsEnabled` | `building/systems.rs`, `throwing_system.rs`, `map_systems.rs` | Jolt sensor flag + contact listener |
| `CollisionStart`, `CollidingEntities`, `TouchDamage` | `collision_handling_system.rs`, `touch_damage_system.rs` | Jolt `ContactListener` → drain into a Bevy `Message` queue |
| `SpatialQuery`, `SpatialQueryFilter`, `ShapeCast` | `avoid_walls_systems.rs`, `approach_and_attack_player_systems.rs` | Jolt `NarrowPhaseQuery` ray/shape casts |
| `PhysicsPlugins`, `PhysicsDebugPlugin`, `PhysicsGizmos` | `main.rs`, `game_state_plugin.rs` | our own `JoltPhysicsPlugin` + debug draw |
| `TransformInterpolation` | `camera/systems.rs` | our own interpolation, or step in `Update` |

## Collision layers today

From `src/general/components/mod.rs`: `ImpassableAll`, `Floor`, `Ball`, `Alien`,
`Player`, `BuildIndicator`, `Sensor`, `PlayerAimSensor`, `AlienSpawnPoint`,
`AlienGoal`. These map cleanly onto Jolt **object layers**, with a broadphase
layer grouping (e.g. static terrain vs. moving actors vs. sensors). The
object-vs-object and object-vs-broadphase filter tables replace avian's
`CollisionLayers` bitmask logic.

## Proposed architecture

A new `src/physics/` module owning a `JoltPhysicsPlugin`:

1. **`JoltWorld` resource** — wraps Jolt's `PhysicsSystem`, `BodyInterface`, temp
   allocator and job system. Non-`Send`? TBD — see open questions.
2. **Component `JoltBody(BodyID)`** — the link from a Bevy entity to its Jolt
   body. Replaces the presence of avian's `RigidBody`.
3. **A `PhysicsBody` authoring component** (our own) describing mass/shape/layer/
   DOFs, so gameplay code stays engine-agnostic. A system turns `PhysicsBody`
   into a Jolt body and attaches `JoltBody`.
4. **Sync-in system** (before step): push kinematic bodies' desired velocities /
   teleports from Bevy into Jolt.
5. **Step system** (fixed 0.05s timestep, matching today): `physics_system.update(...)`.
6. **Sync-out system** (after step): copy Jolt body transforms → Bevy `Transform`
   (and our own `Position`/`Rotation` shims if we keep them).
7. **Contact listener** → a channel drained into Bevy `Message`s so
   `collision_handling_system` / `touch_damage_system` keep working with minimal
   change.
8. **Cleanup** — removing an entity must remove its Jolt body (mirrors the
   collider-strip dance in `clear_game_entities_plugin.rs`).

### Compatibility shim strategy

To avoid editing all 27 files at once, provide thin Bevy components/newtypes named
like avian's (`Position`, `Rotation`, `LinearVelocity`, ...) backed by the bridge.
Gameplay systems read/write those; sync systems reconcile them with Jolt. This
lets us migrate file-by-file and keep the game compiling.

## glam version mismatch (sharp edge)

`rolt` depends on `glam 0.27`, while Bevy 0.18 uses a newer glam. Cargo will
compile **two** glam versions; their `Vec3`/`Quat` types are distinct and do not
`From`-convert automatically. The bridge must convert at the boundary
(`bevy_vec3.to_array()` → `glam027::Vec3::from_array`). Centralize these in one
`convert.rs` so the pain is contained. Confirm the exact Bevy glam version and
whether a compatible `rolt`/`glam` combination exists before committing.

## Open questions

- **Threading / `Send`+`Sync`:** is Jolt's `PhysicsSystem` (via `rolt`) safe to
  hold in a Bevy `Resource`? May need `NonSend` or a dedicated thread.
- **Determinism vs. Bevy scheduling:** does stepping in `FixedUpdate` give us the
  stability we want, and how do we interpolate for rendering?
- **Character controller:** Jolt has a real one. Do we adopt it for the player and
  aliens (replacing `kinematic_movement_system`), or keep our kinematic approach?
- **Shapes from meshes:** avian's `collider-from-mesh` — does `rolt` expose Jolt's
  mesh/convex-hull cooking, and at what cost?
- **Build/link:** `joltc-sys` compiles native C++. Verify it builds cleanly in
  this toolchain and CI before going further.

## Suggested phased rollout

1. **Spike:** stand up `JoltPhysicsPlugin` with a single falling box, stepping and
   syncing back to a Bevy `Transform`. Validate build, threading, glam conversion.
2. **Layers + static terrain:** mirror the map's floor/wall colliders into Jolt.
3. **Dynamic actors:** players, aliens, thrown balls; velocity control.
4. **Queries:** port `SpatialQuery` ray/shape casts used by AI.
5. **Events:** contact listener → messages; port touch damage & collision handling.
6. **Delete avian:** remove the dependency and the compatibility shims that are no
   longer needed.

## API reference (verified against docs.rs for `rolt 0.3.1`)

**Reality check: `rolt 0.3.1` is an early, deliberately thin binding.** Its safe
public API covers world setup, body handles, velocity, and ray casting — and not
much else. Anything richer (shapes, body creation settings, contact events,
character controllers) is **not** in the safe API and must be reached through the
raw `joltc-sys` `JPC_*` types with `unsafe`. This materially raises the cost of
every phase below; budget for writing a fair amount of `unsafe` glue and possibly
upstreaming safe wrappers to `rolt` ourselves.

### What the safe API actually exposes

Full item list from `rolt 0.3.1`:

- **Structs:** `PhysicsSystem`, `Body`, `BodyId`, `BodyInterface`,
  `NarrowPhaseQuery`, `RRayCast`, `RayCastArgs`, `RayCastResult`, `BodyFilterImpl`,
  `ObjectLayer`, `BroadPhaseLayer`, and the filter impls
  (`ObjectLayerFilterImpl`, `ObjectLayerPairFilterImpl`,
  `BroadPhaseLayerFilterImpl`, `BroadPhaseLayerInterfaceImpl`,
  `ObjectVsBroadPhaseLayerFilterImpl`), `Ref<T>`, `Color`, and math types
  (`Vec3`, `Vec4`, `Quat`, `Mat4`, `DVec3`; aliases `RVec3`, `Real`).
- **Traits:** `BodyFilter`, `ObjectLayerFilter`, `ObjectLayerPairFilter`,
  `BroadPhaseLayerFilter`, `BroadPhaseLayerInterface`,
  `ObjectVsBroadPhaseLayerFilter`, plus conversion traits `FromJolt` / `IntoJolt`
  / `IntoRolt` / `RefTarget`.
- **Free functions (global lifecycle):** `register_default_allocator()`,
  `register_types()`, `factory_init()`, `factory_delete()`,
  `unregister_types()`. These must be called once at startup / teardown before
  any physics object exists.

### `PhysicsSystem`

- `PhysicsSystem::new()` — construct.
- `init(...)` — configure max bodies, body mutexes, max body pairs, max contact
  constraints, and the broadphase/object-layer filter implementations.
- `update(delta_time: f32, collision_steps: i32, temp_allocator, job_system)` —
  **`unsafe`**; steps the sim. Takes raw pointers to a temp allocator and a job
  system that we must create and keep alive (via `joltc-sys`).
- `body_interface() -> BodyInterface`
- `narrow_phase_query() -> NarrowPhaseQuery`
- `optimize_broad_phase()`, `draw_bodies(...)` (`unsafe`), `as_raw()`.

Note: no safe `set_gravity` and no safe contact-listener registration are exposed
— both go through `as_raw()` + `joltc-sys`.

### `BodyInterface`

- `create_body(settings: &JPC_BodyCreationSettings) -> Body` — **`unsafe`**, and
  the settings type is the **raw `joltc-sys`** struct, which needs a valid raw
  `Shape` pointer. So *creating any body at all* requires building
  `JPC_BodyCreationSettings` and a shape by hand.
- `add_body(body_id, activation_mode: JPC_Activation)`, `remove_body(body_id)`,
  `destroy_body(body_id)`.
- `linear_velocity(body_id) -> Vec3`, `set_linear_velocity(body_id, Vec3)`.
- `is_active(body_id) -> bool`, `center_of_mass_position(body_id) -> RVec3`.
- `as_raw() -> *mut JPC_BodyInterface`.

**Gaps to route through `joltc-sys` raw calls:** angular velocity get/set,
position/rotation set, full transform read-back, and per-body activation control
beyond the `add_body` flag. Read-back of a body's orientation for the Bevy
`Transform` sync will need raw calls (or `center_of_mass_position` + raw rotation).

### `NarrowPhaseQuery`

- `cast_ray(...)` using `RRayCast` / `RayCastArgs` → `RayCastResult`, with
  `BodyFilter` to scope the query. This covers avian's ray `SpatialQuery` usage in
  the AI systems. **Shape casts** (used by `approach_and_attack_player_systems`)
  are not obviously in the safe API — expect a raw `joltc-sys` narrow-phase shape
  cast, or restructure those checks around ray casts.

### Layers & filtering

Object layers (`ObjectLayer`) and broadphase layers (`BroadPhaseLayer`) plus the
five filter traits/impls are the direct replacement for avian's
`CollisionLayers`/`LayerMask`. We implement the filter traits once to encode our
layer matrix (`ImpassableAll`, `Floor`, `Ball`, `Alien`, `Player`, sensors, etc.)
and hand the impls to `PhysicsSystem::init`.

### Impact of these findings on the plan

- The **Spike (phase 1)** is now the critical de-risking step: just getting one
  raw `JPC_BodyCreationSettings` + shape + stepped body syncing back to a
  `Transform` proves out the `unsafe` boundary, the temp-allocator/job-system
  lifetime, and the glam-version conversion in one go. Do not skip it.
- **Sensors / contact events (phase 5)** are the biggest unknown: with no safe
  `ContactListener`, we register a raw Jolt contact listener via `as_raw()` and
  marshal callbacks into a Bevy `Message` queue. This is the riskiest glue.
- **Character controller:** not in `rolt` at all; if we want Jolt's
  `CharacterVirtual`, it's raw `joltc-sys` or an upstream contribution. Leaning
  toward keeping our current kinematic movement rather than adopting it initially.
- Given the amount of `unsafe`/raw work, seriously weigh whether the maturity win
  justifies it versus staying on avian. A reasonable middle path: keep the spike
  as a throwaway branch to evaluate feel/perf before committing to the full port.
