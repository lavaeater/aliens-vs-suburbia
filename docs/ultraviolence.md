# In The Age of Ultra Violence

## Into the Deep

They came from below, from the bowels of the Earth. Beasts? Men? Man-Beasts? Who could know? They flowed forth towards our homes, our families, and destroyed everything in their path. Mothers, torn to pieces, children, limb from limb, and fathers, castrated. 

Our world crumbled and fell - almost. In the end, we held on, we persevered, through Ultra Violence. When they attack us, kill them all! When we attack them, kill them all.

Only by wading through the blood of our enemies can we see ourselves be free again!

All Hail the Age of Ultra Violence!

## Needed features

To make the game ultra violent, it needs the following:

* Guns guns and guns - this is handled by the inverse-kinematics and hardpoints thing.
* Blood Splatter Decals - hits makes blood fly around and dirty up the maps.
* Body parts, organs and other things, "gibs" if you will. Will fly around and goo stuff up.
* Macabre gameplay, like molotov cocktails, crazy sound effects and dialogue to emphasize the ultra violent nature of the game.
* Destructable terrain
* A feeling of despair

---

# Implementation Plan

Each feature below is grounded in the systems that already exist in the repo, with the
concrete components, files, and events to touch. Order roughly follows dependency and
payoff: the cheap, high-impact gore (blood, gibs) comes first because it re-skins death,
which every other system already produces.

> **Progress (2026-07-24):** shared groundwork, blood (feature 2), and gibs
> (feature 3) are implemented in `src/gore/` — see the ✅ notes below.

## Shared groundwork: a gore/FX layer

Almost every feature here spawns short-lived visual entities (blood, gibs, debris, fire,
muzzle flash) and despawns them on a timer. We already have the exact pattern in
`src/general/systems/death_effect_system.rs` (`DeathEffect { timer }` + `tick_death_effects`).
Rather than copy that five times, introduce one small module first:

- **New module `src/gore/`** with a `GorePlugin`.
- A generic `Ephemeral { timer, fade: bool }` component + one `tick_ephemeral` system that
  scales/fades/despawns — replaces the bespoke `DeathEffect` tick.
- A **pool cap** resource (`GoreBudget { max_decals, max_gibs, live: usize }`) so a big wave
  can't spawn 4000 rigidbodies and tank the frame. When over budget, recycle the oldest
  (keep a `VecDeque<Entity>`) instead of adding.
- A single `Message` — `DamageDealt { target, position, normal, amount, kind: DamageKind }` —
  written by *all* the places that currently mutate `Health` inline
  (`collision_handling_system`, `touch_damage_system`, tower damage, future gun hits).
  Blood, gibs, hitreact audio, and score all subscribe to this one message instead of each
  re-deriving "who got hurt where". This is the single most important refactor in the plan:
  it turns gore into event subscribers rather than special cases sprinkled through combat.

Effort: ~1 day. Everything after this is cheaper because of it.

✅ **Done.** `src/gore/` now holds `DamageDealt` + `EntityDied` messages, an `Ephemeral`
grow/fade-then-despawn component (`tick_ephemeral`), a `GoreBudget` recycler, and a
`LastHit` component (`record_last_hit`) so the death path can attribute the killing blow.
`collision_handling_system` and `touch_damage_system` emit `DamageDealt`;
`health_monitor_system` emits `EntityDied`. `GorePlugin` wires it all up. Explosive/fire
kinds are stubbed for §4.

## 1. Guns, guns and guns

Status: the *hard* part (IK/hardpoint snapping, `src/player/systems/equip.rs`,
`PlayerProps.weapon`) is done — a weapon model rides the hand bone correctly in-game and in
the browser. What's missing is that the weapon doesn't *fire*; the player still throws balls
(`src/general/systems/throwing_system.rs`).

Plan:
- Extend `WeaponProps` in `asset_definition.rs`: `damage`, `fire_rate_per_minute`,
  `projectile_speed`, `spread_deg`, `pellets` (1 = pistol, 8 = shotgun), `muzzle` hardpoint
  role, `auto: bool`, optional `ammo`/`reload`.
- Add a `muzzle` hardpoint to weapon defs (same authoring flow as `grip`, already supported by
  the asset browser). Fire origin/direction = the muzzle frame's world transform, so bullets
  leave the barrel, not the player's belly.
- New `src/player/systems/shoot.rs`: when the equipped weapon exists and the fire control is
  held, spawn projectile(s) from the muzzle along `AutoAim.0` with `spread`, reusing the
  `Ball` collision-layer setup from `throwing_system` (or a lighter raycast for hitscan —
  recommended for pistols/SMGs; keep physics balls for grenades/molotovs). Emit
  `DamageDealt` on hit.
- Keep throwing as the *unarmed / grenade* path. Weapon presence selects shoot vs throw.
- Muzzle flash + shell casing = two `Ephemeral` spawns at the muzzle frame. Recoil = a small
  additive rotation on the weapon transform in `keep_weapons_snapped`, decaying per frame.
- Animation: bind a `shoot` clip via the existing `animation_bindings` map (amy already has
  `run_gun`/`idle_shoot` tags).

Effort: 2–3 days. Depends on the `DamageDealt` message from groundwork.

## 2. Blood splatter decals

Two flavors: **airborne spray** (particles that arc off a hit) and **ground/wall decals**
(persistent stains that dirty the map). We already ship a `packs/post-apocalypse/Blood Splat.glb`
prop used as map decoration — reuse that art.

Plan:
- Subscribe to `DamageDealt`. For each hit, spawn N spray quads as `Ephemeral` billboards
  (there's already a `src/sprite_billboard/` module) launched along `normal` with gravity, or
  cheap unlit stretched quads.
- On spray settle (or immediately, for simplicity) project a **decal** onto the nearest floor
  below via a downward raycast (avian `spatial_query`), spawning a flat alpha-blended quad a
  hair above the floor mesh with random rotation/scale/tint (dried vs fresh). Decals are
  *persistent* (no timer) but counted against `GoreBudget.max_decals` and recycled oldest-first.
- Bevy 0.18 has `ForwardDecal`/`DecalProjector` — evaluate it for true surface-conforming
  stains on walls; if it's fiddly, the projected-quad approach is fine for a top-down-ish
  isometric camera and much cheaper. Start with quads.
- Authoring knob: a handful of blood textures (splat, smear, pool, arterial) picked at random.

Effort: 2 days. Highest visual payoff per hour of the whole list.

✅ **Done (first pass).** `src/gore/blood.rs`: a procedurally-generated crimson splat
texture (no art dependency), a bright grow-and-fade impact puff, and a persistent,
budget-capped ground stain under each hit. Lethal blows bleed ~1.8x. Fire hits are skipped
(they'll scorch in §4). Still a polish pass away from arcing airborne droplets and true
surface-conforming wall decals (`ForwardDecal`).

## 3. Body parts / organs / gibs

On death (currently `health_monitor_system` just `despawn()`s non-players), burst the corpse
into physics chunks.

Plan:
- New `src/gore/gibs.rs`. On the death path (gate on `DamageDealt` that drops health <= 0, or a
  new `EntityDied` message so towers/abilities/DoT all trigger it), spawn 4–8 gib entities:
  small `RigidBody::Dynamic` colliders with pre-authored gib meshes (arm, leg, head, ribcage,
  loose organs) launched with randomized impulse away from the killing blow's `normal`, plus
  spin. Reuse the `Ball` throw physics setup.
- Gibs are `Ephemeral` with a longish timer (5–8 s), then fade and despawn — OR sink into a
  persistent "gore stays" mode gated by `GoreBudget.max_gibs`. Each gib that touches the floor
  also drops a blood decal (feature 2) — this is why decals come first.
- Art: author a small `gibs.glb` pack (generic red chunks work for the alien "man-beasts"); no
  need to butcher the actual rig. A per-enemy `EnemyProps.gib_set` field lets different enemies
  gib differently later.
- Keep the existing orange `DeathEffect` pop as the *impact flash* underneath the gibs, or
  retire it — gibs replace it thematically.

Effort: 2–3 days. Depends on gore groundwork + decals.

✅ **Done (first pass).** `src/gore/gibs.rs`: on `EntityDied`, bursts the corpse into 6
procedural dark-red physics chunks (`RigidBody::Dynamic`, randomized impulse + spin) thrown
away from the killing blow, lingering 6 s then despawning, budget-capped. Chunks collide
with the world but not with aliens/players. Next: per-enemy gib sets (`EnemyProps.gib_set`),
authored gib art, and a blood decal on floor-impact.

## 4. Macabre gameplay (molotovs, SFX, dialogue)

Three sub-features; they're independent, ship in any order.

**Molotov / fire:**
- A throwable weapon variant (grenade path from feature 1). On impact, spawn a `FireField`:
  a sensor collider + an `Ephemeral` fire VFX (billboard flames) that ticks `DamageDealt`
  (`DamageKind::Fire`) to anything overlapping, per second, for a few seconds. Aliens that die
  in fire gib *and* scorch-decal the ground. Scorch = a dark decal (reuses feature 2's pipeline).
- Same pattern generalizes to acid pools, caltrops, etc.

**Crazy sound effects:**
- We already run `bevy_seedling 0.7` for music (`src/music/`). Add a `src/gore/sfx.rs` that
  plays one-shot samples on `DamageDealt` / `EntityDied` / fire tick — wet squelches, bone
  cracks, screams — with pitch/gain jitter and a per-frame voice cap so a wave doesn't become
  white noise. Samples go under `assets/sfx/` (watch the WAV `fmt_pcm` gotcha in CLAUDE.md;
  re-encode with ffmpeg if seedling rejects them).

**Dialogue / barks:**
- There's already a `src/facts/` + `turbofacts` fact system — the right tool for context-aware
  one-liners ("KILL THEM ALL", "for the Motherland", nervous "...are these even monsters?").
  A `BarkRequest { speaker, context }` message picks a line by querying facts (wave number,
  kills, player near death, first-blood, overkill) and plays a VO clip + optional floating HUD
  text. This is also where the game's *theme* lives — obedience vs doubt — so barks should
  drift from gung-ho to disturbed as the body count climbs (a `player_atrocity` fact).

Effort: fire ~2 days; SFX ~1 day; barks ~2–3 days (the writing is the long pole).

## 5. Destructible terrain

Half-built already: map-editor terrain placements take an optional `props.health` and get a
`Health` component (else `Indestructible`) in `map_systems.rs:367`, walls are static bodies,
and `recheck_path_after_tile_opened` (`src/ai/`) already re-opens pathfinding when a tile
frees up. Aliens even have a `MustDestroyTheMap` behavior. The missing link is *applying damage
to terrain and reacting to its destruction*.

Plan:
- Let projectiles/melee/explosions hit terrain: include walls/obstacles as valid `DamageDealt`
  targets (they already have `Health`). Currently `collision_handling_system` only damages
  `HittableTarget`s — add terrain to that or route through the shared message.
- On terrain death: instead of a plain `despawn()`, play a **destruction**: spawn rubble gibs
  (feature 3's pipeline with a debris gib-set), a dust `Ephemeral`, remove the collider, and
  set `MapGraph.path_reopened` so `recheck_path_after_tile_opened` clears alien destroy-behavior
  and re-routes. This wiring largely exists; it needs the death hook to fire it.
- Optional polish: multi-stage meshes (pristine → cracked → rubble) swapped at HP thresholds,
  authored as extra def fields, so walls visibly degrade.
- Design tie-in: destructible cover makes the "despair" real — the map you defend visibly
  erodes wave by wave.

Effort: 2 days for the core loop (most plumbing exists); +1–2 for staged meshes.

## 6. A feeling of despair

This is mood, not a mechanic — it's a coordinated pass across rendering, audio, and pacing.
We already have `MusicMoods { combat, danger }` (`src/music/game_music_plugin.rs`) smoothing
into the soundtrack's intensity, which is the perfect signal to drive everything else off.

Plan:
- **Color grading / atmosphere** (`src/general/systems/lights_systems.rs`, camera): push the
  post-processing toward desaturated, high-contrast, sodium-vapor sickly tones; add volumetric
  fog / haze; a vignette that tightens as `danger` rises. Drive tint and fog density off the
  existing `Intensity` so calm is merely grim and a losing wave is oppressive.
- **Persistence of carnage**: don't clean up. Blood decals and (budgeted) gibs staying on the
  map *is* the despair — the battlefield accretes evidence. Corpses fade slowly, not instantly.
- **Audio bed**: a low ambient dread layer (drones, distant screams, dripping) as an always-on
  seedling channel under the music, its gain riding `danger`. Heartbeat / tinnitus when a player
  is near death (there's already a `hurt`/`goal_pressure` signal feeding `target_danger`).
- **Framing**: barks (feature 4) that undercut heroism; a kill counter that keeps climbing with
  no catharsis; lighting that never quite recovers between waves. Despair is the sum of the
  other five features *refusing to release tension* — so this feature is mostly a tuning/wiring
  task once the rest exist, plus the color-grade + fog + ambient-audio additions.

Effort: 2–3 days, best done last as an integration/tuning pass.

---

# Feature 0 (missing from the list): Cool maps

Bigger and fuzzier than the rest, so here are the options rather than one prescription.

**What we have today.** A tile-grid format (`MapFile` in
`src/general/components/map_components.rs`): a row-major grid of `BitFlags<MapFeatures>`,
plus `decorations`, def-driven `placements`, and `waves`. Two authoring paths already feed it:
a seeded procedural generator (`src/map/map_generator.rs`, xorshift64 + curated poly-pizza prop
palettes) and an in-engine grid **map editor** (`src/map_editor/`, palette filtered by
`ModelType`, plus a TUI variant `src/map_editor_tui/`). Terrain, spawns, goals, and obstacles
are all expressible. This is a real, working, game-specific pipeline — not a throwaway.

### Should maps be generated?

Recommendation: **hybrid, leaning on prefab "chunks", not pure per-tile noise.** Pure
procedural gives infinite but samey levels; pure hand-authoring is slow and doesn't suit a
wave-defense roguellike. The sweet spot is **prefab rooms/streets stitched by the generator**:

- Author a library of small hand-made **map chunks** in the existing editor (a gas station, a
  cul-de-sac, a collapsed overpass, a chapel, a parking lot) — each a `MapFile` fragment with
  its own tiles + placements + decoration, tagged with edge-connectors (which sides have roads
  matching up, spawn-side vs goal-side).
- Extend `map_generator.rs` from "place props on a grid" to "**tile these chunks**" (wave-
  function-collapse-lite / connector matching), then run the existing prop/decoration passes on
  top for variation. Seed still drives reproducibility.
- This reuses everything: the format, the loader, the editor for authoring chunks, the prop
  palettes. It's the lowest-risk high-ceiling path.

### What prefabs?

Two layers:
1. **Model prefabs** (already have): poly-pizza packs — `packs/post-apocalypse/`, `packs/city/`,
   `packs/nature/`, `packs/toon-shooter/`. Ultraviolence wants to lean into
   `post-apocalypse` (already used for the blood-splat prop) + `city` for ruined-suburbia:
   burned cars, barricades, dumpsters, streetlights, chain-link, corpses/props. Curate a
   dedicated `ULTRAVIOLENCE` prop palette in `map_generator.rs` alongside the existing
   `TREES`/`BUSHES` consts.
2. **Layout prefabs** (new): the map-chunk library above, authored as `.ron` `MapFile` fragments
   under `assets/maps/chunks/`.

### LDtk vs. our own format?

Recommendation: **build on our own format; do not adopt LDtk.**

- LDtk is a **2D** editor. This is a 3D game where a tile carries a def path, a rotation in 45°
  steps, height, model type, spawn/goal semantics, and wave data. We'd be bolting a 2D grid
  editor onto a 3D def-driven pipeline and writing an LDtk→`MapFile` importer anyway — all the
  3D-specific data (scale, height, hardpoints, model_type) lives outside LDtk and would need a
  side-channel. Net new dependency + impedance mismatch, for an editor we've *already built*.
- Our in-engine editor has the decisive advantage: **you author in the actual renderer**, with
  the actual models, camera, and physics — WYSIWYG in a way LDtk can't match for 3D.
- The one thing LDtk does better is *fast 2D iteration and a mature UI*. We can close most of
  that gap cheaply: the map-chunk workflow above + the existing TUI editor for quick blockouts.
- **Verdict:** invest the LDtk-integration effort into (a) chunk-stitching in the generator and
  (b) editor UX (chunk stamp/save/load, connector tagging) instead. Same or better result, no
  new dependency, no format bridge to maintain.

Effort: chunk format + loader ~2 days; connector-stitching generator ~4–5 days; editor chunk
stamp/save UX ~2–3 days; ultraviolence prop palette ~1 day. Sequenceable — the palette and a
couple of hand-authored full maps give "cool maps" immediately while the stitcher is built.
