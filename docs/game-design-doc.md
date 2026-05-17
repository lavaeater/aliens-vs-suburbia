# Aliens vs Suburbia

https://otter.ai/u/TDfrlhEGTGLG7raPBGnriEIROBs?view=summary

## Overview

Speaker 1 outlines a game concept called "Aliens vs Suburbia," which is a reimagining of South Park Tower Defense with aliens moving from start points to end points, while players construct barriers to block the aliens. Players can revive each other if they die, but if all players die simultaneously, the game ends. Aliens can destroy barriers if no path is available. The first map could be a picnic area with various features like sand pits or carousels. Additionally, Speaker 1 discusses the need for an asset browser with a file explorer interface, an import function to create run files with settings, and the ability to edit and map animation keys to game states, with potential future updates to include color and material editing.

### Action Items

- [x] Implement the first basic map where aliens move from start points to end points — aliens spawn on the left edge and path to the right edge; map generator enforces this layout.
- [ ] Players build barriers that aliens cannot pass but players can.
- [x] Players can build towers of different whacky kinds that shoot at aliens, slow them down, etc. — Shooter (projectiles), Slow Tower (35% velocity), Area Tower (15 DPS). Cycle build types with arrow keys.
- [x] A meter tracks the percentage of aliens that have passed through — red progress bar + label at top-centre HUD, triggers `LevelState::Failed` when full.
- [x] Implement a revive mechanic where dead players can be revived by other players — hold E near a downed player; 3-second fill with a blue WorldFollower progress bar; removes `PlayerDead`, restores 50% health.
- [x] Implement a game-over condition that triggers when all players die simultaneously — `level_state_system` checks all player health each frame; transitions after a 2 s delay.
- [x] Implement alien behavior where aliens destroy barriers when no open route is available from start points to end points — `DestroyTheMap` AI behavior already in place.
- [x] Add a file explorer–style interface in the asset browser — folder chips navigate into subfolders, Backspace goes up, only files in the current folder are shown (non-recursive).
- [x] Implement an import function in the asset browser — press `I` or click "Import definition" to write `assets/defs/<model>.ron` with hidden_nodes + animation_mapping.
- [ ] This Ron file is used as a resource for using the model in the game — `AssetDefinition` struct defined and save/load implemented; in-game consumption (replacing WEAPON_NODES) not yet wired.
- [x] Add editing capabilities in the asset browser — hidden node toggles, animation mapping editor with ◀/▶ per game-state key. Existing definition auto-loaded when model is opened.
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

### 1. Alien flow: left → right ✅

**Current state:** The map generator places alien spawn points (`tile value 5`) and the goal (`tile value 9`) at arbitrary positions. The `MoveTowardsGoal` AI behavior and A* pathfinding grid already work end-to-end.

**Done:** Map generator (`src/map/map_generator.rs`) rewritten so spawn points are fixed to column 0 (west edge) spread across 2–3 evenly-spaced rows, goal is fixed to column `w-1` (east edge) vertically centred, and player spawns a few columns in from the left at mid-height. House placement still randomised but connectivity-checked.

**Map convention:**
```
W (left)  →  spawn points (tile 5)
            floor tiles with procedurally generated obstacles
E (right) →  alien goal (tile 9)
```

For the hand-crafted `level_01.ron`, switch from `generated: true` to an explicit tile grid that enforces this layout, or add a `direction` field to the map file that the generator respects.

---

### 2. Pass-through meter ✅

**Done:** Red progress bar + "Aliens escaped: X / Y" label added to top-centre HUD (`src/ui/spawn_ui.rs`). `update_alien_meter` system updates it reactively on `Changed<LevelTracker>`. Default cutoff lowered from 600 → 10 for a playable game. Bar fills red; hitting 100% triggers `LevelState::Failed`.

---

### 3. Wave system ✅

**Done:** `WaveManager` resource (`src/alien/wave_manager.rs`) with 3 hardcoded waves (10, 15, 20 aliens at increasing rates). `wave_system` counts down between waves, sets `AlienSpawnPoint.spawn_rate_per_minute` at wave start. `alien_spawner_system` gates on `WaveManager.spawning`. HUD shows "Wave X / Y in Ns" or "Wave X / Y" when active. Win condition requires all waves done AND all aliens killed.

---

### 4. Game-over & win conditions ✅

**Done:** `level_state_system` (`src/game_state/score_keeper.rs`) rewritten:
- **Win:** all aliens spawned and killed → `Completed`.
- **Lose 1:** `aliens_reached_goal >= aliens_win_cut_off` → `Failed`.
- **Lose 2:** all `Player` entities have `health <= 0` simultaneously → `Failed`.
- Fixed every-frame `GotoState` spam — now fires once after a 2-second `end_delay`.

---

### 5. Player death & revive mechanic ✅

**Done:**
- `health_monitor_system` now excludes `Player` entities — players are never despawned automatically.
- `detect_player_death` (`src/player/systems/death_revive.rs`): on `Changed<Health>` with `health <= 0`, inserts `PlayerDead`, zeroes velocity, sends `AnimationKey::Death`, spawns a blue WorldFollower bar above the player.
- `player_revive_system`: living player within 1.8 units + hold `E` fills `revive_progress` over 3 s; on completion restores 50% health, removes `PlayerDead`, despawns bar.
- Keyboard input (`keyboard_input.rs`) gains `Without<PlayerDead>` so downed players can't move.
- Win/lose already correct — health check in `level_state_system` catches the all-dead case.

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
|----------|------|--------|
| 1 | Map left→right spawn/goal placement | ✅ done |
| 2 | Pass-through meter HUD | ✅ done |
| 3 | Game-over / win condition monitor | ✅ done |
| 4 | Player death & revive | ✅ done |
| 5 | Wave definitions in map file | ✅ done |
| 6 | Multiple tower types (slow + area) | ✅ done |
| 7 | Asset browser: folder explorer | ✅ done |
| 8 | AssetDefinition Ron file + import | ✅ done |
| 9 | Animation mapping editor in browser | ✅ done |