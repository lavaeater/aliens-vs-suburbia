# Content creation guide

Everything the game reads from files rather than code: what kinds there are, where each
goes, how to name it, and what picks it up. This page is the map. The details stay in the
docs it links to.

**After adding or changing content, run `cargo run -- --validate`.** It reads every
campaign, map, def, loot table, story, crawl, house and effect through the game's own
loaders. It reports missing files, broken references and facts nothing ever sets, with a
"did you mean" for near misses. `cargo test` runs the same check (`the_shipped_assets_validate`).

## Ground rules

- **Paths are relative to `assets/` with no prefix.** Write `defs/Pistol.ron` and
  `packs/city/Bench.glb`, not `assets/defs/Pistol.ron`. The loaders accept the old
  prefixed form, but an animation source with the prefix silently fails to load.
- **What a file is comes from its name.** Inside a campaign folder, `level_01.map`,
  `level_01_story.ron` and `level_01_crawl_intro.ron` are told apart by their suffixes.
  `--validate` warns about anything else in there.
- **Text:** Latin letters (å ä ö, é, ü), dashes and curly quotes are fine (Fira Mono,
  `src/ui/font.rs`). There are no arrows, emoji or other scripts in the font.
- **Audio:** `.wav` (and `.mp3`/`.ogg` for the intro music). If a wav is rejected with
  `malformed fmt_pcm chunk`, re-encode it: `ffmpeg -i in.wav -c:a pcm_s16le out.wav`.
- **Credits:** third-party models, sounds and fonts get a line in `assets/attributions.txt`
  (sfx sources are listed in `assets/sfx/README.md`).

## At a glance

| I want to add... | File(s) | Where | Code needed? |
|---|---|---|---|
| A campaign | `campaign.ron` | `assets/campaigns/<name>/` | No |
| A level | `<level>.map` | `assets/campaigns/<name>/<level>/` | No |
| Level scripting / objectives | `<level>_story.ron` | next to the level's map | No |
| A story crawl / dialogue box | `<level>_crawl_<what>.ron` | next to the level's map (or `assets/crawls/`) | No |
| The campaign intro flight + music | `intro:` in `campaign.ron`, track in `assets/music/` | | No |
| Combat sound effects | `<prefix>*.wav` | `assets/sfx/` | No (new categories: yes) |
| Bark lines (captions) | | `src/gore/barks.rs` | Yes (lines are constants) |
| Music samples / synth patches | `.wav`, `.glicol` | `assets/instruments/`, `assets/synths/` | Yes (to wire them into the band) |
| A 3D model | `.glb` / `.gltf` | `assets/packs/<pack>/` | No |
| A character, enemy, weapon, item, prop | `<name>.ron` (a def) | `assets/defs/` | No |
| Townsfolk / neighbours | def | `assets/defs/people/` (models in `assets/people/`) | No |
| A story item (letter, key, part) | Item def with `Key(id)` | `assets/defs/story_*.ron` | No |
| A loot table | `<table>.ron` | `assets/loot/` | No |
| A house | `<name>.ron` | `assets/houses/` (house editor) | No |
| A particle effect | `<name>.ron` | `assets/fx/` | Only if new: an `FxKind` to trigger it |
| A palette for the retro look | `<name>.hex` | `assets/palettes/` | No |
| A level from OpenStreetMap | spec `.ron` | `assets/gis/` (raw data in `gis-raw/`) | No |
| Input defaults / settings | `input-bindings.ron`, `game-settings.ron`, `player-settings.ron` | project root | No |

---

## Campaigns and levels

### Campaign

`assets/campaigns/<name>/campaign.ron` lists the levels in order, plus the victory card:

```ron
(
    base: "campaigns/gis/world/world.map",   // optional: the base map levels are cut from
    levels: [
        (map: "campaigns/gis/map_01/map_01.map", title: "...", tagline: "..."),
    ],
    victory_title: "THE TOWN IS HELD",
    victory_lines: ["...", "..."],
    intro: Some((...)),                       // optional, see "Intro" below
)
```

- Play it: `cargo run -- --play --campaign <name>`. The default is `gis`
  (`campaign::DEFAULT_CAMPAIGN`).
- The titles and taglines show on the intermission card between levels.
- **Campaign editor** (`cargo run -- --campaign-editor <name>`): cuts a base map into
  levels and places markers, defs and waves. It also edits each level's intro crawl and
  HUD objective. F5 plays the selected level.

### Level folder

One folder per level, everything the level is:

```
assets/campaigns/<campaign>/<level>/
  <level>.map                 the map and its rules
  <level>_story.ron           its stories (scripting)
  <level>_crawl_<what>.ron    its crawls, one per file
```

New levels made by the campaign editor go to `campaigns/<name>/<name>_NN/`. A crawl's id
is its whole file stem, and ids are global, so a level-prefixed name keeps them unique.

### The map

`.map` is a hand-editable text format, letter per tile. Full reference:
**`docs/map-text-format.md`**. In short:

- `[properties]`: `name`, `stories`, `loot`, `escape_limit`, `weapons` (a fixed loadout
  for the level), `coins`, `townsfolk`, `seed`/`generated`/`generator`, the
  infestation keys (`spawners`, ...) and `base`/`region` for a level cut from a base map.
- `[tiles]`, `[legend]`, `[storeys]`, `[elevation]`.
- `[decorations]` (bare models), `[placements]` (defs), `[houses]`, `[roads]`, `[waves]`
  (`enemy: defs/Spider.ron | count: 10 | rate: 20`), `[markers]`.

Ways to make one:

| Tool | Command | Good for |
|---|---|---|
| A text editor | | small hand-made maps |
| Map editor | menu, or `GameState::MapEditor` | painting tiles, placing defs and houses, waves. Saves to `assets/maps/`; move the file into a level folder to play it. |
| Campaign editor | `--campaign-editor` | cutting levels out of a big base map |
| OSM import | `--import-osm <spec.ron>` | real places (`docs/gis-levels.md`) |
| Generator | `generated: true` + `generator: neighbourhood` | random suburbs (`docs/infestation.md`) |

Play any map outside a campaign: `cargo run -- --play --level <path>`. Maps that aren't
levels (test maps, the editor arena) live in `assets/maps/`.

### Level modes

- **Classic**: aliens walk from `e` spawns to the `x` goal, and too many escaping loses.
- **Infestation**: the `spawners` property. There is no goal. Spawners open, monsters hunt
  the neighbours, and you win by destroying them all. See **`docs/infestation.md`**.

---

## Text: stories, crawls, objectives

### Stories (level scripting)

`<level>_story.ron`, named in the map's `[properties]` (`stories: campaigns/main/level_01/level_01_story.ron`).
It is a list of turbofacts `Story`s: rules wait on facts, and consequences set facts or
emit effects. Format reference with every criterion and consequence:
`turbofacts/assets/stories/example.ron`. Sample: `assets/campaigns/main/level_01/level_01_story.ron`.

```ron
(
    name: "Level 1 intro",
    repeat: false,
    init_facts: [("ObjectiveText", Text("Hold Maple Drive."))],
    rules: [(name: "started", criteria: [BoolIs(key: "LevelStarted", expected: true)])],
    consequences: [Emit(effect: "crawl:level_01_crawl_intro")],
),
```

What you can hook into:

| Kind | Names |
|---|---|
| Facts the game sets | `LevelStarted`, `LevelComplete`, `LevelFailed`, `EnemyKillCount`, `EnemyCount`, `CurrentWave`, `WaveCount`, `AllWavesDone`, `AliensEscaped`, `LivingPlayerCount`, `AllPlayersDead`, `Coins`, `ShotsFired`, `ShotsHit`, `CrawlShowing`, ... (`turbofacts/src/keys.rs`; the full list the validator trusts is `GAME_FACTS` in `src/validate/stories.rs`) |
| Set by content | `ItemFound.<id>` (a `Key` item picked up), `CrawlFinished.<crawl id>` |
| Infestation | `SpawnersTotal/Destroyed/Left`, `NeighboursTotal/Alive/Lost` |
| Townsfolk (every level) | `TownsfolkTotal/Alive/Lost/Indoors` |
| Effects the game acts on | `crawl:<id>`, `level_starting`, `level_complete`, `level_failed` |

**Objectives:** `init_facts` an `Objective.<name>` bool (false) per objective and set it
true when it's done. The level is only won once every `Objective.*` is true
(`ObjectivesDone`). **HUD objective line:** the `ObjectiveText` text fact. Change it with
`SetFact` as the level goes on.

Facts are reset at the start of every match, so nothing leaks between levels or retries.

### Crawls (scrolls and dialogue boxes)

Full reference: **`docs/crawls.md`**. The field list is in the header of
`assets/campaigns/main/level_01/level_01_crawl_intro.ron`.

```ron
(
    title: "EPISODE I: THE LAWN MENACE",
    lines: ["It is a quiet Tuesday on Maple Drive.", "", "..."],
    style: Scroll(tilt: 0.0),     // or Typewriter: a dialog box, one page per line
    speed: 55.0,                  // px/s (Scroll) or chars/s (Typewriter)
    backdrop: 0.85,
    // modal: true, skippable: true, auto_advance: 0.0, hint: "..."
)
```

- Shown by a story: `Emit(effect: "crawl:<file stem>")`. Chain from its end with
  `BoolIs(key: "CrawlFinished.<file stem>", expected: true)`.
- **Dialogue** (a letter, a radio call, a neighbour talking) is a `Typewriter` crawl.
  Use `modal: false` to keep the game running underneath, and `auto_advance` to turn
  pages by itself.
- A crawl no level owns goes in `assets/crawls/` (the folder doesn't exist yet; create it).
- The level waits for crawls before ending, so an outro on `LevelComplete` gets played.

### Story items

An Item def with `kind: Key(id: "letter")` sets `ItemFound.letter` when picked up.
Examples: `assets/defs/story_letter.ron`, `story_fuel.ron`, `story_fuses.ron`,
`story_parts.ron`. Place it with a `[placements]` line or an editor, or put it in a loot
table.

### Campaign and intermission text

These live in `campaign.ron`: level `title`/`tagline`, `victory_title`/`victory_lines`.

### Barks (combat one-liners)

The fading captions on kills, multi-kills, getting hurt and low health. Today they are
constants in **`src/gore/barks.rs`** (`KILL`, `MULTI_KILL`, `HURT`, `LOW_HEALTH`), each with
a `zeal` set and a `haunted` set. The game drifts from the first to the second as the
body count climbs. Adding lines means editing that file. Recorded voice lines go in
`assets/sfx/` as `bark*.wav` (below).

---

## Audio

### Sound effects

Drop `.wav`s in **`assets/sfx/`**. They're picked up at startup by **filename prefix**,
with no code changes. Several files with the same prefix are variants, one played at random
with pitch/volume jitter. Reference: `assets/sfx/README.md`.

| Prefix | Plays when |
|---|---|
| `hit` | a non-lethal hit lands |
| `death` | something dies |
| `gib` | layered under a death |
| `fire` | a fire field ignites (molotov) |
| `shoot` | a shot is fired |
| `bark` | a bark caption appears |
| `heartbeat` | a player bleeds out (one short beat; re-triggered faster near death) |

Prefer mono, 16-bit, 44.1 kHz, trimmed and normalised. Add the source to the table in
`assets/sfx/README.md`. A **new category** (footsteps, reloads, a sound per weapon) needs
an `SfxKind` variant and a trigger in `src/gore/sfx.rs`. Volumes and the voice cap are at
the top of that file.

### Music

The soundtrack is **generative** (`src/music/`, the `rusty_music` submodule). The game
describes the situation (intensity, tension, valence, energy) and a synth band improvises
to it. Most of it is code, but some parts are files:

| What | Where | Notes |
|---|---|---|
| Intro track | `assets/music/` (mp3/ogg/wav) | named in `campaign.ron`'s `intro.music`; fit `intro.timing` to it |
| Samples | `assets/instruments/`, drums in `assets/instruments/drums/` | the surf stabs (`stab.wav`, `hit1/2.wav`), kit samples. A new sample means a line in `src/music/game_music_plugin.rs` |
| Glicol synths | `assets/synths/*.glicol` | the aliens' theremin. A new patch needs a musician in `game_music_plugin.rs` |
| Stingers | code: `game_stingers` in `game_music_plugin.rs` | kills, pickups, downed players, new wave, win/lose; synthesised, no files |

Integration guide for the music crate: `rusty_music/.claude/skills/rusty-music/SKILL.md`.

### Intro (copter flight)

`intro:` in `campaign.ron`: music, the copter model (a `.glb` with its nose along -Z, plus
rotor nodes to spin), seconds per part of the flight, and cloud heights. Reference:
**`docs/intro.md`**. Test with `--intro-at <secs>`, skip with `--no-intro`.

---

## Models and defs

### Models

`.glb` (or `.gltf` + `.bin` in the same folder) under **`assets/packs/<pack>/`**. Models
nothing uses yet go in `art-source/` (outside `assets/`, see its README). Credit CC-BY
models in `assets/attributions.txt`.

### Defs

A def (`assets/defs/<name>.ron`, an `AssetDefinition`) turns a model into something the
game uses. The format is in the *AssetDefinition* section of `CLAUDE.md`. The usual way to
make one is to **import the model in the character editor** (`cargo run -- --editor`,
Library tab). It sets scale, floor lift, hidden nodes, animation mapping, hardpoints and
the editable stats, and it writes the `.ron`.

| `model_type` | What it is | Example |
|---|---|---|
| `Player(..)` | a playable character: ability, weapon + `extra_weapons`, `starting_ammo`, throw rate | `male-hazmat.ron` |
| `Enemy(..)` | a monster: health, speed, coins, `resistances`, `reactions` (e.g. `{ Burning: Ignore }`), melee/ranged attack, `loot_table` | `Stone Golem.ron` |
| `Weapon(..)` | a gun: damage, fire rate, spread, pellets, `ammo` kind, magazine, reload, optional `projectile` or `flame` | `Pistol.ron`, `Flamethrower.ron`, `Snowball.ron` |
| `Item(..)` | a pickup: `HealthPickup`, `AmmoPickup`, `WeaponPickup`, `Coins`, `Key`, `Loot`, `Decorative` | `story_letter.ron`, `Trash Bag.ron` |
| `Terrain(..)` | a placeable obstacle: blocks enemies/players, health, `explodes_on_death`, `loot_table` | `Barrel.ron`, `Barrier *.ron` |
| `Tower(..)` | a buildable tower | (none shipped as a def yet) |

- **Animations:** only list keys that differ from the default resolution (Mesh2Motion
  rig presets and name search fill in the rest). The editor's Animation tab shows what each
  key resolves to. External clips go in `animation_sources`.
- **Weapons:** `grip`/`muzzle` hardpoints, set in the character editor. A character's
  `grip` sits on its hand bone.
- **Enemies in a level:** name the def in a wave (`enemy: defs/Spider.ron`).
- **Neighbours / townsfolk:** any `Player`-type def placed on a map becomes a neighbour.
  The townsfolk and generated yards draw from **`assets/defs/people/`** (models in
  `assets/people/`), kept apart so the player setup doesn't list them.

### Loot tables

**`assets/loot/<table>.ron`**: weighted entries rolled `rolls` times:

```ron
(
    rolls: 1,
    entries: [
        Nothing(weight: 30.0),
        Table(weight: 25.0, table: "small_ammo"),          // nested table
        Item(weight: 15.0, kind: HealthPickup(amount: 25.0), count: 1),
        Item(weight: 4.0, kind: AmmoPickup(kind: Grenade, rounds: 1), count: 1),
    ],
)
```

Used by an enemy's or terrain's `loot_table` (dropped on death) and by loot bags (the
map's `loot` / `loot_table` properties).

---

## Visuals

### Particle effects

**`assets/fx/<name>.ron`**, `bevy_sprinkles` format. Edit them with the sprinkles editor
(`cargo install bevy_sprinkles_editor`) or by hand. One-shots must set
`despawn_on_finish: true` (the `every_fx_in_assets_parses` test checks this). Changing an
existing effect is just the file. A **new** effect needs an `FxKind` and a handle in
`src/fx/mod.rs`, plus something that writes `SpawnFx`. The list of effects and their
triggers is in the `src/fx/` row of `CLAUDE.md`.

### Palettes (retro look)

**`assets/palettes/<name>.hex`**: Lospec format, one `rrggbb` per line, `;` comments, at
most 256 colours. Any palette from lospec.com works, and it joins the Ctrl+J cycle.
`tools/extract_palette.py <png> <name> <count>` makes one from a picture, and
`tools/make_palettes.py` makes ramps. Reference: **`docs/retro-look.md`**.

### Houses

**`assets/houses/<name>.ron`** (`HouseDef`): the outline, walls, windows, doors and roof.
Draw it in the house editor (`cargo run -- --house-editor`, `docs/house-editor.md`) and
place it with `[houses]` in a map or the map editor's House tab. The generated
neighbourhood uses any house that fits a lot.

---

## Real places (OpenStreetMap)

A spec in **`assets/gis/<name>.ron`** (`OsmLevelSpec`; every field is explained in
`assets/gis/example.ron`). It points at raw OSM data, which goes in the gitignored
`gis-raw/`, or in `assets/gis/` to commit it. Import with
`cargo run -- --import-osm assets/gis/<name>.ron`, then cut levels from the result in the
campaign editor. Full walkthrough, with heights and land cover: **`docs/gis-levels.md`**.
The OSM data is ODbL, so keep the credit.

---

## Settings files (project root)

| File | What |
|---|---|
| `game-settings.ron` | camera, lives, bleed-out, `drop_on_death`, `full_arsenal` (testing: spawn with every weapon and full ammo) |
| `player-settings.ron` | the global `anim_mapping` for player characters |
| `input-bindings.ron` | key/pad bindings (also edited in the pause menu's Controls page) |
| `retro-settings.ron` | retro look and sky light settings |

---

## Checklist for new content

1. Put the file where the table above says, named the way it says.
2. Paths inside it are relative to `assets/` with no prefix.
3. `cargo run -- --validate` comes back clean.
4. Play it: `--play --campaign <name>`, `--play --level <path>`, or F5 from the campaign
   editor. **End** wins the current level on the spot.
5. Credit third-party assets (`assets/attributions.txt`, `assets/sfx/README.md`).
