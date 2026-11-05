# GIS levels: real Swedish suburbs as maps

Plan and status for building levels from real map data. The aim is to stand in your own
street, or in a real villakvarter, miljonprogram or radhusområde, and defend it against
aliens.

Three sources, in the order we use them:

1. **OpenStreetMap** (now): no account or order, vector data, good coverage of Swedish
   suburbs. **Phase 1 is built**: `cargo run -- --import-osm ...` (below).
2. **Image import** (later, small): a map picture or a hand-painted PNG, one colour per
   tile type. Cheap, and doubles as "paint a level in any paint program".
3. **Lantmäteriet** (when access is ordered): the same pipeline with better building
   data (house types, exact footprints), through
   [Geodataportalen](https://www.geodata.se/geodataportalen/srv/swe/catalog.search#/home)
   and Geotorget. Licence generally CC BY 4.0.

## Why this fits the game well

- **A tile is one metre** (`src/map/house.rs`). Real coordinates turn into tiles with a
  projection and a floor, no scaling needed unless a level gets too big (see *Size*).
- **Houses are already polygons.** A `HouseDef` is an `outline` of whole-metre corners plus
  rules (walls, windows, doors, roof), and `procgen::resolve_house` turns a `Polygon` into
  wall/door/window tiles. A building footprint from OSM or Lantmäteriet is the same kind of
  thing (phase 4).
- **Terrain types match land cover.** `MapFeatures` has `Floor`, `Grass`, `Mud`, `Snow`,
  `Rock`, `Water`, plus `ImpassableForPlayers` / `ImpassableForEnemies`.
- **The `.map` text format** (`docs/map-text-format.md`) is easy to write and to edit by
  hand, and converts losslessly to `.ron`.

So the importer is an **offline converter**: map data in, a `.map` out. The game never reads
map data at runtime.

## OpenStreetMap: how to make a level (works now)

### 1. Get the data

Either way works; both end up as one file in the project.

**A. openstreetmap.org (simplest).** Zoom to the area, press **Export** in the top bar,
adjust the box if you want (*Manually select a different area*), and press **Export**
again. You get `map.osm`, an XML file with everything in the box. Rename it after the area
and put it in the project, e.g. `gis-raw/granvagen.osm`. The site refuses areas over 50 000
nodes; a few streets is far below that.

**B. overpass-turbo (only the layers the importer uses).** Open
[overpass-turbo.eu](https://overpass-turbo.eu/), pan and zoom to the area, paste this query
and press **Run**:

```text
[out:json][timeout:60];
(
  nwr["building"]({{bbox}});
  way["highway"]({{bbox}});
  nwr["landuse"]({{bbox}});
  nwr["leisure"]({{bbox}});
  nwr["natural"]({{bbox}});
  nwr["amenity"="parking"]({{bbox}});
  nwr["place"="square"]({{bbox}});
  way["barrier"]({{bbox}});
  way["waterway"]({{bbox}});
  way["railway"]({{bbox}});
);
out geom;
```

Then **Export -> raw OSM data** and save it as e.g. `gis-raw/granvagen.json`. `{{bbox}}` is
whatever the map shows. The importer also reads the other Overpass JSON shape
(`out body; >; out skel qt;`).

`gis-raw/` is gitignored. To commit an export, put it in `assets/gis/` instead (credit
OpenStreetMap, see *Licences*).

### 2. Import

Quickest, with every default (`.osm` or `.json`, the importer tells them apart):

```bash
cargo run -- --import-osm gis-raw/granvagen.osm
```

writes `assets/maps/granvagen.map` and prints a report (size, rotation, counts, spawns,
anything it had to work around). Existing files are kept unless you pass `--force`.

For control, write a spec, `assets/gis/<level>.ron` (`OsmLevelSpec`, every field but
`source` optional; `assets/gis/example.ron` lists them all):

```ron
(
    name: "Granvägen",
    source: "gis-raw/granvagen.osm",   // .osm XML or Overpass .json
    // output: Some("assets/maps/granvagen.map"),   // default: assets/maps/<spec stem>.map
    centre: Some((59.3293, 18.0686)),  // (lat, lon); default: the middle of the buildings
    size: Some((120.0, 90.0)),         // metres; default: the buildings plus 6 m
    rotation_deg: None,                // None = square the streets up automatically
    metres_per_tile: 1.0,
    fences: BlocksEnemies,             // BlocksEnemies | BlocksEveryone | Ignore
    goal: Some((59.3291, 18.0690)),    // what the aliens are after; default: the middle
    max_spawns: 4,
    trees: true,
    waves: [],                         // empty = the game's default waves
    loot: Some((min: 1, max: 3, per_tiles: 40)),
    stories: "",
)
```

```bash
cargo run -- --import-osm assets/gis/granvagen.ron --force
```

Click a spot on openstreetmap.org (right click -> *Show address*) to read its lat/lon for
`centre` and `goal`.

### 3. Play it

```bash
cargo run -- --play --level assets/maps/granvagen.map
```

Then touch it up like any level: the `.map` in a text editor, or
`cargo run --features map-editor -- --map-editor --file assets/maps/granvagen.map`.
Rerunning the import overwrites hand edits, so settle the spec first.

### What becomes what

Code: `src/map/osm/` (`data.rs` reads the `.osm` XML or Overpass JSON, `geometry.rs` projects and rasterises,
`classify.rs` holds the tag rules, `mod.rs` builds the level). All pure and unit tested.

| OSM | Level |
|-----|-------|
| `building=*` (not `roof`, not underground) | solid wall tiles (`w`) |
| `highway=*` lines | floor strips: 10 m (primary) / 8 m (secondary, tertiary) / 6 m (residential) / 4 m (service) / 2 m (paths); a `width` tag wins. Tunnels skipped. |
| `highway=*` + `area=yes`, `amenity=parking`, `landuse=retail/commercial/industrial`, `place=square`, paved pitches | floor |
| `landuse=residential/grass/meadow/forest/...`, `leisure=park/garden/playground/pitch`, `natural=wood/scrub/...` | grass. Grass is also the default ground. |
| `natural=water`, `waterway=riverbank`, pools, streams / ditches / rivers (2 / 1 / 8 m) | water |
| `natural=bare_rock/scree`, railways (3 m strip) | rock |
| `landuse=construction/brownfield`, `natural=wetland/sand` | mud |
| `barrier=fence/hedge/railing/guard_rail` | the spec's `fences` rule; default blocks aliens only (players hop over), so the suburb's own fences make the maze |
| `barrier=wall/retaining_wall` | blocks everyone |
| `natural=tree` | a tree decoration |

Ground areas are painted largest first, so smaller ones (a park inside a residential area,
a car park in a retail area) end up on top. Roads stay open through fences and walls (a fence
across a road is a gate). Buildings win over everything.

Then the gameplay layer:

- **Wall around the level**; every road that leaves it becomes an **alien spawn** (the
  widest first, at most `max_spawns`). Aliens come down the street.
- **Goal** (`x`): the spec's `goal`, or the middle, moved to the nearest open tile.
- **Player start**: a few steps from the goal.
- **Path check** (4-connected, aliens' rules): a spawn that can't reach the goal is dropped
  and reported. With no spawn left, the edge tile farthest from the goal is opened instead.

## Problems and decisions

### Rotation

Real streets don't run north-south and the grid is axis-aligned, so a 30-degree street would
come out with staircase walls. The importer works out the area's **main direction** (a
length-weighted histogram of building edge angles folded into 0-90 degrees, its peak) and
turns everything by it, at most 45 degrees either way. Most planned Swedish areas are laid out
on one grid, so this squares most houses up. `rotation_deg: Some(0.0)` keeps north up.

### Size and performance

At 1 m per tile, an 800 x 800 m export is about 640 000 tiles; the hand-made levels are about
20 x 20. The importer takes up to 1024 tiles a side. Measured on `map-01.osm`:

| Size | Tiles | Wall colliders | Loot bags | Lights |
|------|-------|----------------|-----------|--------|
| 50 m | 2.7k | 32 | 87 | 72 |
| 200 m | 41k | 251 | 1 635 | 1 091 |
| 400 m | 162k | 961 | 6 670 | 4 320 |
| 780 m | 610k | ~3 300 | 27 240 | 16 728 |

The tiles aren't the cost: colliders are merged into rectangles. The cost was what is
scattered over the area, so that is streamed:

- **Always loaded:** the tile grid, pathfinding, colliders, spawns and goal. Aliens walk the
  whole map off screen.
- **Streamed by `src/map/streaming.rs`:** lights, loot bags, and floor and wall meshes. They
  are planned once at load and kept per 32 x 32 chunk (meshes built, the rest as data); the
  chunks the camera can see get entities.
- **Budgeted:** only the 16 lamps nearest the camera cast shadows (`SHADOW_LAMPS`). Each
  shadow map is a render pass of its own, and their number was the last big cost.

Debug build, standing still, no aliens yet. The first three rows are the 782 x 782 export, the
last two the 700 x 700 one:

| What | Frame | FPS |
|------|-------|-----|
| Everything spawned at once | no frame in 70 s | - |
| Streaming | | 22 |
| Plus the floor out of shadow maps | 26 ms | 38 |
| Plus chunked floor and wall meshes | 26 ms | 38 |
| Plus 16 shadow lamps | 17 ms | 60 (vsync) |

Chunking the meshes didn't change the frame rate here: the render thread was busy with the
lamps, not with what they drew (61 lamps 26 ms; 16, 8 or 0 lamps 17 ms). It does keep the
entity count down and each lamp's work small.

Next, in order:

1. **Building heights** from `building:levels` (122 of 468 buildings in `map-01.osm`).

Pathfinding is one flow field from the goal (`src/ai/flow_field.rs`), built at load and
repaired tile by tile, instead of A* per alien. Measured in a debug build on 432 000 open
tiles:

| What | Before | Now |
|------|--------|-----|
| An alien spawning | 4-38 ms of A* | an array read |
| Building the field | - | 107-115 ms, once per load |
| A tile closing, median | - | 0.002 ms |
| A tile closing, worst of 4 000 | - | 34 ms (a chokepoint; 2 went over 16 ms) |
| A tile opening, worst of 4 000 | - | 3.6 ms |

With vsync off and the window focused, the map runs at about 95 FPS (10.5 ms) with four
aliens walking. Big waves are not measured yet.

### Water

Water tiles are walkable in the game (they're a terrain colour). Making lakes block needs a
"deep water" look first: an impassable tile is drawn as a wall today. Until then a lake is a
shallow pond that everyone wades through.

### Footprints (phase 4)

Buildings are filled by tile centre, so a house slightly off the grid gets a ragged edge,
and sheds under a few m² may vanish. Squaring footprints up (snap edges within ~10 degrees of
the grid) and turning them into `HouseDef`s with doors and windows is phase 4.

### Elevation

The game is flat; height is ignored. Later, from a DEM: steep slopes -> rock / impassable,
hollows -> mud or water.

## Image import (later)

A PNG plus a small spec: metres per pixel and a colour legend (`#f7f4d0 -> f`,
`#d9d0c9 -> w`, `#aad3df -> ~`, ...). Each pixel takes the nearest legend colour. Works for:

- a map picture exported at a known scale (clean map styles work; contour lines, labels and
  symbols are noise that the importer has to paint over with what surrounds them, so height
  curves are not usable as heights);
- a level painted by hand in any paint program.

It loses what an image doesn't hold (house types, exact shapes). Aerial photos are out:
roofs, trees and asphalt are too hard to tell apart.

## Lantmäteriet (when access is in place)

Ordered and downloaded through [Geotorget](https://geotorget.lantmateriet.se/), found
through Geodataportalen. Ask for **GeoPackage** in **SWEREF 99 TM** (EPSG:3006, metres), clip
the level area with QGIS or `ogr2ogr`, export GeoJSON. A second reader next to
`osm/data.rs` turns it into the same `Feature`s, with its own rules in place of
`classify.rs`; everything after that is shared.

| Need | Dataset | Use |
|------|---------|-----|
| Buildings | **Byggnad Nedladdning, vektor** | Footprints; `objekttyp` / `ändamål` (småhus friliggande / radhus / flerbostadshus / industri ...) pick the house template in phase 4. |
| Roads, land cover, water | **Topografi 10 Nedladdning, vektor** | Roads, `mark` land cover, hydrography. Also 5 m contours. |
| Elevation | **Markhöjdmodell Nedladdning, grid 1+** | 1 m DEM, for slopes. Optional. |
| Heights | **Laserdata Nedladdning** | Roof heights, trees. Optional. |
| Names | **Ortnamn**, **Belägenhetsadress** | Level names, a goal by address ("Granvägen 12"), story crawls. |
| Alternatives | **NMD** (Naturvårdsverket, land cover), **NVDB** (Trafikverket, road widths) | Fill in what the above lack. |

## Licences

- **OpenStreetMap: ODbL.** Credit "© OpenStreetMap contributors" (in the level's story crawl
  or a credits screen). A level made from OSM data is a derived database: it stays under
  ODbL, which is fine for game levels shipped with the game.
- **Lantmäteriet: generally CC BY 4.0.** Imported levels can be committed; credit
  Lantmäteriet the same way. Check each product's metadata, as some are under agreements.
- Raw downloads go in `gis-raw/` (gitignored): they can be large and are only needed to
  re-import. Each spec in `assets/gis/` says where its data came from.

## Phases

| # | What | Status |
|---|------|--------|
| 1 | **OSM import**: `.osm` XML and Overpass JSON readers, projection, automatic rotation, buildings / roads / ground / water / fences / trees, walled border, spawns at road exits, goal, player start, path check. `--import-osm`, `--play --level`. | **Done** (tested on made-up data; needs a first real area and a playtest) |
| 2 | **Tune on real areas**: road widths, what counts as grass, size and performance (see *Size and performance*: rendering is streamed, pathfinding is a flow field), fence rule. Water that blocks. | Started |
| 3 | **More gameplay from the map**: goal by address, street lamps along roads, loot next to houses, decorations from OSM points (benches, bins, cars at parking). | |
| 4 | **Real houses**: squared-up footprints as `HouseDef`s with doors and windows, type from `building=*` (house / detached / terrace / apartments / garage) or Lantmäteriet's `ändamål`; the house mesh work (`docs/house-editor.md`) draws them. | |
| 5 | **Image import** and **Lantmäteriet reader**. | |

## Level ideas

- **Your own street**: the first level, and the best test, because you know how it should
  look.
- **Villakvarter**: detached houses, hedges, cul-de-sacs (aliens come in from the main road,
  the vändplan is a dead end).
- **Radhusområde**: long rows, narrow paths between them: natural chokepoints.
- **Miljonprogram**: slab blocks around courtyards, car-free paths, parking decks; big
  courtyards to build towers in.
- **Bruksort**: a mill town by a river; the river and the bridge decide the level.
- **Sommarstugeområde**: forest, lake, scattered cottages; aliens from the water.

## Open questions

- Which area first? Its export makes the phase 2 test case.
- At 1 m per tile, is a real house the right size next to the characters and towers? Compare
  an imported house with `bungalow` in the game.
- Imported levels in the campaign, or a "your neighbourhood" mode that each player builds
  locally? Both work licence-wise.
