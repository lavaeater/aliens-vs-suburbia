# Developer Guide — Aliens vs Suburbia

A map of the whole game for a programmer sitting down to change something. It answers
two questions: **"where is X implemented?"** and **"where do I tweak X's behavior?"**

Companion docs: `CLAUDE.md` (architecture summary + gotchas), `docs/ultraviolence.md`
(the gore/violence feature plan + status), `docs/inverse-kinematics-hardpoints.md`
(weapon-mounting math), `docs/skein-analysis.md` (Blender-authoring evaluation).

---

## 1. The shape of the code

- **Engine:** Bevy 0.18, ECS + plugins. Physics: avian3d 0.6 (fixed timestep 0.05s).
- **Root plugin:** `GamePlugin` in `src/game_state/game_state_plugin.rs` composes every
  subsystem plugin. Start reading there to see everything that's wired up.
- **Entry point:** `src/main.rs` — module list + `App` construction, adds `GamePlugin`,
  `SkeinPlugin`, inspector, etc.
- **States:** `GameState` in `src/game_state/mod.rs`:
  `Menu, PlayerSetup, InGame, ModelShowcase, CharacterCreator, PolyPizza, AssetBrowser,
  MapEditor`. Gameplay systems run `.run_if(in_state(GameState::InGame))`.
- **Message bus:** custom events derive `Message` and use `MessageReader`/`MessageWriter`,
  registered with `app.add_message::<T>()`. (Bevy 0.18 renamed Event→Message.)

### Scheduling note

Most gameplay is on `Update`. **Input + movement is on `PreUpdate`**
(`StatefulControlPlugin`) so aim/facing is settled before `Update` systems read it. Two
systems in one `add_systems((...))` tuple run in registration order; add `.chain()` to
force it (see the control plugin).

---

## 2. "Where is X?" quick index

| I want to change...                    | Look in                                                   |
|----------------------------------------|-----------------------------------------------------------|
| What plugins exist / load order        | `src/game_state/game_state_plugin.rs`                     |
| Keyboard controls                      | `src/control/keyboard_input.rs`                           |
| Gamepad controls                       | `src/control/gamepad_input.rs`                            |
| **Mouse aim / body facing**            | `src/control/mouse_aim.rs`                                |
| Player movement (physics)              | `src/general/systems/dynamic_movement_system.rs`          |
| Auto-aim (gamepad)                     | `src/player/systems/auto_aim.rs`                          |
| Player spawn / model / scale           | `src/player/systems/spawn_players.rs`                     |
| Special abilities (Q)                  | `src/player/systems/abilities.rs`                         |
| **Weapon equip / hardpoint snapping**  | `src/player/systems/equip.rs`                             |
| **Gun firing / muzzle flash / tracer** | `src/player/systems/shoot.rs`                             |
| **Blood splatter + decals**            | `src/gore/blood.rs`                                       |
| **Gibs (body chunks)**                 | `src/gore/gibs.rs`                                        |
| **Fire / molotov fields + scorch**     | `src/gore/fire.rs`                                        |
| **Destructible terrain (rubble)**      | `src/gore/terrain.rs`                                     |
| **Gore sound effects**                 | `src/gore/sfx.rs`                                         |
| **Barks (one-liners)**                 | `src/gore/barks.rs`                                       |
| The damage/death event bus             | `src/gore/components.rs`                                  |
| Aliens spawning / waves                | `src/alien/`, `src/alien/wave_manager.rs`                 |
| Enemy AI behaviors                     | `src/ai/systems/`                                         |
| Map tiles / pathfinding / walls        | `src/map/`, `src/general/systems/map_systems.rs`          |
| Health / death / touch damage          | `src/general/systems/*health*`, `*touch_damage*`          |
| Economy (coins / wallet)               | `src/general/systems/coin_system.rs`                      |
| Towers (shoot/slow/area)               | `src/towers/systems.rs`                                   |
| Build mode                             | `src/building/systems.rs`                                 |
| Animations                             | `src/animation/`, `src/model_settings/`                   |
| Camera                                 | `src/camera/systems.rs`                                   |
| Music                                  | `src/music/game_music_plugin.rs`                          |
| HUD / menus                            | `src/ui/spawn_ui.rs`                                      |
| Model defs (`.ron`)                    | `src/assets/asset_definition.rs`, `assets/defs/*.ron`     |
| Asset importer tool                    | `src/asset_browser/`                                      |
| Map editor tool                        | `src/map_editor/`                                         |
| Facts / stories engine                 | `src/facts/`, `turbofacts` crate                          |

---

## 3. Player: control, movement, aiming

**Spawn** — `src/player/systems/spawn_players.rs`. Reads `PlayerRoster` (from
`GameState::PlayerSetup`), loads each player's `AssetDefinition`, resolves ability + throw
rate + `weapon`, and inserts `PendingEquip` (see §5). `PlayerModelRoot` marks the scaled
scene child. `ability_for_slot` cycles abilities when a def doesn't specify one.

**Control** is tank-style by default, translated into a `CharacterControl` component
(`src/control/components.rs`): `directions` (W/S), `rotations` (A/D), `triggers`
(Throw/Jump/Build). The keyboard player carries `InputKeyboard`, the gamepad player
`InputGamepad`.

**Movement** — `src/general/systems/dynamic_movement_system.rs`. Players are avian dynamic
bodies; `dynamic_movement_keyboard` sets `LinearVelocity` from facing × `walk_direction` ×
`speed`, and `AngularVelocity.y` from torque × `turn_speed`.

**Aiming** — the fire direction is the `AutoAim(Vec3)` component:
- **Keyboard player → mouse** (`src/control/mouse_aim.rs`): `mouse_aim` projects the cursor
  onto the ground plane (through the orthographic `GameCamera`) and points `AutoAim` at it;
  `mouse_face` turns the body to face `AutoAim` by steering yaw angular velocity. This runs
  *after* movement so it overrides A/D rotation. **To revert to tank-only aim**, remove
  `mouse_face` from the control plugin chain; **to only aim (not turn the body)**, likewise.
- **Gamepad player → auto-aim** (`src/player/systems/auto_aim.rs`): snaps `AutoAim` to the
  closest alien within the FOV cone while Throw is held. FOV width = `PLAYER_FOV_DOT` in
  `src/constants.rs` (0.8; lower = wider). `auto_aim` excludes `InputKeyboard`.

**Death / revive** — `src/player/systems/death_revive.rs` (`PlayerDead` marker; downed, not
despawned).

---

## 4. Special abilities (the Q key)

`src/player/systems/abilities.rs`. `SpecialAbility` enum: `Bombardment` (screen nuke),
`Healing`, `Whirlwind` (speed + `TouchDamage`), `GoldDigger` (vacuum coins), `Molotov`
(rains fire — see §7d). Charge fills by throwing (`AbilityCooldown::add_throw`); fires when
full. `throws_to_charge()` sets cost per ability. Input arrives via the `AbilityInput`
resource (set by keyboard/gamepad). Assigned from `PlayerProps.ability` in the def, mapped
`PlayerAbility → SpecialAbility` in `spawn_players.rs`.

**Add a new ability:** add a variant to `SpecialAbility` *and* `PlayerAbility`
(`asset_definition.rs`), then fill the four match sites the compiler flags
(`throws_to_charge`, `label`, `activate_ability`, the mapping). `Molotov` is the worked
example.

---

## 5. Weapons & hardpoints (mounting a gun on a hand)

The math (`src/assets/hardpoint.rs`, unit-tested) makes a weapon's `grip` frame coincide
with a character's `grip` frame. Full write-up in `docs/inverse-kinematics-hardpoints.md`.

**Hardpoints are generic named frames.** A def's `hardpoints: HashMap<String, Hardpoint>`
(`asset_definition.rs`) maps a role → `Hardpoint { anchor, translation, rotation_euler_deg }`.
Roles are free-form strings; the standard set is authored in the asset browser via
`HARDPOINT_ROLES` in `src/asset_browser/state.rs` (`grip`, `foregrip`, `stock`, `sight`,
`muzzle`). **To add a new mount point, add its name to that list** — storage is already
generic.

**Equip flow** — `src/player/systems/equip.rs`:
1. `spawn_players` inserts `PendingEquip` (resolved from the character's `grip` + the weapon
   def's `grip`/`muzzle` + combat stats).
2. `equip_pending_weapons` waits for the skeleton, spawns the weapon scene parented to the
   grip bone, and attaches `WeaponModel` (snap data) + `Weapon` (combat stats).
3. `keep_weapons_snapped` re-snaps every frame (survives the spawn-frame scale race) and
   applies **recoil**. Bone-baked scale is cancelled by `weapon_local_scale`.

Tweakables: `EQUIP_MAX_TRIES` (how long to wait for the skeleton); the recoil decay/kick in
`keep_weapons_snapped` (`14.0` decay, `0.28` kick set in `shoot.rs`); role names `GRIP_ROLE`
/ `MUZZLE_ROLE`.

---

## 6. Gun firing — muzzle flash, tracer, recoil

`src/player/systems/shoot.rs`. When a player holds Throw *and* has an `EquippedWeapon`,
`shoot_weapons` hitscans from the weapon's `muzzle` world position along `AutoAim`, damages
the first `Health` entity hit (emitting `DamageDealt`), and spawns visuals. `throwing`
(`src/general/systems/throwing_system.rs`) is gated `Without<EquippedWeapon>`, so the same
button throws when unarmed and shoots when armed.

**Where each property lives:**

| Property                        | Where                                                         |
|---------------------------------|--------------------------------------------------------------|
| Damage, fire rate, range, spread, pellets, auto | `WeaponProps` in the weapon `.ron` def (`asset_definition.rs`) |
| Muzzle position (flash/tracer origin) | the weapon def's `muzzle` hardpoint (author in browser) |
| **Muzzle flash** look           | `spawn_muzzle_flash` in `shoot.rs` (color, `Sphere::new(0.06)`, `Ephemeral::new(0.06).with_grow(1.8)`) |
| **Tracer** look                 | `spawn_tracer` in `shoot.rs` (color, `0.02` thickness, `Ephemeral::new(0.05)`) |
| Recoil kick / decay             | `weapon.recoil = 0.28` in `shoot.rs`; decay in `keep_weapons_snapped` |
| What bullets can hit            | the `SpatialQueryFilter` mask in `shoot.rs` (`Alien`, `ImpassableAll`) |

Fire direction is `AutoAim` (so it follows the mouse); the muzzle is only the *visual*
origin. Aim and origin are intentionally decoupled.

---

## 7. The gore / ultraviolence layer (`src/gore/`)

Everything violent hangs off **one message bus** so gore is a set of subscribers, not
special cases. `GorePlugin` (`src/gore/plugin.rs`) wires it all; `GameState::InGame` gates
the `Update` systems; asset setup runs at `Startup`.

### 7a. The bus — `src/gore/components.rs`

- `DamageDealt { target, position, normal, amount, kind, lethal }` — emitted **wherever
  damage is applied**: `collision_handling_system` (thrown balls), `touch_damage_system`
  (melee), `shoot.rs` (guns), `fire.rs` (burning). `DamageKind` = `Ballistic/Blunt/Fire/
  Explosive`.
- `EntityDied { entity, position, normal, kind }` — emitted by `health_monitor_system`
  before it despawns a dead non-player.
- `LastHit` — stamped onto damaged entities by `record_last_hit` so the death path can
  attribute the killing blow's direction/kind.
- `Ephemeral { timer, fade, grow_to, base_scale }` + `tick_ephemeral` — the generic
  grow/fade-then-despawn used by every short-lived visual (blood puff, dust, flame, flash).
- `GoreBudget { max_decals, max_gibs }` — recycles oldest-first so a big wave can't spawn
  thousands of persistent entities. **Tune the caps here.**

**To make anything react to combat, subscribe to `DamageDealt`/`EntityDied`** — don't add a
new special case in the combat systems.

### 7b. Blood — `src/gore/blood.rs`

`spawn_blood_on_damage` (skips `DamageKind::Fire`). Two things per hit: a bright grow-and-
fade **impact puff** and a persistent **ground decal** (budget-capped). The splat texture is
generated procedurally (`make_blood_texture`, `TEX_SIZE`). Lethal hits bleed ×1.8.

Tweak: puff size/lifetime and decal size/darkness in `spawn_blood_on_damage`; texture shape
in `make_blood_texture`.

### 7c. Gibs — `src/gore/gibs.rs`

`spawn_gibs_on_death` bursts a corpse into `GIB_COUNT` (6) dark-red physics chunks on
`EntityDied`, thrown from the killing blow, lasting `GIB_LIFETIME` (6s), budget-capped.
Tweak count/lifetime/impulse/color here. Chunk collision mask (world only, not creatures) is
set in the spawn.

### 7d. Fire / molotov — `src/gore/fire.rs`

`SpawnFire { position, radius, duration, dps }` → `spawn_fire_fields` creates a `FireField`;
`tick_fire_fields` burns creatures inside on a repeating tick (emitting `DamageDealt(Fire)`),
throws flame puffs, and lays a **scorch decal** on burnout. The **Molotov ability**
(`abilities.rs`) writes a ring of `SpawnFire`. `SpawnFire` is the reusable hook for future
thrown molotovs.

Tweak: damage-tick / flame-tick rates in `spawn_fire_fields`; the ring size/count/dps in the
`Molotov` arm of `activate_ability`; scorch look in `setup_fire_assets`.

### 7e. Destructible terrain — `src/gore/terrain.rs`

`destroy_damaged_terrain` owns the death of any `IsObstacle` with `Health` ≤ 0 (walls,
towers): re-opens the tile in the pathfinding grid (so aliens re-route via
`recheck_path_after_tile_opened`), bursts grey **rubble** + a **dust** puff, and despawns it.
`health_monitor_system` is `Without<IsObstacle>` so structures crumble to rubble, not flesh.
Bullets already damage any wall with `Health` (the shoot raycast hits `ImpassableAll`).

Tweak: `CHUNK_COUNT`/`CHUNK_LIFETIME`, rubble color in `setup_debris_assets`. Give a wall HP
by setting `TerrainProps.health: Some(n)` in its def (else it's `Indestructible`).

### 7f. SFX — `src/gore/sfx.rs`

`emit_combat_sfx` turns bus messages into `PlaySfx { kind, gain_db }`; `play_sfx` plays a
one-shot `bevy_seedling` voice with pitch/gain jitter, capped at `MAX_VOICES` (24).
`setup_sfx_bank` scans `assets/sfx/` at startup and loads `.wav`s by filename prefix
(`hit/death/gib/fire/shoot/bark`). **Silent until you add samples** — see
`assets/sfx/README.md`. Tune per-category gain in `emit_combat_sfx`.

### 7g. Barks — `src/gore/barks.rs`

`bark_on_events` shows ultraviolent one-liners as a fading bottom caption (+ `bark*.wav`),
triggered by kills/multikills/player-hurt/near-death on `BARK_COOLDOWN` (3.5s). The
`AtrocityMeter` (body count) drifts the line pools and caption color from zeal → haunted at
`HAUNTED_AT` (45). **Edit the lines** in the `KILL/MULTI_KILL/HURT/LOW_HEALTH` tables (ASCII
only — the default font has no Unicode). Migrating triggers to authored `turbofacts` stories
is the planned refinement.

---

## 8. Aliens, waves, AI

- **Spawning** — `src/alien/systems/spawn_aliens.rs`, driven by `WaveManager`
  (`src/alien/wave_manager.rs`). Waves come from `MapFile.waves` or the hardcoded default in
  `WaveManager::default()`. `wave_system` counts down `wave_timer`, then spawns.
- **AI** — one submodule per behavior under `src/ai/systems/`:
  `approach_and_attack_player`, `avoid_walls`, `move_towards_goal`, `move_forward`,
  `destroy_the_map`. When an alien can't path to the goal it gets `MustDestroyTheMap` and
  chews through walls; `recheck_path_after_tile_opened` clears that when a gap opens.
- **Touch damage** — `src/general/systems/touch_damage_system.rs`
  (`TouchDamage { dps }` on aliens hurts players; emits `DamageDealt(Blunt)`).

Tweak wave composition: `MapFile.waves` in the map `.ron`, or the default in `wave_manager.rs`.

---

## 9. Map, pathfinding, economy, towers, building

- **Map load/spawn** — `src/general/systems/map_systems.rs` builds floor colliders (greedy
  rectangle merge), floor visuals, walls, and def-driven `placements`. `MapFile`
  (`src/general/components/map_components.rs`) is the on-disk format (`assets/maps/*.ron`):
  tile grid, decorations, placements, waves; `generated: true` uses the seeded generator in
  `src/map/map_generator.rs`.
- **Pathfinding** — `MapGraph` (`src/general/resources/map_resources.rs`), A* via the
  `pathfinding` crate. `path_reopened` flag drives alien re-routing.
- **Economy** — `src/general/systems/coin_system.rs`: aliens drop `Coin` on death, players
  auto-collect into the shared `TeamWallet`.
- **Towers** — `src/towers/systems.rs`: `shoot_alien_system`, `slow_alien_system`,
  `area_damage_system`. Stats from `TowerProps` in the def.
- **Building** — `src/building/systems.rs`: enter/exit build mode, preview tint, and
  `execute_build` (checks `TeamWallet`; costs in `tower_cost`).

---

## 10. Presentation: animation, camera, music, UI

- **Animation** — `src/animation/` (state-machine, `AnimationStore`) + `src/model_settings/`
  (`build_player_anim_graph`). Game keys → tag paths → clips via `animation_bindings` /
  `clip_tags` in the def. External clips need identical bone paths (no retargeting).
- **Camera** — `src/camera/systems.rs`: single orthographic isometric `GameCamera`,
  `camera_follow` tracks players; wall-occlusion fade. (`spawn_pixelated_camera` is dead
  code.)
- **Music** — `src/music/game_music_plugin.rs`: generative soundtrack (`rusty_music`
  submodule). `MusicMoods { combat, danger }` computed from game state drives intensity and
  gates instrument channels. This is the signal the future "despair" pass hangs off.
- **UI/HUD** — `src/ui/spawn_ui.rs` via `lava_ui_builder`. HUD shows aliens, wave, coins,
  ability charge, build cost. Note: the bark caption (`src/gore/barks.rs`) is a separate
  bottom-screen `Text` node, not part of this HUD.

---

## 11. Model definitions & authoring tools

- **`AssetDefinition`** (`src/assets/asset_definition.rs`) — the per-model `.ron`
  (`assets/defs/*.ron`): `model_path`, `scale`, `model_type` (`Player/Tower/Terrain/Item/
  Enemy/Weapon` each with props), `hidden_nodes`, `clip_tags`, `animation_bindings`,
  `animation_sources`, `attachments`, `hardpoints`. This is the main data-driven knob for
  per-model behavior.
- **Asset browser** (`src/asset_browser/`, `GameState::AssetBrowser`) — import a GLB, set
  scale/height, tag clips, author hardpoints (incl. `muzzle`), preview a weapon on the model,
  export `.ron` with `I`.
- **Map editor** (`src/map_editor/`, `GameState::MapEditor`) — grid layout, palette filtered
  by `ModelType`, wave editor, save with `S`.

---

## 12. Tuning cheat-sheet

| Effect                          | File : symbol                                              |
|---------------------------------|-----------------------------------------------------------|
| Gun damage/fire rate/spread     | weapon `.ron` `WeaponProps`                                |
| Thrown-ball damage              | `collision_handling_system.rs : BALL_DAMAGE`              |
| Muzzle flash / tracer look      | `shoot.rs : spawn_muzzle_flash / spawn_tracer`           |
| Recoil kick / decay             | `shoot.rs` (`0.28`) / `equip.rs : keep_weapons_snapped`   |
| Blood amount / decal size       | `blood.rs : spawn_blood_on_damage`                        |
| Gib count / lifetime            | `gibs.rs : GIB_COUNT / GIB_LIFETIME`                      |
| Rubble count / lifetime         | `terrain.rs : CHUNK_COUNT / CHUNK_LIFETIME`               |
| Fire dps / duration / ring      | `abilities.rs` Molotov arm; `fire.rs : tick rates`       |
| Max persistent gore             | `components.rs : GoreBudget` caps                         |
| SFX volume / voice cap          | `sfx.rs : emit_combat_sfx gains / MAX_VOICES`            |
| Bark lines / pacing / drift     | `barks.rs : KILL... / BARK_COOLDOWN / HAUNTED_AT`        |
| Aim cone width (gamepad)        | `constants.rs : PLAYER_FOV_DOT`                          |
| Mouse turn responsiveness       | `mouse_aim.rs : mouse_face` gain (`12.0`)                |
| Wave composition                | map `.ron` `waves` / `wave_manager.rs` default           |
| Tower costs                     | `building/systems.rs : tower_cost`                       |
| Physics tick rate               | `game_state_plugin.rs` (`Time::<Fixed>::from_seconds`)   |

---

## 13. Testing

Run: `cargo test`. Tests live in `#[cfg(test)] mod tests` blocks next to the code.

### Pure-logic tests

Prefer these where logic is a plain function or method — they're fast and need no `App`.
Examples in the tree: `assets/hardpoint.rs` (frame math), `gore/components.rs`
(`GoreBudget` recycling), `player/systems/shoot.rs` (`Weapon::from_props`),
`player/systems/abilities.rs` (`AbilityCooldown`), `control/components.rs`
(`CharacterState`), `alien/wave_manager.rs` (wave counters).

### System tests (the Bevy pattern)

Follows [Bevy's `how_to_test_systems`](https://github.com/bevyengine/bevy/blob/main/tests/how_to_test_systems.rs):
build a headless `App`, register messages, add the system, spawn inputs, `app.update()`,
assert. Worked example: `general/systems/health_monitor_system.rs` (dead entity → despawn +
`EntityDied`; live entity untouched).

Skeleton:

```rust
#[test]
fn my_system_does_the_thing() {
    let mut app = App::new();
    app.add_message::<SomeMessage>();
    app.add_systems(Update, my_system);
    let e = app.world_mut().spawn(SomeComponent { .. }).id();

    app.update();

    // component state
    assert!(app.world().get::<SomeComponent>(e).is_some());
    // or: message was emitted — read it with a MessageReader in a small catch system,
    // or inspect `app.world().resource::<Messages<SomeMessage>>()`.
}
```

**Guidelines when adding tests:**
- Test *decisions*, not Bevy plumbing: damage thresholds, cooldown/charge math, state
  resolution, pathfinding-reopen conditions, budget recycling.
- Keep system tests headless — don't add `DefaultPlugins`, rendering, or physics unless the
  logic needs them. Spawn only the components the system's query requires (use `Option<>`
  fields to skip the rest).
- For messages, a tiny "catch" system that pushes into a `Resource` is the most robust way
  to assert what was emitted.
```
