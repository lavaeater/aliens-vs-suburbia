//! The height of the ground anywhere on a map: [`MapFile::elevation`] read between its points.
//!
//! Hills are on unless `--flat` ([`Hills`]). With the flag, or on a map with no heights,
//! [`GroundHeight`] is flat at 0.

use crate::physics::prelude::Collider;
use bevy::math::{Quat, Vec3};
use bevy::prelude::Resource;

use crate::general::components::map_components::{ELEVATION_STEP, MapFile};

/// Present unless `--flat`: bends the world to `MapFile::elevation`.
#[derive(Resource)]
pub struct Hills;

/// The current map's ground, above `TileDefinitions::floor_level`, in world coordinates.
#[derive(Resource, Debug, Clone, Default)]
pub struct GroundHeight {
    /// World height at each lattice point. Empty = flat.
    points: Vec<Vec<f32>>,
    tile: f32,
    /// The lowest and highest of `points`.
    range: (f32, f32),
}

fn lowest_and_highest(points: &[Vec<f32>]) -> (f32, f32) {
    let mut all = points.iter().flatten().copied();
    let Some(first) = all.next() else {
        return (0.0, 0.0);
    };
    all.fold((first, first), |(lo, hi), h| (lo.min(h), hi.max(h)))
}

impl GroundHeight {
    /// A level's ground: its own with `hills` on, else flat.
    pub fn for_level(map: &MapFile, hills: bool, tile: f32) -> Self {
        if hills {
            Self::new(map, tile)
        } else {
            Self::default()
        }
    }

    pub fn new(map: &MapFile, tile: f32) -> Self {
        let points: Vec<Vec<f32>> = map
            .elevation
            .iter()
            .map(|row| row.iter().map(|cm| f32::from(*cm) / 100.0).collect())
            .collect();
        let range = lowest_and_highest(&points);
        Self {
            points,
            tile,
            range,
        }
    }

    /// The lowest and highest ground, `(0, 0)` when flat.
    pub const fn range(&self) -> (f32, f32) {
        self.range
    }

    fn point(&self, col: usize, row: usize) -> f32 {
        self.points
            .get(row)
            .and_then(|points| points.get(col))
            .copied()
            .unwrap_or(0.0)
    }

    /// The height at a point, between the four nearest lattice points. Past the edge of the
    /// lattice it carries on level.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        // Tile (0, 0) of the map is (1, 1) of the padded grid, whose centres are whole tiles.
        let step = ELEVATION_STEP as f32;
        let along = |world: f32, points: usize| {
            let last = points.saturating_sub(1);
            let at = ((world / self.tile - 1.0) / step).clamp(0.0, last as f32);
            (at as usize, (at as usize + 1).min(last), at.fract())
        };
        let columns = self.points.first().map_or(0, Vec::len);
        let (west, east, across) = along(x, columns);
        let (north, south, down) = along(z, self.points.len());
        let row = |row: usize| {
            let start = self.point(west, row);
            start + (self.point(east, row) - start) * across
        };
        let start = row(north);
        start + (row(south) - start) * down
    }

    /// The ground as a collider, for what falls on it, and where it goes, above the floor.
    /// `None` when flat.
    pub fn terrain(&self) -> Option<(Collider, Vec3)> {
        let columns = self.points.first().map_or(0, Vec::len);
        let rows = self.points.len();
        if columns < 2 || rows < 2 {
            return None;
        }
        // `Collider::heightfield` flattens this and hands it to parry, which takes it as
        // `rows` x `columns` stored column by column, whatever the shape it came in. So: that
        // order, cut into `rows` pieces. `the_terrain_collider_lies_where_the_ground_is`.
        let by_columns: Vec<f32> = (0..columns)
            .flat_map(|col| (0..rows).map(move |row| self.point(col, row)))
            .collect();
        let heights = by_columns.chunks(columns).map(<[f32]>::to_vec).collect();
        let step = ELEVATION_STEP as f32 * self.tile;
        let (width, depth) = ((columns - 1) as f32 * step, (rows - 1) as f32 * step);
        let collider = Collider::heightfield(heights, Vec3::new(width, 1.0, depth));
        let centre = Vec3::new(self.tile + width / 2.0, 0.0, self.tile + depth / 2.0);
        Some((collider, centre))
    }

    /// The way the ground faces at a point. Measured a tile either side, so that it turns
    /// smoothly over the lattice and hills shade softly.
    pub fn normal(&self, x: f32, z: f32) -> Vec3 {
        if self.points.is_empty() {
            return Vec3::Y;
        }
        let east = self.height(x + self.tile, z) - self.height(x - self.tile, z);
        let south = self.height(x, z + self.tile) - self.height(x, z - self.tile);
        Vec3::new(-east, 2.0 * self.tile, -south).normalize()
    }

    /// Where a level shot from `from` ends up at `point`, carried over the ground: as high
    /// above it as `from` is. Shots hit in level rays, against tall colliders, so this is
    /// where to draw them and what they hit.
    pub fn carried(&self, from: Vec3, point: Vec3) -> Vec3 {
        let rise = self.height(point.x, point.z) - self.height(from.x, from.z);
        point + Vec3::Y * rise
    }

    /// `dir` turned to run along the ground ahead of `from`, for what flies straight.
    pub fn along(&self, from: Vec3, dir: Vec3) -> Vec3 {
        const LOOK_AHEAD: f32 = 2.0;
        let flat = dir.with_y(0.0).normalize_or_zero();
        let reach = LOOK_AHEAD * self.tile;
        if self.points.is_empty() || flat == Vec3::ZERO {
            return dir;
        }
        let ahead = from + flat * reach;
        let rise = self.height(ahead.x, ahead.z) - self.height(from.x, from.z);
        (flat + Vec3::Y * rise / reach).normalize_or(dir)
    }

    /// The turn that lays something flat, a decal say, on the ground at a point.
    pub fn lie(&self, x: f32, z: f32) -> Quat {
        Quat::from_rotation_arc(Vec3::Y, self.normal(x, z))
    }

    /// The lowest and highest ground under a tile centred at `(x, z)`.
    pub fn span(&self, x: f32, z: f32) -> (f32, f32) {
        let half = self.tile / 2.0;
        let corners = [(-half, -half), (half, -half), (-half, half), (half, half)]
            .map(|(dx, dz)| self.height(x + dx, z + dz));
        let lowest = corners.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = corners.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        (lowest, highest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ground(elevation: Vec<Vec<u16>>) -> GroundHeight {
        GroundHeight::new(
            &MapFile {
                elevation,
                ..MapFile::default()
            },
            1.0,
        )
    }

    #[test]
    fn a_map_without_heights_is_flat() {
        let flat = ground(vec![]);
        assert_eq!(flat.height(40.0, 12.0), 0.0);
        assert_eq!(flat.normal(40.0, 12.0), Vec3::Y);
        assert_eq!(GroundHeight::default().height(1.0, 1.0), 0.0);
    }

    #[test]
    fn heights_land_on_their_tiles_and_blend_between() {
        let hill = ground(vec![vec![0, 200], vec![400, 600]]);
        // Lattice points sit on map tiles (0, 0), (2, 0) ..., padded and so world (1, 1), (3, 1).
        assert_eq!(hill.height(1.0, 1.0), 0.0);
        assert_eq!(hill.height(3.0, 1.0), 2.0);
        assert_eq!(hill.height(1.0, 3.0), 4.0);
        assert_eq!(hill.height(3.0, 3.0), 6.0);
        assert_eq!(hill.height(2.0, 2.0), 3.0);
    }

    #[test]
    fn past_the_edge_the_ground_carries_on_level() {
        let hill = ground(vec![vec![0, 200], vec![400, 600]]);
        assert_eq!(hill.height(0.0, 0.0), 0.0);
        assert_eq!(hill.height(9.0, 1.0), 2.0);
        assert_eq!(hill.height(9.0, 9.0), 6.0);
    }

    #[test]
    fn the_normal_leans_away_from_the_rise() {
        let slope = ground(vec![vec![0, 100, 200, 300, 400]; 5]);
        let normal = slope.normal(5.0, 5.0);
        assert!(normal.x < 0.0 && normal.y > 0.8, "{normal}");
        assert!(normal.z.abs() < 1e-6, "{normal}");
    }

    #[test]
    fn the_terrain_collider_lies_where_the_ground_is() {
        // Different slopes along the two axes, so a swapped grid can't pass.
        let elevation = (0..6)
            .map(|row| (0..4).map(|col| col * 30 + row * 110).collect())
            .collect();
        let hill = ground(elevation);
        let (collider, at) = hill.terrain().unwrap();
        for (x, z) in [(2.0, 2.0), (6.0, 3.0), (4.0, 9.0), (1.5, 10.5)] {
            let (toi, _) = collider
                .cast_ray(
                    at,
                    Quat::IDENTITY,
                    Vec3::new(x, 50.0, z),
                    Vec3::NEG_Y,
                    100.0,
                    true,
                )
                .expect("the ray hits the terrain");
            let height = 50.0 - toi;
            let wanted = hill.height(x, z);
            assert!(
                (height - wanted).abs() < 0.02,
                "({x}, {z}): {height}, not {wanted}"
            );
        }
    }

    #[test]
    fn a_shot_is_carried_over_the_ground() {
        let slope = ground(vec![vec![0, 100, 200, 300, 400]; 5]);
        let end = slope.carried(Vec3::new(2.0, 1.5, 3.0), Vec3::new(6.0, 1.5, 3.0));
        assert!((end.y - 3.5).abs() < 1e-5, "{end}");
    }

    #[test]
    fn straight_flight_follows_the_slope_and_stays_level_on_the_flat() {
        let slope = ground(vec![vec![0, 100, 200, 300, 400]; 5]);
        let dir = slope.along(Vec3::new(3.0, 1.0, 5.0), Vec3::X);
        assert!((dir.y / dir.x - 0.5).abs() < 1e-5, "{dir}");
        assert_eq!(ground(vec![]).along(Vec3::ZERO, Vec3::X), Vec3::X);
    }

    #[test]
    fn a_decal_lies_on_the_slope() {
        let slope = ground(vec![vec![0, 100, 200, 300, 400]; 5]);
        let up = slope.lie(5.0, 5.0) * Vec3::Y;
        assert!(up.distance(slope.normal(5.0, 5.0)) < 1e-5);
        assert_eq!(ground(vec![]).lie(1.0, 1.0), Quat::IDENTITY);
    }

    #[test]
    fn a_flat_level_has_no_terrain() {
        assert!(ground(vec![]).terrain().is_none());
    }

    #[test]
    fn the_span_is_the_corners_of_a_tile() {
        let slope = ground(vec![vec![0, 100, 200]; 3]);
        let (low, high) = slope.span(3.0, 3.0);
        assert!(
            (low - 0.75).abs() < 1e-6 && (high - 1.25).abs() < 1e-6,
            "{low} {high}"
        );
    }
}
