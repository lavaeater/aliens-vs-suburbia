#### Generating a Map
What about drawing a map as a 2D image, a classic, and using that to algorithmically construct the map? Or rather, why not have a data-driven map? Basically the user could draw polygons on some kind of 2D plane. These polygons can then be imbued with properties or components if you will, that define the settings to create the 3D model for the object on the map that the polygon represents. The map definition can then be saved as a ron- or json-file.
##### Example - the House
A house is a polygon that defines the base or foundation of the house. To construct the house we use this polygon to create the walls. Walls are simply the foundation / base of the house plus height - in the simplest case. Variation on the walls is accomplished by applying rules and parameters to it, controlling number and size of windows, for example.
Top level house parameters could be a material, indicators on wether the corners of the house are flat (as in a stone house) or have wind protective planks on the outside. If the material for walls is wood panel it could be defined wether the panels are horizontal or vertical. Top level is then also number of doors going into the house (at least one, normally).
Then going from top down, wall definitions would be their height and then how many windows, size of windows - so the number of windows would then vary so that we define it as the wall having 1 window per 4 meters, max, so a short 4-metre wall would have 0-1 windows, the long side of the house would probably have 2-4 windows plus one of the required doors.
Roof definitions would define what materials, if they are saddle or flat (these are basically swedish construction terms translated directly). Constructing a mesh with a proper tiled roof with a chimney somehwere should be simple.
So a map could then be defined as some polygon that is of type "Grassland" or something - which would impose some kind of variation according to settings and rules for grassland, and on that polygon, representing perhaps a residential street in a suburb, we could draw up some polygons of varying form and size (to create square house, rectangular houses or L-shaped houses) and given rules for house building the would be constructed in some way. The houses could be placed inside "Yard" polygons that have rules that define that they should have fences, some shrubbery and stuff.
Building hierarchical definitions like this could help us in the construction of procedurally generated maps.
If we then assume that a map definition is a jsonfile of polygons + what those polygons represent, then an actual map is the instantiation of that entire map, i.e. the concrete resolution of rules and parameters applied and what that output represents - hopefully then becoming either a map-instance file as ron OR as BSN or some other format that is suitable!
#### The Actual Style of the Actual Game
I am working on several things at once, so we have the models (playable characters and gun handling and stuff), they visual style of the game, map creation etc.
So some thoughts on modifying the shaders and stuff would be removing highlights entirely (to make materials 100% flat), but keep shadows - but perhaps make them entirely black?
One thought would also be to reduce the palette of the game, so that we get a more retro look.
### What to do now
Start working on a simple 2D polygon editor. Every polygon must be closed, we should have options to make perpendicular angles, adding more nodes to a polygon and stuff. Then we should

---

## Implementation Plan: Polygon-Based House / Map Generator

This is a plan for turning the sketch above into something buildable, grounded in what already exists in the codebase rather than starting from scratch.

### Why not throw out the tile grid

`MapFile` (`src/general/components/map_components.rs`) is tile-based: a `Vec<Vec<u64>>` of `BitFlags<MapFeatures>` for pathfinding/collision, plus `placements: Vec<TilePlacement>` for def-driven models on tiles, plus free-floating `decorations`. `map_editor/` already has a working grid painter (palette tabs, brush rotation, `place_at`/`erase_at`, save/load) and `map/map_generator.rs` + `map/scatter.rs` already do seeded procedural generation and prop scattering onto that grid.

The polygon idea shouldn't replace this — it should sit **above** it as an authoring layer. A polygon-defined house is a generator that resolves down to ordinary `TilePlacement`s (for the footprint/walls, snapped to the grid) plus free-form decorations/meshes (for roofs, trim, windows) that get baked into a `MapFile`. That keeps the runtime (`map_loader`, pathfinding, `MapGraph`) completely untouched — it only ever sees tiles and placements, same as today.

### Data model

New types, probably in `src/map/procgen/` (new module):

- `Polygon { points: Vec<Vec2> }` — closed, in map-local (meter) space, not grid cells. Validate closure + non-self-intersection on edit.
- `HouseSpec` — parameters hung off a house polygon: wall material, corner style (flat vs. planked), panel orientation (if wood), door count/placement rule, wall-height, and a `WindowRule { max_per_wall_len: f32, min_len_for_window: f32 }` akin to the "1 window per 4m" idea in the doc above.
- `RoofSpec { style: Saddle | Flat, material, chimney: Option<ChimneySpec> }`.
- `ZoneSpec` — the outer "Grassland"/"Yard"/"Street" polygon type mentioned in the doc, carrying a rule-set (what may be placed inside it, density, prop palette — reusing `scatter.rs`'s `Prop` tables per zone type).
- `MapBlueprint { zones: Vec<(Polygon, ZoneSpec)>, houses: Vec<(Polygon, HouseSpec)>, seed: u64 }` — this is the *authored*, unresolved document (candidate for the ron/json file mentioned in the doc). It is small and human-editable, unlike `MapFile` which is the resolved instance.

### Resolution pipeline (`MapBlueprint` -> `MapFile`)

1. **Rasterize house footprints to tiles.** Walk each house `Polygon`, mark enclosed grid cells as floor, mark boundary-adjacent cells (or edge segments, if we go finer than whole tiles) as wall via existing `TILE_WALL_*` constants.
2. **Wall segmentation + rules.** For each polygon edge, compute its length, decide door/window count from `WindowRule`, and emit `TilePlacement`s referencing wall/window/door def files (new `AssetDefinition`s of `ModelType::Terrain` or a new `Wall`-flavored type) with `rotation_steps` derived from edge direction (reusing the existing 45°-step rotation already in `TilePlacement`).
3. **Roof + non-tile geometry.** Roofs, chimneys, and anything that doesn't cleanly map to a tile become `DecorationItem`s (free `x, y, model, rotation_y, scale` — no collider, matching how `scatter.rs` already dresses maps) or, if procedural mesh generation is wanted later, actual generated `Mesh` assets spawned at map-load time rather than baked into the `.ron`.
4. **Zone-driven scatter.** Once houses are placed, run `scatter.rs`-style prop placement inside each `Yard`/`Grassland` polygon using its `ZoneSpec`'s palette — this is almost exactly what `scatter.rs` does today, just gated by "am I inside this polygon" instead of "is this a floor tile in the generated map."

This pipeline is a pure function `MapBlueprint -> MapFile` with no Bevy dependency, so it can be unit-tested standalone (compare to how `hardpoint.rs`'s snap math is unit-tested) before any UI exists.

### Editor UI

Two reasonable paths, in order of effort:

1. **Cheapest: extend `map_editor`.** Add a "Polygon" tool mode alongside the existing tile brush: click to drop nodes, click-near-start to close, drag a node to move it, a modifier key to snap new edges to perpendicular/axis-aligned angles (the doc's requirement). On close, immediately run the resolution pipeline and stamp the result into the existing `MapEditorState.tiles`/`placements`, so the rest of the editor (palette, wave editor, save) doesn't need to change at all. Houses become "just another brush stroke" the painter can still hand-touch-up afterward.
2. **More ambitious: a standalone 2D polygon canvas** (new `GameState`, or a `PlaygroundSession`-style overlay like `src/playground/`) that edits a whole `MapBlueprint` with zones and houses as first-class objects, live-previews the resolved `MapFile` in a 3D pane (same split-pane trick `src/playground/` already uses — UI pane + clipped `Camera::viewport`), and only writes tiles on "Bake". This is the "right" long-term tool but is a much bigger lift (undo/redo, polygon boolean ops for overlapping zones, etc.).

Given the codebase already has both a working tile editor and a two-pane live-preview pattern (`playground`), **path 1 first** is the pragmatic move: get one procedural house type (rectangular, then L-shaped) resolving correctly and hand-tunable in the existing editor, then decide if the standalone polygon canvas is worth building based on how much friction remains.

### Suggested order of work

1. `HouseSpec`/`Polygon` types + the rasterize-and-wall-segment resolver, unit-tested with no Bevy/UI involved (feed it a rectangle, assert wall tile placements + door/window counts land where expected).
2. Wire a "Rectangle House" brush into `map_editor` that runs the resolver and writes tiles/placements — this alone validates the whole pipeline end-to-end.
3. Generalize the resolver to arbitrary closed polygons (L-shapes etc.), add the perpendicular-snap editing affordance.
4. Add roof/chimney as decorations.
5. Add `ZoneSpec` + zone-scoped scatter, reusing `scatter.rs`'s prop tables.
6. Only then consider the standalone polygon-canvas editor, if the in-place brush proves too limiting.

---

## Thoughts: Styling the Game and Shaders

The doc's instinct (flatten materials, keep shadows but make them starker, cut the palette) is already mostly *available* today, not hypothetical — `bevy_wind_waker_shader` (vendored submodule, already a dependency in `Cargo.toml` and wired up in `src/main.rs` via `FlatShaderPlugin`) ships exactly this:

- `FlatShaderBuilder::shadow_darkness(0.0)` — pushes shadow areas toward pure black without touching lit-area color, which is precisely "keep shadows, make them entirely black."
- `FlatShaderBuilder::highlight_tint(Color::WHITE)` (the default) already avoids adding any highlight tint on top of the base texture — "remove highlights entirely" is close to free; if any residual specular pop is visible it's coming from Bevy's underlying PBR `StandardMaterial` still contributing specular highlights *before* the flat shader's shadow/highlight mix is applied, since the flat shader multiplies onto the lit result rather than replacing the lighting model. Worth checking whether PBR's specular term needs zeroing (e.g. forcing `perceptual_roughness` high / `metallic` 0 on materials the shader touches) to fully kill specular pop, separate from the toon banding.
- `FlatShaderBuilder::color_levels(n)` — this *is* the palette reduction / posterization knob already ("6.0 gives a 216-color palette... lower values give a more extreme poster-art look"). Dropping this from the default 16 down to 4-6 is a one-line experiment (`FlatShaderPlugin::global_with(FlatShaderBuilder::default().color_levels(4.0).shadow_darkness(0.0).build())` in `main.rs`) that directly tests both ideas in the doc at once, with no new shader code.
- There's a second, unused-so-far option in the same crate: `WindWakerShaderBuilder` (`time_of_day` x `weather` hardcoded palettes, plus a rim-light term) — a heavier stylization than the flat shader, probably not what's wanted here since the doc explicitly wants *less* stylized lighting, not Wind-Waker's painterly rim/palette replacement. And `PixelShaderBuilder` (pixelate + color_levels) is a third, orthogonal lever — a retro *resolution* cut rather than a palette cut, stackable with the flat shader's palette cut if a stronger retro look is wanted later.

Suggested next steps, cheapest first:
1. Tune the existing `FlatShaderPlugin::global()` call in `main.rs` with `shadow_darkness` near 0 and `color_levels` in the 4-8 range; look at it in-game against a few different packs (the doc's "reduce the palette... more retro look" is exactly `color_levels`).
2. If specular pop is still visible on lit surfaces, check whether it's coming from `StandardMaterial` PBR terms feeding into the flat shader's lit branch, and consider clamping `perceptual_roughness`/`metallic` on affected materials (either globally in the shader-application system, or per-`AssetDefinition`).
3. Only reach for `WindWakerShaderBuilder` or `PixelShaderBuilder` if the flat-shader tuning alone doesn't get far enough — they're bigger stylistic swings (full palette replacement, resolution downsample) rather than the "flatten what's there" ask in the doc.
4. Since the map generator (`scatter.rs`) already leans into an "ultraviolent suburb" look via its own asset choices, a palette cut should be evaluated together with that scatter content, not in isolation — cutting `color_levels` too far may make blood/debris decals unreadable against posterized floors.
