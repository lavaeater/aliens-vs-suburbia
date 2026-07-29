//! The chunk stitcher: lay prefab [`MapChunk`]s on a coarse grid into a full `MapFile`.
//!
//! Design (see `docs/ultraviolence.md` "Cool maps"): uniform fixed chunks, edge-type
//! connector matching, and a **forced road spine** from the west (alien spawn) edge to
//! the east (goal) edge so a spawn→goal route always exists — no generate-and-retry.
//!
//! The built-in library is arranged so matching always succeeds: every chunk is `Open`
//! on its north/south edges, spine pieces are `Road` east/west, and off-spine pieces are
//! `Open` east/west. So horizontal neighbours in the spine row are Road↔Road, everything
//! else is Open↔Open, and the whole map stays connected while the spine guarantees the
//! critical corridor. Variety comes from which off-spine chunk (and rotation) fills each
//! slot.

use crate::general::components::map_components::MapFile;
use crate::map::chunks::{EdgeType, MapChunk, Side, CHUNK_SIZE};
use crate::map::MapFeatures;
use enumflags2::BitFlags;

// ── Seeded RNG (xorshift64, matching map_generator) ──────────────────────────

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_add(1).wrapping_mul(0x9e3779b97f4a7c15))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() as usize % (hi - lo))
    }
    fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        &slice[self.range(0, slice.len())]
    }
}

fn f(flags: impl Into<BitFlags<MapFeatures>>) -> u64 {
    flags.into().bits()
}

// ── Built-in chunk library ───────────────────────────────────────────────────

/// Chunks that carry the east-west road spine (Road on E+W, Open on N+S).
fn spine_chunks() -> Vec<MapChunk> {
    vec![
        // Open road corridor.
        MapChunk::from_ascii(
            "road_straight",
            &[
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
            ],
            [EdgeType::Open, EdgeType::Road, EdgeType::Open, EdgeType::Road],
        ),
        // Road flanked by cover blocks (buildings pinch the street).
        MapChunk::from_ascii(
            "road_cover",
            &[
                ".##...#",
                ".##...#",
                ".......",
                ".......",
                ".......",
                "#...##.",
                "#...##.",
            ],
            [EdgeType::Open, EdgeType::Road, EdgeType::Open, EdgeType::Road],
        ),
    ]
}

/// Off-spine chunks — `Open` on all four edges, so they mate any neighbour.
fn filler_chunks() -> Vec<MapChunk> {
    vec![
        // Empty lot.
        MapChunk::from_ascii(
            "lot_open",
            &[
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
                ".......",
            ],
            [EdgeType::Open; 4],
        ),
        // A hollow building (walls inside, passable border).
        MapChunk::from_ascii(
            "building",
            &[
                ".......",
                ".#####.",
                ".#...#.",
                ".#...#.",
                ".#...#.",
                ".#####.",
                ".......",
            ],
            [EdgeType::Open; 4],
        ),
        // Scattered rubble cover.
        MapChunk::from_ascii(
            "rubble",
            &[
                ".......",
                "..#..#.",
                ".......",
                "...#...",
                ".#..#..",
                ".......",
                "..#....",
            ],
            [EdgeType::Open; 4],
        ),
    ]
}

// ── Stitcher ─────────────────────────────────────────────────────────────────

/// The chunk grid produced before flattening — exposed for testing the layout logic.
struct Layout {
    /// `chunks_high` rows × `chunks_wide` cols of placed chunks.
    grid: Vec<Vec<MapChunk>>,
    /// Which chunk-row carries the road spine.
    spine_row: usize,
}

/// Choose and place chunks on the coarse grid, honouring connector matching against the
/// already-placed West and North neighbours. The spine row is laid first with road
/// pieces; other slots get a matching filler.
fn build_layout(seed: u64, chunks_wide: usize, chunks_high: usize) -> Layout {
    let mut rng = Rng::new(seed);
    let spine_row = if chunks_high <= 2 { chunks_high / 2 } else { rng.range(1, chunks_high - 1) };

    let spine = spine_chunks();
    let fillers = filler_chunks();

    // Placeholder grid; filled row by row, col by col.
    let mut grid: Vec<Vec<Option<MapChunk>>> = vec![vec![None; chunks_wide]; chunks_high];

    for cr in 0..chunks_high {
        for cc in 0..chunks_wide {
            let west = if cc > 0 { grid[cr][cc - 1].as_ref() } else { None };
            let north = if cr > 0 { grid[cr - 1][cc].as_ref() } else { None };

            // Candidate pool: spine row uses road pieces, everything else fillers.
            let pool = if cr == spine_row { &spine } else { &fillers };

            let chosen = pick_matching(&mut rng, pool, west, north)
                // Fillers are all-Open so this never trips in practice; fall back safely.
                .unwrap_or_else(|| fillers[0].clone());
            grid[cr][cc] = Some(chosen);
        }
    }

    let grid = grid
        .into_iter()
        .map(|row| row.into_iter().map(|c| c.unwrap()).collect())
        .collect();
    Layout { grid, spine_row }
}

/// Pick a chunk (trying rotations) whose West edge matches the east edge of `west` and
/// whose North edge matches the south edge of `north`. Returns `None` if nothing fits.
fn pick_matching(
    rng: &mut Rng,
    pool: &[MapChunk],
    west: Option<&MapChunk>,
    north: Option<&MapChunk>,
) -> Option<MapChunk> {
    // Off-spine chunks may rotate; spine chunks keep their orientation (rot 0 only) so
    // the road stays east-west. Detect "spine" by a Road edge.
    let mut candidates: Vec<MapChunk> = Vec::new();
    for base in pool {
        let is_spine = Side::ALL.iter().any(|&s| base.edge(s) == EdgeType::Road);
        let turns: &[u32] = if is_spine { &[0] } else { &[0, 1, 2, 3] };
        for &t in turns {
            let c = base.rotated(t);
            let ok_w = west.is_none_or(|w| w.edge(Side::East).mates_with(c.edge(Side::West)));
            let ok_n = north.is_none_or(|n| n.edge(Side::South).mates_with(c.edge(Side::North)));
            if ok_w && ok_n {
                candidates.push(c);
            }
        }
    }
    if candidates.is_empty() {
        None
    } else {
        Some(rng.pick(&candidates).clone())
    }
}

/// Stitch a full `MapFile` from prefab chunks. `chunks_wide`/`chunks_high` are in chunks
/// (so the tile map is `chunks_wide*CHUNK_SIZE` × `chunks_high*CHUNK_SIZE`). Always yields
/// a spawn→goal route via the road spine.
pub fn stitch_map(seed: u64, chunks_wide: usize, chunks_high: usize) -> MapFile {
    let cw = chunks_wide.max(2);
    let ch = chunks_high.max(1);
    let layout = build_layout(seed, cw, ch);

    let w = cw * CHUNK_SIZE;
    let h = ch * CHUNK_SIZE;

    // Flatten the chunk grid into one tile grid.
    let mut tiles: Vec<Vec<u64>> = vec![vec![0u64; w]; h];
    for (cr, row) in layout.grid.iter().enumerate() {
        for (cc, chunk) in row.iter().enumerate() {
            for (r, chunk_row) in chunk.tiles.iter().enumerate() {
                for (c, &tile) in chunk_row.iter().enumerate() {
                    tiles[cr * CHUNK_SIZE + r][cc * CHUNK_SIZE + c] = tile;
                }
            }
        }
    }

    // Impassable outer border (except where we punch the spawn/goal below).
    let border = f(MapFeatures::Floor
        | MapFeatures::ImpassableForPlayers
        | MapFeatures::ImpassableForEnemies);
    for c in 0..w {
        tiles[0][c] = border;
        tiles[h - 1][c] = border;
    }
    for row in tiles.iter_mut() {
        row[0] = border;
        row[w - 1] = border;
    }

    // Spawn / goal / player, centred on the spine band.
    let spine_mid = layout.spine_row * CHUNK_SIZE + CHUNK_SIZE / 2;
    tiles[spine_mid][0] = f(MapFeatures::Floor | MapFeatures::EnemySpawn);
    tiles[spine_mid][w - 1] = f(MapFeatures::Floor | MapFeatures::EnemyExit);
    // Player a couple of tiles in from the alien entrance.
    let player_col = (CHUNK_SIZE / 2).min(w - 2);
    tiles[spine_mid][player_col] = f(MapFeatures::Floor | MapFeatures::PlayerSpawn);

    MapFile {
        generated: false,
        seed,
        map_width: w,
        map_height: h,
        tiles,
        decorations: vec![],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile is passable for an enemy if it isn't void and doesn't carry the
    /// enemy-impassable flag (matching how the game reads tiles).
    fn enemy_passable(bits: u64) -> bool {
        if bits == 0 {
            return false;
        }
        let flags = BitFlags::<MapFeatures>::from_bits_truncate(bits);
        flags.contains(MapFeatures::Floor) && !flags.contains(MapFeatures::ImpassableForEnemies)
    }

    fn find(tiles: &[Vec<u64>], flag: MapFeatures) -> Option<(usize, usize)> {
        for (r, row) in tiles.iter().enumerate() {
            for (c, &bits) in row.iter().enumerate() {
                if BitFlags::<MapFeatures>::from_bits_truncate(bits).contains(flag) {
                    return Some((r, c));
                }
            }
        }
        None
    }

    /// BFS over enemy-passable tiles.
    fn connected(tiles: &[Vec<u64>], from: (usize, usize), to: (usize, usize)) -> bool {
        let h = tiles.len();
        let w = tiles[0].len();
        let mut seen = vec![vec![false; w]; h];
        let mut stack = vec![from];
        seen[from.0][from.1] = true;
        while let Some((r, c)) = stack.pop() {
            if (r, c) == to {
                return true;
            }
            for (dr, dc) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nr, nc) = (r as i32 + dr, c as i32 + dc);
                if nr < 0 || nc < 0 || nr as usize >= h || nc as usize >= w {
                    continue;
                }
                let (nr, nc) = (nr as usize, nc as usize);
                if !seen[nr][nc] && enemy_passable(tiles[nr][nc]) {
                    seen[nr][nc] = true;
                    stack.push((nr, nc));
                }
            }
        }
        false
    }

    #[test]
    fn dimensions_are_chunks_times_chunk_size() {
        let map = stitch_map(1, 4, 3);
        assert_eq!(map.map_width, 4 * CHUNK_SIZE);
        assert_eq!(map.map_height, 3 * CHUNK_SIZE);
        assert_eq!(map.tiles.len(), 3 * CHUNK_SIZE);
        assert_eq!(map.tiles[0].len(), 4 * CHUNK_SIZE);
    }

    #[test]
    fn spawn_goal_and_player_are_all_present() {
        let map = stitch_map(7, 5, 3);
        assert!(find(&map.tiles, MapFeatures::EnemySpawn).is_some(), "has an alien spawn");
        assert!(find(&map.tiles, MapFeatures::EnemyExit).is_some(), "has a goal");
        assert!(find(&map.tiles, MapFeatures::PlayerSpawn).is_some(), "has a player spawn");
    }

    #[test]
    fn there_is_always_a_spawn_to_goal_route() {
        // The whole point of the forced spine: never generate an unwinnable map.
        for seed in 0..40u64 {
            let map = stitch_map(seed, 5, 3);
            let spawn = find(&map.tiles, MapFeatures::EnemySpawn).unwrap();
            let goal = find(&map.tiles, MapFeatures::EnemyExit).unwrap();
            assert!(
                connected(&map.tiles, spawn, goal),
                "seed {seed}: aliens must be able to reach the goal"
            );
        }
    }

    #[test]
    fn generation_is_deterministic_for_a_seed() {
        let a = stitch_map(123, 4, 3);
        let b = stitch_map(123, 4, 3);
        assert_eq!(a.tiles, b.tiles, "same seed -> same map");

        let c = stitch_map(124, 4, 3);
        assert_ne!(a.tiles, c.tiles, "different seed -> different map");
    }

    #[test]
    fn tiny_requests_are_clamped_to_a_valid_size() {
        // 0 chunks would be a degenerate map; we clamp up.
        let map = stitch_map(1, 0, 0);
        assert!(map.map_width >= CHUNK_SIZE);
        assert!(map.map_height >= CHUNK_SIZE);
        assert!(find(&map.tiles, MapFeatures::EnemySpawn).is_some());
    }
}
