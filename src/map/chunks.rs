//! Prefab map **chunks** — small hand-authored `MapFile` fragments that the generator
//! stitches together into a full level. See `docs/ultraviolence.md` "Feature 0: Cool
//! maps" for the design.
//!
//! A chunk is a fixed `CHUNK_SIZE`×`CHUNK_SIZE` block of tiles plus four **edge
//! connectors** describing what each side presents to its neighbour ([`EdgeType`]).
//! The stitcher (`src/map/stitch.rs`) lays chunks on a coarse grid, matching connectors
//! on touching edges, and guarantees a spawn→goal road spine.
//!
//! This module is just the data model + rotation + connector logic — pure and unit
//! tested. Chunks are authored in code here for now (a readable ASCII template);
//! loading them from `.ron` and stamping them in the editor are later stages.

use crate::map::MapFeatures;
use enumflags2::BitFlags;
use serde::{Deserialize, Serialize};

/// Edge length of a chunk, in tiles. Uniform fixed size keeps the stitcher simple
/// (every chunk mates any other on a shared coarse-grid edge).
pub const CHUNK_SIZE: usize = 7;

/// A side of a chunk. `North` is row 0, `South` the last row, `West` col 0, `East` the
/// last col (rows increase downward, matching the tile grid and `MapFile`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl Side {
    /// Used by the stitcher to iterate a chunk's neighbours.
    #[allow(dead_code)]
    pub const ALL: [Side; 4] = [Side::North, Side::East, Side::South, Side::West];

    /// The side an adjacent chunk on this side would present back to us.
    /// Part of the connector vocabulary; used by the tagging editor (later stage).
    #[allow(dead_code)]
    pub fn opposite(self) -> Side {
        match self {
            Side::North => Side::South,
            Side::East => Side::West,
            Side::South => Side::North,
            Side::West => Side::East,
        }
    }
}

/// What a chunk edge offers a neighbour. Simple edge-type matching (see `mates_with`):
/// two touching edges are compatible only if they present the same class.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EdgeType {
    /// A wall / blocked edge — nothing crosses here. Part of the authoring vocabulary
    /// (`.ron` chunks / the tagging editor); the built-in library is all Road/Open.
    Wall,
    /// The main passable spine — roads line up spawn→goal.
    Road,
    /// Passable open ground, but not a defined road.
    Open,
}

impl EdgeType {
    /// Two touching edges mate iff they present the same passability class.
    pub fn mates_with(self, other: EdgeType) -> bool {
        self == other
    }

    /// Whether a creature can cross this edge (road or open, not wall).
    /// Part of the connector vocabulary; used by the stitcher's validation (later stage).
    #[allow(dead_code)]
    pub fn is_passable(self) -> bool {
        !matches!(self, EdgeType::Wall)
    }
}

/// Flag bits for a plain floor tile.
fn floor() -> u64 {
    BitFlags::from(MapFeatures::Floor).bits()
}

/// Flag bits for an impassable wall tile.
fn wall() -> u64 {
    (MapFeatures::Floor | MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies)
        .bits()
}

/// A prefab chunk: a fixed-size tile block plus its four edge connectors.
#[derive(Clone, Debug)]
pub struct MapChunk {
    pub name: String,
    /// `CHUNK_SIZE` rows of `CHUNK_SIZE` tile-flag values (row-major, `u64` bits).
    pub tiles: Vec<Vec<u64>>,
    /// Connectors indexed by `Side as usize`.
    pub edges: [EdgeType; 4],
}

impl MapChunk {
    /// Build a chunk from an ASCII template — `CHUNK_SIZE` rows of `CHUNK_SIZE` chars:
    /// `'.'` = floor, `'#'` = wall, `' '`/`'x'` = void. Panics on the wrong shape, so
    /// malformed built-in chunks fail loudly at first use (and in tests).
    pub fn from_ascii(name: &str, rows: &[&str], edges: [EdgeType; 4]) -> Self {
        Self::try_from_ascii(name, rows.iter().map(|r| r.to_string()).collect(), edges)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// Fallible ASCII build (used by the `.ron` loader, where a bad file shouldn't crash
    /// the game). Returns a human-readable error on the wrong shape or an unknown char.
    pub fn try_from_ascii(name: &str, rows: Vec<String>, edges: [EdgeType; 4]) -> Result<Self, String> {
        if rows.len() != CHUNK_SIZE {
            return Err(format!("chunk '{name}': needs {CHUNK_SIZE} rows, got {}", rows.len()));
        }
        let mut tiles = Vec::with_capacity(CHUNK_SIZE);
        for row in &rows {
            if row.chars().count() != CHUNK_SIZE {
                return Err(format!("chunk '{name}': rows must be {CHUNK_SIZE} wide"));
            }
            let mut tile_row = Vec::with_capacity(CHUNK_SIZE);
            for c in row.chars() {
                tile_row.push(match c {
                    '.' => floor(),
                    '#' => wall(),
                    ' ' | 'x' => 0,
                    other => return Err(format!("chunk '{name}': unknown tile char '{other}'")),
                });
            }
            tiles.push(tile_row);
        }
        Ok(Self { name: name.to_string(), tiles, edges })
    }

    /// The connector on a given side.
    pub fn edge(&self, side: Side) -> EdgeType {
        self.edges[side as usize]
    }

    /// A copy rotated clockwise `quarter_turns` × 90°. Rotates both the tile grid and
    /// the connectors, so a rotated chunk still stitches correctly.
    pub fn rotated(&self, quarter_turns: u32) -> MapChunk {
        let mut out = self.clone();
        for _ in 0..(quarter_turns % 4) {
            out = out.rotate_cw();
        }
        out
    }

    /// One clockwise quarter turn.
    fn rotate_cw(&self) -> MapChunk {
        let n = CHUNK_SIZE;
        // new[r][c] = old[n-1-c][r]
        let tiles = (0..n)
            .map(|r| (0..n).map(|c| self.tiles[n - 1 - c][r]).collect())
            .collect();
        // Clockwise: the old West edge becomes the new North, etc.
        let e = &self.edges;
        let edges = [
            e[Side::West as usize],  // North <- West
            e[Side::North as usize], // East  <- North
            e[Side::East as usize],  // South <- East
            e[Side::South as usize], // West  <- South
        ];
        MapChunk { name: self.name.clone(), tiles, edges }
    }
}

/// The on-disk chunk format (`assets/maps/chunks/*.ron`). Authored by hand or by the
/// editor: an ASCII `rows` template (same chars as [`MapChunk::from_ascii`]) plus the
/// four named edge connectors. Kept separate from `MapChunk` so the runtime type stays
/// lean and the file stays human-friendly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkFile {
    pub name: String,
    /// `CHUNK_SIZE` strings of `CHUNK_SIZE` chars: `'.'` floor, `'#'` wall, `' '`/`'x'` void.
    pub rows: Vec<String>,
    pub north: EdgeType,
    pub east: EdgeType,
    pub south: EdgeType,
    pub west: EdgeType,
}

impl ChunkFile {
    /// Parse into a runtime [`MapChunk`], validating the grid shape.
    pub fn into_chunk(self) -> Result<MapChunk, String> {
        MapChunk::try_from_ascii(
            &self.name,
            self.rows,
            [self.north, self.east, self.south, self.west],
        )
    }

    /// Build the on-disk form from a runtime chunk (for the editor's save path / tests).
    #[allow(dead_code)]
    pub fn from_chunk(chunk: &MapChunk) -> Self {
        let rows = chunk
            .tiles
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&bits| {
                        if bits == 0 {
                            ' '
                        } else if BitFlags::<MapFeatures>::from_bits_truncate(bits)
                            .contains(MapFeatures::ImpassableForEnemies)
                        {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect();
        Self {
            name: chunk.name.clone(),
            rows,
            north: chunk.edge(Side::North),
            east: chunk.edge(Side::East),
            south: chunk.edge(Side::South),
            west: chunk.edge(Side::West),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_bits() -> u64 {
        wall()
    }

    /// A chunk with a distinct connector on every side, and a marker wall at (0,0) so we
    /// can track where the corner lands after rotation.
    fn asymmetric() -> MapChunk {
        MapChunk::from_ascii(
            "test",
            &[
                "#......",
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
            ],
            [EdgeType::Road, EdgeType::Open, EdgeType::Wall, EdgeType::Open],
        )
    }

    #[test]
    fn side_opposites_are_correct() {
        assert_eq!(Side::North.opposite(), Side::South);
        assert_eq!(Side::East.opposite(), Side::West);
    }

    #[test]
    fn edge_matching_is_same_class_only() {
        assert!(EdgeType::Road.mates_with(EdgeType::Road));
        assert!(!EdgeType::Road.mates_with(EdgeType::Open));
        assert!(!EdgeType::Wall.mates_with(EdgeType::Open));
        assert!(!EdgeType::Wall.is_passable());
        assert!(EdgeType::Road.is_passable() && EdgeType::Open.is_passable());
    }

    #[test]
    fn from_ascii_maps_chars_to_flags() {
        let c = asymmetric();
        assert_eq!(c.tiles[0][0], wall_bits(), "'#' is a wall");
        assert_eq!(c.tiles[0][1], floor(), "'.' is floor");
        assert_eq!(c.tiles.len(), CHUNK_SIZE);
    }

    #[test]
    fn one_rotation_cycles_the_connectors_clockwise() {
        let c = asymmetric().rotated(1);
        // North should now carry what West had (Open), East what North had (Road), ...
        assert_eq!(c.edge(Side::North), EdgeType::Open);
        assert_eq!(c.edge(Side::East), EdgeType::Road);
        assert_eq!(c.edge(Side::South), EdgeType::Open);
        assert_eq!(c.edge(Side::West), EdgeType::Wall);
    }

    #[test]
    fn the_marker_corner_moves_top_left_to_top_right_on_a_cw_turn() {
        let c = asymmetric().rotated(1);
        // old (0,0) -> new (0, n-1) under a clockwise turn.
        assert_eq!(c.tiles[0][CHUNK_SIZE - 1], wall_bits());
        assert_eq!(c.tiles[0][0], floor());
    }

    #[test]
    fn four_rotations_return_to_the_original() {
        let original = asymmetric();
        let full = original.rotated(4);
        assert_eq!(full.tiles, original.tiles);
        assert_eq!(full.edges, original.edges);
    }
}
