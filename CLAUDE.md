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
Menu → ModelShowcase / PolyPizza
```

Most gameplay systems use `.run_if(in_state(InGame))`. Physics runs on a fixed timestep of 0.05s.

### Module Map

| Module | Responsibility |
|--------|---------------|
| `src/ai/` | Enemy AI behaviors: `ApproachAndAttackPlayer`, `AvoidWalls`, `MoveTowardsGoal`, `DestroyTheMap`. `recheck_path_after_tile_opened` clears stale destroy-behavior when a tile opens. |
| `src/alien/` | Alien spawning (wave-based via `WaveManager`). `wave_manager.rs` drives wave progression; waves can come from `MapFile.waves` or fall back to hardcoded defaults. |
| `src/player/` | Player character: physics, auto-aim, scene loading, outline rendering, death/revive, special abilities (`src/player/systems/abilities.rs`), torso twist (`systems/torso_twist.rs`). |
| `src/towers/` | Tower entities: shooting, slow, area-damage sensors and cooldown systems. |
| `src/control/` | Input: keyboard (`keyboard_input.rs`), gamepad (`gamepad_input.rs`), mouse aim (`mouse_aim.rs`). `Q` key fires special ability via `AbilityInput` resource. **Both schemes are twin-stick and camera-relative**: WASD / the left stick walk in *world* space rotated by `GameSettings::yaw_degrees` (W or stick-up = up the screen, A/D strafe rather than rotate); the mouse / right stick sets `AutoAim`. Movement and facing are decoupled — `torso_twist::face_movement_direction` steers the body toward the direction of travel and the spine twists the rest of the way to the aim. Buttons are remappable via `GamepadBindings` (`bindings.rs`), loaded from `gamepad-bindings.ron` at the project root with the same `load`/`save` pattern as `GameSettings` — defaults are R2 fire, Square build mode, Cross place, Circle cancel, Triangle ability, d-pad to cycle the build item. Firing sets `ControlCommand::Throw` and the build buttons write the same messages the keyboard's B/Space/Escape/arrows do. With the right stick released, `auto_aim` (closest-in-FOV) takes over while firing. Pads are bound to players by `assign_gamepads` from `PlayerRoster::devices` (slot 0 = keyboard, slot N = pad N-1), with a fallback that hands a connected pad to the lone keyboard player when the setup screen was skipped. |
| `src/building/` | Build mode: enter/exit, tile placement preview, tower construction. Checks `TeamWallet` for cost. |
| `src/map/` | Tile-based level: map generator, pathfinding grid (`MapGraph`), wall/floor/obstacle spawning. `map_loader` now also spawns editor `placements` from `MapFile`. |
| `src/general/` | Core mechanics: collision, `Health`/health bars, `TouchDamage`, `Indestructible`, `Coin`/`TeamWallet` economy, physics throws, lighting, kinematic movement, tile tracking. |
| `src/animation/` | State-machine animations. `AnimationStore` maps model keys to `AnimationGraph` handles. External animation sources supported via `animation_sources` in defs. |
| `src/ui/` | Menu, HUD (`spawn_ui.rs`). HUD shows: aliens, wave info, coins, build cost, ability cooldown. |
| `src/music/` | Generative soundtrack via the `rusty_music` submodule (path dep). `GameMusicPlugin` spawns the band; `MusicMoods` holds two intensity measures (`combat`, `danger`) computed from game state, smoothed into the global `Intensity` and gating musician channels (`Ambient`/`Groove`/`Combat`/`Danger`) via `Muted` with hysteresis. Samples live in `assets/instruments/` (copied from `rusty_music/assets/samples/`). |
| `src/camera/` | Isometric camera tracking with wall occlusion fading. |
| `src/assets/` | `AssetDefinition` (`asset_definition.rs`) — the core per-model def type persisted to `assets/defs/*.ron`. `hardpoint.rs` — weapon-snap frame algebra. `gizmos.rs` — skeleton and hardpoint overlay drawing, shared by the asset browser and the playground. |
| `src/asset_browser/` | In-engine tool for importing models: browse GLB files, set scale/height, toggle hidden nodes, tag each animation clip with a free-form hierarchical path and bind game animation keys to those tag paths, add external animation sources, set `ModelType`, overlay the skinned skeleton (`B`), and attach weapon models to bones (sockets) with live numeric-nudge offset editing. Press `I` to export `.ron`. |
| `src/player_setup/` | `GameState::PlayerSetup` screen. Keyboard (Enter) and gamepad (South) to join slots, arrow keys / d-pad to pick model. Writes `PlayerRoster` resource. |
| `src/playground/` | Sandbox tweaking screen (`docs/playground.md`). **Not a `GameState`** — it runs in `GameState::InGame` with a `PlaygroundSession` resource inserted, so every gameplay system works there untouched. `state::in_playground` / `state::in_normal_game` gate the four things that differ: map loading, the HUD, the win/lose transition, and waves (`WaveManager::default()` ships hardcoded waves, so `waves: []` in the map is *not* enough — `silence_waves` clears them). Two panes: the left is UI, the right is the ordinary game camera with `Camera::viewport` clipped to the pane. `dummies.rs` keeps three static `Alien`s standing on `DummyPost`s that respawn them. `models.rs` lists imported player defs and swaps the live player between them via `PlayerRoster` (a two-frame despawn/respawn), plus a file browser that "imports" a `.glb` by writing a minimal def. `debug.rs` toggles physics/skeleton/hardpoint overlays, drawing via `assets::gizmos` (shared with the asset browser) — note it scopes joints to the player's subtree with `joints_under`, since the dummies are skinned too. `hardpoints.rs` edits hardpoints on the live model, on either side of the grip (`HardpointSide`): the character's land in `PlayerAssetDef`, the weapon's in `PlaygroundWeaponDef` (loaded from `PlayerProps::weapon` by `sync_weapon_def`, which keys on the *path* — keying on `PlayerAssetDef::is_changed()` would discard edits, since every nudge marks it changed). `keep_weapons_snapped` rebuilds the held weapon from `WeaponModel::char_grip`/`weapon_grip` every frame, so the setters are all that's needed for it to follow; the weapon's `muzzle` goes into `Weapon::muzzle`, where `shoot_weapons` takes the tracer origin from. Dirty state and Save are per side (two defs, two files); weapon frames are model-origin relative, so the bone picker is character-only. Nothing is written until Save. `animation.rs` plays animation keys on the live character and re-binds them to `clip_tags` paths (live — `build_player_anim_graph` keys its rebuild off `animation_bindings`). Camera/model sliders are the HUD's own F1/F2 panels, spawned rather than duplicated. `prefs.rs` persists the last model worn to `playground-prefs.ron` (gitignored) so a session resumes on the rig you were working on. `cargo run -- --playground` boots straight in. |
| `src/map_editor/` | `GameState::MapEditor`. Grid-based map layout tool. Palette sidebar filtered by `ModelType`. Left-click to place, right-click erase, `R` rotate, `S` save. Wave editor on right panel. |
| `src/model_settings/` | Live model hot-reload, `build_player_anim_graph` — builds the player animation graph, resolves `stem|clip` values against external GLTF sources. |
| `src/inspection/` | Dev tooling via `bevy-inspector-egui`. |

### AssetDefinition (the model def format)

Stored at `assets/defs/<model-stem>.ron`. Fields:
- `model_path` — path relative to `assets/` folder (e.g. `"packs/toon-shooter/characters/Soldier.glb"`)
- `scale` — uniform scale (computed as `target_height_m / mesh_aabb_height` in asset browser)
- `model_type` — `Player(PlayerProps)`, `Tower(TowerProps)`, `Terrain(TerrainProps)`, `Item(ItemProps)`, `Enemy(EnemyProps)`, or `Weapon(WeaponProps)`
- `hidden_nodes` — node names to hide (e.g. weapon nodes)
- `clip_tags` — `HashMap<clip_name, tag_path>`. Free-form hierarchical tag per clip (e.g. `"CharacterArmature|Run_Shoot" -> "Combat/Ranged/RunShoot"`). Clip name is the model's own clip, or `"SourceStem|ClipName"` for external sources.
- `animation_bindings` — `HashMap<game_state_key, tag_path>`. Binds a game key to a tag path; at runtime `AssetDefinition::resolved_clip` maps game key -> tag path -> the clip carrying that tag.
- `animation_mapping` — **legacy** `HashMap<game_state_key, clip_fragment>`. Superseded by `clip_tags` + `animation_bindings`; still read at runtime as a fallback and auto-migrated into tags/bindings when an old def is loaded in the asset browser.
- `animation_sources` — paths (relative to `assets/`) of external GLB/GLTF animation files. **No `assets/` prefix** — same convention as `model_path`.
- `attachments` — `Vec<Attachment>` of models socketed to bones (e.g. a held rifle). Each has `bone` (bone entity name), `model_path`, and a local offset (`translation`, `rotation_euler_deg`, `scale`). Manual, fixed props; authored/previewed in the asset browser only.
- `hardpoints` — `HashMap<String, Hardpoint>` of named connection frames (role -> frame) for dynamic weapon snapping, on both characters (`grip` anchored to a hand bone) and weapons (`grip`/`foregrip`/`stock`/`sight`, anchor `None` = model origin). `Hardpoint { anchor: Option<String>, translation, rotation_euler_deg }`. The snap math (`src/assets/hardpoint.rs`, unit-tested) makes a weapon's `grip` coincide with a character's `grip`. Authored + previewed in the asset browser (`H` toggles frame gizmos). Equipped in-game via `PlayerProps.weapon` (a weapon def path) — `src/player/systems/equip.rs` snaps it onto the character's `grip` bone once the skeleton spawns, using the same `hardpoint::snap_transform` as the browser preview. Two-bone IK for the support hand is a future stage. See `docs/inverse-kinematics-hardpoints.md`.

### Economy

`TeamWallet` resource tracks shared coins. Aliens drop `Coin` entities on death. Players auto-collect within `PickupRange`. Tower placement costs are checked in `execute_build` (`src/building/systems.rs`).

### Special Abilities

`SpecialAbility` enum on players: `Bombardment`, `Healing`, `Whirlwind`, `GoldDigger`. Activated with `Q` key. Cooldowns via `AbilityCooldown` component. Assigned from `PlayerProps.ability` in the def, or cycled by slot index.

### Torso Twist (aim offset)

`src/player/systems/torso_twist.rs` — the legs face the direction of travel while the
upper body (and the weapon parented to the grip bone) faces `AutoAim`. This is **forward**
kinematics, not IK: a per-spine-bone rotation, no solver. Two invariants keep it working:

- **Rotate about world up, not the bone's own axis.** `twisted_local` conjugates the world
  yaw by the parent's world rotation and pre-multiplies the *animated local* pose. Working
  from the bone's `GlobalTransform` instead would reuse last frame's already-twisted pose
  and wind the torso further every frame.
- **Ordering.** `apply_torso_twist` runs in `PostUpdate`, `.after(AnimationSystems)` and
  `.before(TransformSystems::Propagate)` — earlier and `animate_targets` stomps it, later
  and it never propagates.

The chain comes from `AssetDefinition.aim_bones` (bone name + weight), defaulting to the
mixamo `Spine`/`Spine1`/`Spine2` chain with rising weights. Twist is clamped to
`TWIST_LIMIT_DEGREES` (60); past that the body turns to absorb the excess. **F7 toggles it**
at runtime for A/B comparison.

### Physics & Collision

Uses **avian3d 0.7**. Collision layers in `src/general/components/mod.rs`: `ImpassableAll`, `Floor`, `Ball`, `Alien`, `Player`, `BuildIndicator`, `Sensor`, `PlayerAimSensor`, `AlienSpawnPoint`, `AlienGoal`.

### AI Pattern

Each behavior has its own submodule under `src/ai/`. When aliens can't find a path, `MustDestroyTheMap` is inserted. When a tile is re-opened (`path_reopened` flag on `MapGraph`), `recheck_path_after_tile_opened` clears destroy-behavior from all aliens if a normal path now exists.

### Bevy 0.19 Patterns

- **Messages not Events**: Custom event types derive `Message` and use `MessageReader`/`MessageWriter`. `add_message::<T>()` registers them.
- **`IntoScheduleConfigs`** must be explicitly imported when using `.run_if()`, `.after()`, `.before()`.
- **`ChildOf`** replaces `Parent` for parent traversal.
- **`despawn()`** recursively despawns children by default.
- **`Query::single()`** returns `Result` (replaces `get_single()`).
- **`PhysicsSystems::Writeback`** replaces `PhysicsSet::Sync` for camera ordering.
- **`AnimationTarget` / `AnimationGraph`**: clips are driven via UUID-based target IDs. External animation files only work if the target model shares identical bone paths (no automatic retargeting).
- **Font**: Bevy's default embedded font is ASCII-only. All UI text must use plain ASCII — no Unicode arrows, em-dashes, emoji, etc.

### Key Dependencies

- `bevy 0.19` — game engine
- `avian3d 0.7` — 3D physics (`parry-f32` feature required)
- `pathfinding 4.6.0` — A* grid navigation
- `bevy_mod_outline 0.13` — entity outlines
- `bevy-inspector-egui 0.37` — runtime debug inspector
- `lava_ui_builder` — local UI helper crate used throughout for panels and buttons
- `rusty_music` — local submodule (path dep, excluded from the workspace): generative music plugin built on `bevy_seedling 0.8` (pinned to a git rev; no 0.19 crates.io release yet)
- `ron` — serialization for all `.ron` files

### Assets

- 3D models: `.glb`/`.gltf` files under `assets/packs/`
- Model definitions: `assets/defs/*.ron`
- Maps: `assets/maps/*.ron` (format: `MapFile` in `src/general/components/map_components.rs`)
- Settings: `game-settings.ron`, `player-settings.ron`, `gamepad-bindings.ron` at project root

### Known Gotchas

- **System order matters**: systems in a single `add_systems(Update, (...))` tuple run sequentially. If two systems share a dirty flag, the one that clears it must run after the one that reads it — or use separate flags (see `nodes_dirty` vs `nodes_ui_dirty` in `src/asset_browser/`).
- **Animation sources path format**: always relative to `assets/` with no prefix. `"packs/foo/bar.glb"` is correct; `"assets/packs/foo/bar.glb"` is wrong and will silently fail to load.
- **GLTF vs GLB**: both work. `.gltf` + `.bin` sidecar files load identically to `.glb` — keep them in the same folder.
- **WAV loading**: `bevy_seedling`'s symphonia-based loader rejects some wavs with `malformed fmt_pcm chunk` even though other tools play them fine. Fix by re-encoding: `ffmpeg -i in.wav -c:a pcm_s16le out.wav` (this happened with `pluck.wav`).
