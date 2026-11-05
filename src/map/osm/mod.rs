//! Levels from OpenStreetMap: an `.osm` file (openstreetmap.org's Export button) or an
//! Overpass JSON export in, a `.map` level out.
//! Plan and usage: `docs/gis-levels.md`.
//!
//! `cargo run -- --import-osm assets/gis/<level>.ron` reads the import spec
//! ([`OsmLevelSpec`]), the OSM export it names, and writes the level. Passing the export
//! itself (`--import-osm gis-raw/street.osm`) imports it with the default spec. The game
//! never reads OSM data; the `.map` it writes is an ordinary level to touch up by hand.
//!
//! What becomes what (`classify.rs`): buildings are solid walls, roads and car parks
//! floor, parks and gardens grass, water water, fences and hedges block the aliens (players
//! hop over; [`FenceRule`]), walls block everyone, trees are decorations, forests get pines. The level is
//! walled in, and every road that leaves it becomes an alien spawn. The goal is the spot
//! the spec names (or the middle), the player starts next to it, and spawns that can't reach
//! the goal are dropped and reported.
// Grid code: indices come from the grid's own ranges or are checked with `get`.
#![allow(clippy::indexing_slicing)]

pub mod classify;
pub mod data;
pub mod geometry;
mod land_cover;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::general::components::map_components::{DecorationItem, LootSpread, MapFile, WaveDef};
use crate::map::{BitFlags, MapFeatures};
pub use classify::FenceRule;
use classify::{Ground, Paint, classify};
use data::{LatLon, OsmData, Shape};
use geometry::{Frame, Point, Projection};

/// A level is never larger than this many tiles a side; past it the spec has to set a
/// smaller `size` or a larger `metres_per_tile`.
pub const MAX_SIDE: usize = 1024;

/// Space left around the buildings when the spec gives no `size`, metres per side.
const MARGIN_M: f64 = 6.0;

/// The import spec, `assets/gis/<level>.ron`. Everything but `source` has a default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmLevelSpec {
    /// Display name of the level.
    #[serde(default)]
    pub name: String,
    /// The OSM data, relative to the project root: an `.osm` XML file or an Overpass JSON
    /// export.
    pub source: String,
    /// Where to write the level. Default: `assets/maps/<spec file stem>.map`.
    #[serde(default)]
    pub output: Option<String>,
    /// Middle of the level, `(lat, lon)`. Default: the middle of the buildings.
    #[serde(default)]
    pub centre: Option<(f64, f64)>,
    /// Width and height of the level, metres (before rotation). Default: the extent of the
    /// buildings plus a margin.
    #[serde(default)]
    pub size: Option<(f64, f64)>,
    /// Turn the area this many degrees counter-clockwise so streets follow the grid.
    /// Default: worked out from the building edges. `Some(0.0)` keeps north up.
    #[serde(default)]
    pub rotation_deg: Option<f64>,
    #[serde(default = "one")]
    pub metres_per_tile: f64,
    #[serde(default)]
    pub fences: FenceRule,
    /// What the aliens are after, `(lat, lon)`. Default: the middle of the level.
    #[serde(default)]
    pub goal: Option<(f64, f64)>,
    /// At most this many alien spawns, the widest roads first.
    #[serde(default = "four")]
    pub max_spawns: usize,
    /// Lantmäteriet land cover (`land_cover.rs`), a GeoJSON file. Its forests join OSM's.
    #[serde(default)]
    pub land_cover: Option<String>,
    /// Trees as decorations: mapped ones (`natural=tree`), and pines in the forests.
    #[serde(default = "yes")]
    pub trees: bool,
    /// Copied into the level. Empty = the game's default waves.
    #[serde(default)]
    pub waves: Vec<WaveDef>,
    #[serde(default = "default_loot")]
    pub loot: Option<LootSpread>,
    /// The level's story file, copied into the level.
    #[serde(default)]
    pub stories: String,
}

const fn one() -> f64 {
    1.0
}
const fn four() -> usize {
    4
}
const fn yes() -> bool {
    true
}
fn default_loot() -> Option<LootSpread> {
    Some(LootSpread {
        per_tiles: 40,
        ..LootSpread::default()
    })
}

impl OsmLevelSpec {
    /// The default spec for an export.
    pub fn for_source(source: impl Into<String>) -> Self {
        Self {
            name: String::new(),
            source: source.into(),
            output: None,
            centre: None,
            size: None,
            rotation_deg: None,
            metres_per_tile: one(),
            fences: FenceRule::default(),
            goal: None,
            max_spawns: four(),
            land_cover: None,
            trees: yes(),
            waves: Vec::new(),
            loot: default_loot(),
            stories: String::new(),
        }
    }
}

/// An imported level and what the importer has to say about it.
#[derive(Debug)]
pub struct Imported {
    pub map: MapFile,
    pub report: Vec<String>,
}

/// Pines for a forest, with their models' height. Drawn [`FOREST_HEIGHT`] tall, not to scale:
/// the camera sits about 3.2 above the floor at its default zoom.
const PINES: &[(&str, f32)] = &[
    ("packs/nature/Pine.glb", 8.49),
    ("packs/nature/Pine-79gmlLnweB.glb", 10.0),
    ("packs/nature/Pine-Zt62gceKXZ.glb", 7.14),
];
const FOREST_HEIGHT: f32 = 2.0;
/// A forest has at most one tree in each square of this many tiles, a tile clear of the
/// next square's, so it can never close a way through.
const FOREST_SPACING: usize = 4;

/// Tree models for `natural=tree`, with the height they're drawn at (as in the generator).
const TREES: &[(&str, f32)] = &[
    ("packs/nature/Tree.glb", 4.0),
    ("packs/nature/Tree-aVOxaHRPWe.glb", 4.5),
    ("packs/city/Tree.glb", 4.0),
    ("packs/toon-shooter/Tree.glb", 3.5),
    ("packs/nature/Pine.glb", 3.5),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Open,
    Road,
    Building,
    Wall,
    Fence,
    Tree,
}

fn bits(flags: impl Into<BitFlags<MapFeatures>>) -> u64 {
    flags.into().bits()
}

fn ground_bits(ground: Ground) -> u64 {
    bits(match ground {
        Ground::Grass => MapFeatures::Grass,
        Ground::Floor => MapFeatures::Floor,
        Ground::Mud => MapFeatures::Mud,
        Ground::Rock => MapFeatures::Rock,
        Ground::Water => MapFeatures::Water,
    })
}

fn wall_bits() -> u64 {
    bits(MapFeatures::Floor | MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies)
}

fn blocks_everyone() -> u64 {
    bits(MapFeatures::ImpassableForEnemies | MapFeatures::ImpassableForPlayers)
}

fn enemy_passable(tile: u64) -> bool {
    tile != 0 && !BitFlags::<MapFeatures>::from_bits_truncate(tile).contains(MapFeatures::ImpassableForEnemies)
}

fn player_passable(tile: u64) -> bool {
    tile != 0 && !BitFlags::<MapFeatures>::from_bits_truncate(tile).contains(MapFeatures::ImpassableForPlayers)
}

/// A number that looks random and is the same every import.
const fn tile_hash(col: usize, row: usize) -> usize {
    let mixed = (col as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((row as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    ((mixed ^ (mixed >> 29)) >> 16) as usize
}

/// Scatter pines over the `forest` tiles: one to a square of [`FOREST_SPACING`], on open
/// grass with open ground all around, so roads and doorsteps stay clear. Each blocks its tile.
fn plant_forest(
    forest: &[Vec<bool>],
    layer: &mut [Vec<Layer>],
    ground: &[Vec<Ground>],
) -> Vec<DecorationItem> {
    let open = |layer: &[Vec<Layer>], col: usize, row: usize| {
        layer.get(row).and_then(|r| r.get(col)) == Some(&Layer::Open)
    };
    let (height, width) = (layer.len(), layer.first().map_or(0, Vec::len));
    let mut pines = Vec::new();
    for top in (0..height).step_by(FOREST_SPACING) {
        for left in (0..width).step_by(FOREST_SPACING) {
            let hash = tile_hash(left, top);
            // Short of the square's last column and row: that is the gap to the next.
            let room = FOREST_SPACING - 1;
            let (col, row) = (left + hash % room, top + (hash >> 8) % room);
            let clear = (col.saturating_sub(1)..=col + 1)
                .all(|c| (row.saturating_sub(1)..=row + 1).all(|r| open(layer, c, r)));
            let on_grass = ground.get(row).and_then(|r| r.get(col)) == Some(&Ground::Grass);
            let wooded = forest.get(row).and_then(|r| r.get(col)) == Some(&true);
            let inside = col > 0 && row > 0;
            if !(clear && on_grass && wooded && inside) {
                continue;
            }
            let Some(&(model, model_height)) = PINES.get((hash >> 16) % PINES.len()) else {
                continue;
            };
            if let Some(tile) = layer.get_mut(row).and_then(|r| r.get_mut(col)) {
                *tile = Layer::Tree;
            }
            let size = 0.8 + ((hash >> 20) % 5) as f32 * 0.1; // 0.8 to 1.2
            pines.push(DecorationItem {
                x: col as i32,
                y: row as i32,
                model: model.to_string(),
                rotation_y: ((hash >> 24) % 8) as f32 * 45.0,
                scale: FOREST_HEIGHT * size / model_height,
            });
        }
    }
    pines
}

/// A feature in metres, with what it paints.
struct Projected {
    id: i64,
    paint: Paint,
    storeys: u8,
    rings: Vec<Vec<Point>>,
}

/// Turn OSM data into a level.
pub fn import(spec: &OsmLevelSpec, data: &OsmData) -> Result<Imported, String> {
    let mut report = Vec::new();
    if spec.metres_per_tile <= 0.0 {
        return Err("metres_per_tile must be above 0".into());
    }

    let classified: Vec<(Paint, &data::Feature)> = data
        .features
        .iter()
        .filter_map(|f| classify(&f.tags, matches!(f.shape, Shape::Area(_))).map(|p| (p, f)))
        .collect();
    if classified.is_empty() {
        return Err("the export has nothing a level can use (no buildings, roads or land)".into());
    }

    let centre = match spec.centre {
        Some((lat, lon)) => LatLon { lat, lon },
        None => {
            let buildings: Vec<&data::Feature> = classified
                .iter()
                .filter(|(p, _)| *p == Paint::Building)
                .map(|(_, f)| *f)
                .collect();
            let source: Vec<&data::Feature> = if buildings.is_empty() {
                classified.iter().map(|(_, f)| *f).collect()
            } else {
                buildings
            };
            middle(source.iter().flat_map(|f| points(&f.shape)))
                .ok_or("the export has no coordinates")?
        }
    };
    let projection = Projection::new(centre);

    let projected: Vec<Projected> = classified
        .iter()
        .map(|(paint, f)| Projected {
            id: f.id,
            paint: *paint,
            storeys: if *paint == Paint::Building {
                classify::storeys(&f.tags)
            } else {
                0
            },
            rings: match &f.shape {
                Shape::Point(p) => vec![vec![projection.project(*p)]],
                Shape::Line(line) => vec![line.iter().map(|p| projection.project(*p)).collect()],
                Shape::Area(rings) => rings
                    .iter()
                    .map(|r| r.iter().map(|p| projection.project(*p)).collect())
                    .collect(),
            },
        })
        .collect();

    let rotation = match spec.rotation_deg {
        Some(degrees) => degrees.to_radians(),
        None => geometry::dominant_rotation(
            projected
                .iter()
                .filter(|f| f.paint == Paint::Building)
                .flat_map(|f| f.rings.iter())
                .flat_map(|ring| ring.windows(2).map(|w| (w[0], w[1]))),
        ),
    };

    let (width_m, height_m) = match spec.size {
        Some(size) => size,
        None => {
            let turned = Frame {
                rotation,
                metres_per_tile: 1.0,
                width: 0,
                height: 0,
            };
            let buildings: Vec<Point> = projected
                .iter()
                .filter(|f| f.paint == Paint::Building)
                .flat_map(|f| f.rings.iter().flatten())
                .map(|p| turned.to_tiles(*p))
                .collect();
            let half = |axis: fn(&Point) -> f64| {
                buildings.iter().map(|p| axis(p).abs()).fold(0.0, f64::max)
            };
            (2.0 * (half(|p| p.0) + MARGIN_M), 2.0 * (half(|p| p.1) + MARGIN_M))
        }
    };
    // One extra tile each side for the wall around the level.
    let width = (width_m / spec.metres_per_tile).ceil().max(4.0) as usize + 2;
    let height = (height_m / spec.metres_per_tile).ceil().max(4.0) as usize + 2;
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!(
            "the level would be {width} x {height} tiles (max {MAX_SIDE} a side); set a smaller \
             `size` or a larger `metres_per_tile` in the spec"
        ));
    }
    let frame = Frame {
        rotation,
        metres_per_tile: spec.metres_per_tile,
        width,
        height,
    };
    report.push(format!(
        "{width} x {height} tiles ({:.0} x {:.0} m), turned {:.1} degrees",
        width_m,
        height_m,
        rotation.to_degrees()
    ));

    let in_tiles = |f: &Projected| -> Vec<Vec<Point>> {
        f.rings
            .iter()
            .map(|r| r.iter().map(|p| frame.to_tiles(*p)).collect())
            .collect()
    };

    let mut ground = vec![vec![Ground::Grass; width]; height];
    let mut layer = vec![vec![Layer::Open; width]; height];
    let mut decorations = Vec::new();
    let mut counts = [0usize; 6]; // buildings, roads, fences, walls, trees, areas

    // Ground areas, largest first so the small ones end up on top.
    let mut areas: Vec<(f64, Ground, Vec<Vec<Point>>)> = projected
        .iter()
        .filter_map(|f| {
            let g = match f.paint {
                Paint::Ground(g) => g,
                Paint::Forest => Ground::Grass,
                _ => return None,
            };
            let area = f.rings.iter().map(|r| geometry::ring_area(r)).sum::<f64>();
            Some((area, g, in_tiles(f)))
        })
        .collect();
    areas.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, g, rings) in &areas {
        for (col, row) in geometry::fill_rings(rings, width, height) {
            ground[row][col] = *g;
        }
    }
    counts[5] = areas.len();

    let tiles_wide = |metres: f64| metres / spec.metres_per_tile;
    for f in &projected {
        if let Paint::Strip { ground: g, width: w } = f.paint {
            for line in in_tiles(f) {
                for (col, row) in geometry::stroke_line(&line, tiles_wide(w), width, height) {
                    ground[row][col] = g;
                }
            }
        }
    }
    for f in &projected {
        if let Paint::Road { width: w } = f.paint {
            counts[1] += 1;
            for line in in_tiles(f) {
                for (col, row) in geometry::stroke_line(&line, tiles_wide(w), width, height) {
                    ground[row][col] = Ground::Floor;
                    layer[row][col] = Layer::Road;
                }
            }
        }
    }
    let mut storeys = vec![vec![0u8; width]; height];
    for f in projected.iter().filter(|f| f.paint == Paint::Building) {
        counts[0] += 1;
        for (col, row) in geometry::fill_rings(&in_tiles(f), width, height) {
            layer[row][col] = Layer::Building;
            storeys[row][col] = storeys[row][col].max(f.storeys);
        }
    }
    for f in &projected {
        let (kind, index) = match f.paint {
            Paint::Wall => (Layer::Wall, 3),
            Paint::Fence if spec.fences != FenceRule::Ignore => (Layer::Fence, 2),
            _ => continue,
        };
        counts[index] += 1;
        for line in in_tiles(f) {
            for (col, row) in geometry::stroke_line(&line, 0.0, width, height) {
                // Roads stay open (a fence across a road is a gate), houses stay houses.
                if matches!(layer[row][col], Layer::Open | Layer::Fence) {
                    layer[row][col] = kind;
                }
            }
        }
    }
    if spec.trees {
        for f in projected.iter().filter(|f| f.paint == Paint::Tree) {
            let Some(&p) = f.rings.first().and_then(|r| r.first()) else {
                continue;
            };
            let Some((col, row)) = frame.cell(p) else {
                continue;
            };
            if layer[row][col] != Layer::Open || ground[row][col] == Ground::Water {
                continue;
            }
            let pick = f.id.unsigned_abs() as usize;
            let Some(&(model, scale)) = TREES.get(pick % TREES.len()) else {
                continue;
            };
            counts[4] += 1;
            decorations.push(DecorationItem {
                x: col as i32,
                y: row as i32,
                model: model.to_string(),
                rotation_y: (pick / TREES.len() % 8) as f32 * 45.0,
                scale,
            });
        }

        let mut forest = vec![vec![false; width]; height];
        for f in projected.iter().filter(|f| f.paint == Paint::Forest) {
            for (col, row) in geometry::fill_rings(&in_tiles(f), width, height) {
                forest[row][col] = true;
            }
        }
        let pines = plant_forest(&forest, &mut layer, &ground);
        report.push(format!("{} pines in the forests", pines.len()));
        decorations.extend(pines);
    }
    report.push(format!(
        "{} buildings, {} roads and paths, {} fences/hedges, {} walls, {} trees, {} ground areas",
        counts[0], counts[1], counts[2], counts[3], counts[4], counts[5]
    ));

    let fence_bits = match spec.fences {
        FenceRule::BlocksEnemies => bits(MapFeatures::ImpassableForEnemies),
        FenceRule::BlocksEveryone => {
            bits(MapFeatures::ImpassableForEnemies | MapFeatures::ImpassableForPlayers)
        }
        FenceRule::Ignore => 0,
    };
    let mut tiles: Vec<Vec<u64>> = (0..height)
        .map(|row| {
            (0..width)
                .map(|col| match layer[row][col] {
                    Layer::Building | Layer::Wall => wall_bits(),
                    Layer::Tree => ground_bits(ground[row][col]) | blocks_everyone(),
                    Layer::Road => ground_bits(Ground::Floor),
                    Layer::Fence => ground_bits(ground[row][col]) | fence_bits,
                    Layer::Open => ground_bits(ground[row][col]),
                })
                .collect()
        })
        .collect();

    // A wall or fence drawn across a building took the tile.
    for (storeys, layer) in storeys.iter_mut().zip(&layer) {
        for (storeys, layer) in storeys.iter_mut().zip(layer) {
            if *layer != Layer::Building {
                *storeys = 0;
            }
        }
    }

    // Wall the level in; the roads that cross the edge are where the aliens come from.
    let perimeter = perimeter(width, height);
    let mut runs: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut previous_was_road = false;
    for &(col, row) in &perimeter {
        let road = layer[row][col] == Layer::Road;
        if road {
            match runs.last_mut() {
                Some(run) if previous_was_road => run.push((col, row)),
                _ => runs.push(vec![(col, row)]),
            }
        }
        previous_was_road = road;
    }
    // A run that wraps past the start of the perimeter is one road.
    if runs.len() > 1
        && perimeter.first().is_some_and(|&(c, r)| layer[r][c] == Layer::Road)
        && perimeter.last().is_some_and(|&(c, r)| layer[r][c] == Layer::Road)
        && let Some(last) = runs.pop()
        && let Some(first) = runs.first_mut()
    {
        first.splice(0..0, last);
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.len()));
    for &(col, row) in &perimeter {
        tiles[row][col] = wall_bits();
    }
    let mut spawns: Vec<(usize, usize)> = runs
        .iter()
        .take(spec.max_spawns)
        .filter_map(|run| run.get(run.len() / 2).copied())
        .collect();

    // The goal: the named spot or the middle, moved to the nearest open tile.
    let target = match spec.goal {
        Some((lat, lon)) => frame.to_tiles(projection.project(LatLon { lat, lon })),
        None => (width as f64 / 2.0, height as f64 / 2.0),
    };
    let goal = (1..height - 1)
        .flat_map(|row| (1..width - 1).map(move |col| (col, row)))
        .filter(|&(col, row)| enemy_passable(tiles[row][col]) && player_passable(tiles[row][col]))
        .min_by(|a, b| {
            let d = |&(c, r): &(usize, usize)| {
                (c as f64 + 0.5 - target.0).powi(2) + (r as f64 + 0.5 - target.1).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
        .ok_or("no open ground for the goal: the level is all buildings")?;
    tiles[goal.1][goal.0] = bits(MapFeatures::Floor | MapFeatures::EnemyExit);

    let spawn_bits = bits(MapFeatures::Floor | MapFeatures::EnemySpawn);
    for &(col, row) in &spawns {
        tiles[row][col] = spawn_bits;
    }
    let reach = distances(&tiles, goal, enemy_passable);
    let before = spawns.len();
    spawns.retain(|&(col, row)| reach[row][col].is_some());
    for &(col, row) in &perimeter {
        if tiles[row][col] == spawn_bits && !spawns.contains(&(col, row)) {
            tiles[row][col] = wall_bits();
        }
    }
    if spawns.len() < before {
        report.push(format!(
            "dropped {} alien spawn(s) with no path to the goal",
            before - spawns.len()
        ));
    }
    if spawns.is_empty() {
        // No road leaves the level (or none reaches the goal): open the edge where the
        // aliens have the longest walk.
        let farthest = perimeter
            .iter()
            .filter_map(|&(col, row)| {
                let inward = inward(col, row, width, height)?;
                reach[inward.1][inward.0].map(|d| (d, (col, row)))
            })
            .max_by_key(|(d, _)| *d);
        match farthest {
            Some((_, (col, row))) => {
                tiles[row][col] = spawn_bits;
                spawns.push((col, row));
                report.push(
                    "no road leaves the level, so the aliens come from the edge farthest from the goal"
                        .into(),
                );
            }
            None => report.push("no alien spawn could be placed; add one by hand".into()),
        }
    }
    report.push(format!("{} alien spawn(s)", spawns.len()));

    // The players start a few steps from the goal.
    let walk = distances(&tiles, goal, player_passable);
    let start = (0..height)
        .flat_map(|row| (0..width).map(move |col| (col, row)))
        .filter(|&(col, row)| {
            (col, row) != goal && enemy_passable(tiles[row][col]) && tiles[row][col] != spawn_bits
        })
        .filter_map(|(col, row)| walk[row][col].map(|d| (d, (col, row))))
        .min_by_key(|(d, _)| d.abs_diff(4))
        .map(|(_, cell)| cell);
    match start {
        Some((col, row)) => tiles[row][col] = bits(MapFeatures::Floor | MapFeatures::PlayerSpawn),
        None => report.push("no room for the player start next to the goal; add one by hand".into()),
    }

    let name = if spec.name.is_empty() {
        "OpenStreetMap level".to_string()
    } else {
        spec.name.clone()
    };
    Ok(Imported {
        map: MapFile {
            name,
            tiles,
            storeys,
            decorations,
            waves: spec.waves.clone(),
            stories: spec.stories.clone(),
            loot: spec.loot.clone(),
            ..MapFile::default()
        },
        report,
    })
}

fn points(shape: &Shape) -> Vec<LatLon> {
    match shape {
        Shape::Point(p) => vec![*p],
        Shape::Line(line) => line.clone(),
        Shape::Area(rings) => rings.iter().flatten().copied().collect(),
    }
}

/// The middle of the points' lat/lon bounding box.
fn middle(points: impl Iterator<Item = LatLon>) -> Option<LatLon> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for p in points {
        let b = bounds.get_or_insert((p.lat, p.lon, p.lat, p.lon));
        *b = (b.0.min(p.lat), b.1.min(p.lon), b.2.max(p.lat), b.3.max(p.lon));
    }
    bounds.map(|(s, w, n, e)| LatLon {
        lat: (s + n) / 2.0,
        lon: (w + e) / 2.0,
    })
}

/// The edge tiles, clockwise from the top-left corner.
fn perimeter(width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut cells: Vec<(usize, usize)> = (0..width).map(|c| (c, 0)).collect();
    cells.extend((1..height).map(|r| (width - 1, r)));
    cells.extend((0..width - 1).rev().map(|c| (c, height - 1)));
    cells.extend((1..height - 1).rev().map(|r| (0, r)));
    cells
}

/// The tile just inside an edge tile (none for corners).
fn inward(col: usize, row: usize, width: usize, height: usize) -> Option<(usize, usize)> {
    let corner = (col == 0 || col == width - 1) && (row == 0 || row == height - 1);
    if corner {
        return None;
    }
    Some(if row == 0 {
        (col, 1)
    } else if row == height - 1 {
        (col, row - 1)
    } else if col == 0 {
        (1, row)
    } else {
        (col - 1, row)
    })
}

/// Steps from `from` to every tile, 4-connected over tiles `passable` allows.
fn distances(
    tiles: &[Vec<u64>],
    from: (usize, usize),
    passable: fn(u64) -> bool,
) -> Vec<Vec<Option<u32>>> {
    let height = tiles.len();
    let width = tiles.first().map_or(0, Vec::len);
    let mut dist = vec![vec![None; width]; height];
    let mut queue = VecDeque::from([(from, 0u32)]);
    dist[from.1][from.0] = Some(0);
    while let Some(((col, row), d)) = queue.pop_front() {
        let neighbours = [
            (col.wrapping_sub(1), row),
            (col + 1, row),
            (col, row.wrapping_sub(1)),
            (col, row + 1),
        ];
        for (c, r) in neighbours {
            let Some(&tile) = tiles.get(r).and_then(|t| t.get(c)) else {
                continue;
            };
            if dist[r][c].is_none() && passable(tile) {
                dist[r][c] = Some(d + 1);
                queue.push_back(((c, r), d + 1));
            }
        }
    }
    dist
}

/// `--import-osm <spec.ron | export.osm | export.json> [--force]`: write the level and print the report.
pub fn run_import(input: &Path, force: bool) -> Result<PathBuf, String> {
    let is_spec = input.extension().is_some_and(|e| e == "ron");
    let spec = if is_spec {
        let text = std::fs::read_to_string(input)
            .map_err(|e| format!("cannot read {}: {e}", input.display()))?;
        ron::from_str::<OsmLevelSpec>(&text)
            .map_err(|e| format!("cannot parse {}: {e}", input.display()))?
    } else {
        OsmLevelSpec::for_source(input.to_string_lossy())
    };
    let stem = input
        .file_stem()
        .map_or_else(|| "osm_level".to_string(), |s| s.to_string_lossy().into_owned());
    let output = spec
        .output
        .clone()
        .map_or_else(|| PathBuf::from(format!("assets/maps/{stem}.map")), PathBuf::from);
    if output.exists() && !force {
        return Err(format!(
            "{} already exists; pass --force to overwrite it",
            output.display()
        ));
    }
    let json = std::fs::read_to_string(&spec.source)
        .map_err(|e| format!("cannot read the OSM export {}: {e}", spec.source))?;
    let mut data = data::parse(&json)?;
    if let Some(path) = &spec.land_cover {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read the land cover export {path}: {e}"))?;
        data.features.extend(land_cover::forests(&text)?);
    }
    let mut imported = import(&spec, &data)?;
    if imported.map.name == "OpenStreetMap level" {
        imported.map.name = stem;
    }
    crate::map::text_format::write_map_file(&output, &imported.map)?;
    for line in &imported.report {
        println!("  {line}");
    }
    println!("Wrote {}", output.display());
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small made-up street at 59 N: a road running east-west through the middle, two
    /// houses north of it, a hedge between their gardens, a pond in the south-west, a
    /// tree. One degree of latitude is ~111 km, so 0.0001 is ~11 m.
    const STREET: &str = r#"{"elements":[
        {"type":"way","id":1,"tags":{"highway":"residential"},
         "geometry":[{"lat":59.0,"lon":17.9990},{"lat":59.0,"lon":18.0010}]},
        {"type":"way","id":2,"tags":{"building":"house"},
         "geometry":[{"lat":59.0001,"lon":17.9996},{"lat":59.0001,"lon":17.9998},
                     {"lat":59.0002,"lon":17.9998},{"lat":59.0002,"lon":17.9996},
                     {"lat":59.0001,"lon":17.9996}]},
        {"type":"way","id":3,"tags":{"building":"house"},
         "geometry":[{"lat":59.0001,"lon":18.0002},{"lat":59.0001,"lon":18.0004},
                     {"lat":59.0002,"lon":18.0004},{"lat":59.0002,"lon":18.0002},
                     {"lat":59.0001,"lon":18.0002}]},
        {"type":"way","id":4,"tags":{"barrier":"hedge"},
         "geometry":[{"lat":58.99995,"lon":18.0},{"lat":59.0003,"lon":18.0}]},
        {"type":"way","id":5,"tags":{"natural":"water"},
         "geometry":[{"lat":58.9998,"lon":17.9995},{"lat":58.9998,"lon":17.9997},
                     {"lat":58.9999,"lon":17.9997},{"lat":58.9999,"lon":17.9995},
                     {"lat":58.9998,"lon":17.9995}]},
        {"type":"node","id":6,"lat":59.00015,"lon":18.0006,"tags":{"natural":"tree"}}
    ]}"#;

    fn spec() -> OsmLevelSpec {
        let mut spec = OsmLevelSpec::for_source("test.json");
        spec.centre = Some((59.0, 18.0));
        spec.size = Some((80.0, 60.0));
        spec.rotation_deg = Some(0.0);
        spec
    }

    fn import_street(spec: &OsmLevelSpec) -> Imported {
        import(spec, &data::parse(STREET).unwrap()).unwrap()
    }

    fn count(map: &MapFile, value: u64) -> usize {
        map.tiles.iter().flatten().filter(|t| **t == value).count()
    }

    #[test]
    fn default_spec_parses_with_defaults() {
        let spec = OsmLevelSpec::for_source("x.json");
        assert_eq!(spec.source, "x.json");
        assert!((spec.metres_per_tile - 1.0).abs() < f64::EPSILON);
        assert_eq!(spec.max_spawns, 4);
        assert!(spec.trees);
        assert_eq!(spec.fences, FenceRule::BlocksEnemies);
    }

    #[test]
    fn buildings_stand_as_tall_as_their_tags_say() {
        let imported = import_street(&spec());
        let tallest = imported.map.storeys.iter().flatten().max().copied();
        assert_eq!(tallest, Some(2), "untagged houses guess two storeys");
        for (row, storeys) in imported.map.tiles.iter().zip(&imported.map.storeys) {
            for (&tile, &storeys) in row.iter().zip(storeys) {
                assert!(storeys == 0 || tile == wall_bits(), "only walls have storeys");
            }
        }

        let tags = |pairs: &[(&str, &str)]| -> data::Tags {
            pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect()
        };
        let storeys = |pairs: &[(&str, &str)]| classify::storeys(&tags(pairs));
        assert_eq!(storeys(&[("building", "yes"), ("building:levels", "4")]), 4);
        assert_eq!(storeys(&[("building", "shed"), ("building:levels", "2.5")]), 3);
        assert_eq!(storeys(&[("building", "yes"), ("height", "9 m")]), 3);
        assert_eq!(storeys(&[("building", "yes"), ("building:levels", "30")]), 9);
        assert_eq!(storeys(&[("building", "garage")]), 1);
        assert_eq!(storeys(&[("building", "apartments")]), 4);
        assert_eq!(storeys(&[("building", "yes"), ("building:levels", "many")]), 2);
    }

    fn square_forest(size: usize) -> (Vec<Vec<bool>>, Vec<Vec<Layer>>, Vec<Vec<Ground>>) {
        (
            vec![vec![true; size]; size],
            vec![vec![Layer::Open; size]; size],
            vec![vec![Ground::Grass; size]; size],
        )
    }

    #[test]
    fn a_forest_never_closes_a_way_through() {
        let (forest, mut layer, ground) = square_forest(80);
        let pines = plant_forest(&forest, &mut layer, &ground);
        assert!(pines.len() > 300, "a real forest: {}", pines.len());
        for row in &layer {
            assert!(row.iter().any(|t| *t == Layer::Open));
        }
        // No two pines touch, not even at a corner.
        for a in &pines {
            for b in &pines {
                let apart = (a.x - b.x).abs().max((a.y - b.y).abs());
                assert!(apart != 1, "{},{} touches {},{}", a.x, a.y, b.x, b.y);
            }
        }
    }

    #[test]
    fn pines_keep_off_what_is_not_open_grass() {
        let (forest, mut layer, mut ground) = square_forest(40);
        for row in &mut layer {
            row[20] = Layer::Road;
        }
        ground[5] = vec![Ground::Water; 40];
        for pine in plant_forest(&forest, &mut layer, &ground) {
            assert!((pine.x - 20).abs() > 1, "next to the road: {}", pine.x);
            assert_ne!(pine.y, 5, "in the water");
        }
    }

    #[test]
    fn the_same_forest_grows_every_import() {
        let (forest, mut layer, ground) = square_forest(40);
        let first = plant_forest(&forest, &mut layer.clone(), &ground);
        let second = plant_forest(&forest, &mut layer, &ground);
        assert_eq!(first.len(), second.len());
        assert!(first.iter().zip(&second).all(|(a, b)| (a.x, a.y) == (b.x, b.y)));
    }

    #[test]
    fn imports_a_street() {
        let imported = import_street(&spec());
        let map = &imported.map;
        assert_eq!(map.tiles.len(), 62);
        assert!(map.tiles.iter().all(|row| row.len() == 82));

        // Two houses of ~11 x 11 m.
        let walls = count(map, wall_bits()) - perimeter(82, 62).len();
        assert!((180..=300).contains(&walls), "{walls} wall tiles");
        // The road, the goal and the start.
        assert!(count(map, ground_bits(Ground::Floor)) > 300);
        assert_eq!(count(map, bits(MapFeatures::Floor | MapFeatures::EnemyExit)), 1);
        assert_eq!(count(map, bits(MapFeatures::Floor | MapFeatures::PlayerSpawn)), 1);
        // The road leaves the level on both sides: two spawns, on the edge.
        let spawn = bits(MapFeatures::Floor | MapFeatures::EnemySpawn);
        assert_eq!(count(map, spawn), 2, "{:?}", imported.report);
        assert!(map.tiles.iter().all(|row| row.first() != Some(&ground_bits(Ground::Floor))));
        // The pond and the hedge (aliens can't pass, players can).
        assert!(count(map, ground_bits(Ground::Water)) > 50);
        assert!(count(map, ground_bits(Ground::Grass) | bits(MapFeatures::ImpassableForEnemies)) > 10);
        // The road stays open through the hedge.
        let middle_row = &map.tiles[31];
        assert!(middle_row[1..81].iter().all(|t| enemy_passable(*t) || *t == spawn));
        assert_eq!(map.decorations.len(), 1);
    }

    #[test]
    fn the_output_is_a_valid_text_map() {
        let map = import_street(&spec()).map;
        let text = crate::map::text_format::to_text(&map).unwrap();
        let back = crate::map::text_format::parse(&text).unwrap();
        assert_eq!(back.tiles, map.tiles);
    }

    #[test]
    fn spawns_cut_off_from_the_goal_are_dropped() {
        // A wall right across the level, between the two ends of the road.
        let mut json = STREET.trim_end_matches("]}").to_string();
        json.push_str(
            r#",{"type":"way","id":7,"tags":{"barrier":"wall"},
                "geometry":[{"lat":58.999,"lon":18.0003},{"lat":59.001,"lon":18.0003}]}]}"#,
        );
        let imported = import(&spec(), &data::parse(&json).unwrap()).unwrap();
        // Walls don't cut roads, so both spawns still reach the goal ...
        let spawn = bits(MapFeatures::Floor | MapFeatures::EnemySpawn);
        assert_eq!(count(&imported.map, spawn), 2);
        // ... but with fences blocking everyone and no road, the far side is cut off.
        let mut s = spec();
        s.goal = Some((58.9997, 17.9990));
        let without_road: String = json.replace("\"highway\":\"residential\"", "\"highway\":\"proposed\"");
        let imported = import(&s, &data::parse(&without_road).unwrap()).unwrap();
        assert_eq!(count(&imported.map, spawn), 1, "{:?}", imported.report);
        assert!(imported.report.iter().any(|l| l.contains("farthest")));
    }

    #[test]
    fn size_defaults_to_the_buildings() {
        let mut s = spec();
        s.size = None;
        s.centre = None;
        let map = import_street(&s).map;
        // Houses span ~46 m east-west and ~11 m north-south, plus 6 m each side and the wall.
        let w = map.tiles.first().map_or(0, Vec::len);
        assert!((55..=65).contains(&w), "{w}");
        assert!((22..=28).contains(&map.tiles.len()), "{}", map.tiles.len());
    }

    #[test]
    fn too_big_is_an_error() {
        let mut s = spec();
        s.size = Some((5000.0, 100.0));
        assert!(import(&s, &data::parse(STREET).unwrap()).is_err());
    }

    #[test]
    fn the_example_spec_parses() {
        let text = std::fs::read_to_string("assets/gis/example.ron").unwrap();
        let spec: OsmLevelSpec = ron::from_str(&text).unwrap();
        assert_eq!(spec.source, "gis-raw/example.osm");
        assert_eq!(spec.loot.map(|l| l.per_tiles), Some(40));
    }

    #[test]
    fn perimeter_goes_round_once() {
        let p = perimeter(5, 4);
        assert_eq!(p.len(), 14);
        let mut unique = p.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 14);
    }
}
