# Aliens vs Suburbia

https://otter.ai/u/TDfrlhEGTGLG7raPBGnriEIROBs?view=summary

## Overview

Speaker 1 outlines a game concept called "Aliens vs Suburbia," which is a reimagining of South Park Tower Defense with aliens moving from start points to end points, while players construct barriers to block the aliens. Players can revive each other if they die, but if all players die simultaneously, the game ends. Aliens can destroy barriers if no path is available. The first map could be a picnic area with various features like sand pits or carousels. Additionally, Speaker 1 discusses the need for an asset browser with a file explorer interface, an import function to create run files with settings, and the ability to edit and map animation keys to game states, with potential future updates to include color and material editing.

### Action Items

- [ ] Implement the first basic map where aliens move from start points to end points
- [ ] players build barriers that aliens cannot pass but players can, 
- [ ] Players can build towers of different whacky kinds that shoot at aliens, slow them down, etc.
- [ ] A meter tracks the percentage of aliens that have passed through.
- [ ] Implement a revive mechanic where dead players can be revived by other players.
- [ ] Implement a game-over condition that triggers when all players die simultaneously.
- [ ] Implement alien behavior where aliens destroy barriers when no open route is available from start points to end points.
- [ ] Add a file explorer–style interface in the asset browser that lets users browse folders and view their contents without recursively scanning all folders.
- [ ] Implement an import function in the asset browser that creates a Ron file with settings such as named nodes in the model and the names of the animations. Also contains path to actual model file.
- [ ] This Ron file is used as a resource for using the model in the game.
- [ ] Add editing capabilities in the asset browser so imported assets can be modified, including hiding named nodes (for example, guns in the Toon Shooter case) and mapping animation keys or animation names to game states.
- [ ] Plan for future support in the asset browser to allow editing of colors, materials, and special shaders on imported assets.

## Outline

### Game Mechanics and Initial Map Design

Speaker 1 discusses the concept of "Aliens vs Suburbia," suggesting a base game similar to South Park, where aliens move from start points to end points.
Players construct barriers that monsters cannot pass, but players can, and a meter indicates the percentage of aliens that have passed through.
If the meter reaches zero, the game is over; if players die, they can be revived by other players, but if all players die simultaneously, the game ends.
Aliens destroy barriers if no open route is available from start points to end points.
The first map could be named "picnic area," with spots like sand pits or carousels, and players could throw sand to evolve the game.

### Asset Browser and Import Function

Speaker 1 introduces a new section regarding the asset browser, suggesting a file explorer-type interface to browse through folders and show their contents.
The import function should create a run file with settings named nodes in the model, including the names of animations and other details.
Players should be able to edit these settings, such as hiding named nodes, like guns in the Tomb Shooter case.
The ability to map animation keys or names to game states is proposed.
Future enhancements could include editing colors or materials, setting special shaders and materials on objects.

---

## Implementation Plan

This section describes the architecture and sequencing for each action item above, grounded in the current codebase state.

---

### 1. Alien flow: left → right

**Current state:** The map generator places alien spawn points (`tile value 5`) and the goal (`tile value 9`) at arbitrary positions. The `MoveTowardsGoal` AI behavior and A* pathfinding grid already work end-to-end.

**Change:** Fix the map generator so spawn points always appear along the **left column** (or first 1–2 columns) and the goal always on the **right column** (or last column). Player spawn moves to the **middle-left** area. This is a pure change to `src/map/map_generator.rs` — no engine change needed.

**Map convention:**
```
W (left)  →  spawn points (tile 5)
            floor tiles with procedurally generated obstacles
E (right) →  alien goal (tile 9)
```

For the hand-crafted `level_01.ron`, switch from `generated: true` to an explicit tile grid that enforces this layout, or add a `direction` field to the map file that the generator respects.

---

### 2. Pass-through meter

**Current state:** `LevelTracker` (in `src/game_state/score_keeper.rs`) already tracks `aliens_reached_goal` and `aliens_win_cut_off`. The `AlienGoal` collision layer and events fire when an alien reaches the goal. What's missing is a HUD element showing the ratio.

**Change:** Add a progress bar to the in-game HUD (`src/ui/spawn_ui.rs`) bound to `LevelTracker.aliens_reached_goal / aliens_win_cut_off`. When the ratio hits 1.0, trigger `LevelState::Failed`. This reuses the existing `ProgressBar` widget from `lava_ui_builder`.

The bar reads as "aliens escaped" — it fills as bad things happen, so the player wants to keep it empty.

---

### 3. Wave system

**Current state:** `AlienSpawnPoint` has a `spawn_rate_per_minute` and `LevelTracker` has `aliens_to_spawn` / `aliens_left_to_spawn`. Spawning runs but isn't wave-gated.

**Change:** Add a `WaveDefinition` to the map/level file:
```ron
waves: [
    (alien_count: 10, spawn_rate: 5.0, delay_before: 10.0),
    (alien_count: 20, spawn_rate: 8.0, delay_before: 30.0),
]
```

A `wave_system` in `src/alien/` reads the current wave, counts down `delay_before`, then enables spawning at `spawn_rate` until `alien_count` are spawned. Once all waves are done and no aliens remain, `LevelState::Completed`.

`LevelTracker` gains `current_wave: usize` and `wave_timer: f32`.

---

### 4. Game-over & win conditions

**Current state:** `LevelState` has `NotStarted / InProgress / Completed / Failed` variants and `LevelTracker` is a resource. The health system fires events when entities die. What's missing is the system that reads player deaths and alien counts to transition state.

**Changes:**
- `level_monitor_system`: runs each frame, checks `LevelState`; when `Failed` or `Completed`, fires a `GameOver` message and transitions to a result screen.
- **Lose 1:** `aliens_reached_goal >= aliens_win_cut_off` → `Failed`.
- **Lose 2:** all `Player` entities have `Health <= 0` simultaneously → `Failed` (distinct from revive window, see below).
- **Win:** all waves spawned + no aliens alive + goal not breached → `Completed`.

---

### 5. Player death & revive mechanic

**Current state:** The `Health` component and `health_monitor_system` exist. There is currently no distinction between "dead" and "eliminated."

**Architecture:**

Add a `PlayerDead` component (marker). When a player's health reaches 0:
- Mark with `PlayerDead`, freeze their physics, play death animation.
- Start a `revive_timer: f32` counting down (e.g. 30 s). If it reaches 0 with no revive, the player is permanently eliminated this wave.

Add a `revive_system`:
- Queries living players near (`< 1.5 units`) a `PlayerDead` entity.
- While the reviving player holds the interact key (`E`/gamepad south), a `revive_progress: f32` fills up (takes ~3 s).
- On completion: remove `PlayerDead`, restore partial health, clear timer.

**Game-over trigger:** count living (non-`PlayerDead`) players. If zero, `LevelState::Failed`.

A small radial progress indicator (using `WorldFollower` from `lava_ui_builder`) shows revive progress above the downed player.

---

### 6. Barrier & tower placement

**Current state:** The building system (`src/building/`) lets the player place tiles. Only one tower type exists (`TowerShooter` that fires projectiles). Barriers block alien pathfinding via the `Impassable` collision layer.

**Tower type architecture:**

Define towers as Ron files under `assets/towers/`:
```ron
// assets/towers/shooter.ron
(
    name: "Shooter",
    icon: "towers/icons/shooter.png",
    model: "toon-shooter/Tower.glb",
    effect: Shoot(damage: 10, rate_per_minute: 30, range: 3.0),
)
```

A `TowerDefinition` resource (or asset) is loaded at startup. The build-mode UI lists available towers; `ChangeBuildIndicator` cycles through them. The placed entity gets the appropriate `TowerEffect` component:

```rust
enum TowerEffect {
    Shoot { damage: f32, rate_per_minute: f32, range: f32 },
    Slow  { factor: f32, duration: f32, range: f32 },
    Area  { damage: f32, radius: f32, rate_per_minute: f32 },
}
```

The existing `shoot_alien_system` becomes one branch; `slow_alien_system` and `area_damage_system` are added in `src/towers/systems/`.

**Barrier vs tower distinction:** Barriers (impassable walls) use the current `Impassable` collision layer and trigger alien path recalculation. Towers sit on floor tiles and do not block movement.

---

### 7. Asset browser: file explorer

**Current state:** `scan_assets()` in `src/asset_browser/state.rs` recursively scans `assets/packs/toon-shooter/` and returns a flat list of GLB paths.

**Change:** Replace with a two-panel explorer:
- **Left panel (folders):** shows immediate subdirectories of a root folder. Clicking a folder sets it as the active folder.
- **Right panel (files):** shows GLB/GLTF files in the active folder only (not recursive).

`AssetBrowserState` gains `current_folder: String` and `folders: Vec<String>` alongside the existing `files: Vec<String>`. A `scan_current_folder()` helper replaces `scan_assets()`.

---

### 8. Asset definition Ron file (import)

**Architecture:**

Define a new asset type `AssetDefinition` (Ron-serializable):
```ron
// assets/defs/soldier.ron
(
    model_path: "packs/toon-shooter/characters/Character Soldier.glb",
    hidden_nodes: ["AK", "Pistol", "Revolver", "Revolver_Small", ...],
    animation_mapping: {
        "idle":     "CharacterArmature|Idle",
        "walk":     "CharacterArmature|Walk",
        "throwing": "CharacterArmature|Punch",
        "death":    "CharacterArmature|Death",
    },
    // future: material_overrides, shader_preset
)
```

**Import flow in asset browser:**
1. User selects a GLB in the file explorer.
2. Presses `I` (or "Import" button) → asset browser reads the current `hidden_nodes` toggle state and `anim_mapping` from the viewer into a `AssetDefinition` struct.
3. Writes it to `assets/defs/<model-name>.ron`.
4. If a `.ron` already exists for that model, the editor loads it first so the user edits an existing definition.

**In-game usage:**
- `AssetsPlugin` loads `AssetDefinition` files at startup via `asset_server.load()` (registered as a custom `Asset`).
- `ModelSettings` references an `AssetDefinition` asset handle instead of the raw GLB path.
- `hide_player_weapon_nodes` reads `hidden_nodes` from the loaded definition instead of the hardcoded `WEAPON_NODES` constant.
- `build_player_anim_graph` reads `animation_mapping` from the definition instead of `ModelSettings.anim_mapping`.

This makes `WEAPON_NODES` and the per-field `AnimMapping` struct obsolete once migrated.

---

### 9. Asset browser: animation key mapping editor

**Current state:** The browser lets you cycle through clips by index but has no way to say "this clip = this game state."

**Change:** Add a second section below the node list in the browser panel: an **Animation Mapping** table. Each row shows a game-state label (`idle`, `walk`, `throwing`, etc.) with the currently mapped clip name (editable by cycling through available clips with `←`/`→`). This directly edits `AssetBrowserState.anim_mapping: HashMap<String, String>` and is written out when the user imports.

---

### Implementation sequence

| Priority | Item | Status |
|---|---|---|
| 1 | Map left→right spawn/goal placement | in progress |
| 2 | Pass-through meter HUD | not started |
| 3 | Game-over / win condition monitor | skeleton exists |
| 4 | Player death & revive | not started |
| 5 | Wave definitions in map file | not started |
| 6 | Multiple tower types (slow + area) | not started |
| 7 | Asset browser: folder explorer | not started |
| 8 | AssetDefinition Ron file + import | not started |
| 9 | Animation mapping editor in browser | not started |