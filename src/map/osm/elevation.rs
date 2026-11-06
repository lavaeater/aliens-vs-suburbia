//! Ground heights from Lantmäteriet's Markhöjdmodell (`docs/gis-levels.md`), saved as an ESRI
//! ASCII grid in SWEREF 99 TM by `tools/fetch_heights.py`, sampled onto a level's lattice
//! ([`ELEVATION_STEP`]). All pure.
// Grid maths: the lattice is built rectangular here, its indices come from its own ranges.
#![allow(clippy::indexing_slicing)]

use std::f64::consts::FRAC_PI_2;

use super::data::LatLon;
use super::geometry::{Frame, Projection};
use crate::general::components::map_components::ELEVATION_STEP;

/// Passes of a 3 x 3 box over the lattice. Takes the cliffs off and leaves the hills.
const SMOOTHING: usize = 2;

/// A north-up grid of heights in metres, in SWEREF 99 TM. Cells hold their centre's height.
#[derive(Debug, Clone)]
pub struct HeightGrid {
    west: f64,
    north: f64,
    cell: f64,
    cols: usize,
    rows: usize,
    /// Row by row from the north. NaN = no data.
    heights: Vec<f32>,
}

impl HeightGrid {
    /// Read an ESRI ASCII grid.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut tokens = text.split_ascii_whitespace().peekable();
        let (mut cols, mut rows, mut west, mut south, mut cell) = (None, None, None, None, None);
        let mut nodata = None;
        while let Some(key) = tokens.next_if(|t| t.starts_with(|c: char| c.is_ascii_alphabetic())) {
            let value = tokens
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .ok_or_else(|| format!("no number after `{key}`"))?;
            match key.to_ascii_lowercase().as_str() {
                "ncols" => cols = Some(value as usize),
                "nrows" => rows = Some(value as usize),
                "xllcorner" => west = Some(value),
                "yllcorner" => south = Some(value),
                "cellsize" => cell = Some(value),
                "nodata_value" => nodata = Some(value as f32),
                _ => return Err(format!("unknown header `{key}`")),
            }
        }
        let missing = |name: &str| format!("the grid has no `{name}`");
        let cols = cols.ok_or_else(|| missing("ncols"))?;
        let rows = rows.ok_or_else(|| missing("nrows"))?;
        let cell = cell
            .filter(|c| *c > 0.0)
            .ok_or_else(|| missing("cellsize"))?;
        let west = west.ok_or_else(|| missing("xllcorner"))?;
        let south = south.ok_or_else(|| missing("yllcorner"))?;
        let heights = tokens
            .map(|t| match t.parse::<f32>() {
                Ok(v) if Some(v) == nodata => Ok(f32::NAN),
                Ok(v) => Ok(v),
                Err(_) => Err(format!("`{t}` is not a height")),
            })
            .collect::<Result<Vec<f32>, String>>()?;
        if heights.len() != cols * rows {
            return Err(format!(
                "{} heights for {cols} x {rows} cells",
                heights.len()
            ));
        }
        Ok(Self {
            west,
            north: south + rows as f64 * cell,
            cell,
            cols,
            rows,
            heights,
        })
    }

    fn cell_at(&self, col: usize, row: usize) -> Option<f64> {
        (col < self.cols && row < self.rows)
            .then(|| self.heights.get(row * self.cols + col))
            .flatten()
            .map(|h| f64::from(*h))
            .filter(|h| !h.is_nan())
    }

    /// The height at a point, between the four nearest cell centres. `None` off the grid or
    /// next to a cell with no data.
    pub fn at(&self, east: f64, north: f64) -> Option<f64> {
        let x = (east - self.west) / self.cell - 0.5;
        let y = (self.north - north) / self.cell - 0.5;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let (col, row) = (x as usize, y as usize);
        let (fx, fy) = (x.fract(), y.fract());
        let top = self.cell_at(col, row)? * (1.0 - fx) + self.cell_at(col + 1, row)? * fx;
        let bottom =
            self.cell_at(col, row + 1)? * (1.0 - fx) + self.cell_at(col + 1, row + 1)? * fx;
        Some(top * (1.0 - fy) + bottom * fy)
    }
}

/// Lat/lon to SWEREF 99 TM (east, north): Krüger's series on GRS 80, as Lantmäteriet gives it.
/// Good to well under a metre in Sweden. Note the `asinh`.
pub fn sweref(p: LatLon) -> (f64, f64) {
    const FLATTENING: f64 = 1.0 / 298.257_222_101;
    const SEMI_MAJOR: f64 = 6_378_137.0;
    const SCALE: f64 = 0.9996;
    const FALSE_EASTING: f64 = 500_000.0;
    const MERIDIAN: f64 = 15.0;

    let n = FLATTENING / (2.0 - FLATTENING);
    let radius = SEMI_MAJOR / (1.0 + n) * (1.0 + n.powi(2) / 4.0 + n.powi(4) / 64.0);
    let eccentricity = 2.0 * n.sqrt() / (1.0 + n);
    let (phi, lambda) = (p.lat.to_radians(), (p.lon - MERIDIAN).to_radians());
    debug_assert!(phi.abs() < FRAC_PI_2);
    let t = (phi.sin().atanh() - eccentricity * (eccentricity * phi.sin()).atanh()).sinh();
    let xi = t.atan2(lambda.cos());
    let eta = (lambda.sin() / t.hypot(lambda.cos())).asinh();
    let series = [
        n / 2.0 - 2.0 * n.powi(2) / 3.0 + 5.0 * n.powi(3) / 16.0,
        13.0 * n.powi(2) / 48.0 - 3.0 * n.powi(3) / 5.0,
        61.0 * n.powi(3) / 240.0,
    ];
    let (mut north, mut east) = (xi, eta);
    for (i, alpha) in series.iter().enumerate() {
        let k = 2.0 * (i + 1) as f64;
        north += alpha * (k * xi).sin() * (k * eta).cosh();
        east += alpha * (k * xi).cos() * (k * eta).sinh();
    }
    (
        FALSE_EASTING + SCALE * radius * east,
        SCALE * radius * north,
    )
}

/// Lattice points across a side of this many tiles, the last past the far edge if need be.
const fn lattice_points(tiles: usize) -> usize {
    tiles.saturating_sub(1).div_ceil(ELEVATION_STEP) + 1
}

fn smoothed(grid: &[Vec<f64>]) -> Vec<Vec<f64>> {
    /// Up to three items around `at`.
    fn around<T>(items: &[T], at: usize) -> &[T] {
        let last = (at + 2).min(items.len());
        &items[at.saturating_sub(1)..last]
    }
    grid.iter()
        .enumerate()
        .map(|(row, cells)| {
            (0..cells.len())
                .map(|col| {
                    let near = around(grid, row).iter().flat_map(|r| around(r, col));
                    let (sum, count) = near.fold((0.0, 0.0), |(s, n), h| (s + h, n + 1.0));
                    sum / count
                })
                .collect()
        })
        .collect()
}

/// A level's lattice: centimetres of world height above its lowest point, `scale` world units
/// to the metre. A point `(i, j)` sits on the centre of tile `(i * STEP, j * STEP)`.
pub fn lattice(
    grid: &HeightGrid,
    frame: &Frame,
    projection: &Projection,
    scale: f64,
) -> Result<Vec<Vec<u16>>, String> {
    let (cols, rows) = (lattice_points(frame.width), lattice_points(frame.height));
    let mut metres = vec![vec![0.0; cols]; rows];
    for (j, row) in metres.iter_mut().enumerate() {
        for (i, height) in row.iter_mut().enumerate() {
            let tile = (
                (i * ELEVATION_STEP) as f64 + 0.5,
                (j * ELEVATION_STEP) as f64 + 0.5,
            );
            let (east, north) = sweref(projection.unproject(frame.metres_at(tile)));
            *height = grid
                .at(east, north)
                .ok_or_else(|| format!("the height grid doesn't reach tile {tile:?}"))?;
        }
    }
    for _ in 0..SMOOTHING {
        metres = smoothed(&metres);
    }
    let lowest = metres
        .iter()
        .flatten()
        .copied()
        .fold(f64::INFINITY, f64::min);
    Ok(metres
        .iter()
        .map(|row| {
            row.iter()
                .map(|h| ((h - lowest) * scale * 100.0).round() as u16)
                .collect()
        })
        .collect())
}

/// A lattice's relief for the report: highest point in world units, and the slope in degrees
/// that 99% of it stays under, and the steepest.
pub fn relief(lattice: &[Vec<u16>]) -> (f32, f32, f32) {
    let run = (ELEVATION_STEP * 100) as f32;
    let mut slopes: Vec<f32> = Vec::new();
    for (j, row) in lattice.iter().enumerate() {
        for (i, &h) in row.iter().enumerate() {
            let east = row.get(i + 1).map_or(0.0, |&e| f32::from(e) - f32::from(h));
            let south = lattice
                .get(j + 1)
                .and_then(|r| r.get(i))
                .map_or(0.0, |&s| f32::from(s) - f32::from(h));
            slopes.push((east.hypot(south) / run).atan().to_degrees());
        }
    }
    slopes.sort_by(f32::total_cmp);
    let highest = lattice.iter().flatten().copied().max().unwrap_or(0);
    let nearly = slopes.get(slopes.len() * 99 / 100).copied().unwrap_or(0.0);
    let steepest = slopes.last().copied().unwrap_or(0.0);
    (f32::from(highest) / 100.0, nearly, steepest)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3 x 2 cells of 2 m, west edge 100, south edge 200, so the north edge is 204.
    const GRID: &str = "ncols 3\nnrows 2\nxllcorner 100\nyllcorner 200\ncellsize 2\n\
                        NODATA_value -9999\n10 20 30\n14 24 -9999\n";

    #[test]
    fn reads_a_cell_at_its_centre_and_between_cells() {
        let grid = HeightGrid::parse(GRID).unwrap();
        assert_eq!(grid.at(101.0, 203.0), Some(10.0));
        assert_eq!(grid.at(102.0, 203.0), Some(15.0));
        assert_eq!(grid.at(102.0, 202.0), Some(17.0));
    }

    #[test]
    fn no_height_off_the_grid_or_beside_no_data() {
        let grid = HeightGrid::parse(GRID).unwrap();
        assert_eq!(grid.at(99.0, 203.0), None);
        assert_eq!(grid.at(101.0, 205.0), None);
        assert_eq!(grid.at(104.0, 202.0), None, "the next cell has no data");
    }

    #[test]
    fn a_grid_of_the_wrong_size_is_an_error() {
        let text = "ncols 2\nnrows 2\nxllcorner 0\nyllcorner 0\ncellsize 1\n1 2 3\n";
        assert!(HeightGrid::parse(text).unwrap_err().contains("3 heights"));
    }

    #[test]
    fn sweref_on_the_central_meridian_is_at_the_false_easting() {
        let (east, north) = sweref(LatLon {
            lat: 59.0,
            lon: 15.0,
        });
        assert!((east - 500_000.0).abs() < 1e-6);
        assert!((6_500_000.0..6_600_000.0).contains(&north), "{north}");
    }

    #[test]
    fn sweref_puts_the_level_in_its_height_file() {
        // map-01's centre, in the 10 x 10 km file m656_65.
        let (east, north) = sweref(LatLon {
            lat: 59.199295,
            lon: 17.753444,
        });
        assert!((650_000.0..660_000.0).contains(&east), "{east}");
        assert!((6_560_000.0..6_570_000.0).contains(&north), "{north}");
    }

    fn level(size: usize) -> (Frame, Projection, LatLon) {
        let centre = LatLon {
            lat: 59.2,
            lon: 17.75,
        };
        let frame = Frame {
            rotation: -0.5_f64.to_radians(),
            metres_per_tile: 1.0,
            width: size,
            height: size,
        };
        (frame, Projection::new(centre), centre)
    }

    /// A grid sloping up to the east, `rise` metres to the metre, around `centre`.
    fn slope(centre: LatLon, rise: f64) -> HeightGrid {
        let (east, north) = sweref(centre);
        let (cols, half) = (200_usize, 200.0);
        let row: Vec<String> = (0..cols)
            .map(|c| format!("{:.3}", (c as f64 * 2.0) * rise))
            .collect();
        let rows = vec![row.join(" "); cols].join("\n");
        let text = format!(
            "ncols {cols}\nnrows {cols}\nxllcorner {}\nyllcorner {}\ncellsize 2\n\
             NODATA_value -9999\n{rows}\n",
            east - half,
            north - half,
        );
        HeightGrid::parse(&text).unwrap()
    }

    #[test]
    fn the_lattice_starts_at_zero_and_scales_the_rise() {
        let (frame, projection, centre) = level(100);
        let grid = slope(centre, 0.2);
        let lattice = lattice(&grid, &frame, &projection, 0.25).unwrap();
        assert_eq!(lattice.len(), 51, "a point every two tiles, both edges");
        assert_eq!(lattice.iter().flatten().copied().min(), Some(0));
        // In the middle, away from the edges the smoothing clamps: 0.2 * 0.25 per tile.
        let rise = f32::from(lattice[25][30]) - f32::from(lattice[25][20]);
        assert!((rise - 100.0).abs() < 3.0, "20 tiles rise {rise} cm");
    }

    #[test]
    fn a_level_past_the_grid_is_an_error() {
        let (frame, projection, centre) = level(600);
        let err = lattice(&slope(centre, 0.2), &frame, &projection, 0.25).unwrap_err();
        assert!(err.contains("doesn't reach"), "{err}");
    }

    #[test]
    fn relief_reads_the_steepest_slope() {
        // 200 cm over a two tile step is 45 degrees.
        let (highest, _, steepest) = relief(&[vec![0, 200]]);
        assert_eq!(highest, 2.0);
        assert!((steepest - 45.0).abs() < 0.01, "{steepest}");
    }
}
