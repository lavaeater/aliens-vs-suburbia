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

Uses **bevy_xpbd_3d** with collision layers defined in `src/general/`: `Impassable`, `Floor`, `Ball`, `Alien`, `Player`.

### AI Pattern

Big Brain actions/scorers are split into `components/`, `systems/`, and action structs per behavior. Each behavior (e.g., `ApproachAndAttackPlayer`, `AvoidWalls`) has its own submodule under `src/ai/`.

### Key Dependencies

- `bevy 0.12.1` — game engine (current branch `bevy@0.16` is an in-progress upgrade)
- `bevy_xpbd_3d 0.3.2` — 3D physics
- `big-brain 0.12.0` — utility AI
- `pathfinding 4.6.0` — A* grid navigation
- `bevy_mod_outline` — entity outlines
- `bevy-inspector-egui` — runtime debug inspector (toggle via commented-out `WorldInspectorPlugin` in `main.rs`)

### Assets

3D models are `.glb` files under `assets/`. Characters are in `assets/girl/` (player) and `assets/quaternius/` (alien). Map tiles are in `assets/map/`.
