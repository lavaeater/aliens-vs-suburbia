# Gameplay MVP Roadmap

Companion to `docs/gameplay.md`. Every heading there has a matching section here, in the
same order, with what already exists in the code, what is missing, and the concrete steps to
close the gap. File references point at the current implementation so each step starts from
something real.

Legend for **Status**: `done` = nothing to do, `partial` = exists but needs the listed work,
`missing` = start from scratch, `human` = content work, not code.

The suggested build order is at the end (see *Phases*); the sections themselves follow
`gameplay.md`.

---

## Multiplayer

**Status: done (2026-09-20).** `camera_follow` (`src/camera/systems.rs`) aims at the
weighted centroid of every `CameraTarget` (downed players count half), smooths it into the
`CameraFocus` resource and zooms out (`fit_factor`) until everyone fits. Tunables in
`GameSettings`: `fit_margin`, `fit_zoom_max`, `focus_smoothing`. Not yet playtested with
real pads - the numbers are first guesses.

**Steps** (all done)

1. Add a `CameraTarget { weight: f32 }` component (`src/camera/components.rs`) and insert it
   on every player in `spawn_players.rs`. Weight lets a downed player count for less later.
2. Rewrite `camera_follow` to compute the weighted centroid of all `CameraTarget` positions
   and aim at that. Keep the existing `CameraOffset` math untouched.
3. Add a `CameraFocus` resource holding the smoothed centroid so the HUD, split-screen and
   Bombardment ("all aliens on screen") can read the same point instead of re-deriving it.
4. Smooth the centroid with a simple exponential lerp so a player joining or dying does not
   snap the view.
5. Zoom-to-fit: scale `GameSettings::zoom` (or a runtime override) by the bounding radius of
   the players, clamped to `[min_zoom, max_zoom]`. This is the cheap fallback that makes
   *Split screen* optional for the first playtests.

---

## Split screen

**Status: missing.** The camera is a single `Camera3d`; the playground already proves that
`Camera::viewport` clipping works (`src/playground/`), which is the building block.

This is the largest single item on the list. Do *Multiplayer* step 5 first and playtest; only
build this if zoom-to-fit is not enough.

**Steps**

1. **Grouping.** Each frame, cluster players by distance: union-find over pairs closer than
   `split_distance` (a `GameSettings` field). Output `Vec<Group { members, centroid }>`.
   Pure function, unit-tested.
2. **Layouts.** A function `layout(groups: usize, aspect) -> Vec<Rect>` returning viewport
   rects in normalized window coordinates: 1 = full, 2 = vertical halves, 3-4 = quadrants
   (fourth quadrant shows the map overview or stays blank when there are 3 groups).
3. **Camera pool.** Spawn one `GameCamera` per possible group (max 4) at `OnEnter(InGame)`,
   all but the first with `Camera::is_active = false`. A `CameraSlot(usize)` component ties a
   camera to a layout index.
4. **Assignment system** (`PostUpdate`, after physics writeback, before `camera_follow`):
   activate/deactivate cameras to match the group count, set each `Camera::viewport` from
   the layout, and write the group's centroid into the camera's follow target. `camera_follow`
   from *Multiplayer* then works per camera unchanged.
5. **Hysteresis.** Splitting at 12 units and merging at 10 avoids flicker at the boundary.
   Optionally animate the viewport rect over ~0.3 s.
6. **UI.** Bevy UI is per window, not per camera; the HUD from *HUD Information* is
   already one quarter per player at the bottom, so it does not need to move. Health bars
   (`WorldFollower`) project through one camera - add a `UiTargetCamera` per follower or
   accept that they only show in the first viewport.
7. **Occlusion fading** (`src/camera/`) must run per camera; make its query iterate all
   active `GameCamera`s.

---

## Weapons

**Status: partial.** Guns exist as `Weapon` defs (`assets/defs/Pistol.ron`,
`Assault Rifle.ron`) with `WeaponProps { damage, fire_rate_per_minute, range, spread_deg,
pellets, auto, hands }` (`src/assets/asset_definition.rs`), equipped via `PlayerProps.weapon`
(`src/player/systems/equip.rs`) and fired hitscan in `shoot_weapons`
(`src/player/systems/shoot.rs`). There is **no ammo of any kind** - every weapon fires forever.

**Steps**

1. Add `ammo: AmmoKind` and `magazine: u32` to `WeaponProps` (see *Ammo* for the enum).
   `Weapon` (runtime) gets `rounds_in_mag`, `reload_secs`, `reloading: Option<Timer>`.
2. Reload: on empty magazine or a reload input (add `ControlCommand::Reload`, keyboard `R`,
   gamepad `West`/Square is taken by build mode - use `North`+hold or a new binding in
   `gamepad-bindings.ron`), start the timer, draw from the player's `AmmoPouch`
   (see *Ammo*), and play `AnimationKey::Reload` if the model has one.
3. Weapon switching: give the player a `Weapons { slots: Vec<Entity>, active: usize }`
   component. `equip.rs` currently snaps one weapon at spawn; extend it so non-active weapons
   are hidden (`Visibility::Hidden`) rather than despawned, and `1`/`2`/`3` or d-pad
   up/down cycle `active`.
4. Make hitscan vs projectile a per-def choice: `WeaponProps.projectile: Option<ProjectileProps>`
   - when present, `shoot_weapons` spawns a physics `Ball`-like entity through the existing
   `throwing_system` path instead of a ray. This is what grenade launchers and molotov
   launchers use (see *Thrown weapons*).
5. Add 2-3 more weapon defs via the asset browser (shotgun: `pellets: 8`, SMG: `auto: true`)
   so the difference is felt in play.

---

## Items

**Status: partial.** `ItemKind` (`src/assets/asset_definition.rs`) already has `Decorative`
and `HealthPickup { amount }`, but nothing at runtime *applies* a health pickup - the only
code touching it is the asset-browser label.

**Steps**

1. Extend `ItemKind`: `HealthPickup { amount }`, `AmmoPickup { kind: AmmoKind, rounds }`,
   `WeaponPickup { def: String }`, `Coins { value }`, `Key { id }` (for *Maps* / objectives).
   Keep `Decorative`.
2. `src/items/` module with an `Item(ItemKind)` component plus a `Pickup` marker. When the
   map loader (`map_systems.rs`) or the loot system spawns an `Item`-typed def, attach both.
3. Pickup system generalised from `coin_pickup_system` (`src/general/systems/coin_system.rs`):
   distance check against `PickupRange`, then `match kind` to apply the effect (heal, add
   ammo, equip weapon, add coins). Emit an `ItemPickedUp { player, kind }` message so HUD and
   SFX can react.
4. Migrate `Coin` onto this path (`Coin` becomes `Item(ItemKind::Coins)`), deleting the
   special-case coin system once tests are moved.
5. Bob/spin visual for pickups so they read as collectable (small `Update` system on
   `Pickup`, mirrors what `Coin` does today).

---

## Ammo

**Status: missing.**

**Steps**

1. `AmmoKind` enum in `asset_definition.rs`: `Pistol`, `Rifle`, `Shells`, `Grenade`,
   `Molotov`, `Infinite`. Serde-serialised so defs can name it.
2. `AmmoPouch(HashMap<AmmoKind, u32>)` component on the player, with per-kind caps in a
   `AmmoCaps` resource loaded from `game-settings.ron` (or hardcoded defaults first).
3. `shoot_weapons` decrements `Weapon::rounds_in_mag`; reload moves rounds from the pouch.
   `AmmoKind::Infinite` skips all of it (keeps the current behaviour for the playground).
4. Starting loadout: `PlayerProps.starting_ammo: Vec<(AmmoKind, u32)>`.
5. HUD hook: expose `(rounds_in_mag, pouch[kind])` for *HUD Information*.
6. Throwables (grenade, molotov) are ammo kinds, not weapons - a player with `Grenade: 3` can
   throw three (see *Thrown weapons*).

---

## Pickups

**Status: partial.** Coins drop and are auto-collected. Map-placed items via
`MapFile.placements` spawn but are inert.

**Steps**

1. Depends on *Items* steps 1-3; once those exist, map placements with `Item` defs are
   pickups for free.
2. Map editor: the palette already filters by `ModelType`; add an "Item" filter tab and show
   the `ItemKind` in the placement tooltip so level authors can see what they placed.
3. Spawn pickups from *Loot Drops* (enemy death) and from *Death* (dropped inventory).
4. Optional respawning pickups: `RespawnTimer` on a placement so a med-kit comes back after
   N seconds; useful for arena-style maps.

---

## Health

**Status: done.** `Health { health, max_health }` (`src/general/components/mod.rs`), health
bars via `AddHealthBar`, `health_monitor_system` despawns non-players at 0 and emits
`EntityDied`. `Health::full/apply/heal/is_dead` added 2026-09-20.

**Decision:** defs keep `f32` health (`health: 100.0` is what every existing `.ron` and the
asset-browser editors write; ron will not parse `100.0` as an `i32`). The `as i32` cast
happens once at each spawn site via `Health::full`.

---

## Damage

**Status: done (2026-09-20).** `src/general/damage.rs` owns it: every hit site writes an
`ApplyDamage` message and `apply_damage` is the only system that lowers `Health`. It
checks `Indestructible`, the `DamageRules` matrix (`Faction` on players/aliens/structures;
friendly fire off, aliens hurt structures, players do not - flip in the resource), applies
`DamageResistances` (from `resistances` in Enemy/Tower/Terrain props), and emits
`DamageDealt` + score events + the alien-counter decrement exactly once per kill. Found and
fixed on the way: tower/fire kills never decremented `AlienCounter`, `AlienKilled` carried
the alien instead of the killer so per-player kill score never counted, tower balls were
"thrown" by themselves, and Whirlwind's `TouchDamage` could only ever hurt players.

**Steps** (all done; step 6 tests live next to each writer plus `damage.rs`)

1. Introduce a single `ApplyDamage { target, amount, kind, position, normal, source }`
   message and one `apply_damage` system that: mutates `Health`, respects `Indestructible`,
   emits `DamageDealt` (gore) and `GameTrackingEvent::ShotHit/AlienKilled` (score). All the
   sites above become writers. This is the prerequisite for explosions, ammo types and
   friendly fire to be decisions instead of copy-paste.
2. Damage matrix: a `Faction` component (`Player`, `Alien`, `Structure`) and a
   `DamageRules` resource saying who can hurt whom (`friendly_fire: bool`,
   `aliens_hurt_structures: bool`). `apply_damage` consults it.
3. Per-`DamageKind` multipliers on the target (`DamageResistances` component, e.g. walls
   ignore `Blunt`, take 2x `Explosive`) - this is where "different weapons do different
   damage to walls and towers" lives, without special cases.
4. Make walls and towers valid targets: ensure `TerrainProps.health: Some(_)` spawns a
   `Health` in `map_systems.rs` (only `blocks_enemies` is read today) and that
   `shoot_weapons`' `targets` query covers them.
5. Fix `Bombardment` to go through `ApplyDamage` so it produces blood and score like every
   other hit.
6. Tests: `touch_damage_system` and `collision_handling_system` already have message-catching
   tests; move them to `apply_damage` and add cases for the matrix.

---

## Death

**Status: partial.** Down-but-not-out exists: `detect_player_death` inserts `PlayerDead`,
`player_revive_system` lets a living player hold `E` for 3 s within 1.8 units
(`src/player/systems/death_revive.rs`). No lives, no respawn, no drops, no gamepad revive,
and if *all* players are down nothing ends the level.

**Steps**

1. `Lives(u32)` component, from `GameSettings.lives_per_player` (default 3). Decrement on
   death.
2. Bleed-out: add `bleed_out: Timer` (10 s) to `PlayerDead`. Revive before it expires =
   current behaviour. Expiry = `PlayerRespawning { timer: 5 s, lives_left }` and despawn
   the body via the existing gore path (`EntityDied`).
3. Drop inventory: on death, spawn `Item` pickups for the active weapon (`WeaponPickup`)
   and the pouch contents (`AmmoPickup`) at the body position (needs *Items*).
4. Respawn placement: default = nearest free tile to the death position that is not
   within N units of an alien. While `PlayerRespawning`, left/right on the player's own
   device cycles a `respawn_anchor: Option<Entity>` through living players; on timer end,
   spawn beside the anchor if set. Reuse `spawn_players.rs` so the model, weapon and
   animation graph come from the same code as the initial spawn.
5. Gamepad revive: `ControlCommand::Interact` bound to `East` hold (or a new
   `gamepad-bindings.ron` entry); `player_revive_system` reads the command instead of
   `KeyCode::KeyE` directly.
6. Game over: when every player is `PlayerDead` with no lives left (or all
   `PlayerRespawning` with 0 lives), set `LevelState::Failed` in `score_keeper.rs`.
7. HUD: show lives next to the character name (*HUD Information*).

---

## Loot Drops

**Status: missing** (coins drop unconditionally in `spawn_coins_on_alien_death`). The Kotlin
reference is `~/projects/java/turbo-rocket-ultra/core/src/main/kotlin/gamePlay/pickups/LootTable.kt`:
weighted roll over `contents`, `count` rolls per evaluation, `always` entries, nested tables,
and a `NullValue` entry for "nothing".

**Steps**

1. `src/loot/` with:
   ```rust
   #[derive(Serialize, Deserialize)]
   pub enum LootEntry {
       Nothing { weight: f32 },
       Item { weight: f32, kind: ItemKind, count: u32 },
       Table { weight: f32, table: String },      // by name, nesting
   }
   pub struct LootTable { pub rolls: u32, pub always: Vec<LootEntry>, pub entries: Vec<LootEntry> }
   ```
   `LootTable::roll(&self, rng, &LootTables) -> Vec<(ItemKind, u32)>`. Pure, unit-tested
   (deterministic RNG, check `Nothing` weight is honoured, nested tables resolve).
2. `LootTables` resource loaded from `assets/loot/*.ron` (one table per file, keyed by stem).
3. `EnemyProps.loot_table: Option<String>`; `TerrainProps.loot_table` for breakable crates.
4. `spawn_loot_on_death` system subscribing to `EntityDied`, replacing
   `spawn_coins_on_alien_death`. Scatter drops in a small ring so several items do not stack.
5. Asset browser / map editor: dropdown to pick a loot table on an enemy or terrain def.

---

## Towers

**Status: partial.** `TowerShooter`, `TowerSlow`, `TowerArea` exist (`src/towers/`) with
`TowerProps { health, cost, range, damage, fire_rate_per_minute, resistances }`. Steps 1-2
done 2026-09-20.

**Steps**

1. ~~Route tower damage through `ApplyDamage`.~~ Done; the tower (sensor) is the source.
2. ~~Towers as `Faction::Structure`.~~ Done, on the root and the sensor child.
3. Tower kind in the def: `TowerProps.kind: TowerKind { Shooter, Slow, Area }` plus kind-
   specific numbers, so the map editor and build menu can list them from defs instead of
   code.
4. Build menu: cycle through all `Tower`-typed defs (build items already cycle with
   arrows/d-pad) and show cost + a one-line description in the HUD.
5. Evaluate in play: repair (spend coins to heal), sell (refund 50%), upgrade tiers - only
   after a playtest says towers matter.

---

## Tower construction

**Status: done** (`src/building/`). Only touch if playtests find issues.

**Steps**

1. Gamepad placement preview visibility - confirm the `BuildIndicator` is readable at the
   isometric angle on a TV.
2. Block placement on tiles occupied by pickups or downed players.

---

## Thrown weapons

**Status: partial.** Unarmed players throw physics balls (`throwing_system.rs`,
`collision_handling_system.rs` applies `Ballistic` damage on hit). `SpecialAbility::Molotov`
spawns `FireField`s in a ring, but there is no thrown molotov and no grenade.

**Steps**

1. `Throwable` def type or `ItemKind::Throwable { kind: ThrowableKind }`:
   `Grenade { fuse_secs, explosion }`, `Molotov { fire: FireProps }`. Consumes an ammo kind
   from *Ammo*.
2. Throw input: dedicated `ControlCommand::ThrowSpecial` (keyboard `G`, gamepad `L1`),
   separate from fire so a gun-wielder can still lob a grenade.
3. Spawn via the existing ball path with a `Fuse(Timer)` component; on expiry (grenade) or
   first `CollisionStarted` (molotov) emit `Explode`/`SpawnFire` at the entity's position.
4. Arc aiming: throw velocity from a fixed lob angle and the aim distance clamped to
   `max_throw_range`; draw a dotted arc while the button is held (gizmo or ephemeral meshes).
5. Retire `SpecialAbility::Molotov` in favour of the molotov throwable, or keep it as the
   "ring of fire" ultimate - decide after both exist.

---

## Explosions

**Status: missing.** `DamageKind::Explosive` is declared in `src/gore/components.rs` and
`spawn_flash` in `abilities.rs` is a usable placeholder visual.

**Steps**

1. `Explode { position, radius, max_damage, impulse, kind }` message in `src/general/`.
2. `explosion_system`: `SpatialQuery::shape_intersections` (avian) with a sphere of `radius`
   over layers `Alien | Player | ImpassableAll | Structure`. For each hit, `falloff = 1 -
   (dist / radius)^2` (clamped 0..1); write `ApplyDamage { amount: max_damage * falloff,
   kind: Explosive }` and apply `ExternalImpulse` (dynamic bodies) or a kinematic shove
   (`LinearVelocity += dir * impulse * falloff` for players/aliens using kinematic movement).
   Optional line-of-sight raycast to the hit so walls shield.
3. Visual: reuse `spawn_flash`, add a burst of `Gib`-style debris from `src/gore/gibs.rs`
   and a `FireField` if `kind == Fire`. SFX through `src/gore/sfx.rs`.
4. Camera shake: `CameraShake { amplitude, decay }` component read by `camera_follow`.
5. Writers: grenades (*Thrown weapons*), Bombardment (drop the flat 75 damage, emit N
   `Explode`s around `CameraFocus`), exploding barrels (`TerrainProps.explodes_on_death`),
   a future rocket launcher.
6. Unit test the falloff and the "walls block" rule in a headless app.

---

## Stories (goals, objectives, win and loss conditions)

**Status: done** per `gameplay.md`. `turbofacts` is wired in (`src/facts/`), and
`score_keeper.rs` holds win/lose (`aliens_win_cut_off`, `LevelState`).

**Steps** (only what other sections need)

1. Add the *Death* game-over condition to `LevelState::Failed`.
2. Add facts for `players_alive`, `lives_left`, `pickups_collected` so stories can react to
   the new systems.
3. `LevelState::Completed` currently goes to `Menu` after 2 s; *Maps* needs it to advance to
   the next level instead.

---

## Human-readable definition formats

**Status: done** (ron everywhere: defs, maps, settings, bindings).

**Steps**

1. New formats introduced by this roadmap, all ron: `assets/loot/*.ron` (*Loot Drops*),
   `assets/campaign.ron` (*Maps*), `assets/crawls/*.ron` (*On-Screen Crawls*).
2. Every new def field gets `#[serde(default)]` so old files keep loading.

---

## Gamepad support

**Status: partial** - implemented (`src/control/gamepad_input.rs`, `bindings.rs`) but
untested in a real session.

**Steps**

1. Test session checklist: join in setup, move, aim with right stick, fire, build mode
   (Square), place (Cross), cancel (Circle), ability (Triangle), cycle build item (d-pad).
   Log every gap in this doc.
2. Bind new commands added above: reload, interact/revive, throw special, weapon cycle,
   respawn-anchor cycle. Add them to `GamepadBindings` and the default `gamepad-bindings.ron`.
3. Deadzone and stick-curve fields in `GameSettings` if the twin-stick aim feels twitchy.
4. Two pads + keyboard on one machine (`assign_gamepads` slot mapping).

---

## Game setup screen

**Status: done.** All suggested changes are ticked in `gameplay.md`.

**Steps**

1. Show the character's weapon and ability under the model so the choice means something
   (read `PlayerProps` of the highlighted def).
2. Start button gating: with one player joined, "Enter again starts" is fine; with several,
   require all Ready - already implemented, just verify with two pads.

---

## At least 4 playable characters

**Status: human.** Mesh2Motion characters are available; each needs an
`assets/defs/<name>.ron` via the asset browser (scale, hidden nodes, clip tags, bindings,
`grip` hardpoint, weapon).

**Steps**

1. Import four characters in the asset browser; verify `Idle/Walk/Run/Death` bindings and the
   `grip` hardpoint in the playground (`cargo run -- --playground`).
2. Give each a distinct `PlayerProps { ability, weapon, starting_ammo }` so the roster
   choice has gameplay weight.
3. Playground checklist per character: torso twist, weapon snap, death animation, revive
   animation.

---

## At least 5 enemies

**Status: partial / human.** Four `Enemy` defs exist (`Alien`, `Blue Demon`, `Demon`,
`Bunny`) but `spawn_aliens` (`src/alien/systems/spawn_aliens.rs`) always uses
`game_assets.alien_scene` and `Health::default()`; `WaveDef.enemy_def` is stored but never
read by the wave manager. So today every enemy is the same alien.

**Steps**

1. Make `SpawnAlien` carry the def path; `spawn_aliens` loads the `AssetDefinition`,
   applies `EnemyProps { health, speed, coin_drop }` and the model + animation graph the same
   way `spawn_players.rs` does for players.
2. `wave_manager.rs` passes `WaveDef.enemy_def` into `SpawnAlien`.
3. Extend `EnemyProps` with behaviour knobs: `touch_dps`, `attack: Melee | Ranged { weapon }`,
   `prefers: Players | Goal | Structures`, `loot_table`. Map these onto the existing AI
   components (`ApproachAndAttackPlayerData`, `MoveTowardsGoalData`, `MustDestroyTheMap`).
4. Design five archetypes: runner (fast, low HP), brute (slow, high HP, hurts walls), ranged
   (stops at range and shoots via the player weapon path), swarm (tiny, many), exploder
   (emits `Explode` on death). Each is a def plus, at most, one new AI submodule (ranged).
5. Health bars scaled to `max_health` so a brute reads as tanky.

---

## Maps

**Status: partial.** Maps exist (`assets/maps/level_01..02, map_1..3, level_666`) and the
map/house editors work, but `map_systems.rs` hardcodes `assets/maps/level_01.ron` and
completion returns to the menu.

**Steps**

1. `assets/campaign.ron`: `Campaign { levels: Vec<LevelEntry { map, title, intro_crawl:
   Option<String>, outro_crawl: Option<String> }> }`.
2. `CampaignProgress { index }` resource; `load_map` reads `campaign.levels[index].map`.
   On `LevelState::Completed`, increment and re-enter `InGame` (via `Menu`-less
   `GotoState`), on the last level go to a `Victory` screen. `Failed` offers retry.
3. Carry player state between levels: `Lives`, `AmmoPouch`, weapons, `TeamWallet` survive
   the `clear_game_entities_plugin` sweep (store a `CarryOver` resource on exit, apply on
   spawn).
4. Author five maps with a difficulty curve: 1 tutorial (one lane, pistol ammo everywhere),
   2 two lanes, 3 introduce brutes + destructible walls, 4 arena with pickups respawning,
   5 boss/exploder finale. Use `waves` per map.
5. Level-select in the menu for testing (dev only).

---

## Transitions

**Status: missing.** State changes are hard cuts.

**Steps**

1. `src/transitions/` with a full-screen `Node` overlay (`ZIndex` max) and a
   `Transition { kind: Fade | Wipe, phase: Out | In, timer }` resource.
2. `GotoState` becomes two-phase: fade out (0.3 s), then the real state switch, then fade in.
   Every existing `GotoState` writer keeps working; only `goto_state_system` changes.
3. Level-start card: title from `campaign.ron` shown for 1.5 s over the fade-in.
4. "Wave N incoming" banner using the same overlay module.

---

## Filters or VFX

**Status: partial.** `spawn_pixelated_camera` in `src/camera/systems.rs` already renders to a
480x360 texture with nearest sampling and blits it - it is just not used.

**Steps**

1. Wire `spawn_pixelated_camera` behind `GameSettings.pixelate: bool` and make the
   `PixelCanvas` sprite follow window resizes (the `resize_canvas` system exists).
2. CRT: a post-process `Material2d` on the canvas sprite with scanlines, slight barrel
   distortion and vignette (WGSL shader in `assets/shaders/crt.wgsl`). Toggle with `F8`.
3. Note for *Split screen*: pixelation renders each viewport camera to the same texture -
   the canvas approach still works because viewports are set on the 3D cameras.
4. Keep both off by default until the art direction is decided; they are cheap to toggle.

---

## HUD Information

**Status: partial.** Steps 1-3 done 2026-09-20: `src/ui/player_hud.rs` spawns a bottom
bar with four `PlayerHudSlot`s keyed on the new `PlayerSlot` component; each shows name
(def stem), health bar + numbers, weapon (`Name` on the equipped weapon entity), an
`Ammo: --` placeholder and ability charge; downed players read "NAME - DOWN". Team info is
one row top-left. Lives (step 2) wait for *Death*.

**Steps**

1. ~~Bottom bar with four `PlayerHudSlot(usize)` children.~~ Done.
2. ~~Per slot: name, health, weapon, ammo, ability.~~ Done except lives (needs *Death*)
   and real ammo numbers (needs *Ammo*).
3. ~~Move team-wide info to a slim top bar.~~ Done.
4. Downed state: slot dims and shows "DOWN - 8s" from `PlayerDead.bleed_out`; respawning
   shows the countdown and the chosen anchor player's name.
5. ASCII only (Bevy default font).

---

## On-Screen Crawls

**Status: missing.**

**Steps**

1. `assets/crawls/<name>.ron`: `Crawl { lines: Vec<String>, speed: f32, style: Star |
   Dialog }`.
2. `src/crawl/`: `ShowCrawl(name)` message; spawns a `StateMarker` UI root with the text.
   `Star` style: text scrolls upward and shrinks (scale `Transform` on the UI node, or a
   perspective-tilted `Camera2d` quad); `Dialog` style: a bottom box that types text in at
   `speed` chars/s.
3. Skip with fire/Enter/Cross; on finish emit `CrawlFinished`.
4. Hook into *Maps*: `intro_crawl` plays after the level-start fade-in and before
   `LevelState::InProgress` (waves wait), `outro_crawl` before advancing.
5. Stories can trigger a crawl mid-level through a `StoryEffect` handler in
   `src/facts/game_integration.rs` (currently only logs effects).

---

## Phases

Ordered so every phase ends in something playtestable with friends and family.

| Phase | Goal | Sections | Notes |
|-------|------|----------|-------|
| 1 | Co-op that does not fight the camera | Multiplayer (1-5), Gamepad support (1), HUD Information (1-3) | **Code done 2026-09-20.** Remaining: the gamepad test session (human). |
| 2 | One damage pipeline | Damage (1-6), Health (1-2), Towers (1-2) | **Done 2026-09-20.** 249 tests green. |
| 3 | Guns feel different | Ammo, Weapons (1-3, 5), Items (1-3), Pickups (1-2) | Ammo scarcity is the first real tuning knob. |
| 4 | Dying matters | Death (1-7), Loot Drops, HUD Information (4) | Lives, bleed-out, drops, respawn anchors. |
| 5 | Things go boom | Explosions, Thrown weapons, Weapons (4) | Grenades, molotovs, Bombardment rewrite. |
| 6 | Enemy variety | At least 5 enemies, Towers (3-5) | Def-driven spawning first, then archetypes. |
| 7 | A campaign | Maps, Stories (1-3), Transitions, On-Screen Crawls | Five maps strung together with fades and crawls. |
| 8 | Polish | Split screen, Filters or VFX, Game setup screen, At least 4 playable characters | Split screen only if phase 1's zoom-to-fit fails in testing. Character imports can happen any time. |

Each phase should end with a `cargo test` pass and a session note appended to
`docs/gameplay.md` saying what was played and what felt wrong; those notes drive the next
phase's tuning.
