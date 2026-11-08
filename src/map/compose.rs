//! Levels on a base map: one imported suburb, cut into several levels.
//!
//! A base map (`campaigns/gis/world/world.map`) is the place: terrain, buildings, roads,
//! hills, trees. A level that names it (`base:` in its `.map`) holds only what is its own:
//! the `region` of the base it is played on, its [`Marker`]s (player starts, alien spawns,
//! the goal, in base map tiles), and its rules, waves, stories and placements.
//!
//! [`compose_level`] builds the map the game plays from the two: an ordinary [`MapFile`],
//! so nothing after the loader (`text_format::load_level`) knows the difference.
//! [`split_level`] goes the other way, turning a whole level (an OpenStreetMap import)
//! into a base and a level that composes back to it.
// Grid code: indices come from the grid's own ranges or are checked with `get`.
#![allow(clippy::indexing_slicing)]

use crate::general::components::map_components::{
    ELEVATION_STEP, MapFile, Marker, MarkerKind, Region, RoadLine,
};
use crate::map::osm::geometry::clip_polyline;
use crate::map::{BitFlags, MapFeatures};

/// The smallest region a level can have (it needs an inside past its walls).
pub const MIN_SIZE: i32 = 5;

fn wall_bits() -> u64 {
    (MapFeatures::Floor | MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies)
        .bits()
}

fn marker_flag(kind: MarkerKind) -> MapFeatures {
    match kind {
        MarkerKind::Player => MapFeatures::PlayerSpawn,
        MarkerKind::Spawn => MapFeatures::EnemySpawn,
        MarkerKind::Goal => MapFeatures::EnemyExit,
    }
}

fn marker_flags() -> BitFlags<MapFeatures> {
    MapFeatures::PlayerSpawn | MapFeatures::EnemySpawn | MapFeatures::EnemyExit
}

fn terrain_flags() -> BitFlags<MapFeatures> {
    MapFeatures::Water
        | MapFeatures::Grass
        | MapFeatures::Floor
        | MapFeatures::Mud
        | MapFeatures::Snow
        | MapFeatures::Rock
}

/// A tile with a marker on it: its ground (floor if it has none, as on a wall), open to
/// whoever the marker is for, and the marker's flag.
pub fn stamp(tile: u64, kind: MarkerKind) -> u64 {
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(tile);
    let mut ground = flags & terrain_flags();
    if ground.is_empty() {
        ground = MapFeatures::Floor.into();
    }
    (ground | marker_flag(kind)).bits()
}

/// The markers already in a map's tiles, in its own coordinates.
pub fn markers_in(tiles: &[Vec<u64>]) -> Vec<Marker> {
    let mut markers = Vec::new();
    for (y, row) in tiles.iter().enumerate() {
        for (x, &tile) in row.iter().enumerate() {
            let flags = BitFlags::<MapFeatures>::from_bits_truncate(tile);
            for kind in MarkerKind::ALL {
                if flags.contains(marker_flag(kind)) {
                    markers.push(Marker {
                        kind,
                        x: x as i32,
                        y: y as i32,
                    });
                }
            }
        }
    }
    markers
}

/// The whole of a map as a region.
pub fn whole(map: &MapFile) -> Region {
    Region::new(
        0,
        0,
        map.tiles.first().map_or(0, Vec::len) as i32,
        map.tiles.len() as i32,
    )
}

/// The region a level on `base` is played on, as composing uses it: its origin moved
/// down to an even tile (the elevation lattice has a point every [`ELEVATION_STEP`]
/// tiles), its far edges kept, clipped to the base.
pub fn effective_region(base: &MapFile, level: &MapFile) -> Result<Region, String> {
    let full = whole(base);
    let asked = level.region.unwrap_or(full);
    let step = ELEVATION_STEP as i32;
    let (right, bottom) = (asked.x + asked.width, asked.y + asked.height);
    let x = asked.x.max(0) / step * step;
    let y = asked.y.max(0) / step * step;
    let region = Region::new(x, y, right.min(full.width) - x, bottom.min(full.height) - y);
    if region.width < MIN_SIZE || region.height < MIN_SIZE {
        return Err(format!(
            "region {},{} {}x{} is too small, or outside the base ({}x{}); at least {MIN_SIZE}x{MIN_SIZE}",
            asked.x, asked.y, asked.width, asked.height, full.width, full.height
        ));
    }
    Ok(region)
}

/// The map the game plays for `level` (which names `base`): `base` cut to the level's
/// region, walled round, its own spawns, goal and player start cleared and the level's
/// markers stamped in, with the level's rules, waves, stories and placements.
///
/// Errors when the base is itself a level on a base, generated, or the region or a marker
/// is outside it.
pub fn compose_level(base: &MapFile, level: &MapFile) -> Result<MapFile, String> {
    if !base.base.is_empty() {
        return Err("the base map is itself a level on a base map".into());
    }
    if base.generated {
        return Err("the base map is generated; a base needs tiles".into());
    }
    let region = effective_region(base, level)?;
    let (x0, y0) = (region.x as usize, region.y as usize);
    let (width, height) = (region.width as usize, region.height as usize);

    let crop = |rows: &[Vec<u64>]| -> Vec<Vec<u64>> {
        rows.iter()
            .skip(y0)
            .take(height)
            .map(|row| {
                let mut row: Vec<u64> = row.iter().skip(x0).take(width).copied().collect();
                row.resize(width, 0);
                row
            })
            .collect()
    };
    let mut tiles = crop(&base.tiles);
    for row in &mut tiles {
        for tile in row.iter_mut() {
            *tile = (BitFlags::<MapFeatures>::from_bits_truncate(*tile) & !marker_flags()).bits();
        }
    }
    let local = Region::new(0, 0, region.width, region.height);
    for (row, cols) in tiles.iter_mut().enumerate() {
        for (col, tile) in cols.iter_mut().enumerate() {
            if local.on_edge(col as i32, row as i32) {
                *tile = wall_bits();
            }
        }
    }

    let mut storeys: Vec<Vec<u8>> = if base.storeys.is_empty() {
        Vec::new()
    } else {
        (0..height)
            .map(|row| {
                (0..width)
                    .map(|col| {
                        base.storeys
                            .get(y0 + row)
                            .and_then(|r| r.get(x0 + col))
                            .copied()
                            .unwrap_or(0)
                    })
                    .collect()
            })
            .collect()
    };

    for marker in &level.markers {
        if !region.contains(marker.x, marker.y) {
            return Err(format!(
                "{} marker at {}, {} is outside the region ({}, {} {}x{})",
                marker.kind.name(),
                marker.x,
                marker.y,
                region.x,
                region.y,
                region.width,
                region.height
            ));
        }
        let (col, row) = ((marker.x - region.x) as usize, (marker.y - region.y) as usize);
        tiles[row][col] = stamp(tiles[row][col], marker.kind);
        if let Some(s) = storeys.get_mut(row).and_then(|r| r.get_mut(col)) {
            *s = 0;
        }
    }

    let inside = |x: i32, y: i32| region.contains(x, y) && !region.on_edge(x, y);
    let (dx, dy) = (region.x, region.y);
    let decorations = base
        .decorations
        .iter()
        .chain(&level.decorations)
        .filter(|d| inside(d.x, d.y))
        .map(|d| {
            let mut d = d.clone();
            d.x -= dx;
            d.y -= dy;
            d
        })
        .collect();
    let placements = base
        .placements
        .iter()
        .chain(&level.placements)
        .filter(|p| inside(p.x, p.y))
        .map(|p| {
            let mut p = p.clone();
            p.x -= dx;
            p.y -= dy;
            p
        })
        .collect();
    let houses = base
        .houses
        .iter()
        .chain(&level.houses)
        .filter(|h| inside(h.x, h.y))
        .map(|h| {
            let mut h = h.clone();
            h.x -= dx;
            h.y -= dy;
            h
        })
        .collect();

    Ok(MapFile {
        name: if level.name.is_empty() {
            base.name.clone()
        } else {
            level.name.clone()
        },
        map_width: width,
        map_height: height,
        tiles,
        storeys,
        elevation: crop_elevation(&base.elevation, region),
        decorations,
        placements,
        roads: crop_roads(&base.roads, region),
        houses,
        waves: level.waves.clone(),
        stories: level.stories.clone(),
        loot: level.loot.clone(),
        escape_limit: level.escape_limit,
        weapons: level.weapons.clone(),
        coins: level.coins,
        infestation: level.infestation.clone(),
        ..MapFile::default()
    })
}

/// The elevation lattice under `region` (whose origin is on the lattice), lowered so its
/// lowest point is 0 again.
fn crop_elevation(elevation: &[Vec<u16>], region: Region) -> Vec<Vec<u16>> {
    if elevation.is_empty() {
        return Vec::new();
    }
    let step = ELEVATION_STEP;
    let (i0, j0) = (region.x as usize / step, region.y as usize / step);
    // Enough points to cover the far tiles (as the importer writes it: one past).
    let (wide, high) = (
        region.width as usize / step + 1,
        region.height as usize / step + 1,
    );
    let mut lattice: Vec<Vec<u16>> = elevation
        .iter()
        .skip(j0)
        .take(high)
        .map(|row| row.iter().skip(i0).take(wide).copied().collect())
        .collect();
    let lowest = lattice.iter().flatten().copied().min().unwrap_or(0);
    for h in lattice.iter_mut().flatten() {
        *h -= lowest;
    }
    lattice
}

/// The roads, moved to the region's corner and clipped to it.
fn crop_roads(roads: &[RoadLine], region: Region) -> Vec<RoadLine> {
    let (dx, dy) = (f64::from(region.x), f64::from(region.y));
    // Map coordinates put a tile's centre on its index, so the tiles span -0.5..w-0.5.
    let lo = (-0.5, -0.5);
    let hi = (f64::from(region.width) - 0.5, f64::from(region.height) - 0.5);
    let mut out = Vec::new();
    for road in roads {
        let line: Vec<(f64, f64)> = road
            .points
            .iter()
            .map(|&[x, y]| (f64::from(x) - dx, f64::from(y) - dy))
            .collect();
        for piece in clip_polyline(&line, lo, hi) {
            let round = |v: f64| (v * 100.0).round() as f32 / 100.0;
            out.push(RoadLine {
                kind: road.kind,
                width: road.width,
                points: piece.iter().map(|&(x, y)| [round(x), round(y)]).collect(),
            });
        }
    }
    out
}

/// Split a whole level into a base map and a level on it (`base_ref`, the base's content
/// path) that composes back to the same tiles: the level gets the markers, rules, waves,
/// stories and placements, the base the place. An alien spawn on the base's edge is
/// walled up again in the base.
pub fn split_level(map: &MapFile, base_ref: &str) -> (MapFile, MapFile) {
    let markers = markers_in(&map.tiles);
    let full = whole(map);
    let mut base = MapFile {
        name: map.name.clone(),
        map_width: map.map_width,
        map_height: map.map_height,
        tiles: map.tiles.clone(),
        storeys: map.storeys.clone(),
        elevation: map.elevation.clone(),
        decorations: map.decorations.clone(),
        roads: map.roads.clone(),
        houses: map.houses.clone(),
        ..MapFile::default()
    };
    for (row, cols) in base.tiles.iter_mut().enumerate() {
        for (col, tile) in cols.iter_mut().enumerate() {
            let flags = BitFlags::<MapFeatures>::from_bits_truncate(*tile);
            if flags.intersects(marker_flags()) {
                *tile = if full.on_edge(col as i32, row as i32) {
                    wall_bits()
                } else {
                    (flags & !marker_flags()).bits()
                };
            }
        }
    }
    let level = MapFile {
        name: map.name.clone(),
        base: base_ref.to_string(),
        region: Some(full),
        markers,
        placements: map.placements.clone(),
        waves: map.waves.clone(),
        stories: map.stories.clone(),
        loot: map.loot.clone(),
        escape_limit: map.escape_limit,
        weapons: map.weapons.clone(),
        coins: map.coins,
        infestation: map.infestation.clone(),
        map_width: 0,
        map_height: 0,
        ..MapFile::default()
    };
    (base, level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::{DecorationItem, RoadKind};

    fn bits(f: impl Into<BitFlags<MapFeatures>>) -> u64 {
        f.into().bits()
    }

    /// A 20 x 16 walled field of grass, a building at 8..10, 6..8, rising to the east.
    fn base() -> MapFile {
        let (w, h) = (20, 16);
        let full = Region::new(0, 0, w, h);
        let tiles = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        if full.on_edge(x, y) || ((8..10).contains(&x) && (6..8).contains(&y)) {
                            wall_bits()
                        } else {
                            bits(MapFeatures::Grass)
                        }
                    })
                    .collect()
            })
            .collect();
        MapFile {
            name: "Field".into(),
            tiles,
            storeys: (0..h)
                .map(|y| {
                    (0..w)
                        .map(|x| u8::from((8..10).contains(&x) && (6..8).contains(&y)) * 2)
                        .collect()
                })
                .collect(),
            elevation: (0..=h / 2)
                .map(|_| (0..=w / 2).map(|i| 100 + i as u16 * 10).collect())
                .collect(),
            decorations: vec![
                DecorationItem {
                    x: 5,
                    y: 5,
                    model: "tree.glb".into(),
                    rotation_y: 0.0,
                    scale: 1.0,
                },
                DecorationItem {
                    x: 1,
                    y: 1,
                    model: "tree.glb".into(),
                    rotation_y: 0.0,
                    scale: 1.0,
                },
            ],
            roads: vec![RoadLine {
                kind: RoadKind::Street,
                width: 2.0,
                points: vec![[0.0, 10.0], [19.0, 10.0]],
            }],
            ..MapFile::default()
        }
    }

    fn level(region: Region, markers: &[(MarkerKind, i32, i32)]) -> MapFile {
        MapFile {
            name: "Corner".into(),
            base: "campaigns/x/world/world.map".into(),
            region: Some(region),
            markers: markers
                .iter()
                .map(|&(kind, x, y)| Marker { kind, x, y })
                .collect(),
            escape_limit: Some(3),
            ..MapFile::default()
        }
    }

    #[test]
    fn a_level_is_its_region_walled_round_with_its_markers() {
        let level = level(
            Region::new(4, 4, 10, 10),
            &[
                (MarkerKind::Spawn, 4, 10),
                (MarkerKind::Goal, 8, 10),
                (MarkerKind::Player, 7, 11),
            ],
        );
        let map = compose_level(&base(), &level).unwrap();
        assert_eq!((map.tiles.len(), map.tiles[0].len()), (10, 10));
        assert_eq!((map.map_width, map.map_height), (10, 10));
        let local = Region::new(0, 0, 10, 10);
        for (y, row) in map.tiles.iter().enumerate() {
            for (x, &t) in row.iter().enumerate() {
                if local.on_edge(x as i32, y as i32) && (x, y) != (0, 6) {
                    assert_eq!(t, wall_bits(), "edge {x},{y}");
                }
            }
        }
        assert_eq!(map.tiles[6][0], bits(MapFeatures::Floor | MapFeatures::EnemySpawn));
        assert_eq!(map.tiles[6][4], bits(MapFeatures::Grass | MapFeatures::EnemyExit));
        assert_eq!(map.tiles[7][3], bits(MapFeatures::Grass | MapFeatures::PlayerSpawn));
        assert_eq!(map.tiles[2][4], wall_bits(), "the building at 8, 6");
        assert_eq!(map.escape_limit, Some(3));
        assert_eq!(map.name, "Corner");
        assert!(map.base.is_empty() && map.markers.is_empty() && map.region.is_none());
        assert!(crate::map::walls::enclosed_voids(&map).is_empty());
    }

    #[test]
    fn decorations_and_roads_move_with_the_region() {
        let map = compose_level(&base(), &level(Region::new(4, 4, 10, 10), &[])).unwrap();
        assert_eq!(map.decorations.len(), 1, "the one at 1, 1 is outside");
        assert_eq!((map.decorations[0].x, map.decorations[0].y), (1, 1));
        assert_eq!(map.roads.len(), 1);
        let xs: Vec<f32> = map.roads[0].points.iter().map(|p| p[0]).collect();
        assert_eq!(xs, vec![-0.5, 9.5], "clipped to the region's tiles");
        assert!(map.roads[0].points.iter().all(|p| (p[1] - 6.0).abs() < 1e-4));
    }

    #[test]
    fn an_odd_origin_moves_to_the_elevation_lattice() {
        let level = level(Region::new(5, 3, 9, 9), &[]);
        let region = effective_region(&base(), &level).unwrap();
        assert_eq!(region, Region::new(4, 2, 10, 10), "far edges kept");
        let map = compose_level(&base(), &level).unwrap();
        // Lattice point (2, 1) of the base, rebased: the base rises 10 a point eastward.
        assert_eq!(map.elevation[0][0], 0, "lowest is 0 again");
        assert_eq!(map.elevation[0][1], 10);
        assert_eq!(map.elevation.len(), 6);
        assert_eq!(map.elevation[0].len(), 6);
    }

    #[test]
    fn markers_and_regions_outside_are_errors() {
        let outside = level(Region::new(4, 4, 10, 10), &[(MarkerKind::Goal, 1, 1)]);
        assert!(compose_level(&base(), &outside).unwrap_err().contains("outside"));
        let tiny = level(Region::new(18, 14, 10, 10), &[]);
        assert!(compose_level(&base(), &tiny).unwrap_err().contains("too small"));
    }

    #[test]
    fn a_marker_on_a_building_opens_its_tile() {
        let map = compose_level(
            &base(),
            &level(Region::new(4, 4, 10, 10), &[(MarkerKind::Goal, 8, 6)]),
        )
        .unwrap();
        assert_eq!(map.tiles[2][4], bits(MapFeatures::Floor | MapFeatures::EnemyExit));
        assert_eq!(map.storeys[2][4], 0);
    }

    #[test]
    fn a_split_level_composes_back_to_itself() {
        let mut whole_level = base();
        whole_level.tiles[10][0] = bits(MapFeatures::Floor | MapFeatures::EnemySpawn);
        whole_level.tiles[10][10] = bits(MapFeatures::Grass | MapFeatures::EnemyExit);
        whole_level.tiles[11][10] = bits(MapFeatures::Grass | MapFeatures::PlayerSpawn);
        whole_level.escape_limit = Some(4);
        whole_level.decorations.retain(|d| d.x > 1);
        let (base, level) = split_level(&whole_level, "campaigns/x/world/world.map");
        assert_eq!(base.tiles[10][0], wall_bits(), "the base's edge closed again");
        assert_eq!(base.tiles[10][10], bits(MapFeatures::Grass));
        assert_eq!(level.markers.len(), 3);
        let again = compose_level(&base, &level).unwrap();
        assert_eq!(again.tiles, whole_level.tiles);
        assert_eq!(again.storeys, whole_level.storeys);
        assert_eq!(again.escape_limit, Some(4));
        assert_eq!(again.decorations.len(), whole_level.decorations.len());
        assert_eq!(again.roads, whole_level.roads);
        let rebased: Vec<Vec<u16>> = whole_level
            .elevation
            .iter()
            .map(|r| r.iter().map(|h| h - 100).collect())
            .collect();
        assert_eq!(again.elevation, rebased);
    }
}
