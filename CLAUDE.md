# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Aliens vs Suburbia is a 3D tower defense game built with Rust and the Bevy game engine. Players defend against waves of aliens by building towers, throwing objects, and using special abilities. The game design document is at `docs/game-design-doc.md`.

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

```
Menu → PlayerSetup → InGame
Menu → AssetBrowser
Menu → MapEditor
Menu → ModelShowcase / CharacterCreator / PolyPizza
```

Most gameplay systems use `.run_if(in_state(InGame))`. Physics runs on a fixed timestep of 0.05s.

### Module Map

| Module | Responsibility |
|--------|---------------|
| `src/ai/` | Enemy AI behaviors: `ApproachAndAttackPlayer`, `AvoidWalls`, `MoveTowardsGoal`, `DestroyTheMap`. `recheck_path_after_tile_opened` clears stale destroy-behavior when a tile opens. |
| `src/alien/` | Alien spawning (wave-based via `WaveManager`). `wave_manager.rs` drives wave progression; waves can come from `MapFile.waves` or fall back to hardcoded defaults. |
| `src/player/` | Player character: physics, auto-aim, scene loading, outline rendering, death/revive, special abilities (`src/player/systems/abilities.rs`). |
| `src/towers/` | Tower entities: shooting, slow, area-damage sensors and cooldown systems. |
| `src/control/` | Input: keyboard (`keyboard_input.rs`), gamepad (`gamepad_input.rs`). `Q` key fires special ability via `AbilityInput` resource. |
| `src/building/` | Build mode: enter/exit, tile placement preview, tower construction. Checks `TeamWallet` for cost. |
| `src/map/` | Tile-based level: map generator, pathfinding grid (`MapGraph`), wall/floor/obstacle spawning. `map_loader` now also spawns editor `placements` from `MapFile`. |
| `src/general/` | Core mechanics: collision, `Health`/health bars, `TouchDamage`, `Indestructible`, `Coin`/`TeamWallet` economy, physics throws, lighting, kinematic movement, tile tracking. |
| `src/animation/` | State-machine animations. `AnimationStore` maps model keys to `AnimationGraph` handles. External animation sources supported via `animation_sources` in defs. |
| `src/ui/` | Menu, HUD (`spawn_ui.rs`). HUD shows: aliens, wave info, coins, build cost, ability cooldown. |
| `src/camera/` | Isometric camera tracking with wall occlusion fading. |
| `src/assets/` | `AssetDefinition` (`asset_definition.rs`) — the core per-model def type persisted to `assets/defs/*.ron`. |
| `src/asset_browser/` | In-engine tool for importing models: browse GLB files, set scale/height, toggle hidden nodes, map animation clips to game states, add external animation sources, set `ModelType`. Press `I` to export `.ron`. |
| `src/player_setup/` | `GameState::PlayerSetup` screen. Keyboard (Enter) and gamepad (South) to join slots, arrow keys / d-pad to pick model. Writes `PlayerRoster` resource. |
| `src/map_editor/` | `GameState::MapEditor`. Grid-based map layout tool. Palette sidebar filtered by `ModelType`. Left-click to place, right-click erase, `R` rotate, `S` save. Wave editor on right panel. |
| `src/model_settings/` | Live model hot-reload, `build_player_anim_graph` — builds the player animation graph, resolves `stem|clip` values against external GLTF sources. |
| `src/inspection/` | Dev tooling via `bevy-inspector-egui`. |

### AssetDefinition (the model def format)

Stored at `assets/defs/<model-stem>.ron`. Fields:
- `model_path` — path relative to `assets/` folder (e.g. `"packs/toon-shooter/characters/Soldier.glb"`)
- `scale` — uniform scale (computed as `target_height_m / mesh_aabb_height` in asset browser)
- `model_type` — `Player(PlayerProps)`, `Tower(TowerProps)`, `Terrain(TerrainProps)`, `Item(ItemProps)`, or `Enemy(EnemyProps)`
- `hidden_nodes` — node names to hide (e.g. weapon nodes)
- `animation_mapping` — `HashMap<game_state_key, clip_fragment>`. Values may be plain (`"idle"`) or `"SourceStem|ClipName"` for external sources.
- `animation_sources` — paths (relative to `assets/`) of external GLB/GLTF animation files. **No `assets/` prefix** — same convention as `model_path`.

### Economy

`TeamWallet` resource tracks shared coins. Aliens drop `Coin` entities on death. Players auto-collect within `PickupRange`. Tower placement costs are checked in `execute_build` (`src/building/systems.rs`).

### Special Abilities

`SpecialAbility` enum on players: `Bombardment`, `Healing`, `Whirlwind`, `GoldDigger`. Activated with `Q` key. Cooldowns via `AbilityCooldown` component. Assigned from `PlayerProps.ability` in the def, or cycled by slot index.

### Physics & Collision

Uses **avian3d 0.6**. Collision layers in `src/general/components/mod.rs`: `Impassable`, `Floor`, `Ball`, `Alien`, `Player`, `BuildIndicator`, `Sensor`, `PlayerAimSensor`, `AlienSpawnPoint`, `AlienGoal`.

### AI Pattern

Each behavior has its own submodule under `src/ai/`. When aliens can't find a path, `MustDestroyTheMap` is inserted. When a tile is re-opened (`path_reopened` flag on `MapGraph`), `recheck_path_after_tile_opened` clears destroy-behavior from all aliens if a normal path now exists.

### Bevy 0.18 Patterns

- **Messages not Events**: Custom event types derive `Message` and use `MessageReader`/`MessageWriter`. `add_message::<T>()` registers them.
- **`IntoScheduleConfigs`** must be explicitly imported when using `.run_if()`, `.after()`, `.before()`.
- **`ChildOf`** replaces `Parent` for parent traversal.
- **`despawn()`** recursively despawns children by default.
- **`Query::single()`** returns `Result` (replaces `get_single()`).
- **`PhysicsSystems::Writeback`** replaces `PhysicsSet::Sync` for camera ordering.
- **`AnimationTarget` / `AnimationGraph`**: clips are driven via UUID-based target IDs. External animation files only work if the target model shares identical bone paths (no automatic retargeting).
- **Font**: Bevy's default embedded font is ASCII-only. All UI text must use plain ASCII — no Unicode arrows, em-dashes, emoji, etc.

### Key Dependencies

- `bevy 0.18` — game engine
- `avian3d 0.6` — 3D physics (`parry-f32` feature required)
- `pathfinding 4.6.0` — A* grid navigation
- `bevy_mod_outline 0.12` — entity outlines
- `bevy-inspector-egui 0.36` — runtime debug inspector
- `lava_ui_builder` — local UI helper crate used throughout for panels and buttons
- `ron` — serialization for all `.ron` files

### Assets

- 3D models: `.glb`/`.gltf` files under `assets/packs/`
- Model definitions: `assets/defs/*.ron`
- Maps: `assets/maps/*.ron` (format: `MapFile` in `src/general/components/map_components.rs`)
- Settings: `game-settings.ron`, `player-settings.ron` at project root

### Known Gotchas

- **System order matters**: systems in a single `add_systems(Update, (...))` tuple run sequentially. If two systems share a dirty flag, the one that clears it must run after the one that reads it — or use separate flags (see `nodes_dirty` vs `nodes_ui_dirty` in `src/asset_browser/`).
- **Animation sources path format**: always relative to `assets/` with no prefix. `"packs/foo/bar.glb"` is correct; `"assets/packs/foo/bar.glb"` is wrong and will silently fail to load.
- **GLTF vs GLB**: both work. `.gltf` + `.bin` sidecar files load identically to `.glb` — keep them in the same folder.
