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

### 11. Model Types

The asset browser - some models might actually be monster models. We should  in the .ron file store some kind of "type of model", being player, tower, terrain/map, item and of course the lovely "enemy" type - thus making models available for usage as enemies in game. Enemies in turn - if something is marked as an enemy, we should be able to set some properties for them such as Health and Speed (to start with for the easiest early implementations). These properties should perhaps be stored in the normal .ron-file for models, we should perhaps have some kind of enum with variants for this, like Player, Terrain, Item and then Enemy(EnemyProps) which could define which Components and their values should be added to any spawned entities when we add them to the game.

A nice way to make the game not have a dark void in  the background is for us to extend the floor out to infinity. I have realized that the floor geometry isn't that important right now, it is the obstacles that matter. So the map will have some kind of bounds yes, that CAN be walls, but could also just be invisible collider geometry.

So the map size should be definable in X * Y squares  (squares that are used today to define the floor).

### 12. Player Setup Screen / Multiplayer Support

Ability to select different players / models. Basic stuff like, "Press Enter or X to join game" - doing so gives the player the ability to then select between available player models. When in this model selection state, pressing enter / x (gamepad support) starts the game. 

### 13. Terrain, Towers and Items

When defining different types of models in the asset browser, if they are of type Terrain/Map (you can choose name) one should be able to assign a value for the model indicating that they are blocking or not, as in "are they traversible". Traversible is fine-grained by the way, Terrain features could be traversible by players, enemies or both. Terrain features that are not traversible by enemies should probably also have a Health value. This could be put in the ModelType Enum variant Terrain(TerrainProps) or something. If they don't have a Health value, they should be indestructible - this could probably be a marker component as well, right? Items should have Item(ItemProps), they could be of different types as well, such as simply Decorative or Pickups, off the top of my head. Pickups would be things like Health and Crates with loot (weapons and other stuff, not implemented now, but health could be). Towers need to be able to set Tower Health, Tower Cost (in Coins), Range, Damage, Fire Rate, and whatnot. Everything required to describe what a tower does (we have some towers defined I think).

### 14. Loot drops

Aah. Players must of course use some kind of currency to build towers in-game. Currency is in the form of Coins and different enemy types drop different amounts of Coins when dying. Players pick them up by moving near enough -  property different players can have is PickupRange.

### 15. Player special abilities

Apart from Players having different stats such as Throw Range (all weapons are thrown balls at start of game), Throw Damage and Throw Rate, Movement Speed and Health, they must all have a Special Ability. The four basic ones that we need at start of game is: Bombardment, the entire visible area of the screen being hit with a bombardment dealing heavy damage to enemies. Heave Damage being defined as 100% lethal to enemies that have 75 in max health, i.e. doing 75 damage. Another is Healing - healing all players and towers in some range of the player. Third being Whirlwind, enabling the Player to run around at ten times normal speed while spinning simply killing everything they touch - lasting for a fairly short amount of time. A fourth being Gold Digger - the player automatically sucks up all loose Coins on the map.

### 16. Damage to players

Enemies have, at the start, no offensive weapons - they are transitioning through the map and it is the job of the players to stop them - they do however deal damage by touch - different enemies deal different amounts of damage. If a player dies, they lie where they lie. If another player stands by them for 1-2 seconds they are revived, are immortal for five seconds and can move away from danger. 

### 17. Enemy Wrecking the Labyrinth

I think we have A* path finding. Every time we add stuff to the map that are blocking to enemies, we have to recalculate enemy paths. If an enemy cannot find a path to an exit, we recalculate a new, shortest possible route, disregarding blocking, destructible, terrain features. Then they get to work, moving along that most direct path and attacking terrain features in their path. If a player removes some kind of blocking terrain to free up a regular path, we recalculate paths again. 

### 18. Map Editor Workflow description

So, yeah, basically, this isn't a feature as it is a description of how I imagine all of these features work together to create maps etc. The asset browser is used to "import" models into the game. This entails setting scale, defining type and depending on type setting specific properties for those types, like player, tower, enemy, etc. Then, in a new screen, called Map Editor, the player defines the SIZE of the map and then simply starts adding items to squares. A square may contain 1 terrain feature (all squares have a floor to start with) such as a wall, a game feature (such as start area, exit area, player spawn area, etc etc). This gives me the idea that areas must be able to contain multiple terrain features, if we imagine adding beds, crates, walls, water features, whatever. Then we add waves by simply pressing a button called "add wave" - a wave is an enemy type, a rate of spawn, a number of total enemies. We can add any number of Waves to a map.

---

## Implementation Plan: Items 10–18

---

### Suggested implementation order

Dependencies drive the order. Item 11 (model types) enriches the `.ron` format that items 10, 12, 13 and 15 all depend on. Enemy touch damage (16) and pathfinding robustness (17) are largely plumbing on top of existing systems, so they go early for immediate gameplay feel. The map editor (10 + 18) is last because it is the largest feature and requires all type/property definitions to be solid first.

| Step | Item | Rationale |
|------|------|-----------|
| 1 | **11** — Model types in asset browser + RON | Foundational: everything else references `ModelType` |
| 2 | **16** — Enemy touch damage | Quick win; makes the game actually dangerous |
| 3 | **17** — Pathfinding recalculation robustness | Core loop correctness; largely already wired, needs verification |
| 4 | **12** — Player setup / model selection screen | Self-contained; unlocks character variety before deeper content work |
| 5 | **13** — Terrain, tower and item runtime properties | Extends 11 into actual in-game components |
| 6 | **14** — Coin drops and tower economy | Adds resource loop; needed before special abilities make sense |
| 7 | **15** — Player special abilities | Character differentiation, relies on economy + enemy damage being real |
| 8 | **10 + 18** — Map editor | Largest feature; needs 11 and 13 solid to know what to place and how |


---

### 11. Model Types — Implementation

**Goal:** every `.ron` def carries a `ModelType` tag and associated properties; the asset browser exposes a type picker and relevant property fields.

**`AssetDefinition` change:**
```rust
#[derive(Serialize, Deserialize)]
pub enum ModelType {
    Player,
    Tower(TowerProps),
    Terrain(TerrainProps),
    Item(ItemProps),
    Enemy(EnemyProps),
}

pub struct EnemyProps  { pub health: f32, pub speed: f32, pub coin_drop: u32 }
pub struct TowerProps  { pub health: f32, pub cost: u32, pub range: f32,
                         pub damage: f32, pub fire_rate_per_minute: f32 }
pub struct TerrainProps { pub blocks_enemies: bool, pub blocks_players: bool,
                          pub health: Option<f32> }  // None = indestructible
pub struct ItemProps   { pub kind: ItemKind }
pub enum   ItemKind    { Decorative, HealthPickup { amount: f32 }, CratePickup }
```

`AssetDefinition` gains `pub model_type: ModelType` (default `ModelType::Player` or an `Unknown` variant).

**Asset browser UI:** after the height row, add a "— Type —" section. A row of type-chip buttons (Player / Tower / Terrain / Item / Enemy) sets `AssetBrowserState.model_type`. Selecting a type that has props (Tower, Terrain, Enemy) reveals a small inline property editor — simple numeric fields using `+`/`-` buttons like the height row. `export_definition` writes the type + props. `load_definition` restores them.

**In-game use:** when spawning terrain, towers or enemies at runtime, read `model_type` from the def to insert the right components (`Health`, `TowerShooter`, `AlienProperties`, etc.).

**Infinite floor:** extend `src/map/` floor generation to tile out an extra `N` rows and columns beyond the defined grid using a simple `Plane` mesh with the floor material, so the void never shows.

---

### 16. Enemy Touch Damage — Implementation

**Goal:** walking into a player deals damage; existing death/revive mechanic handles the rest.

This is entirely additive to the existing `src/general/` collision systems.

**New component:**
```rust
#[derive(Component)]
pub struct TouchDamage { pub dps: f32 }
```

Add `TouchDamage { dps: 5.0 }` (or from `EnemyProps`) when spawning aliens.

**New system** `touch_damage_system` in `src/general/systems/`:
- Query `(&CollidingEntities, &TouchDamage)` on alien entities.
- For each colliding entity that has `(&mut Health, Without<PlayerDead>)` and is on the `Player` collision layer, subtract `dps * delta_time` per frame.
- Re-uses the existing `Health` component and `health_monitor_system`; no extra death logic needed.

Register in `GeneralPlugin` with `.run_if(in_state(InGame))`.

---

### 17. Pathfinding Recalculation Robustness — Implementation

**Current state:** `DestroyTheMap` AI already targets blocking tiles. The A* grid in `src/map/` is rebuilt when tiles change. The main gap is the *fallback path* (ignore-blocking route) when no normal path exists.

**Changes:**
1. **Trigger on tile change:** `pathfinding_dirty` flag already exists; verify it is set whenever a tile is placed *or removed* (building and destruction both). Add the missing removal side if absent.
2. **Fallback path:** when A* with the impassable layer returns no path, run a second A* pass on a grid that treats all tiles as passable. Store the result as `AlienPath::Destructive`. `DestroyTheMap` then follows this path, attacking each blocking tile it encounters.
3. **Player-removes-blocker:** the same tile-change trigger already covers this — enemies get a fresh normal path on the next frame if one now exists.
4. **Guard against thrashing:** only recalculate when the grid actually changed (already guarded by the dirty flag).

No new files needed; changes are confined to `src/map/pathfinding.rs` and `src/ai/behaviors/move_towards_goal/`.

---

### 12. Player Setup Screen — Implementation

**New game state:** `GameState::PlayerSetup` sits between `Menu` and `InGame`.

**Flow:**
1. Menu "Play" button → `PlayerSetup`.
2. A large "Press Enter / A to join" prompt per potential slot (up to 4). Keyboard Enter or Gamepad A marks that slot as joined and activates a model carousel for it.
3. Carousel shows `Player`-typed model defs (filtered from `assets/defs/`). Left/right d-pad or arrow keys cycle models; a 3D preview renders the selected model (re-use the asset browser's viewer camera approach).
4. Once all joined players confirm (Enter/A again), transition to `InGame` with each player's chosen `AssetDefinition` path stored in a `PlayerRoster` resource.
5. `spawn_players` reads `PlayerRoster` instead of the hardcoded `player-settings.ron` model path.

**New files:** `src/player_setup/` plugin mirroring the asset browser structure (state, ui, plugin).

---

### 13. Terrain, Towers and Items Runtime Properties — Implementation

This converts the `ModelType` data written in step 11 into actual in-game components.

**Terrain:** when `map_generator` or the map editor places a terrain model:
- Read its def → `TerrainProps`.
- `blocks_enemies` → insert `Impassable` collider layer.
- `blocks_players` → insert `PlayerImpassable` collider layer (new layer).
- `health: Some(h)` → insert `Health(h)`; absence → insert `Indestructible` marker component.
- `Indestructible` causes `health_monitor_system` and `DestroyTheMap` to skip the entity.

**Towers:** reading `TowerProps` from the def replaces the current hardcoded `TowerShooter` values. `TowerShooter` already carries `damage`, `range`, etc.; just populate from the def.

**Items:**
- `Decorative` → spawn with model only, no gameplay components.
- `HealthPickup { amount }` → insert `HealthPickup(amount)` component. A new `pickup_system` queries `(&CollidingEntities, With<HealthPickup>)` on player entities and heals them on contact, then despawns the pickup.

---

### 14. Coin Drops and Economy — Implementation

**New resource:** `PlayerWallet { coins: u32 }` per player (or a shared `TeamWallet`).

**On enemy death:** `health_monitor_system` already despawns enemies. Extend it (or add a `on_enemy_death` observer) to spawn a `Coin` entity at the enemy's last position:
```rust
#[derive(Component)]
pub struct Coin { pub value: u32 }
```
Value comes from `EnemyProps.coin_drop`.

**Pickup:** `coin_pickup_system` queries `(&CollidingEntities, With<Player>)` → for each colliding `Coin` entity within `PickupRange`, add its value to the wallet, despawn it. `PickupRange` is a float component on player (default 1.5 units, set from player's def `PlayerProps`).

**Tower placement cost:** `building_system` checks `TeamWallet.coins >= tower_def.cost` before placing; deducts on placement.

**HUD:** add a coin counter to the existing in-game HUD (`src/ui/spawn_ui.rs`).

---

### 15. Player Special Abilities — Implementation

**New component:**
```rust
#[derive(Component)]
pub enum SpecialAbility {
    Bombardment,
    Healing,
    Whirlwind,
    GoldDigger,
}

#[derive(Component)]
pub struct AbilityCooldown { pub remaining: f32, pub total: f32 }
```

Set from the player def (`PlayerProps.ability`). Activated by a dedicated key/button (e.g. Space or gamepad LB).

**Effect systems** in `src/player/systems/abilities/`:

- **Bombardment:** spawn a grid of `ExplosionZone` entities covering the camera viewport. Each deals 75 damage to enemies within radius on the next frame, then despawns. 5 s cooldown.
- **Healing:** query all `Health` components on players and towers within `healing_range` (e.g. 6 units), restore a flat amount. 12 s cooldown.
- **Whirlwind:** insert `WhirlwindActive` marker; the movement system multiplies velocity by 10× and adds a `TouchDamage` with very high dps while active. Remove after 4 s. 20 s cooldown.
- **GoldDigger:** iterate all `Coin` entities on the map, move them to the player's position (or directly to wallet). 15 s cooldown.

**HUD:** show ability icon + cooldown pie-overlay per player slot.

---

### 10 + 18. Map Editor — Implementation

**New game state:** `GameState::MapEditor`.

**Two-step entry:**
1. "New map" prompts for width × height (tile grid size).
2. Editor opens showing the grid from above (orthographic top-down camera).

**Left panel (palette):**
- Tabs: Terrain | Items | Towers | Enemies | Special (spawn/exit/player-spawn markers).
- Each tab lists models of that `ModelType` from `assets/defs/`. Clicking selects the active brush.

**Grid interaction:**
- Left-click on a tile → place current brush (a tile may hold one terrain feature + multiple items).
- Right-click → remove top-most placed item.
- R key → rotate active brush 45° (stored as `rotation_steps: u8`, applied as `Quat::from_rotation_y(steps * PI/4)`).
- A tile that already has a terrain feature can receive items on top; a second terrain feature replaces the first.

**Special markers:** spawn areas, exit areas, and player-spawn squares are placed as special brush types. They write tile types 5, 9, and player-spawn into the ron map.

**Wave editor (right panel):**
- "Add Wave" button appends a `WaveDef { enemy_def: String, count: u32, spawn_rate_per_minute: f32 }` to a list.
- Each wave row has remove button + numeric spinners.
- Waves are written into the map `.ron` file alongside the tile grid.

**Save / load:**
- "Save" writes to `assets/maps/<name>.ron` using the existing `LevelMap` RON format, extended with a `placements: Vec<TilePlacement>` field listing model defs and rotations per tile coordinate.
- "Load" reads an existing map file and reconstructs the editor state.

**Runtime map loading:** `map_generator` reads `placements`, looks up each def, and spawns the model with the stored rotation and runtime components (from `ModelType`).

**New files:** `src/map_editor/` (plugin, state, ui, camera). The heavy lifting reuses `scan_folder` from the asset browser and `LevelMap` from the map module.