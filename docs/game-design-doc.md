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
- [x] This Ron file is used as a resource for using the model in the game — `PlayerAssetDef` resource loaded at startup and on model change; `hide_player_weapon_nodes` uses `hidden_nodes` from the def (falls back to `WEAPON_NODES`); `build_player_anim_graph` uses `animation_mapping` from the def (falls back to `ModelSettings.anim_mapping` then defaults).
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

----

## Next features for complete game!

### 10. Map Editor Prototype

A potential map editor prototype would ideally be a very simple modal editor to draw some kind of terrain, add terrain features such that either are or are not blocked for alien movement. Ability to add start areas, exit areas, potentially drawing tracks for aliens to follow. For any square we should be able to add items, rotate them (in 45-degree increments, mostly to align features to proper cardinal directions). The items in questions should be any models that have .ron files associated with them. 
It would also be nice to have a wave editor - nothing complex, just the ability to add waves where the definition is simply what type of alien a certain wave contains, the number of them and so on. So we could have anything from 1-n number of waves. This then influences the progress indicator - not of course that we have a total number of waves and number of enemies per the current wave.

### 11. Enemy Types

The asset browser - some models might actually be monster models. We should  in the .ron file store some kind of "type of model", being player, terrain/map, item and of course the lovely "enemy" type - thus making models available for usage as enemies in game. Enemies in turn - if something is marked as an enemy, we should be able to set some properties for them such as Health and Speed (to start with for the easiest early implementations). These properties should perhaps be stored in the normal .ron-file for models, we should perhaps have some kind of enum with variants for this, like Player, Terrain, Item and then Enemy(EnemyProps) which could define which Components and their values should be added to any spawned entities when we add them to the game.

A nice way to make the game not have a dark void in  the background is for us to extend the floor out to infinity. I have realized that the floor geometry isn't that important right now, it is the obstacles that matter. So the map will have some kind of bounds yes, that CAN be walls, but could also just be invisible collider geometry.

So the map size should be definable in X * Y squares  (squares that are used today to define the floor).

### 12. Player Setup Screen / Multiplayer Support

Ability to select different players / models. Basic stuff like, "Press Enter or X to join game" - doing so gives the player the ability to then select between available player models. When in this model selection state, pressing enter / x (gamepad support) starts the game. 

### 13. Terrain and Items

When defining different types of models in the asset browser, if they are of type Terrain/Map (you can choose name) one should be able to assign a value for the model indicating that they are blocking or not, as in "are they traversible". Traversible is fine-grained by the way, Terrain features could be traversible by players, enemies or both. Terrain features that are not traversible by enemies should probably also have a Health value. This could be put in the ModelType Enum variant Terrain(TerrainProps) or something. If they don't have a Health value, they should be indestructible - this could probably be a marker component as well, right? Items should have Item(ItemProps), they could be of different types as well, such as simply Decorative or Pickups, off the top of my head. Pickups would be things like Health and Crates with loot (weapons and other stuff, not implemented now, but health could be).

### 14. Loot drops

Aah. Players must of course use some kind of currency to build towers in-game. Currency is in the form of Coins and different enemy types drop different amounts of Coins when dying. Players pick them up by moving near enough -  property different players can have is PickupRange.

### 15. Player special abilities

Apart from Players having different stats such as Throw Range (all weapons are thrown balls at start of game), Throw Damage and Throw Rate, Movement Speed and Health, they must all have a Special Ability. The four basic ones that we need at start of game is: Bombardment, the entire visible area of the screen being hit with a bombardment dealing heavy damage to enemies. Heave Damage being defined as 100% lethal to enemies that have 75 in max health, i.e. doing 75 damage. Another is Healing - healing all players and towers in some range of the player. Third being Whirlwind, enabling the Player to run around at ten times normal speed while spinning simply killing everything they touch - lasting for a fairly short amount of time. A fourth being Gold Digger - the player automatically sucks up all loose Coins on the map.

### 16. Damage to players

Enemies have, at the start, no offensive weapons - they are transitioning through the map and it is the job of the players to stop them - they do however deal damage by touch - different enemies deal different amounts of damage. If a player dies, they lie where they lie. If another player stands by them for 1-2 seconds they are revived, are immortal for five seconds and can move away from danger. 

### 17. Enemy Wrecking the Labyrinth

I think we have A* path finding. Every time we add stuff to the map that are blocking to enemies, we have to recalculate enemy paths. If an enemy cannot find a path to an exit, we recalculate a new, shortest possible route, disregarding blocking, destructible, terrain features. Then they get to work, moving along that most direct path and attacking terrain features in their path. If a player removes some kind of blocking terrain to free up a regular path, we recalculate paths again. 
