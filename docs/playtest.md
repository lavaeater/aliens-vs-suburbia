# Playtest

## Purpose

This is to be considered a sort of spike. Take the game in its current state and take it to a play-testable state. To do that we simply have to decide what needs to be in a play-testable game - but also what the playtest should be. I think of the playtest as an event. It could be inviting friends or family to play the game in some form or another, together, on a big screen.

## The Playtest

The game supports four players, so up to four players, a big screen and some snacks.

## The Game

### Crashes

Weird crashes need to be found. How do we do that? We make a stupid player-ai for controlling the player and walking hither and dither and basically playing the game to find crashes?

### Levels

We need 5+ levels, showcasing our ability to write "stories" integrated with turbofacts to control the flow of the game, moving from one level to another in the game and of course having a splash screen at the end with some kind of win message. There should be some challenge to actually clearing a level and also fun things to do. One level can very well be an actual tower defense level with snowball tossing as the only weapon as in South Park Tower Defense

#### Status (2026-10-24)

The campaign is in (`assets/campaigns/main/`, `src/campaign/`): five levels, a card between
levels (next level / retry after a loss), and a victory screen after level five. Start
Game plays it from level 1; `cargo run -- --play --level N` jumps straight to level N, and
**End** wins the current level on the spot (to skip ahead during a playtest).

| # | Level | Map | What is special |
|---|-------|-----|-----------------|
| 1 | Maple Drive | `level_01.map` | the intro; the strange letter (a story item); aliens then bunnies |
| 2 | The Community Centre | `level_02.map` | three bungalows (houses from the house editor); find fuel, spare parts and fuses (story objectives on the HUD) before clearing the street counts |
| 3 | Snow Day | `level_03.map` | snowballs only (`weapons:`), 250 coins for towers, a zig-zag path, only 8 may escape |
| 4 | Night of the Golems | `level_04.map` | two spawns, stone golems, barrels to blow up, loot bags; radio chatter at wave 2 |
| 5 | The Mothership | `level_05.map` | the boss wave (Alpaking Evolved) with its own announcement; only 5 may escape |

Each level is a folder, `assets/campaigns/main/level_0N/`, with its map, its stories
(`level_0N_story.ron`) and crawls (`level_0N_crawl_*.ron`): an intro, mid-level chatter driven by `CurrentWave` or items found, and
an outro. A level's rules are map properties: `escape_limit`, `weapons`, `coins`
(`docs/map-text-format.md`).

Untested by hand so far: balance, the snowball's look and feel (a small rock model, white
projectile), the story item sizes, and the flow through all five levels in one sitting.

### Playable Characters

We have several playable mesh2motion characters downloaded and ready to go. We need to move and modify the hardpoint for them - for a quick fix we could simply copy the ones we have in male-hazmat to the others and work from there, as it is already tweaked to "perfection".

### Voice Acting

Record dialog, stings, readings of the crawls etc. 

### SFX

Monster SFX

### Monsters

We need to add more and varied monsters. I'd love to have humanoid models but twist and get freaky with them. I'd love to have an enemy or monster that has tentacles.

## GIS campaign: plan (2026-11-08)

Goal: take the GIS levels (`docs/gis-levels.md`, campaign `gis`) to a somewhat complete
stage. Look and feel comes later. The idea: import one suburb or town as a **base map**, then
cut it into **levels** in a **campaign editor**. Several levels share the same base map, and
each one sets its own player starts, alien spawns, goal, waves and rules.

Order: the campaign editor first (C1-C5), then life in the streets (NPCs), then the effects.
The height tint is a quick win that can go in at any point.

### Where we are

- `assets/campaigns/gis/map_01/map_01.map` is a single 236 x 236 level straight from the
  importer. Its spawns (`e`), goal (`x`) and player start (`p`) are tile letters the importer
  wrote, so the gameplay is baked into the terrain.
- The map editor (`src/map_editor/`) spawns one UI node per tile (`grid.rs`,
  `CELL_SIZE` 24 px). That is fine at 20 x 20 but too much at 236 x 236 (56k nodes), and it
  has no view of a base map with levels on top. The campaign editor needs its own view.
- `campaign.ron` lists levels as plain map paths.

### C1. Data format: base map + level overlays (no UI)

```
assets/campaigns/gis/
  campaign.ron              # levels in order, plus `base: "campaigns/gis/world/world.map"`
  world/world.map           # the importer's output: terrain, buildings, roads, elevation,
                            # storeys, decorations. Gameplay letters are ignored.
  hill_top/hill_top.map     # a level: `base:` + `region:` + markers, no [tiles]
  hill_top/hill_top_story.ron, hill_top_crawl_intro.ron ...
```

A level `.map` that names a `base:` holds only what is the level's own:

- `[properties]`: `base: campaigns/gis/world/world.map`, `region: x, y, w, h` (base map tiles),
  `mode: defend | infestation`, plus today's rules (`escape_limit`, `coins`, `weapons`,
  `loot`, `stories`).
- `[markers]` (new): `player at 40, 12` (up to 4), `spawn at 3, 50`, `goal at 60, 60`,
  `spawner at ...` (infestation). Coordinates are **base map** tiles, so moving or resizing a
  region does not move what is in it.
- `[waves]`, `[placements]` as today (placements also in base coordinates).
- Later: `[edits]`, sparse tile overrides (a barricade, a fence opened up), so a level can
  change the base without a copy of it.

`compose_level(base, level) -> MapFile` (pure, in `src/map/compose.rs`) builds an ordinary
`MapFile`, so nothing downstream changes:

1. Crop `tiles`, `storeys` and `elevation` to the region. The region's origin snaps to
   `ELEVATION_STEP` (2) so the elevation lattice lines up.
2. Clip `roads` (`osm::geometry::clip_polyline`). Keep the decorations, houses and
   placements that fall inside, shifted to the region.
3. Clear the base's `e`/`p`/`x` letters and stamp the level's markers.
4. Wall the border. That is the importer's own rule, so `enclosed_voids` stays happy.
5. Copy rules, waves, stories and loot.

`text_format::load_level` composes when it finds `base:` and caches the parsed base. A
279 kB `.map` parses for every level today, which is fine for now.

The importer stays as it is. Its output becomes `world.map`. Move the spawn-at-road-exits
and path-check code out of `osm::import` (`perimeter`, `inward`, `distances`, the spawn
picking) into a module that works on any `MapFile` + region. The editor's "suggest spawns"
then uses it too.

Tests:
- A composed level has the region's size, a closed border, and its markers in place.
- Elevation lines up after an odd-origin snap.
- Roads are clipped.
- Converting `map_01` to base + one level composes to (almost) the same level.
- `the_shipped_campaigns_read_and_every_level_loads` and `--validate` cover composed
  levels, including a level whose markers fall outside its region (error).

### C2. Campaign editor: the world view and regions

`GameState::CampaignEditor`, `cargo run -- --campaign-editor gis` (and a menu entry).

- The base map is drawn as **one texture**, a pixel per tile (`ui::minimap::tile_color`,
  plus the hill tint below and road lines on top), on a `Camera2d` with pan (MMB/drag) and
  zoom (wheel). The texture is built once on load, which is cheap at any size the importer
  allows (1024 x 1024).
- Each level's region is drawn as a coloured outlined rectangle with its number and title.
  Regions may overlap (one fight in the school yard, a later one in the whole block).
- The left panel lists the levels in campaign order: add, delete, move up/down, title,
  tagline, and the victory title/lines. It edits `campaign.ron`.
- Draw a new region by dragging. Move or resize one by its edges. A readout shows the size
  in tiles and the open tiles, with a warning past the sizes `docs/gis-levels.md` measured as
  costly.
- Save writes `campaign.ron` and each level file. Dirty tracking and "unsaved changes"
  follow the character editor's doc pattern (`def != saved`).
- Keyboard-first like the map editor where it is cheap (`modal.rs` patterns). Mouse first
  is fine here.

### C3. Campaign editor: inside a level

Double-click a region (or Enter) to focus it: the view zooms to it and dims the rest.

- Marker tools: player start (1-4, slot colours from `player_ring::slot_color`), alien
  spawn, goal, infestation spawner. Click to place, drag to move, Delete to remove.
- **Suggest spawns**: the roads that cross the region's border (the code moved out in C1),
  widest first. Accept them one by one or all at once.
- **Live path check**: a flow field from the goal (`ai::flow_field`) over the composed level.
  Tiles no alien can reach are tinted, a spawn that can't reach the goal turns red, and
  player starts on blocked tiles are flagged.
- Placements: place defs (barrels, towers, story items, `ItemKind::Key`) with the same
  catalog the map editor scans.

### C4. Campaign editor: level rules, waves, story

- A properties panel: mode, `escape_limit`, `coins`, `weapons` (pick defs), loot spread.
- Waves: the map editor's wave panel, reused (move it into a shared widget if it isn't one
  already).
- Story: "create story" writes `<level>_story.ron` and an intro crawl from a template (the
  `map_01` ones) and links them. Editing stays in a text editor for now.
- Validate in place: run `validate::maps` on the composed level and list the problems in the
  panel.

### C5. Play from the editor

**Play** (F5) saves, composes and starts the focused level (`StartLevel`, as `--play --level`
does). Leaving the match (pause menu -> quit, or End) returns to the campaign editor with
the same level focused. This is the loop that makes the editor worth having: place, play,
adjust.

Then cut `world.map` into 4-6 real levels, with stories, and playtest the `gis` campaign end
to end.

### Height tint on grass (quick win)

`floor_mesh::hill_shade` shades by **slope** only, lighter facing north west and darker
facing away. Add **altitude**: lower ground slightly darker, higher slightly lighter, as a
gentle gradient between the level's lowest and highest point (`GroundHeight` knows both).
Multiply the two, so a hill reads both by its sides and by its top, and valleys sink.
Apply it to grass only, or to all ground at half strength, and try both.

- It is a few lines plus a test (`higher_ground_is_lighter`). Tune the strength (try about
  +/-15%).
- Watch the retro palette (`src/retro/`): a subtle tint can quantise to the same palette
  colour. The dither helps, but the step may need to be bigger with the retro look on.
- The minimap and the campaign editor's texture can use the same function, so their hills
  read the same.

### Life in the streets: NPCs with behaviour trees

Today's neighbours (`infestation/neighbours.rs`) idle in place, flee monsters and wave for
help. The plan is people who live there: they leave a house, walk the pavements to
somewhere (a neighbour's door, the shop, a bench, the car), stay a while and go home. When
aliens come they panic, hide indoors or run to the players.

**Library: `bevy_behave` (recommended).** It is ECS native: an action node spawns a task
entity with your components, and your ordinary systems run it and report success or
failure. It has trigger-based conditions, works with Bevy 0.19 (0.6 on git `main`, crates.io
has 0.5; pin a rev as with `bevy_sprinkles`), and is MIT/Apache. Trees are built in code;
there is no asset format. If we want trees in RON we can write a small builder ourselves.

**`beetry`, checked as the alternative:** a generic Rust behaviour tree framework with a
visual editor (Beehive) and JSON trees. Against it for us: it isn't Bevy-aware, its runtime
needs Tokio, it has no blackboard (nodes talk over typed channels), and it describes itself
as early stage (12 stars). The editor is tempting, but it would sit beside the ECS, not in
it. Revisit if we ever want designers editing trees visually.

Steps:

1. **Places from the map** (pure, tested): doors (an open tile next to a building, facing
   the nearest road; real doors once phase 4's houses exist), pavements and paths from
   `MapFile::roads`, benches, parking, gardens. One `PlaceIndex` per level.
2. **Walking**: A* on `MapGraph` with a cost that prefers pavements and paths, then follow
   the waypoints. Use the hunters' movement (`hunters.rs`: own velocity and facing, with
   `CreatureAnim`).
3. **Tree v1**: `Sequence[pick errand, walk there, do it (idle/anim, wait), walk home, stay
   in]`, under a selector that preempts on **danger** (a monster within N tiles, a gunshot
   or explosion heard): flee (today's logic), hide in the nearest door (despawn into the
   house, maybe come out later), or follow a player.
4. **Schedules and needs** later: a time of day, people with homes (`Resident { home }`),
   and a few per-def traits (brave / coward). Nice for stories ("save the Svenssons", facts on
   who survived).
5. Budget: a few dozen NPCs streamed with the chunks near the camera, like decorations.
   Far-away people are simulated coarsely or not at all.

### Smoke and fire

- **Now, with `bevy_sprinkles`** (already in, `src/fx/`): a `smoke` effect (soft
  alpha-blended billboards rising, growing and fading, drifting with a wind vector) added to
  `gore::fire` so every `FireField` smokes. Add a **house fire**: a building's tiles on fire
  after an explosion or molotov, with a tall smoke column and lit windows. Also add a smoke
  grenade as a visual only. Particles don't block sight, and that's fine for looks.
- **Later, `bevy_smokescreen`** for gameplay smoke: real volumetric smoke that fills rooms,
  flows round `SmokeObstacle`s, gets carved by bullets and cleared by explosions, with a
  `blocks_sight` query (aliens lose track of players in it). It needs **Bevy 0.20**
  (crate 0.2, 0.20 only). Its lighting knows one directional and eight point lights, and no
  spot lights yet. Our lamps and flashlights are mostly spots, so check how it looks at
  night before committing to it.

### Fog

- **Distance fog** (Bevy's `DistanceFog` on the game camera): cheap, and it gives depth with
  a near top-down camera mostly as a colour wash. Try it per lighting preset (Night: blue-black).
- **Ground fog in the valleys**: the levels have real hills now, so fog that pools in low
  ground would show the terrain off. Options:
  1. Bevy's `FogVolume` boxes over low areas (volumetric, costs a raymarch, needs
     `VolumetricFog` on the camera).
  2. A cheap fake: a few large, slow, low-alpha sprinkles billboards placed on tiles below
     a height threshold.
  3. Bake it into the floor tint (fog colour mixed in by depth below a level).
  Try 3 and 2 first. Check every option against the retro pass, which renders at low
  resolution, so volumetrics may be cheaper than usual.

### Grass

`bevy_grass` (GEMI, MIT) fits well: blades are built on the GPU near the camera each frame,
with wind presets. `GrassInteractor` flattens grass around characters, Avian bodies trample
it, and `GrassGround::Heightmap` / mesh / a height function follow our hills. A density mask
keeps it off roads, paths and floors. **It needs Bevy 0.20** (0.20.x only).

Keep checking it against the retro look and the top-down camera. Blades seen from above
read as noise, so short dense grass with tip colour variation will probably work better
than tall grass.

### The Bevy 0.20 gate

Both `bevy_grass` and `bevy_smokescreen` are 0.20 only.
`docs/bevy-0.20-upgrade.md` names two blockers:

- `bevy_mod_outline`: **cleared**, 0.14.0 shipped 2026-11-07 for Bevy 0.20.
- `bevy_seedling`: still blocked. Master is 0.8.0 on Bevy 0.19 and there is no 0.20
  branch, so `rusty_music` (and with it the soundtrack) holds the game on 0.19.

So grass and volumetric smoke wait for seedling, or we patch seedling ourselves (it is
mostly `bevy_ecs`/`bevy_app`/`bevy_asset`, likely a mechanical bump). Everything else in this
plan works on 0.19.

### Order

| # | What | Needs | Status (2026-11-08) |
|---|------|-------|--------|
| 1 | C1 data format + `compose_level`, `map_01` split into `world.map` + one level | - | **Done** (`map::compose`, `--split-level`; `map_01` composes back to the same tiles) |
| 2 | C2 world view, regions, level list | 1 | **Done** (`src/campaign_editor/`) |
| 3 | C3 markers, suggest spawns, live path check | 2 | **Done**, placements too (items, terrain, towers, people; R turns, right click removes) |
| 4 | C5 play from the editor | 3 | **Done** (F5) |
| 5 | C4 rules, waves, story stubs, validation panel | 3 | **Done**: escape limit, coins, townsfolk, waves (count, rate, enemy), weapons, loot bags (on/off, density, table), story stubs |
| 6 | Cut 4-6 levels, write their stories, playtest the campaign | 4, 5 | **Five levels** on one base, each played briefly unattended (loads, townsfolk, fog banks, HUD): 1 Up The Hill (80 x 80 round the hill), 2 The Blocks (north), 3 Fire Down Below (south-west; grenade launchers and pistols only, so houses burn), 4 The Long Road (east; 300 coins for towers, 6 may escape), 5 The Whole Town (all of it, an infestation: 8 spawners, 30 townsfolk to keep alive). Placeholder crawls. **Not done: a real playtest by people** (balance, the full flow through all five, how the finale feels at 236 x 236) |
| - | Height tint (any time, about an hour) | - | **Done** (`floor_mesh::ground_shade`, +/-15%) |
| 7 | Smoke from fires + house fires (sprinkles) | - | **Done**: `smoke.ron` over every fire field, `gore::house_fire` (a fire next to a building sets it alight: roof flames, `smoke_column.ron`, glow). Not yet: a smoke grenade, smoke that blocks sight (`bevy_smokescreen`, Bevy 0.20) |
| 8 | NPCs: places, walking, `bevy_behave` tree v1 | - | **Done** (`src/townsfolk/`): doors and street spots from the map, errands, lingering, home, indoors, fleeing. Not yet: schedules, traits, hiding from danger in the nearest door, stories about who survived |
| 9 | Fog: distance fog, then valley fog | - | **Done**: depth haze on the camera (`camera::fog`, Ctrl+O / `--fog`, blended with `gore::despair`'s dread haze, which already owned the camera's fog), and fog banks in the low ground of hilly levels (`map::valley_fog`). At night it reads as darkness more than mist; tune `fog_color` and `ground_fog.ron` in a playtest |
| 10 | Bevy 0.20 (after seedling), then `bevy_grass`, `bevy_smokescreen` | seedling |

