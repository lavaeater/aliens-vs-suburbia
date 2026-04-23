# Roadmap — Aliens vs Suburbia

_Last updated: 2026-04-23_

## What's done

The core gameplay loop is fully functional:

- Player model, alien model, tower model, obstacle model
- Tile-based map with pathfinding grid
- Player can throw objects (auto-aim), build walls/towers, and enter/exit build mode
- Aliens pathfind toward the goal, avoid walls, attack the player, and destroy blocking tiles when no path exists
- Shooting tower with range sensor
- Health system with world-space health bars
- Scoring HUD, game-end conditions (win/lose)
- Start menu and start screen
- Isometric camera
- Entity outlines (bevy_mod_outline)
- Controller support (input handled; see note below)
- Toon shooter models loaded
- Space editor integration for level prefabs
- Bevy 0.18 migration complete

---

## What isn't done — prioritized

Priorities: **P1 = needed to feel like a game**, **P2 = significant quality-of-life**, **P3 = content / replayability**, **P4 = aspirational / post-MVP**.

---

### P1 — Core feel (do these first)

#### Animation state machine
The player and aliens have animations loaded but no state machine driving them. Characters look static or wrong during movement, combat, and building.

_Suggestion:_ Implement a simple state enum (`Idle`, `Walking`, `Throwing`, `Building`, `Crawling`) as a component, drive it from velocity and action systems, and map it to the GLB animation clips already loaded. The infrastructure in `src/animation/` is already partially in place.

#### Death effects
When aliens or towers die, nothing happens visually. This makes the game feel hollow.

_Suggestion:_ Start with a simple particle burst or a brief scale-up/fade-out tween on despawn. Even a one-frame flash goes a long way. Look at `bevy_hanabi` (GPU particles) or roll a simple timer-based "death debris" spawner.

#### Build mode visual feedback — RED / GREEN indicators
Players can build on occupied tiles without clear feedback on what's legal. The Kanban notes "Use RED and GREEN to indicate OK places."

_Suggestion:_ The build indicator entity already exists. Add a material swap (green = valid, red = blocked) driven by a tile occupancy check each frame in build mode.

#### Walls go see-through when player is behind them
Isometric perspective means walls constantly occlude the player. This is a playability issue, not just polish.

_Suggestion:_ Use a screen-space alpha fade or a dithered cutout shader on wall entities when the camera-to-player ray passes through them. `bevy_mod_outline` is already in the project; a custom material with distance-based alpha is the typical approach.

---

### P2 — Quality of life and polish

#### More tower types
One tower type makes the strategic layer shallow. The game concept implies build variety is core to the TD loop.

_Suggestion:_ Add at least two more before shipping: a slow-field tower (AoE debuff, no projectile) and a wall-mounted spike/trap. The shooting tower's code is a good template. Tie in "Towers as Files" (P3) once the second type is working.

#### UI improvements (HUD + settings)
From `ui-using-lava-ui-builder.md`: the HUD needs to be more informative, and there should be a settings/debug panel that persists to `game-settings.ron`.

_Suggestion:_ 
- HUD: add current wave number, aliens remaining, and a build-mode indicator. The lava_ui_builder integration done today makes this straightforward.
- Settings panel: camera zoom, speed multipliers, etc. Use `bevy_ron` or Bevy's `serde` support. Forward-compatible RON with `#[serde(default)]` handles missing fields on load.

#### Controller support — finish it
The docs note controller input is wired but the `GamepadControl` component isn't attached and `KeyboardControl` isn't swapped out.

_Suggestion:_ On `GamepadConnectionEvent`, attach `GamepadControl` to the player and remove `KeyboardControl`. This is low-effort given the input plumbing is already there.

#### Visual pazazz
From `Visual Pazazz.md` and the devlog: atmospheric lighting, screen effects. Currently the game is flat.

_Suggestion:_ Explore in this order (cheapest first):
1. Point lights on tower muzzle flashes
2. A subtle vignette post-process
3. Screen-space ambient occlusion (Bevy 0.18 has SSAO built in — just enable it on the camera)
4. Bloom on projectiles

---

### P3 — Content and replayability

#### Levels as Files
The map is currently hardcoded. The devlog and todo note that levels should be loadable from files using prefab tiles.

_Suggestion:_ Define a simple RON or JSON level format: a 2D grid of tile type IDs, plus spawn point and goal coordinates. Load it at `InGame` enter. Blender/space_editor prefabs can be tile variants referenced by ID. Start with one hand-authored level file before worrying about the editor pipeline.

#### Towers as Files
From `Towers as Files.md`: tower definitions should be data-driven, not hardcoded.

_Suggestion:_ Define a `TowerDef` struct (range, fire rate, damage, model path, cost) and load tower definitions from RON assets. Wire into the build UI as selectable tower types. Enables adding towers without recompiling.

#### Wave system / level progression
There's no concept of waves — aliens just spawn on a timer up to a cap of 50. This limits the tension arc the game concept describes (stress, chaos).

_Suggestion:_ Define a simple wave script: `[(delay, alien_type, count)]`. Between waves give the player a short build window. This single change makes the game feel much more like a tower defense.

---

### P4 — Aspirational / post-MVP

#### Multiple playable characters
The concept (and devlog) describes a group of school friends with different abilities. Right now there's one player.

_Suggestion:_ Defer until the single-player loop is polished. When ready: extract player abilities into a `CharacterClass` component (throw power, build speed, special ability). The character select screen can live in the start menu.

#### Multiplayer
The todo doc is empty, which is telling. This is a large feature.

_Suggestion:_ Don't start this until Levels as Files and wave progression are solid. Bevy doesn't have a built-in netcode solution — evaluate `bevy_replicon` or `lightyear` when the time comes. Local co-op (split input, shared screen) is a much cheaper first step and fits the concept perfectly.

#### Narrative / story intro
The vision describes a scene-setting intro (kids coming home, aliens invade). This is pure content work and belongs last.

---

## Suggested sequence

```
Now       → Animation state machine + death effects (P1, highest impact/effort ratio)
            Build mode RED/GREEN feedback (P1, small effort)
            Finish controller support (P2, very small effort)

Next      → See-through walls (P1, medium effort)
            Second and third tower types (P2)
            HUD improvements + settings persistence (P2)

Then      → Levels as Files + wave system (P3, unlocks content work)
            Towers as Files (P3, follows naturally)

Later     → Visual pazazz pass (P2/P3)
            Multiple characters (P4)
            Multiplayer (P4)
```
