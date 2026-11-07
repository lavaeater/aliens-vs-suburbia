//! Something to see where the map is impassable. `map_loader` gives those tiles
//! colliders; this plans their walls per chunk (`wall_mesh` builds the geometry, `streaming`
//! shows the chunks near the camera).
//!
//! Three looks, one per kind of blocking:
//!
//! - **Solid** (blocks everyone): opaque masonry with a capped top. A tile with storeys
//!   (`MapFile::storeys`, an imported building) stands that tall, under a roof.
//! - **Blocks players only**: a translucent green membrane, alien stuff the aliens walk
//!   through.
//! - **Blocks aliens only**: a low, translucent amber barrier players step over.
//!
//! A tile that already has a placed model (a house wall from the house tool, a fence) or a
//! decoration standing on it (a forest's pine) shows that instead.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::general::components::map_components::MapFile;
use crate::map::ground::GroundHeight;
use crate::map::streaming::{Chunk, chunk_of};
use crate::map::wall_mesh::{WallBase, WallCap, WallMesh, WallStyle, build_walls};
use crate::map::{BitFlags, MapFeatures};

/// The map's tiles with the one-tile border of void `map_loader` adds, so grid coordinates
/// here match the colliders and the floor (raw tile `(x, y)` is `(x + 1, y + 1)`).
pub fn padded_tiles(map: &MapFile) -> Vec<Vec<u64>> {
    let cols = map.tiles.iter().map(Vec::len).max().unwrap_or(0) + 2;
    let mut padded = vec![vec![0u64; cols]; map.tiles.len() + 2];
    for (out, row) in padded.iter_mut().skip(1).zip(&map.tiles) {
        for (cell, &raw) in out.iter_mut().skip(1).zip(row) {
            *cell = raw;
        }
    }
    padded
}

/// Void tiles walled in by the map, map coordinates `(x, y)`. Void only belongs at the edge
/// of the world: inside the map it shows as a hole into the background, so levels fill it
/// with walls instead. A void counts as the edge if void connects it to the outside.
#[cfg(test)]
pub fn enclosed_voids(map: &MapFile) -> Vec<(usize, usize)> {
    let padded = padded_tiles(map);
    let is_void = |x: usize, y: usize| padded.get(y).and_then(|row| row.get(x)) == Some(&0);
    let mut outside = HashSet::from([(0, 0)]);
    let mut todo = vec![(0usize, 0usize)];
    while let Some((x, y)) = todo.pop() {
        let around = [(x + 1, y), (x.wrapping_sub(1), y), (x, y + 1), (x, y.wrapping_sub(1))];
        for next in around {
            if is_void(next.0, next.1) && outside.insert(next) {
                todo.push(next);
            }
        }
    }
    let mut enclosed = Vec::new();
    for (y, row) in map.tiles.iter().enumerate() {
        for (x, &raw) in row.iter().enumerate() {
            if raw == 0 && !outside.contains(&(x + 1, y + 1)) {
                enclosed.push((x, y));
            }
        }
    }
    enclosed
}

/// How much taller each storey past the first makes a building (the first is a plain wall's
/// height). Compressed, not to scale: the game camera sits about 3.2 above the floor at its
/// default zoom, and what comes near that fills the screen.
pub const STOREY_RISE: f32 = 0.35;
/// The tallest a building gets, however many storeys it has.
pub const TALLEST_BUILDING: f32 = 2.3;

const ROOF_COLOR: [f32; 4] = [0.46, 0.3, 0.26, 1.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WallKind {
    Solid,
    BlocksPlayers,
    BlocksAliens,
}

impl WallKind {
    /// What kind of wall a tile is. Void is not a wall: it is the edge of the world, and
    /// the background plane shows there.
    pub fn of(raw: u64) -> Option<Self> {
        if raw == 0 {
            return None;
        }
        let f = BitFlags::<MapFeatures>::from_bits_truncate(raw);
        match (
            f.contains(MapFeatures::ImpassableForPlayers),
            f.contains(MapFeatures::ImpassableForEnemies),
        ) {
            (true, true) => Some(Self::Solid),
            (true, false) => Some(Self::BlocksPlayers),
            (false, true) => Some(Self::BlocksAliens),
            (false, false) => None,
        }
    }

    pub const fn style(self) -> WallStyle {
        match self {
            Self::Solid => WallStyle {
                height: 0.9,
                body_color: [0.58, 0.5, 0.44, 1.0],
                cap: Some(WallCap {
                    thickness: 0.08,
                    overhang: 0.04,
                    color: [0.78, 0.75, 0.69, 1.0],
                }),
                jitter: 0.08,
                foot_shade: 0.55,
            },
            Self::BlocksPlayers => WallStyle {
                height: 0.7,
                body_color: [0.4, 1.0, 0.55, 0.35],
                cap: None,
                jitter: 0.0,
                foot_shade: 1.0,
            },
            Self::BlocksAliens => WallStyle {
                height: 0.3,
                body_color: [1.0, 0.65, 0.15, 0.45],
                cap: None,
                jitter: 0.0,
                foot_shade: 1.0,
            },
        }
    }

    /// This kind's look at so many storeys. Only solid walls have storeys.
    pub fn style_at(self, storeys: u8) -> WallStyle {
        let style = self.style();
        match style.cap {
            Some(cap) if storeys > 0 => WallStyle {
                height: (style.height + f32::from(storeys - 1) * STOREY_RISE).min(TALLEST_BUILDING),
                cap: Some(WallCap {
                    color: ROOF_COLOR,
                    ..cap
                }),
                ..style
            },
            _ => style,
        }
    }

    pub fn material(self) -> StandardMaterial {
        match self {
            Self::Solid => StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.92,
                ..default()
            },
            Self::BlocksPlayers | Self::BlocksAliens => {
                let [r, g, b, _] = self.style().body_color;
                StandardMaterial {
                    base_color: Color::WHITE,
                    emissive: LinearRgba::rgb(r, g, b) * 0.6,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                }
            }
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Solid => "Walls",
            Self::BlocksPlayers => "Walls (block players)",
            Self::BlocksAliens => "Walls (block aliens)",
        }
    }
}

/// Where wall tiles stand on the ground. `building` says which tiles are part of a building:
/// joined ones stand level, on the highest ground under them, or roofs would run downhill.
/// Other walls follow the ground.
fn wall_bases(
    walls: impl IntoIterator<Item = (i32, i32)>,
    building: impl Fn((i32, i32)) -> bool,
    tile: f32,
    floor_y: f32,
    ground: &GroundHeight,
) -> HashMap<(i32, i32), WallBase> {
    let span = |(col, row): (i32, i32)| ground.span(tile * col as f32, tile * row as f32);
    let mut bases: HashMap<(i32, i32), WallBase> = walls
        .into_iter()
        .map(|at| {
            let (low, _) = span(at);
            let centre = ground.height(tile * at.0 as f32, tile * at.1 as f32);
            let base = WallBase {
                floor: floor_y + centre,
                foot: floor_y + low,
            };
            (at, base)
        })
        .collect();

    let mut seen = HashSet::new();
    let starts: Vec<(i32, i32)> = bases.keys().copied().filter(|at| building(*at)).collect();
    for start in starts {
        if !seen.insert(start) {
            continue;
        }
        let mut block = vec![start];
        let mut next = 0;
        while let Some(&(col, row)) = block.get(next) {
            next += 1;
            for at in [
                (col + 1, row),
                (col - 1, row),
                (col, row + 1),
                (col, row - 1),
            ] {
                if bases.contains_key(&at) && building(at) && seen.insert(at) {
                    block.push(at);
                }
            }
        }
        let highest = block
            .iter()
            .map(|at| span(*at).1)
            .fold(f32::NEG_INFINITY, f32::max);
        for at in block {
            if let Some(base) = bases.get_mut(&at) {
                base.floor = floor_y + highest;
            }
        }
    }
    bases
}

/// A map's walls, one mesh per [`Chunk`], kind and height. A side on a chunk's edge is judged
/// against the real neighbour, so the seams between chunks don't show.
pub fn plan_walls(
    map: &MapFile,
    tiles: &[Vec<u64>],
    tile: f32,
    floor_y: f32,
    ground: &GroundHeight,
) -> Vec<(Chunk, WallKind, WallMesh)> {
    let kind_at = |(col, row): (i32, i32)| -> Option<WallKind> {
        let raw = *tiles
            .get(usize::try_from(row).ok()?)?
            .get(usize::try_from(col).ok()?)?;
        WallKind::of(raw)
    };
    // `tiles` is padded and `MapFile::storeys` isn't.
    let storeys_at = |(col, row): (i32, i32)| -> u8 {
        usize::try_from(row - 1)
            .ok()
            .zip(usize::try_from(col - 1).ok())
            .and_then(|(row, col)| map.storeys.get(row)?.get(col).copied())
            .unwrap_or(0)
    };
    let height_at = |at: (i32, i32)| -> Option<f32> {
        kind_at(at).map(|kind| kind.style_at(storeys_at(at)).height)
    };
    let modelled: HashSet<(i32, i32)> = map
        .placements
        .iter()
        .map(|p| (p.x, p.y))
        .chain(map.decorations.iter().map(|d| (d.x, d.y)))
        .map(|(x, y)| (x + 1, y + 1))
        .collect();

    let mut cells: HashMap<(Chunk, WallKind, u8), HashSet<(i32, i32)>> = HashMap::new();
    for (row, cols) in tiles.iter().enumerate() {
        for col in 0..cols.len() {
            let at = (col as i32, row as i32);
            if let Some(kind) = kind_at(at)
                && !modelled.contains(&at)
            {
                let storeys = if kind == WallKind::Solid {
                    storeys_at(at)
                } else {
                    0
                };
                cells
                    .entry((chunk_of(at), kind, storeys))
                    .or_default()
                    .insert(at);
            }
        }
    }

    let all_walls = tiles
        .iter()
        .enumerate()
        .flat_map(|(row, cols)| (0..cols.len()).map(move |col| (col as i32, row as i32)));
    let bases = wall_bases(
        all_walls.filter(|at| kind_at(*at).is_some()),
        |at| kind_at(at) == Some(WallKind::Solid) && storeys_at(at) > 0,
        tile,
        floor_y,
        ground,
    );
    let base = |at| bases.get(&at).copied().unwrap_or(WallBase::flat(floor_y));
    let top_at = |at| height_at(at).map(|height| base(at).floor + height);

    cells
        .into_iter()
        .filter_map(|((chunk, kind, storeys), cells)| {
            let style = kind.style_at(storeys);
            // A side is hidden by a neighbour reaching at least as high, of any kind.
            let hides = |from, to| top_at(from).zip(top_at(to)).is_some_and(|(a, b)| b >= a);
            let built = build_walls(&cells, hides, tile, base, &style);
            (!built.is_empty()).then_some((chunk, kind, built))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::streaming::CHUNK_TILES;

    const CHUNK_SIZE: usize = CHUNK_TILES as usize;

    fn solid() -> u64 {
        MapFeatures::Floor as u64
            | MapFeatures::ImpassableForPlayers as u64
            | MapFeatures::ImpassableForEnemies as u64
    }

    #[test]
    fn padding_adds_a_void_border_and_shifts_by_one() {
        let map = MapFile {
            tiles: vec![vec![1, 2], vec![3, 4]],
            ..default()
        };
        let p = padded_tiles(&map);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], vec![0; 4]);
        assert_eq!(p[1], vec![0, 1, 2, 0]);
        assert_eq!(p[2][2], 4);
    }

    #[test]
    fn only_void_cut_off_from_the_edge_is_enclosed() {
        // A hole in the middle, a notch open to the edge, and a short row.
        let map = MapFile {
            tiles: vec![
                vec![9, 9, 9, 9, 9],
                vec![9, 0, 9, 9, 0],
                vec![9, 9, 9, 0, 0],
                vec![9, 9, 9],
            ],
            ..default()
        };
        assert_eq!(enclosed_voids(&map), vec![(1, 1)]);
    }

    #[test]
    fn wall_kinds_follow_the_impassable_flags() {
        let floor = MapFeatures::Floor as u64;
        let players = MapFeatures::ImpassableForPlayers as u64;
        let aliens = MapFeatures::ImpassableForEnemies as u64;
        assert_eq!(WallKind::of(0), None, "void is not a wall");
        assert_eq!(WallKind::of(floor), None);
        assert_eq!(
            WallKind::of(floor | players | aliens),
            Some(WallKind::Solid)
        );
        assert_eq!(WallKind::of(floor | players), Some(WallKind::BlocksPlayers));
        assert_eq!(WallKind::of(floor | aliens), Some(WallKind::BlocksAliens));
    }

    #[test]
    fn a_building_stands_level_and_other_walls_follow_the_ground() {
        let ground = GroundHeight::new(
            &MapFile {
                elevation: vec![vec![0, 100, 200, 300, 400]; 5],
                ..default()
            },
            1.0,
        );
        let walls = [(2, 2), (3, 2), (4, 2), (7, 2)];
        let bases = wall_bases(walls, |(col, _)| col < 5, 1.0, 0.0, &ground);
        let (west, east) = (bases[&(2, 2)], bases[&(4, 2)]);
        assert_eq!(west.floor, east.floor, "one height for the block");
        assert!(
            west.floor > ground.height(4.5, 2.0) - 1e-6,
            "on its highest ground"
        );
        assert!(west.foot < east.foot, "each side reaches its own ground");
        assert_eq!(bases[&(7, 2)].floor, ground.height(7.0, 2.0));
    }

    /// A straight wall across a chunk border must come out as it would in one piece.
    #[test]
    fn a_wall_across_a_chunk_border_has_no_seam() {
        let floor = MapFeatures::Floor as u64;
        let wall = floor
            | MapFeatures::ImpassableForPlayers as u64
            | MapFeatures::ImpassableForEnemies as u64;
        let tiles = vec![vec![wall; CHUNK_SIZE + 1]];

        let planned = plan_walls(
            &MapFile::default(),
            &tiles,
            1.0,
            0.0,
            &GroundHeight::default(),
        );
        assert_eq!(planned.len(), 2, "one mesh on each side of the border");

        let run = 0..=CHUNK_TILES;
        let cells = run.clone().map(|col| (col, 0)).collect();
        let hides = |_, (col, row): (i32, i32)| row == 0 && run.contains(&col);
        let base = |_| WallBase::flat(0.0);
        let whole = build_walls(&cells, hides, 1.0, base, &WallKind::Solid.style());
        let indices: usize = planned.iter().map(|(_, _, mesh)| mesh.indices.len()).sum();
        assert_eq!(indices, whole.indices.len());
    }

    fn top_of(mesh: &WallMesh) -> f32 {
        mesh.positions
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
    }

    /// Two tiles side by side in the map, the second with `storeys`, padded.
    fn pair(first: u64, second: u64, storeys: u8) -> (MapFile, Vec<Vec<u64>>) {
        let map = MapFile {
            tiles: vec![vec![first, second]],
            storeys: vec![vec![0, storeys]],
            ..default()
        };
        let tiles = padded_tiles(&map);
        (map, tiles)
    }

    #[test]
    fn a_tile_with_storeys_stands_that_tall() {
        let wall = solid();
        let (map, tiles) = pair(wall, wall, 3);
        let planned = plan_walls(&map, &tiles, 1.0, 0.0, &GroundHeight::default());
        assert_eq!(planned.len(), 2, "one mesh per height");
        let mut tops: Vec<f32> = planned.iter().map(|(_, _, mesh)| top_of(mesh)).collect();
        tops.sort_by(f32::total_cmp);
        assert!((tops[0] - WallKind::Solid.style().height).abs() < 1e-4);
        let three_storeys = WallKind::Solid.style().height + 2.0 * STOREY_RISE;
        assert!((tops[1] - three_storeys).abs() < 1e-4);
    }

    #[test]
    fn the_taller_side_shows_above_a_lower_neighbour() {
        let wall = solid();
        let (map, tiles) = pair(wall, wall, 2);
        let planned = plan_walls(&map, &tiles, 1.0, 0.0, &GroundHeight::default());
        let side_faces_at_x = |x: f32| -> usize {
            planned
                .iter()
                .map(|(_, _, mesh)| {
                    mesh.positions
                        .chunks(4)
                        .filter(|q| q.iter().all(|p| (p[0] - x).abs() < 1e-4))
                        .count()
                })
                .sum()
        };
        // Tiles are centred on their column; the padded ones at 1 and 2 meet at x = 1.5.
        assert_eq!(side_faces_at_x(1.5), 1, "the tall tile's side, once");
    }

    #[test]
    fn no_building_outgrows_the_cap() {
        assert!((WallKind::Solid.style_at(9).height - TALLEST_BUILDING).abs() < 1e-4);
    }

    #[test]
    fn only_solid_walls_take_storeys() {
        let floor = MapFeatures::Floor as u64;
        let fence = floor | MapFeatures::ImpassableForEnemies as u64;
        let (map, tiles) = pair(fence, fence, 4);
        let planned = plan_walls(&map, &tiles, 1.0, 0.0, &GroundHeight::default());
        assert_eq!(planned.len(), 1, "both tiles are the same low fence");
        assert!(top_of(&planned[0].2) < 1.0);
    }

    #[test]
    fn a_tree_on_a_blocked_tile_stands_in_for_the_wall() {
        let map = MapFile {
            tiles: vec![vec![solid()]],
            decorations: vec![crate::general::components::map_components::DecorationItem {
                x: 0,
                y: 0,
                model: String::new(),
                rotation_y: 0.0,
                scale: 1.0,
            }],
            ..default()
        };
        assert!(
            plan_walls(
                &map,
                &padded_tiles(&map),
                1.0,
                0.0,
                &GroundHeight::default()
            )
            .is_empty()
        );
    }

    #[test]
    fn a_tile_with_a_placed_model_gets_no_wall() {
        let wall = MapFeatures::Floor as u64
            | MapFeatures::ImpassableForPlayers as u64
            | MapFeatures::ImpassableForEnemies as u64;
        let map = MapFile {
            tiles: vec![vec![wall]],
            placements: vec![crate::general::components::map_components::TilePlacement {
                x: 0,
                y: 0,
                ..default()
            }],
            ..default()
        };
        assert!(
            plan_walls(
                &map,
                &padded_tiles(&map),
                1.0,
                0.0,
                &GroundHeight::default()
            )
            .is_empty()
        );
    }
}
