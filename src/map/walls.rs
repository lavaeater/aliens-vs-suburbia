//! Something to see where the map is impassable. `map_loader` gives those tiles
//! colliders; this plans their walls per chunk (`wall_mesh` builds the geometry, `streaming`
//! shows the chunks near the camera).
//!
//! Three looks, one per kind of blocking:
//!
//! - **Solid** (blocks everyone): opaque masonry with a capped top.
//! - **Blocks players only**: a translucent green membrane, alien stuff the aliens walk
//!   through.
//! - **Blocks aliens only**: a low, translucent amber barrier players step over.
//!
//! A tile that already has a placed model (a house wall from the house tool, a fence)
//! shows that model instead.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::general::components::map_components::MapFile;
use crate::map::streaming::{Chunk, chunk_of};
use crate::map::wall_mesh::{WallCap, WallMesh, WallStyle, build_walls};
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

/// A map's walls, one mesh per [`Chunk`] and kind. A side on a chunk's edge is judged against
/// the real neighbour, so the seams between chunks don't show.
pub fn plan_walls(
    map: &MapFile,
    tiles: &[Vec<u64>],
    tile: f32,
    floor_y: f32,
) -> Vec<(Chunk, WallKind, WallMesh)> {
    let kind_at = |(col, row): (i32, i32)| -> Option<WallKind> {
        let raw = *tiles
            .get(usize::try_from(row).ok()?)?
            .get(usize::try_from(col).ok()?)?;
        WallKind::of(raw)
    };
    let modelled: HashSet<(i32, i32)> = map.placements.iter().map(|p| (p.x + 1, p.y + 1)).collect();

    let mut cells: HashMap<(Chunk, WallKind), HashSet<(i32, i32)>> = HashMap::new();
    for (row, cols) in tiles.iter().enumerate() {
        for col in 0..cols.len() {
            let at = (col as i32, row as i32);
            if let Some(kind) = kind_at(at)
                && !modelled.contains(&at)
            {
                cells.entry((chunk_of(at), kind)).or_default().insert(at);
            }
        }
    }

    cells
        .into_iter()
        .filter_map(|((chunk, kind), cells)| {
            let style = kind.style();
            // A side is hidden by a neighbour at least as tall: the same kind, or a taller
            // wall of another kind.
            let hides = |c| {
                kind_at(c)
                    .is_some_and(|other| other == kind || other.style().height >= style.height)
            };
            let built = build_walls(&cells, hides, tile, floor_y, &style);
            (!built.is_empty()).then_some((chunk, kind, built))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::streaming::CHUNK_TILES;

    const CHUNK_SIZE: usize = CHUNK_TILES as usize;

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

    /// A straight wall across a chunk border must come out as it would in one piece.
    #[test]
    fn a_wall_across_a_chunk_border_has_no_seam() {
        let floor = MapFeatures::Floor as u64;
        let wall = floor
            | MapFeatures::ImpassableForPlayers as u64
            | MapFeatures::ImpassableForEnemies as u64;
        let tiles = vec![vec![wall; CHUNK_SIZE + 1]];

        let planned = plan_walls(&MapFile::default(), &tiles, 1.0, 0.0);
        assert_eq!(planned.len(), 2, "one mesh on each side of the border");

        let run = 0..=CHUNK_TILES;
        let cells = run.clone().map(|col| (col, 0)).collect();
        let hides = |(col, row): (i32, i32)| row == 0 && run.contains(&col);
        let whole = build_walls(&cells, hides, 1.0, 0.0, &WallKind::Solid.style());
        let indices: usize = planned.iter().map(|(_, _, mesh)| mesh.indices.len()).sum();
        assert_eq!(indices, whole.indices.len());
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
        assert!(plan_walls(&map, &padded_tiles(&map), 1.0, 0.0).is_empty());
    }
}
