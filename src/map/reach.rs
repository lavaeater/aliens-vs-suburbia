//! Who can walk where on a tile grid: the edge of a level, the steps from one tile to every
//! other, and the roads that leave a level (where its aliens come from).
//!
//! Shared by the OpenStreetMap importer (`map::osm`), levels on a base map
//! (`map::compose`) and the campaign editor.
// Grid code: indices come from the grid's own ranges or are checked with `get`.
#![allow(clippy::indexing_slicing)]

use std::collections::VecDeque;

use crate::general::components::map_components::{RoadKind, RoadLine};
use crate::map::{BitFlags, MapFeatures};

/// Whether aliens can walk on a tile.
pub fn enemy_passable(tile: u64) -> bool {
    tile != 0
        && !BitFlags::<MapFeatures>::from_bits_truncate(tile)
            .contains(MapFeatures::ImpassableForEnemies)
}

/// Whether players can walk on a tile.
pub fn player_passable(tile: u64) -> bool {
    tile != 0
        && !BitFlags::<MapFeatures>::from_bits_truncate(tile)
            .contains(MapFeatures::ImpassableForPlayers)
}

/// The edge tiles of a `width` x `height` grid, clockwise from the top-left corner.
pub fn perimeter(width: usize, height: usize) -> Vec<(usize, usize)> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut cells: Vec<(usize, usize)> = (0..width).map(|c| (c, 0)).collect();
    cells.extend((1..height).map(|r| (width - 1, r)));
    if height > 1 {
        cells.extend((0..width - 1).rev().map(|c| (c, height - 1)));
    }
    if width > 1 {
        cells.extend((1..height.saturating_sub(1)).rev().map(|r| (0, r)));
    }
    cells
}

/// The tile just inside an edge tile (none for corners).
pub fn inward(col: usize, row: usize, width: usize, height: usize) -> Option<(usize, usize)> {
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
pub fn distances(
    tiles: &[Vec<u64>],
    from: (usize, usize),
    passable: fn(u64) -> bool,
) -> Vec<Vec<Option<u32>>> {
    let height = tiles.len();
    let width = tiles.first().map_or(0, Vec::len);
    let mut dist = vec![vec![None; width]; height];
    if from.1 >= height || from.0 >= width {
        return dist;
    }
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

/// Runs of consecutive edge tiles that `is_exit` picks, longest first, a run that wraps
/// past the top-left corner counted as one.
pub fn edge_runs(
    width: usize,
    height: usize,
    is_exit: impl Fn(usize, usize) -> bool,
) -> Vec<Vec<(usize, usize)>> {
    let perimeter = perimeter(width, height);
    let mut runs: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut previous = false;
    for &(col, row) in &perimeter {
        let exit = is_exit(col, row);
        if exit {
            match runs.last_mut() {
                Some(run) if previous => run.push((col, row)),
                _ => runs.push(vec![(col, row)]),
            }
        }
        previous = exit;
    }
    if runs.len() > 1
        && perimeter.first().is_some_and(|&(c, r)| is_exit(c, r))
        && perimeter.last().is_some_and(|&(c, r)| is_exit(c, r))
        && let Some(last) = runs.pop()
        && let Some(first) = runs.first_mut()
    {
        first.splice(0..0, last);
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.len()));
    runs
}

/// Whether tile `(col, row)`'s centre is on a road's carriageway (paths do not count:
/// aliens come down streets). Coordinates are map tiles, like [`RoadLine::points`].
pub fn on_road(roads: &[RoadLine], col: usize, row: usize) -> bool {
    let p = (col as f32, row as f32);
    roads.iter().filter(|r| r.kind != RoadKind::Path).any(|road| {
        let half = (road.width / 2.0).max(0.5);
        road.points.windows(2).any(|pair| {
            let ([ax, ay], [bx, by]) = (pair[0], pair[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((p.0 - ax) * dx + (p.1 - ay) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (cx, cy) = (ax + t * dx - p.0, ay + t * dy - p.1);
            cx * cx + cy * cy <= half * half
        })
    })
}

/// Where aliens could come in: the middle of each road that crosses the edge of the map,
/// widest first, at most `max`. A tile just inside the edge has to be open to aliens, so
/// a road that runs along the edge or into a building does not count.
pub fn suggest_spawns(tiles: &[Vec<u64>], roads: &[RoadLine], max: usize) -> Vec<(usize, usize)> {
    let height = tiles.len();
    let width = tiles.first().map_or(0, Vec::len);
    if width < 3 || height < 3 {
        return Vec::new();
    }
    let open_inside = |col: usize, row: usize| {
        inward(col, row, width, height).is_some_and(|(c, r)| enemy_passable(tiles[r][c]))
    };
    edge_runs(width, height, |col, row| {
        on_road(roads, col, row) && open_inside(col, row)
    })
    .iter()
    .take(max)
    .filter_map(|run| run.get(run.len() / 2).copied())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perimeter_goes_round_once() {
        let cells = perimeter(4, 3);
        assert_eq!(cells.len(), 10);
        let unique: std::collections::BTreeSet<_> = cells.iter().collect();
        assert_eq!(unique.len(), cells.len());
    }

    #[test]
    fn a_road_across_the_edge_is_a_spawn_and_one_along_it_is_not() {
        let open = BitFlags::from(MapFeatures::Grass).bits();
        let tiles = vec![vec![open; 10]; 8];
        let across = RoadLine {
            kind: RoadKind::Street,
            width: 2.0,
            points: vec![[5.0, -3.0], [5.0, 4.0]],
        };
        let path = RoadLine {
            kind: RoadKind::Path,
            width: 2.0,
            points: vec![[-2.0, 6.0], [3.0, 6.0]],
        };
        let spawns = suggest_spawns(&tiles, &[across, path], 4);
        assert_eq!(spawns.len(), 1, "{spawns:?}");
        assert_eq!(spawns[0].1, 0, "on the top edge");
        assert!((4..=6).contains(&spawns[0].0));
    }
}
