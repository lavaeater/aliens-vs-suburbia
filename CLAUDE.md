# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Aliens vs Suburbia is a 3D tower defense game built with Rust and the Bevy game engine. Players defend against waves of aliens by building towers, throwing objects, and using special abilities. The game design document is at `docs/not-right-now/game-design-doc.md`.

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
| `src/control/` | Input via **leafwing-input-manager** (0.21). `bindings.rs`: the `PlayerAction` enum (`Actionlike`; four move buttons, fire, build, weapons, ability, pause, ..., plus `Move`/`Aim` dual axes for the sticks) and `ControlBindings`, loaded from `input-bindings.ron` at the project root (same `load`/`save` pattern as `GameSettings`; actions missing from the file fill in from the defaults): two key/mouse slots and one pad button per action. Defaults: LMB/Space fire, B build mode, LMB/Space place, Esc/RMB cancel, Q ability, Esc pause; pad R2 fire, Square build, Cross place, Circle cancel, Triangle ability, Start pause. `player_input.rs`: `sync_input_maps` gives each player an `InputMap` for its device (`keyboard_map`, or `gamepad_map(pad)` tied to its pad; tracked by `ControlScheme`) and rebuilds them when the bindings change; `gate_player_input` disables `ActionState`s while paused or under a `GameInputBlock` (`input_gate`, pure: a panel under the pointer disables only `Fire`); `player_input` is the one path for both schemes and writes `CharacterControl`, `AutoAim` (right stick) and the build/weapon/ability messages. The chain runs in `PreUpdate` after `InputManagerSystem::ManualControl`. **Both schemes are twin-stick and camera-relative**: move keys / the left stick walk in *world* space rotated by `GameSettings::yaw_degrees`; the mouse (`mouse_aim.rs`) / right stick sets `AutoAim`. Movement and facing are decoupled (`torso_twist::face_movement_direction`). Screens with no player entity (pause menu, respawn picker) check devices through `ControlBindings::kbm_just_pressed`/`pad_just_pressed`. Pads are bound to players by `assign_gamepads` (`gamepad_input.rs`) from `PlayerRoster::devices` (slot 0 = keyboard, slot N = pad N-1), with a fallback that hands a connected pad to the lone keyboard player when the setup screen was skipped. |
| `src/building/` | Build mode: enter/exit, tile placement preview, tower construction. Checks `TeamWallet` for cost. |
| `src/map/` | Tile-based level: map generator, pathfinding grid (`MapGraph`), wall/floor/obstacle spawning. `map_loader` now also spawns editor `placements` from `MapFile`. |
| `src/general/` | Core mechanics: collision, `Health`/health bars, `TouchDamage`, `Indestructible`, `Coin`/`TeamWallet` economy, physics throws, lighting, kinematic movement, tile tracking. |
| `src/animation/` | State-machine animations. `AnimationStore` maps model keys to `AnimationGraph` handles. External animation sources supported via `animation_sources` in defs. `clips.rs` resolves key -> clip (see `animations` under *AssetDefinition*). Intents go on `CharacterState`'s stack (`src/control/components.rs`): holding fire with a gun pushes `IdleShoot` (`shoot::aim_animation`), which resolves to `IdleShoot`/`WalkShoot`. A rig with an aim pose but no walk-and-aim clip (Mesh2Motion) plays `WalkShoot` as two masked nodes, the walk on the legs and the aim above the first spine bone (`ClipSet::upper_body_layer`, `body_mask.rs`; `AnimationStore` maps a key to `KeyNodes`, play/stop it with `AnimationStore::play`/`stop`); `Throwing` (ball, grenade) is a one-shot left by `end_finished_throws` when its clip ends. `PreviewAnimation` plays a key outright, bypassing the stack (the editor's preview). |
| `src/ui/` | Menu, HUD (`spawn_ui.rs`). HUD shows: aliens, wave info, coins, build cost, ability cooldown. `pause_menu.rs`: the pause binding opens it in a normal match (not the editor). `PauseState` (a sub-state of `InGame`, in `game_state/mod.rs`) `Paused` pauses `Time<Virtual>` and closes all player input; pages Main (Resume / Controls / Quit to menu) and Controls (click a cell, press the input; Delete clears, Esc cancels; saves `input-bindings.ron` on every change). |
| `src/music/` | Generative soundtrack via the `rusty_music` submodule (path dep, `synth` feature). `GameMusicPlugin` adds `MusicPlugin`/`SituationPlugin`/`MixPlugin`/`SynthPlugin` and spawns an improvising synth band (`rusty_music::improv`: acid `WalkingBass`, `VoiceLedPad`, chip `ChimeArp`, `GrooveDrummer` kit pumping the pads, `MotifSoloist` lead, a glicol theremin `assets/synths/theremin.glicol`) plus the sampled surf `ChordStabs` (`players.rs`). The game only *describes* the situation: `describe_game` / `describe_menu` write `MusicState::target` (intensity/tension/valence/energy); layers enter via `Layer`, tempo via `TempoFollows` (energy), mode via valence, mix via `MixScenes` (`calm`/`siege`/`dread`). `game_stingers` fires quantised `Stinger`s (kills, pickups, downed players, new wave, win/lose). Spawn points hum with `Proximity`, heard from a `MusicListener` that follows `CameraFocus`. `MusicMoods::danger` is a smoothed copy kept for `gore::despair`. Samples live in `assets/instruments/`. Integration guide: `rusty_music/.claude/skills/rusty-music/SKILL.md`. |
| `src/camera/` | Isometric camera tracking with wall occlusion fading. |
| `src/items/` | `Item(ItemKind)` + `Pickup` on the ground: med-kits, ammo, guns, coins, keys. `SpawnItem` builds a visual (guns use their def's model); `pickup_items` applies the effect (`apply_pickup`, pure) and emits `ItemPickedUp` for the HUD toast. Coins are items with a `Coin` marker for GoldDigger. |
| `src/loot/` | Weighted drop tables from `assets/loot/*.ron` (`Nothing` / `Item` / nested `Table`, `always` entries). Anything with `LootDrop(name)` rolls its table the frame its health hits zero (`spawn_loot_on_death`, ordered before the despawn systems). |
| `src/general/damage.rs` | The one damage sink - see *Damage pipeline* below. |
| `src/general/explosion.rs` | `Explode` message -> radial `ApplyDamage` with quadratic falloff, wall shielding, velocity shove, flash/debris, optional fire, `CameraShake`. `ExplodesOnDeath` for barrels. |
| `src/general/projectiles.rs` | Physics projectiles for grenades, molotovs and launcher guns: one `Projectile` with an `Impact` (Damage / Explode / Fire) and optional fuse. `throw_special` (G / L1) lobs from the `AmmoPouch`. |
| `src/player/ammo.rs`, `src/player/systems/loadout.rs` | `AmmoPouch` (per `AmmoKind`, capped) and `Weapons` (carried guns; switching despawns the held gun and queues a fresh `PendingEquip`, magazine carried over). |
| `src/player/systems/death_revive.rs` | Downed -> revive (Interact, E / Circle) or bleed out -> drop kit, spend a life, `RespawnQueue` -> `SpawnPlayer { slot, lives }`. Team wipe fails the level. |
| `src/alien/enemy_defs.rs` | Def-driven enemies: `EnemyDefCache`, per-def animation graphs (`build_graph` shared with the player), `RangedAttack`. Waves name the def (`WaveDef.enemy_def`); a wave without one spawns the built-in quaternius alien. |
| `src/assets/` | `AssetDefinition` (`asset_definition.rs`) — the core per-model def type persisted to `assets/defs/*.ron`. `hardpoint.rs` — weapon-snap frame algebra. `gizmos.rs` — skeleton and hardpoint overlay drawing, shared by the asset browser and the character editor. `gltf_info.rs` — reads a `.glb`'s JSON directly (mesh/clip/bone counts, bind pose) without `AssetServer`. |
| `src/asset_browser/` | In-engine tool for importing models: browse GLB files, set scale/height, toggle hidden nodes, list the clips (read-only, with the keys the def maps to each; mapping is done in the character editor), add external animation sources, set `ModelType`, overlay the skinned skeleton (`B`), and attach weapon models to bones (sockets) with live numeric-nudge offset editing. Press `I` to export `.ron`. |
| `src/player_setup/` | `GameState::PlayerSetup` screen. Keyboard (Enter) and gamepad (South) to join slots, arrow keys / d-pad to pick model. Writes `PlayerRoster` resource. |
| `src/character_editor/` | The **Character Editor** (`docs/character-editor.md` - the spec and plan; `docs/archive/playground.md` - its history as the playground). Replacing both the playground and, eventually, the asset browser. **Not a `GameState`** — it runs in `GameState::InGame` with an `EditorSession` resource inserted, so every gameplay system works there untouched. `state::in_editor` / `state::in_normal_game` gate the four things that differ: map loading, the HUD, the win/lose transition, and waves (`WaveManager::default()` ships hardcoded waves, so `waves: []` in the map is *not* enough — `silence_waves` clears them). The arena (`assets/maps/playground.ron`) is empty: no enemies. Layout (`shell.rs`): top bar (doc bar), a tabbed left pane (`ActiveTab`/`TabBody`, each tab one scroll container — no nested scrolling lists), the game viewport (the ordinary game camera with `Camera::viewport` clipped to `EditorViewportPane`, debug toggles on a toolbar floating over it), and a status bar. `focus.rs` writes `control::components::GameInputBlock` (only present in the editor) from the hover map: pointer over any panel freezes mouse aim, inspecting or `EditorInputFocus::text_capture` also blocks key presses (releases always pass, so nothing sticks); gamepads are never blocked. **`doc.rs` is the core: every panel edits `EditorDoc` (the character), `WeaponDoc` (its held gun, from `PlayerProps::weapon` or a weapon opened from the library, see `EditorFocus::held_weapon` / `expected_live`) or `PropDoc` (an enemy/tower/terrain/item opened on its own), never `PlayerAssetDef` directly.** `EditorFocus::subject` is what is open; actions go through the `Docs` system param. Studio = the orbit camera (top-bar switch or F11); a prop forces it and is spawned by `prop.rs` with the character hidden. Dirty is computed (`def != saved`), one Save writes every dirty doc to the path it was opened from, Revert restores them, and anything that would drop edits (open another model, equip a different gun, leave) goes through `doc::request`, which asks first via the doc bar. `sync_doc_to_live` copies the doc into `PlayerAssetDef` (hardpoints, animations, sources apply live) or, for spawn-time fields (`needs_respawn`: model, scale, type/props incl. the weapon, hidden nodes, attachments, aim bones), respawns from the doc through `PlayerRoster::def_overrides` so the respawn shows the unsaved edits. `models.rs` (`EditorLibrary`) holds the library's state and does the two-frame despawn/respawn (`swap_player_model`; `swap_in_progress` covers queued -> despawned -> `awaiting_spawn`). `library.rs` is the Library tab: one list from `library_rows` (defs grouped by `DefKind` + files with a `FileState`), a search box (`text_field.rs`; focus sets `EditorInputFocus::text_capture`, Up/Down/Enter arrive as `FieldKey`), type chips, preview-before-import (`DefDoc::preview` has no path and is never dirty) and `Docs::import_as`, which never overwrites an existing def. `debug.rs` (`EditorOverlays`) toggles physics/skeleton/hardpoint overlays via `assets::gizmos`, scoped to the player's subtree with `joints_under`. `hardpoints.rs` edits either side of the grip (`HardpointSide`); frames are stored relative to their anchor but nudged along `NudgeSpace::Model` (the model root's axes: X left, Y up, Z forward, via `ActiveFrame`, recomputed each frame by `track_active_hardpoint`) or the anchor bone's own, with `draw_nudge_axes` drawing arrows for whichever is active; `suggest_hardpoints.rs` guesses every role from the files (`ModelShape`: a character's bind pose via `gltf_info::bind_pose_world`, Mesh2Motion's prop rule; a weapon's mesh via `gltf_info::mesh_points`, assuming barrel +X / up +Y), offered as "suggest" rows in the panel; `keep_weapons_snapped` rebuilds the held weapon from `WeaponModel::char_grip`/`weapon_grip` every frame, and the apply systems also fire on `Added<EquippedWeapon>`, because an equip resolves frames from the files on disk. The weapon's `muzzle` goes into `Weapon::muzzle`, where `shoot_weapons` takes the tracer origin from; weapon frames are model-origin relative, so the bone picker is character-only. Tabs (each edits whatever is open via `SideDocs::subject_mut`): `model_tab.rs` (file info, scale + set-height from the measured model, hidden nodes; a character's def scale is not read in game, F2 sizes players), `animation_tab.rs` (one row per `AnimationKey` showing the clip it resolves to and why - def / rig preset / by name / fallback; picking a key previews it on a character via `PreviewAnimation`, which bypasses `CharacterState`, and lists every clip to map it to), `props_tab.rs` (type with confirm; a character's torso-twist chain via `aim_bones.rs`, which can suggest one from a non-mixamo rig's spine bones; editable stats via the tested `fields`/`set_number`/`set_toggle`/`set_choice` table; nested data left to the `.ron`). `animation.rs` holds the pure helpers (`clip_set`, `map`/`unmap`, `resolution_label`). Camera/model sliders are the HUD's own F1/F2 panels, spawned rather than duplicated. `prefs.rs` persists the last model worn, the last tab and Studio/Live to `editor-prefs.ron` (always via `EditorPrefs::update`, which keeps the other fields) (gitignored; falls back to reading the old `playground-prefs.ron`). `inspect.rs` is the hardpoint-placing mode: F11 swaps in a Blender-style orbit camera (MMB orbit, Shift pan, Ctrl/wheel zoom, F frame the selected hardpoint; `camera_follow` is gated off by `orbit_camera_active`), F12 holds the **bind pose** read from the skin's inverse bind matrices (`assets::gltf_info::rest_pose` - not the node transforms, which on `amy.glb` hold an animated frame), written after the twist/IK chain so nothing else needs gating. Either one freezes the player's movement and aim. `cargo run -- --editor` (or `--playground`) boots straight in. |
| `src/map_editor/` | `GameState::MapEditor`. Grid-based map layout tool. Palette sidebar filtered by `ModelType`. Left-click to place, right-click erase, `R` rotate, `S` save. Wave editor on right panel. |
| `src/model_settings/` | Live model hot-reload, `build_player_anim_graph` — builds the player animation graph, resolves `stem|clip` values against external GLTF sources. |
| `src/inspection/` | Dev tooling via `bevy-inspector-egui`. |

### AssetDefinition (the model def format)

Stored at `assets/defs/<model-stem>.ron`. Fields:
- `model_path` — path relative to `assets/` folder (e.g. `"packs/toon-shooter/characters/Soldier.glb"`)
- `scale` — uniform scale (computed as `target_height_m / mesh_aabb_height` in asset browser)
- `model_type` — `Player(PlayerProps)`, `Tower(TowerProps)`, `Terrain(TerrainProps)`, `Item(ItemProps)`, `Enemy(EnemyProps)`, or `Weapon(WeaponProps)`
- `hidden_nodes` — node names to hide (e.g. weapon nodes)
- `animations` — `BTreeMap<AnimationKey, clip_name>` (`{ Idle: "Idle_A", Run: "Sprint" }`). Clip name is the model's own clip, or `"SourceStem|ClipName"` for external sources. Only list what differs from the default: `animation::clips::ClipSet::resolve` (shared by `build_graph` and the editor) resolves each key as def entry -> Mesh2Motion rig preset (`Rig`, detected from the clip names; human/fox/horse/spider/kaiju/snake/shark/bird/dragon, names from `mesh2motion-app/static/animations/`) -> the global `player-settings.ron` `anim_mapping` (players only, deliberately below the preset) -> name search on `default_search` -> `AnimationKey::fallback` (WalkShoot -> Walk, Throwing -> Punch, ...). The old `clip_tags` / `animation_bindings` / `animation_mapping` are gone; `every_def_in_assets_parses` fails on a def that still has them (serde would silently skip them).
- `animation_sources` — paths (relative to `assets/`) of external GLB/GLTF animation files. **No `assets/` prefix** — same convention as `model_path`.
- `attachments` — `Vec<Attachment>` of models socketed to bones (e.g. a held rifle). Each has `bone` (bone entity name), `model_path`, and a local offset (`translation`, `rotation_euler_deg`, `scale`). Manual, fixed props; authored/previewed in the asset browser only.
- `hardpoints` — `HashMap<String, Hardpoint>` of named connection frames (role -> frame) for dynamic weapon snapping, on both characters (`grip` anchored to a hand bone) and weapons (`grip`/`foregrip`/`stock`/`sight`, anchor `None` = model origin). `Hardpoint { anchor: Option<String>, translation, rotation_euler_deg }`. The snap math (`src/assets/hardpoint.rs`, unit-tested) makes a weapon's `grip` coincide with a character's `grip`. Authored + previewed in the asset browser (`H` toggles frame gizmos). Equipped in-game via `PlayerProps.weapon` (a weapon def path) — `src/player/systems/equip.rs` snaps it onto the character's `grip` bone once the skeleton spawns, using the same `hardpoint::snap_transform` as the browser preview. Two-bone IK for the support hand is a future stage. See `docs/not-right-now/inverse-kinematics-hardpoints.md`.

### Damage pipeline

`src/general/damage.rs`. **Only `apply_damage` lowers `Health`.** Anything that hurts
something writes an `ApplyDamage { target, amount, kind, position, normal, source }` message
(builder: `ApplyDamage::at(..).from(source).along(normal)`). `apply_damage` checks
`Indestructible`, the `DamageRules` resource against the `Faction` of source and target
(`Player`/`Alien`/`Structure`; no faction = always allowed), scales by `DamageResistances`
(from `resistances` in Enemy/Tower/Terrain props), then emits `DamageDealt` (gore),
`ShotHit`/`AlienKilled` (score, credited to `source`) and decrements `AlienCounter` once per
kill. It runs in `GamePlugin`'s `Update` tuple before the coin/death systems. Healing edits
`Health` directly (`Health::heal`).

### Multiplayer camera

`camera_follow` aims at the weighted centroid of every `CameraTarget` (each player gets one
in `spawn_players`), smooths it into the `CameraFocus` resource, and zooms out
(`fit_factor`) until all targets fit. Tunables: `GameSettings::{fit_margin, fit_zoom_max,
focus_smoothing}`. Players carry `PlayerSlot(usize)`; the bottom HUD bar
(`src/ui/player_hud.rs`) keys on it.

### Roadmap

`docs/gameplay.md` lists the MVP features; `docs/roadmap.md` tracks each one with status,
file references and what is left. Phases 1-6 landed 2026-09-20 **without a runtime playtest**
- expect tuning (camera fit, tower ranges, enemy stats, Shotgun/SMG hardpoints).

### Economy

`TeamWallet` resource tracks shared coins. Aliens drop `Coin` entities on death. Players auto-collect within `PickupRange`. Tower placement costs are checked in `execute_build` (`src/building/systems.rs`).

### Special Abilities

`SpecialAbility` enum on players: `Bombardment`, `Healing`, `Whirlwind`, `GoldDigger`. Activated with the `Ability` action (Q / Triangle). Cooldowns via `AbilityCooldown` component. Assigned from `PlayerProps.ability` in the def, or cycled by slot index.

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

**Strides.** While walking, `choose_stride` picks a `Stride` (`control::components`:
Forward / Back / Left / Right) from which quarter around the walk direction the aim falls in
(`pick_stride`, 15 degree hysteresis), and `face_movement_direction` turns the body to
`stride_facing` so the feet keep travelling along the walk direction while the twist covers
the rest (at most ~50 degrees with all four). `CharacterState.stride` turns `Walk` into
`WalkBack` / `StrafeLeft` / `StrafeRight` (and their `*Shoot` aim-layered variants); it is
set through the `SetStride` message (`stride_handler`). Strafes are used only when the rig has
both clips; Back always is: with no `WalkBack` clip, `AnimationStore::nodes`/`speed` play
`Walk` in reverse (`AnimationKey::reversed_from`).

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
- `leafwing-input-manager 0.21` — input actions and maps (0.22 is the Bevy 0.20 release; see `docs/bevy-0.20-upgrade.md`)
- `lava_ui_builder` — local UI helper crate used throughout for panels and buttons
- `rusty_music` — local submodule (path dep, excluded from the workspace): generative music plugin built on `bevy_seedling 0.8` (pinned to a git rev; no 0.19 crates.io release yet)
- `ron` — serialization for all `.ron` files

### Assets

- 3D models: `.glb`/`.gltf` files under `assets/packs/`
- Model definitions: `assets/defs/*.ron`
- Maps: `assets/maps/*.ron` (format: `MapFile` in `src/general/components/map_components.rs`)
- Text maps: `assets/maps/*.map`, a hand-editable letter-per-tile companion to the RON format (`src/map/text_format.rs`, `docs/map-text-format.md`). Lossless both ways; `--convert-map <in> [<out>]` converts; a `.map` next to a level's `.ron` replaces it when the game loads that level (`text_format::load_level`).
- Settings: `game-settings.ron`, `player-settings.ron`, `input-bindings.ron` at project root

### Known Gotchas

- **System order matters**: systems in a single `add_systems(Update, (...))` tuple run sequentially. If two systems share a dirty flag, the one that clears it must run after the one that reads it — or use separate flags (see `nodes_dirty` vs `nodes_ui_dirty` in `src/asset_browser/`).
- **Animation sources path format**: always relative to `assets/` with no prefix. `"packs/foo/bar.glb"` is correct; `"assets/packs/foo/bar.glb"` is wrong and will silently fail to load.
- **GLTF vs GLB**: both work. `.gltf` + `.bin` sidecar files load identically to `.glb` — keep them in the same folder.
- **WAV loading**: `bevy_seedling`'s symphonia-based loader rejects some wavs with `malformed fmt_pcm chunk` even though other tools play them fine. Fix by re-encoding: `ffmpeg -i in.wav -c:a pcm_s16le out.wav` (this happened with `pluck.wav`).
