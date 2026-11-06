//! The height of the ground anywhere on a map: [`MapFile::elevation`] read between its points.
//!
//! Hills are behind `--hills` ([`HillsPreview`]) until everything that stands on the ground
//! follows it. Without the flag, or on a map with no heights, [`GroundHeight`] is flat at 0.

use bevy::math::Vec3;
use bevy::prelude::Resource;

use crate::general::components::map_components::{ELEVATION_STEP, MapFile};

/// Insert it to bend the world to `MapFile::elevation`. Characters and colliders stay on the
/// flat floor for now, so a player walks unseen beneath the hills.
#[derive(Resource)]
pub struct HillsPreview;

/// The current map's ground, above `TileDefinitions::floor_level`, in world coordinates.
#[derive(Resource, Debug, Clone, Default)]
pub struct GroundHeight {
    /// World height at each lattice point. Empty = flat.
    points: Vec<Vec<f32>>,
    tile: f32,
}

impl GroundHeight {
    pub fn new(map: &MapFile, tile: f32) -> Self {
        let points = map
            .elevation
            .iter()
            .map(|row| row.iter().map(|cm| f32::from(*cm) / 100.0).collect())
            .collect();
        Self { points, tile }
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
    fn the_span_is_the_corners_of_a_tile() {
        let slope = ground(vec![vec![0, 100, 200]; 3]);
        let (low, high) = slope.span(3.0, 3.0);
        assert!(
            (low - 0.75).abs() < 1e-6 && (high - 1.25).abs() < 1e-6,
            "{low} {high}"
        );
    }
}
