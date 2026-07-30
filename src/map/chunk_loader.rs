//! Load prefab chunks from `assets/maps/chunks/*.ron` into a stitcher library.
//!
//! A `.ron` chunk is a [`ChunkFile`] — an ASCII `rows` template plus four named edge
//! connectors (see `chunks.rs`). This module parses them, splits them into the spine
//! and filler pools the stitcher expects (a chunk with any `Road` edge is a spine
//! piece), and stitches. A missing/empty/broken chunk dir falls back to the built-in
//! library, so authored chunks *augment* the game rather than being able to break it.

use crate::general::components::map_components::MapFile;
use crate::map::chunks::{ChunkFile, EdgeType, MapChunk, Side};
use crate::map::stitch::stitch_map_with_library;

/// Parse one `.ron` chunk file's text into a [`MapChunk`], validating its shape.
pub fn parse_chunk_ron(text: &str) -> Result<MapChunk, String> {
    let file: ChunkFile = ron::from_str(text).map_err(|e| e.to_string())?;
    file.into_chunk()
}

/// A chunk is a spine piece iff it carries the road on any edge; otherwise it's filler.
fn is_spine(chunk: &MapChunk) -> bool {
    Side::ALL.iter().any(|&s| chunk.edge(s) == EdgeType::Road)
}

/// Read every `*.ron` under `dir`, returning `(spine_chunks, filler_chunks)`. Files that
/// fail to parse are skipped with a warning rather than aborting the load. Returns empty
/// vecs if the directory is absent — callers fall back to the built-in library.
pub fn load_chunk_library(dir: &str) -> (Vec<MapChunk>, Vec<MapChunk>) {
    let mut spine = Vec::new();
    let mut filler = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return (spine, filler);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        match parse_chunk_ron(&text) {
            Ok(chunk) => {
                if is_spine(&chunk) {
                    spine.push(chunk);
                } else {
                    filler.push(chunk);
                }
            }
            Err(e) => bevy::log::warn!("skipping chunk {}: {e}", path.display()),
        }
    }
    (spine, filler)
}

/// Stitch a map using chunks loaded from `dir`, falling back to the built-in library for
/// whichever pool the directory doesn't supply. This is the runtime entry the editor
/// uses, so hand-authored chunks show up without a rebuild.
pub fn stitch_map_from_dir(seed: u64, chunks_wide: usize, chunks_high: usize, dir: &str) -> MapFile {
    let (spine, filler) = load_chunk_library(dir);
    stitch_map_with_library(seed, chunks_wide, chunks_high, &spine, &filler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::chunks::CHUNK_SIZE;

    fn sample_ron(name: &str, road: bool) -> String {
        let rows: Vec<String> = (0..CHUNK_SIZE).map(|_| ".".repeat(CHUNK_SIZE)).collect();
        let ew = if road { "Road" } else { "Open" };
        let rows_ron: Vec<String> = rows.iter().map(|r| format!("\"{r}\"")).collect();
        format!(
            "(name: \"{name}\", rows: [{}], north: Open, east: {ew}, south: Open, west: {ew})",
            rows_ron.join(", ")
        )
    }

    #[test]
    fn parses_a_well_formed_chunk() {
        let chunk = parse_chunk_ron(&sample_ron("road", true)).expect("should parse");
        assert_eq!(chunk.name, "road");
        assert_eq!(chunk.edge(Side::East), EdgeType::Road);
        assert_eq!(chunk.tiles.len(), CHUNK_SIZE);
    }

    #[test]
    fn a_wrong_sized_grid_is_a_clean_error_not_a_panic() {
        // Only two rows — must be rejected.
        let bad = "(name: \"bad\", rows: [\".......\", \".......\"], north: Open, east: Open, south: Open, west: Open)";
        assert!(parse_chunk_ron(bad).is_err());
    }

    #[test]
    fn round_trips_through_ron() {
        let original = parse_chunk_ron(&sample_ron("rt", false)).unwrap();
        let file = ChunkFile::from_chunk(&original);
        let text = ron::to_string(&file).unwrap();
        let reparsed = parse_chunk_ron(&text).unwrap();
        assert_eq!(reparsed.tiles, original.tiles);
        assert_eq!(reparsed.edges, original.edges);
    }

    #[test]
    fn a_road_edge_classifies_as_spine_open_as_filler() {
        let road = parse_chunk_ron(&sample_ron("r", true)).unwrap();
        let open = parse_chunk_ron(&sample_ron("o", false)).unwrap();
        assert!(is_spine(&road));
        assert!(!is_spine(&open));
    }

    #[test]
    fn a_missing_directory_yields_an_empty_library() {
        let (spine, filler) = load_chunk_library("this/path/does/not/exist");
        assert!(spine.is_empty() && filler.is_empty());
    }

    #[test]
    fn stitch_from_a_missing_dir_still_produces_a_valid_map() {
        // Falls back to the built-in library, so a spawn/goal map still comes out.
        let map = stitch_map_from_dir(4, 5, 3, "this/path/does/not/exist");
        assert_eq!(map.map_width, 5 * CHUNK_SIZE);
        assert!(!map.decorations.is_empty());
    }

    /// Validates the hand-authored chunk files actually parse and stitch — catches a typo
    /// in a shipped `.ron` (wrong row width, bad edge name) at test time, not in-game.
    /// Runs from the crate root (where cargo test executes), so the relative path holds.
    #[test]
    fn the_shipped_chunk_assets_parse_and_stitch() {
        use crate::map::stitch::stitch_map_with_library;
        use crate::map::MapFeatures;
        use enumflags2::BitFlags;

        let (spine, filler) = load_chunk_library("assets/maps/chunks");
        assert!(!spine.is_empty(), "shipped chunks should include a road/spine piece");
        assert!(!filler.is_empty(), "shipped chunks should include fillers");

        let map = stitch_map_with_library(2, 5, 3, &spine, &filler);
        let has = |flag| {
            map.tiles.iter().flatten().any(|&bits| {
                BitFlags::<MapFeatures>::from_bits_truncate(bits).contains(flag)
            })
        };
        assert!(has(MapFeatures::EnemySpawn) && has(MapFeatures::EnemyExit));
    }
}
