# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Aliens vs Suburbia is a 3D tower defense game built with Rust and the Bevy game engine. The player defends against alien invaders by building towers and throwing objects.

## Commands

```bash
# Run the game (dev mode — optimizes dependencies but not game code)
cargo run

# Release build
cargo run --release

# Check for compile errors without building
cargo check

# Run tests
cargo test
```

Dev builds use `opt-level = 3` for all dependencies (configured in `Cargo.toml`) for acceptable frame rates without a full release build.

## Architecture

The game follows Bevy's **plugin-based ECS architecture**. Each subsystem lives in its own module and registers its components, systems, and events via a `Plugin` impl. `GamePlugin` (in `src/game_state/game_state_plugin.rs`) is the root plugin that composes everything.

### Game States

Two states: `Menu` and `InGame`. Most gameplay systems use `.run_if(in_state(InGame))`. Physics runs on a fixed timestep of 0.05s.

### Module Map

| Module | Responsibility |
|--------|---------------|
| `src/ai/` | Enemy decision-making via the **Big Brain** library (scorers + actions pattern). Behaviors: approach/attack player, avoid walls, destroy blocked tiles, move forward. |
| `src/alien/` | Alien spawning (timer-based, max 50 aliens) |
| `src/player/` | Player character: physics, auto-aim, scene loading, outline rendering |
| `src/towers/` | Tower entities: shooting, range sensors, cooldown, Big Brain action selection |
| `src/control/` | Input handling for both keyboard and gamepad |
| `src/building/` | Build mode: enter/exit, tile placement preview, tower construction events |
| `src/map/` | Tile-based level: walls, floors, obstacles, towers; pathfinding grid for AI |
| `src/general/` | Core mechanics: collision, health/health bars, physics throws, lighting, kinematic movement, tile tracking |
| `src/animation/` | State-machine animations (Idle, Walking, Throwing, Crawling, Building) loaded from GLB clips |
| `src/ui/` | Menu screen and in-game HUD |
| `src/camera/` | Isometric camera tracking |
| `src/assets/` | Coordinated asset loading |
| `src/inspection/` | Dev tooling via `bevy-inspector-egui` |

### Physics & Collision

Uses **avian3d 0.6** (successor to `bevy_xpbd_3d`) with collision layers defined in `src/general/components/mod.rs`: `Impassable`, `Floor`, `Ball`, `Alien`, `Player`, `BuildIndicator`, `Sensor`, `PlayerAimSensor`, `AlienSpawnPoint`, `AlienGoal`.

### AI Pattern

AI behaviors are implemented directly without an external library. Each behavior (e.g., `ApproachAndAttackPlayer`, `AvoidWalls`, `MoveTowardsGoal`, `DestroyTheMap`) has its own submodule under `src/ai/` split into `components/` and `systems/`.

### Bevy 0.18 Patterns

Key API changes active in this codebase:
- **Messages not Events**: Custom event types derive `Message` (not `Event`) and use `MessageReader`/`MessageWriter`. `add_message::<T>()` registers them. Bevy built-in events (e.g. `KeyboardInput`, `GamepadConnectionEvent`) are also `Message`.
- **`IntoScheduleConfigs`** must be explicitly imported when using `.run_if()`, `.after()`, `.before()` — it is in `bevy::prelude` but requires a specific import.
- **`ChildOf`** replaces `Parent` for parent traversal. `.parent()` returns the parent entity.
- **`despawn()`** recursively despawns children by default (no more `despawn_recursive()`).
- **`Query::single()`** replaces `get_single()` (returns `Result`).
- **`PhysicsSystems::Writeback`** replaces `PhysicsSet::Sync` for ordering camera after physics.

### Key Dependencies

- `bevy 0.18` — game engine
- `avian3d 0.6` — 3D physics (requires `parry-f32` feature)
- `pathfinding 4.6.0` — A* grid navigation
- `bevy_mod_outline 0.12` — entity outlines
- `bevy-inspector-egui 0.36` — runtime debug inspector (active via `InspectorPlugin`)

### Assets

3D models are `.glb` files under `assets/`. Characters are in `assets/girl/` (player) and `assets/quaternius/` (alien). Map tiles are in `assets/map/`.
