//! Polygon-based house generation — the first step of the plan in
//! `docs/procedural-maps.md`. Pure geometry + rules, no Bevy dependency, so it can be
//! unit-tested standalone and driven either by the map editor's "Rectangle House" brush
//! or, later, a full polygon canvas.
//!
//! A `Polygon` (in tile-grid coordinates) is resolved against a `HouseSpec` into a ring
//! of wall/door/window cells plus an interior floor — `resolve_house` never touches
//! `MapFile` directly, `apply_house_to_map` does that translation so the pure resolver
//! stays trivially testable.

use crate::general::components::map_components::{MapFile, TilePlacement};
use crate::map::MapFeatures;

/// A closed polygon in tile-grid coordinates. `points` are corner coordinates in
/// continuous grid space (e.g. `(0, 0)` to `(6, 4)` describes a 6x4-tile rectangle);
/// cell `(x, y)` is considered "inside" using its center `(x + 0.5, y + 0.5)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    pub points: Vec<(i32, i32)>,
}

impl Polygon {
    pub fn rectangle(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        Self { points: vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)] }
    }

    /// Inclusive integer bounding box of the polygon's cells: `(min_x, min_y, max_x, max_y)`.
    pub fn bounds(&self) -> Option<(i32, i32, i32, i32)> {
        if self.points.is_empty() { return None; }
        let min_x = self.points.iter().map(|p| p.0).min().unwrap();
        let max_x = self.points.iter().map(|p| p.0).max().unwrap();
        let min_y = self.points.iter().map(|p| p.1).min().unwrap();
        let max_y = self.points.iter().map(|p| p.1).max().unwrap();
        Some((min_x, min_y, max_x.saturating_sub(1), max_y.saturating_sub(1)))
    }

    /// Even-odd point-in-polygon test against a cell's center.
    pub fn contains_cell(&self, x: i32, y: i32) -> bool {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let n = self.points.len();
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = (self.points[i].0 as f32, self.points[i].1 as f32);
            let (xj, yj) = (self.points[j].0 as f32, self.points[j].1 as f32);
            if (yi > py) != (yj > py) {
                let x_intersect = xi + (py - yi) / (yj - yi) * (xj - xi);
                if px < x_intersect { inside = !inside; }
            }
            j = i;
        }
        inside
    }
}

/// Governs how many windows a wall run gets, per the doc's
/// "1 window per 4 meters, max" rule of thumb.
#[derive(Debug, Clone)]
pub struct WindowRule {
    /// A run shorter than this (in tiles) never gets a window.
    pub min_len_for_window: f32,
    /// Upper bound on window count: `floor(run_len / max_per_wall_len)`.
    pub max_per_wall_len: f32,
}

impl Default for WindowRule {
    fn default() -> Self {
        Self { min_len_for_window: 2.0, max_per_wall_len: 4.0 }
    }
}

#[derive(Debug, Clone)]
pub struct HouseSpec {
    /// Def path for plain wall segments.
    pub wall_def: String,
    /// Def path for door segments. `None` = doors are just a passable gap (no model).
    pub door_def: Option<String>,
    /// Def path for window segments. `None` = windows fall back to `wall_def`.
    pub window_def: Option<String>,
    /// Total doors across the whole house (all placed on its longest wall run).
    pub door_count: u32,
    pub window_rule: WindowRule,
}

impl HouseSpec {
    pub fn new(wall_def: impl Into<String>) -> Self {
        Self {
            wall_def: wall_def.into(),
            door_def: None,
            window_def: None,
            door_count: 1,
            window_rule: WindowRule::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallCellKind { Wall, Door, Window }

/// Cardinal direction the wall cell faces (which side is exterior). Maps to the
/// existing 45-degree `rotation_steps` convention used by `TilePlacement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing { North, East, South, West }

impl Facing {
    pub fn rotation_steps(self) -> u8 {
        match self {
            Facing::North => 0,
            Facing::East => 2,
            Facing::South => 4,
            Facing::West => 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WallCell {
    pub x: i32,
    pub y: i32,
    pub kind: WallCellKind,
    pub facing: Facing,
}

#[derive(Debug, Clone, Default)]
pub struct HouseResolveResult {
    /// Interior cells that are not part of the wall ring — plain walkable floor.
    pub floor_cells: Vec<(i32, i32)>,
    /// The wall ring: every boundary cell, classified as wall/door/window.
    pub wall_cells: Vec<WallCell>,
}

impl HouseResolveResult {
    pub fn door_count(&self) -> usize {
        self.wall_cells.iter().filter(|c| c.kind == WallCellKind::Door).count()
    }
    pub fn window_count(&self) -> usize {
        self.wall_cells.iter().filter(|c| c.kind == WallCellKind::Window).count()
    }
    pub fn wall_count(&self) -> usize {
        self.wall_cells.iter().filter(|c| c.kind == WallCellKind::Wall).count()
    }
}

struct Run {
    facing: Facing,
    cells: Vec<(i32, i32)>,
}

/// Groups the boundary cells into straight runs per facing, so door/window counts can
/// be computed per wall side rather than around the whole ring at once.
fn group_runs(boundary: &[(i32, i32, Facing)]) -> Vec<Run> {
    let mut runs = Vec::new();

    // North/South runs are contiguous along x for a fixed y; East/West along y for a fixed x.
    for facing in [Facing::North, Facing::South] {
        let mut cells: Vec<(i32, i32)> = boundary.iter()
            .filter(|(_, _, f)| *f == facing)
            .map(|(x, y, _)| (*x, *y))
            .collect();
        cells.sort_by_key(|(x, y)| (*y, *x));
        let mut i = 0;
        while i < cells.len() {
            let mut j = i + 1;
            while j < cells.len() && cells[j].1 == cells[i].1 && cells[j].0 == cells[j - 1].0 + 1 { j += 1; }
            runs.push(Run { facing, cells: cells[i..j].to_vec() });
            i = j;
        }
    }
    for facing in [Facing::East, Facing::West] {
        let mut cells: Vec<(i32, i32)> = boundary.iter()
            .filter(|(_, _, f)| *f == facing)
            .map(|(x, y, _)| (*x, *y))
            .collect();
        cells.sort_by_key(|(x, y)| (*x, *y));
        let mut i = 0;
        while i < cells.len() {
            let mut j = i + 1;
            while j < cells.len() && cells[j].0 == cells[i].0 && cells[j].1 == cells[j - 1].1 + 1 { j += 1; }
            runs.push(Run { facing, cells: cells[i..j].to_vec() });
            i = j;
        }
    }
    runs
}

/// Evenly spaces `count` feature slots inside a run of `len` cells, preferring the
/// interior of the run over its corner-adjacent ends.
fn evenly_spaced_indices(len: usize, count: usize) -> Vec<usize> {
    if count == 0 || len == 0 { return Vec::new(); }
    let step = len as f32 / (count + 1) as f32;
    let mut out: Vec<usize> = (1..=count)
        .map(|i| ((i as f32 * step).round() as usize).min(len - 1))
        .collect();
    out.dedup();
    out
}

/// Resolves a closed `Polygon` + `HouseSpec` into wall-ring classification and interior
/// floor, without touching `MapFile`/Bevy. Pure and deterministic.
pub fn resolve_house(polygon: &Polygon, spec: &HouseSpec) -> HouseResolveResult {
    let mut result = HouseResolveResult::default();
    let Some((min_x, min_y, max_x, max_y)) = polygon.bounds() else { return result };

    let inside = |x: i32, y: i32| -> bool {
        if x < min_x || y < min_y || x > max_x || y > max_y { false }
        else { polygon.contains_cell(x, y) }
    };

    let mut boundary: Vec<(i32, i32, Facing)> = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if !inside(x, y) { continue; }
            // Priority order N, E, S, W: a corner cell is exterior on two sides but
            // gets a single facing/placement, matching the doc's "simplest case" tone.
            let facing = if !inside(x, y - 1) { Some(Facing::North) }
                else if !inside(x + 1, y) { Some(Facing::East) }
                else if !inside(x, y + 1) { Some(Facing::South) }
                else if !inside(x - 1, y) { Some(Facing::West) }
                else { None };

            match facing {
                Some(f) => boundary.push((x, y, f)),
                None => result.floor_cells.push((x, y)),
            }
        }
    }

    let runs = group_runs(&boundary);
    let longest_run_idx = runs.iter().enumerate()
        .max_by_key(|(_, r)| r.cells.len())
        .map(|(i, _)| i);

    for (idx, run) in runs.iter().enumerate() {
        let len = run.cells.len();
        let capacity = if (len as f32) < spec.window_rule.min_len_for_window { 0 }
            else { (len as f32 / spec.window_rule.max_per_wall_len).floor() as usize };

        let doors_here = if Some(idx) == longest_run_idx {
            (spec.door_count as usize).min(capacity)
        } else { 0 };
        let windows_here = capacity - doors_here;

        let feature_slots = evenly_spaced_indices(len, doors_here + windows_here);
        for (slot_i, &cell_idx) in feature_slots.iter().enumerate() {
            let (x, y) = run.cells[cell_idx];
            let kind = if slot_i < doors_here { WallCellKind::Door } else { WallCellKind::Window };
            result.wall_cells.push(WallCell { x, y, kind, facing: run.facing });
        }
        let feature_cells: std::collections::HashSet<usize> = feature_slots.into_iter().collect();
        for (i, &(x, y)) in run.cells.iter().enumerate() {
            if feature_cells.contains(&i) { continue; }
            result.wall_cells.push(WallCell { x, y, kind: WallCellKind::Wall, facing: run.facing });
        }
    }

    result
}

/// Stamps a resolved house into a raw tile grid + placement list, replacing whatever was
/// there before at the affected cells. Shared by `apply_house_to_map` (whole `MapFile`)
/// and the map editor, which keeps its grid as loose `tiles`/`placements` fields.
pub fn apply_house(
    result: &HouseResolveResult,
    spec: &HouseSpec,
    width: usize,
    height: usize,
    tiles: &mut [Vec<u64>],
    placements: &mut Vec<TilePlacement>,
) {
    const TILE_FLOOR: u64 = MapFeatures::Floor as u64;
    const TILE_WALL_ALL: u64 = MapFeatures::Floor as u64
        | MapFeatures::ImpassableForPlayers as u64
        | MapFeatures::ImpassableForEnemies as u64;

    let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height;

    for &(x, y) in &result.floor_cells {
        if !in_bounds(x, y) { continue; }
        tiles[y as usize][x as usize] = TILE_FLOOR;
        placements.retain(|p| !(p.x == x && p.y == y));
    }

    for cell in &result.wall_cells {
        if !in_bounds(cell.x, cell.y) { continue; }
        placements.retain(|p| !(p.x == cell.x && p.y == cell.y));

        let def_path = match cell.kind {
            WallCellKind::Wall => Some(spec.wall_def.clone()),
            WallCellKind::Window => Some(spec.window_def.clone().unwrap_or_else(|| spec.wall_def.clone())),
            WallCellKind::Door => spec.door_def.clone(),
        };

        tiles[cell.y as usize][cell.x as usize] = match cell.kind {
            WallCellKind::Door => TILE_FLOOR, // passable gap
            _ => TILE_WALL_ALL,
        };

        if let Some(def_path) = def_path {
            placements.push(TilePlacement {
                x: cell.x,
                y: cell.y,
                def_path,
                rotation_steps: cell.facing.rotation_steps(),
            });
        }
    }
}

/// Convenience wrapper of [`apply_house`] for a whole `MapFile`.
pub fn apply_house_to_map(result: &HouseResolveResult, spec: &HouseSpec, map: &mut MapFile) {
    apply_house(result, spec, map.map_width, map.map_height, &mut map.tiles, &mut map.placements);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> HouseSpec {
        HouseSpec::new("assets/defs/Brick Wall-GhSmk5IA5i.ron")
    }

    #[test]
    fn rectangle_interior_and_ring_counts() {
        let poly = Polygon::rectangle(0, 0, 6, 4);
        let result = resolve_house(&poly, &spec());

        // 6x4 rectangle: 24 cells total, ring is the 16-cell perimeter, 8 interior.
        assert_eq!(result.floor_cells.len(), 8);
        assert_eq!(result.wall_cells.len(), 16);
    }

    #[test]
    fn one_door_on_longest_wall_one_window_elsewhere() {
        let poly = Polygon::rectangle(0, 0, 6, 4);
        let result = resolve_house(&poly, &spec());

        assert_eq!(result.door_count(), 1, "exactly the requested door count");
        assert_eq!(result.window_count(), 1, "short 2-tile sides are below min_len_for_window");
        assert_eq!(result.wall_count(), 14);

        let door = result.wall_cells.iter().find(|c| c.kind == WallCellKind::Door).unwrap();
        assert_eq!(door.facing, Facing::North, "door goes on the longer of the two tied-longest runs, found first");
    }

    #[test]
    fn short_walls_get_no_windows() {
        let poly = Polygon::rectangle(0, 0, 2, 2);
        let mut s = spec();
        s.door_count = 0;
        let result = resolve_house(&poly, &s);
        // Every cell in a 2x2 house is boundary; each run is only 2 tiles, at the
        // min_len_for_window threshold but with max_per_wall_len=4 the capacity floors to 0.
        assert_eq!(result.floor_cells.len(), 0);
        assert_eq!(result.window_count(), 0);
        assert_eq!(result.wall_count(), result.wall_cells.len());
    }

    /// The resolver is written per-cell against `Polygon::contains_cell`, never assuming
    /// convexity or a fixed edge count, so an L-shaped house should "just work" — this
    /// cross-checks that against a brute-force containment scan rather than hand-computed
    /// magic numbers, which would be fragile and easy to get wrong by hand for a concave
    /// shape.
    #[test]
    fn l_shape_boundary_matches_bruteforce_containment() {
        // A 6x4 block with a 2x3 notch hanging off the bottom-right (4..6, 4..7).
        let poly = Polygon { points: vec![(0, 0), (6, 0), (6, 7), (4, 7), (4, 4), (0, 4)] };
        let mut s = spec();
        s.door_count = 1;
        let result = resolve_house(&poly, &s);

        let (min_x, min_y, max_x, max_y) = poly.bounds().unwrap();
        let mut total_inside = 0;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if poly.contains_cell(x, y) { total_inside += 1; }
            }
        }
        assert_eq!(result.floor_cells.len() + result.wall_cells.len(), total_inside);

        for cell in &result.wall_cells {
            let neighbors_outside = [
                (cell.x - 1, cell.y), (cell.x + 1, cell.y), (cell.x, cell.y - 1), (cell.x, cell.y + 1),
            ].iter().filter(|&&(nx, ny)| !poly.contains_cell(nx, ny)).count();
            assert!(neighbors_outside >= 1, "wall cell ({}, {}) should border the outside", cell.x, cell.y);
        }
        for &(x, y) in &result.floor_cells {
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                assert!(poly.contains_cell(nx, ny), "floor cell ({x}, {y}) should be fully interior");
            }
        }

        assert!(result.door_count() >= 1);
        assert!(result.window_count() >= 1, "a shape this size should get at least one window somewhere");
    }

    #[test]
    fn apply_to_map_marks_door_as_passable() {
        let poly = Polygon::rectangle(0, 0, 6, 4);
        let mut s = spec();
        s.door_def = Some("assets/defs/door.ron".to_string());
        let result = resolve_house(&poly, &s);

        let mut map = MapFile {
            map_width: 10,
            map_height: 10,
            tiles: vec![vec![0u64; 10]; 10],
            ..Default::default()
        };
        apply_house_to_map(&result, &s, &mut map);

        let door = result.wall_cells.iter().find(|c| c.kind == WallCellKind::Door).unwrap();
        let tile = map.tiles[door.y as usize][door.x as usize];
        assert_eq!(tile & (MapFeatures::ImpassableForPlayers as u64), 0, "door tile must stay passable");
        assert!(map.placements.iter().any(|p| p.x == door.x && p.y == door.y && p.def_path == "assets/defs/door.ron"));

        let wall = result.wall_cells.iter().find(|c| c.kind == WallCellKind::Wall).unwrap();
        let wall_tile = map.tiles[wall.y as usize][wall.x as usize];
        assert_ne!(wall_tile & (MapFeatures::ImpassableForPlayers as u64), 0, "plain wall tile blocks players");
    }
}
