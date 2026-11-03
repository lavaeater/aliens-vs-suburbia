# GIS levels: real Swedish suburbs as maps

Plan and suggestions for building levels from Swedish geodata (found through
[Geodataportalen](https://www.geodata.se/geodataportalen/srv/swe/catalog.search#/home),
downloaded mostly from Lantmäteriet's Geotorget). The aim is to stand in your own street,
or in a real villakvarter, miljonprogram or radhusområde, and defend it against aliens.

Status: **plan only**. Nothing here is built yet.

## Why this fits the game well

The level format already speaks the language of a map:

- **A tile is one metre** (`src/map/house.rs`: "a map tile is one metre"). SWEREF 99 TM
  (EPSG:3006), the Swedish national grid, is in metres too, so a GIS coordinate turns into
  a tile index with a subtraction and a floor. No scaling is needed unless a level gets too
  big (see *Size*).
- **Houses are already polygons.** A `HouseDef` is an `outline` of whole-metre corners plus
  rules (walls, windows, doors, roof), and `procgen::resolve_house` turns a `Polygon` into
  wall/door/window tiles. A building footprint from Lantmäteriet is the same kind of thing.
- **Terrain types match land cover.** `MapFeatures` has `Floor`, `Grass`, `Mud`, `Snow`,
  `Rock`, `Water`, plus `ImpassableForPlayers` / `ImpassableForEnemies`. Land cover
  classes map straight onto these.
- **The `.map` text format** (`docs/map-text-format.md`) is easy to write and easy to edit by
  hand. An importer that writes a `.map` gives you a level you can then touch up in a text
  editor or the TUI map editor, and it converts to and from `.ron` without loss.

So the importer is an **offline converter**: GIS files in, a `.map` out. The game never reads
GIS data at runtime.

## Data sources

Geodataportalen is a **catalogue**: it describes datasets and links to where you get them.
Most of what we want is Lantmäteriet's and is ordered and downloaded through
[Geotorget](https://geotorget.lantmateriet.se/) with the account you already have. Each
product's metadata page in the portal states its licence. **Check it before committing
anything derived from it** (see *Licences*).

| Need | Dataset | Owner | Use in the game |
|------|---------|-------|-----------------|
| Buildings | **Byggnad Nedladdning, vektor** | Lantmäteriet | Footprints -> houses / walls. Attributes (`objekttyp`, `ändamål`: småhus friliggande / radhus / flerbostadshus / industri / samhällsfunktion ...) choose the house template. |
| Roads, land cover, water | **Topografi 10 Nedladdning, vektor** | Lantmäteriet | Roads -> floor (asphalt), land cover (`mark`) -> grass / rock / water / mud, hydrography -> water. Also has 5 m contours. |
| Elevation (later) | **Markhöjdmodell Nedladdning, grid 1+** | Lantmäteriet | 1 m DEM. Steep slopes -> rock / impassable; ditches -> mud. Optional. |
| Building and tree heights (later) | **Laserdata Nedladdning** | Lantmäteriet | Roof heights for the generated house mesh; trees -> decorations. Optional. |
| Reference picture | **Ortofoto Nedladdning** | Lantmäteriet | Only for checking the import by eye in QGIS, never in the game. |
| Names and flavour | **Ortnamn**, **Belägenhetsadress** | Lantmäteriet | Level name, street names and house numbers for story crawls ("They landed on Granvägen 12"). |
| Fine land cover (alternative) | **Nationella marktäckedata (NMD)** | Naturvårdsverket | 10 m raster: forest / open land / water / built-up. Coarser than a tile but good for forests and fields around a suburb. |
| Road classes (alternative) | **NVDB** | Trafikverket | Road width, speed limits, cycle paths. Wider roads -> wider floor strips. |
| Fallback / gap filler | **OpenStreetMap** | OSM (ODbL) | Fences, hedges, playgrounds, parking lots, sometimes better tagged than the official data. Licence needs attribution and share-alike for the derived data. |

The products I would start with are **Byggnad** and **Topografi 10**: between them they
give buildings, roads, land cover and water, enough for a whole level. The others come
later.

### Download format

Ask Geotorget for **GeoPackage** (`.gpkg`) in **SWEREF 99 TM** (EPSG:3006). A GeoPackage is one
SQLite file holding several layers and opens directly in QGIS and `ogr2ogr`. Products are
delivered per municipality or per tile sheet. Clip out the level area yourself (step 1
below).

## Pipeline

```text
Geotorget (.gpkg, EPSG:3006)
   |  1. clip to the level box (QGIS or ogr2ogr), drop unused layers
   v
gis-raw/<level>/*.geojson           (gitignored, can be large and/or licensed)
   |  2. cargo run --features gis-import -- --import-gis assets/gis/<level>.ron
   v
assets/maps/<level>.map              (committed; hand-edit from here)
   |  3. hand pass: spawns, goal, waves, story, decorations
   v
playable level
```

### 1. Clip (no code, use QGIS or ogr2ogr)

Pick the area in QGIS over the orthophoto, write down its corner coordinates and clip:

```bash
ogr2ogr -f GeoJSON -t_srs EPSG:3006 \
  -spat 674000 6580000 674200 6580200 -clipsrc spat_extent \
  gis-raw/granvagen/byggnad.geojson byggnad.gpkg byggnad
```

GeoJSON keeps the Rust side pure (no GDAL native dependency to build on every machine). The
`geojson` and `geo` crates read it and do the geometry (clipping, simplifying, point in
polygon, rotation).

### 2. Import spec: one file per level, so the import can be rerun

`assets/gis/<level>.ron` describes how a level was made, so a level can be imported again
with tweaked rules without redoing the hand pass from scratch:

```ron
(
    name: "Granvägen",
    // Bottom-left and top-right corners of the box, SWEREF 99 TM metres.
    bbox: (674000.0, 6580000.0, 674200.0, 6580200.0),
    // Turn the area so the main street runs along the grid (see "Rotation").
    // None = work it out from the building edges.
    rotation_deg: None,
    metres_per_tile: 1.0,
    layers: (
        buildings: "gis-raw/granvagen/byggnad.geojson",
        roads: Some("gis-raw/granvagen/vaglinje.geojson"),
        land_cover: Some("gis-raw/granvagen/mark.geojson"),
        water: Some("gis-raw/granvagen/hydro.geojson"),
    ),
    // Byggnad attribute value -> what to stamp.
    buildings: {
        "Småhus friliggande": House("bungalow"),
        "Småhus radhus": House("radhus"),
        "Flerbostadshus": Walls,
        "Komplementbyggnad": Placement("assets/defs/shed.ron"),
    },
    // Land cover class -> tile letter.
    land_cover: {
        "Öppen mark": 'g',
        "Skog": 'g',
        "Vatten": '~',
        "Bebyggelse": 'f',
    },
    // Road width in metres by class, for drawing road lines as strips.
    road_width: { "default": 6.0, "gång- och cykelväg": 2.5 },
    waves: [],
)
```

The importer:

1. **Transform.** Subtract the bbox origin, rotate by `rotation_deg` about the box centre,
   divide by `metres_per_tile`, **flip y** (GIS y grows north, the tile grid's row 0 is at
   the top).
2. **Paint the ground** in layers, last wins: default floor -> land cover polygons
   (rasterised by cell centre, `Polygon::contains_cell`-style) -> water -> roads (lines
   buffered to `road_width`, then rasterised) -> buildings.
3. **Buildings.** Snap each footprint to whole-metre corners, simplify it (Douglas-Peucker,
   about 0.5 m) and make it right-angled (see *Footprints*). Then, by the attribute rule:
   - `House(name)`: write a `HousePlacement` when the footprint is close to the template's,
     or (better, stage 4) generate a per-building `HouseDef` with the real outline and the
     template's walls/windows/doors/roof, saved as `assets/houses/<level>_<n>.ron`.
   - `Walls`: fill the footprint as `w` (blocks everyone). Fine for blocks of flats, schools,
     shops.
   - `Placement(def)`: one def placement at the centre (sheds, garages, kiosks).
4. **Border.** Surround the box with walls, then open gaps where roads leave it: those gaps
   are where aliens come from (see *Gameplay*).
5. **Check** that a path exists from every enemy spawn to the goal over the same rules
   `MapGraph` uses. Report problems; don't fix them silently.
6. **Write** `assets/maps/<level>.map`: `[properties]` (name, `loot`), `[tiles]`, a `[legend]`
   for any combinations without a letter, `[houses]`, `[placements]`, `[waves]`. Refuse to
   overwrite without `--force`, the same as `--convert-map`.

### 3. Hand pass

The import gives you geography, not gameplay. Spawns, the goal, waves, story and props are
still yours to place, in the `.map` or the map editor. The importer can propose them (next
section); you decide.

## Gameplay from geography

Rules the importer can use to *suggest* a layout. Each one is written to the `.map` as a
comment or as a starting point:

- **Enemy spawns** where a road leaves the box: aliens come down the street. Or a forest
  edge, a field, a lake shore ("they came out of the water").
- **Goal** (`x`): a building you name in the spec (`goal_address: "Granvägen 12"`, looked up
  in Belägenhetsadress), or the centre of the box. Aliens head for "your house".
- **Player spawn** next to the goal.
- **Property boundaries** (Fastighetsindelning, or OSM fences/hedges) -> hedges and fences as
  `blocks_players` or `blocks_enemies` wall lines. The suburb's own fences become the
  maze, which is the point of tower defence in a villakvarter.
- **Gardens** -> grass, so they are open ground to build towers on. Driveways and parking
  -> floor.
- **Trees** (laser data or NMD forest) -> scattered tree decorations, no colliders.
- **Lights**: `map_lights::plan_lights` already covers open floor. Later, put street lamps
  along road centre lines.
- **Loot**: `loot: 1-3 per 40` as usual. Later, put more bags next to buildings than in the
  open.

## Problems to solve

### Rotation

Real streets don't run north-south; the grid is axis-aligned. Rasterising a 30-degree street
gives staircase walls and saw-tooth houses. Suggestion: work out the **main direction** of
the area (a histogram of building edge angles folded to 0-90 degrees; take the peak) and
rotate the whole area by it before rasterising. Most Swedish planned areas (villakvarter,
miljonprogram) are laid out on one or two grids, so a single rotation squares most houses
up. `rotation_deg` in the spec overrides it.

### Footprints

After rotating, most footprints are rectangles or L-shapes a few degrees off square. Make
them right-angled: snap each edge to the nearest multiple of 90 degrees when it is within
about 10 degrees, otherwise keep it as a staircase. Drop buildings under 6 m² (or make them
placements). Merge buildings that touch (radhus rows, terraced garages) into one outline,
because that is how they stand as walls.

### Size

At 1 m per tile, a residential block of 200 x 200 m is 40 000 tiles; the current levels
are about 20 x 20. Points to measure early:

- A* in `MapGraph` over 40k cells, per alien, when walls change.
- `walls::spawn_walls` (merged mesh: should be fine), `plan_lights` (5-tile spacing ->
  hundreds of lights; needs a cap or larger spacing for big maps), physics colliders per
  wall tile.
- Playability: a level that takes three minutes to walk across is not tower defence.

Suggestions: start with **80 x 80 m** (one or two streets and their gardens), and keep
`metres_per_tile` in the spec so a large area can be imported at 2 m per tile if needed.
Houses then shrink to half their size in tiles, which still reads as a house.

### Elevation

The game is flat. Ignore height at first. Later, from the DEM: slopes steeper than
about 30 degrees -> rock / blocks players, hollows -> mud or water. A real terrain mesh is a
separate project.

### Licences

Check the licence in each product's metadata before committing derived data.

- **CC0** (much of Lantmäteriet's open data, e.g. Topografi 250): commit freely.
- **CC BY**: commit, and credit the source in the level's crawl or the credits.
- **Restricted** (ordered under an agreement through your account): **do not commit** the raw
  data *or* the generated `.map`, because the tiles are derived from it. Keep those levels
  local or replace the source with an open one (OSM, NMD).
- **OSM (ODbL)**: attribution; the derived database is share-alike.

Raw GIS files go in `gis-raw/`, which is **gitignored** regardless of licence: they are large
and only needed to re-import. Each `assets/gis/<level>.ron` should say where its data came
from and under what licence, in a comment.

## Phases

| # | What | Done when |
|---|------|-----------|
| 0 | **Explore.** Pick a test area (your own street). Download Byggnad + Topografi 10 for it; look at the layers and attribute values in QGIS over the orthophoto. Note the real `ändamål` / land cover values. | A list of the attribute values we need rules for, and one GeoJSON clip per layer in `gis-raw/`. |
| 1 | **Buildings only.** `gis-import` feature + `--import-gis`; transform, buildings as `w` walls, a border, one spawn/goal by hand. Pure functions (`transform`, `rasterise_polygon`, `snap_footprint`) with unit tests, like `procgen` and `wall_mesh`. | The street loads in the game and you recognise it. |
| 2 | **Ground.** Roads (buffered lines), land cover, water. Automatic rotation. Path check. | Gardens are grass, streets are floor, the lake is water, houses are square. |
| 3 | **Gameplay suggestions.** Spawns at road exits, goal by address, fences/hedges as directional walls, trees as decorations. | A level that plays without a hand pass (it gets one anyway). |
| 4 | **Real houses.** A `HouseDef` per building from its own footprint and the template rules from the attribute; the house mesh work (`docs/house-editor.md`) then draws them. Roof heights from laser data if the licence allows. | Houses have doors, windows and the right shape. |
| 5 | **Polish.** Street names and addresses in story crawls, lights along streets, `[houses]` instead of tiles where it fits, an in-game "import area" preview in the map editor (stretch). | A level that tells you where you are. |

Phases 1-2 are the core. Each phase is small enough for one session and is useful on its own.

## Level ideas

- **Your own street**: the obvious first level, and the best test, because you know what it
  should look like.
- **Villakvarter**: detached houses, hedges, cul-de-sacs (aliens come in from the main road,
  the vändplan is a dead end). The game's home ground.
- **Radhusområde**: long rows, narrow paths between them: natural chokepoints.
- **Miljonprogram**: big slab blocks around courtyards, car-free paths, parking decks. Large
  open courtyards make good tower ground.
- **Bruksort**: a mill town by a river; the river and the bridge decide the level.
- **Sommarstugeområde**: forest, lake, scattered cottages; aliens from the water.

## Open questions

- Which products does your Geotorget account have, and under what licences? This decides
  what can be committed.
- What area first? Its coordinates give the test fixture for phase 1.
- At 1 m per tile, is a real house the right size next to the characters and towers? Check
  one imported house against `bungalow` in the game before importing a whole street.
- Should imported levels go in the campaign, or stay a "your neighbourhood" mode built by
  each player locally (which would also avoid the licence question)?
