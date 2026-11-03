//! Geometry for the OSM import: lat/lon to metres, metres to tiles, and drawing polygons
//! and lines into a tile grid. All pure.
//!
//! Coordinates go through three spaces:
//! 1. **lat/lon** (WGS84, what OSM stores).
//! 2. **metres** east/north of the level centre ([`Projection`]: a local equirectangular
//!    projection, off by well under a tile over the few hundred metres a level covers).
//! 3. **tiles** ([`Frame`]): turned by the level rotation, scaled by metres per tile, and
//!    y flipped, because the tile grid's row 0 is the top (north) edge.
// Grid maths: indices are bounds-checked with `get`/`get_mut` or come from clamped ranges.
#![allow(clippy::indexing_slicing)]

use super::data::LatLon;

/// Mean Earth radius, metres.
const EARTH_RADIUS: f64 = 6_371_008.8;

pub type Point = (f64, f64);

/// Local projection around a centre: metres east (x) and north (y) of it.
#[derive(Debug, Clone, Copy)]
pub struct Projection {
    centre: LatLon,
    metres_per_degree_lat: f64,
    metres_per_degree_lon: f64,
}

impl Projection {
    pub fn new(centre: LatLon) -> Self {
        let per_degree = EARTH_RADIUS * std::f64::consts::PI / 180.0;
        Self {
            centre,
            metres_per_degree_lat: per_degree,
            metres_per_degree_lon: per_degree * centre.lat.to_radians().cos(),
        }
    }

    pub fn project(&self, p: LatLon) -> Point {
        (
            (p.lon - self.centre.lon) * self.metres_per_degree_lon,
            (p.lat - self.centre.lat) * self.metres_per_degree_lat,
        )
    }
}

/// Metres (around the centre) to tile coordinates. A tile `(col, row)` covers
/// `col..col+1` by `row..row+1` in tile coordinates; its centre is at `+0.5`.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// Counter-clockwise rotation applied to the data, radians.
    pub rotation: f64,
    pub metres_per_tile: f64,
    pub width: usize,
    pub height: usize,
}

impl Frame {
    pub fn to_tiles(&self, (x, y): Point) -> Point {
        let (sin, cos) = self.rotation.sin_cos();
        let (rx, ry) = (x * cos - y * sin, x * sin + y * cos);
        (
            rx / self.metres_per_tile + self.width as f64 / 2.0,
            self.height as f64 / 2.0 - ry / self.metres_per_tile,
        )
    }

    /// The tile a point falls in, if it is inside the grid.
    pub fn cell(&self, p: Point) -> Option<(usize, usize)> {
        let (x, y) = self.to_tiles(p);
        if x < 0.0 || y < 0.0 || x >= self.width as f64 || y >= self.height as f64 {
            return None;
        }
        Some((x as usize, y as usize))
    }
}

/// Every cell whose centre lies inside the rings (even-odd), by scanline.
pub fn fill_rings(rings: &[Vec<Point>], width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut cells = Vec::new();
    let mut crossings = Vec::new();
    for row in 0..height {
        let yc = row as f64 + 0.5;
        crossings.clear();
        for ring in rings {
            for pair in ring.windows(2) {
                let [(x0, y0), (x1, y1)] = [pair[0], pair[1]];
                if (y0 > yc) != (y1 > yc) {
                    crossings.push(x0 + (yc - y0) / (y1 - y0) * (x1 - x0));
                }
            }
        }
        crossings.sort_by(f64::total_cmp);
        for span in crossings.chunks_exact(2) {
            // Cells whose centre (col + 0.5) is in [span0, span1).
            let first = (span[0] - 0.5).ceil().max(0.0);
            let last = (span[1] - 0.5).ceil().min(width as f64);
            let (first, last) = (first as usize, last.max(0.0) as usize);
            cells.extend((first..last).map(|col| (col, row)));
        }
    }
    cells
}

/// Every cell whose centre is within `width / 2` of the polyline. The radius never drops
/// below half a cell's diagonal, so even a thin fence comes out as an unbroken (4-connected)
/// line of cells that nothing slips through diagonally.
pub fn stroke_line(points: &[Point], width: f64, grid_w: usize, grid_h: usize) -> Vec<(usize, usize)> {
    let radius = (width / 2.0).max(std::f64::consts::FRAC_1_SQRT_2);
    let mut cells = Vec::new();
    for pair in points.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let x0 = (a.0.min(b.0) - radius).floor().max(0.0) as usize;
        let x1 = ((a.0.max(b.0) + radius).ceil().max(0.0) as usize).min(grid_w);
        let y0 = (a.1.min(b.1) - radius).floor().max(0.0) as usize;
        let y1 = ((a.1.max(b.1) + radius).ceil().max(0.0) as usize).min(grid_h);
        for row in y0..y1 {
            for col in x0..x1 {
                let c = (col as f64 + 0.5, row as f64 + 0.5);
                if distance_to_segment(c, a, b) <= radius {
                    cells.push((col, row));
                }
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (cx * cx + cy * cy).sqrt()
}

/// Area of a ring, square metres (shoelace, unsigned).
pub fn ring_area(ring: &[Point]) -> f64 {
    ring.windows(2)
        .map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1)
        .sum::<f64>()
        .abs()
        / 2.0
}

/// The rotation (radians, within +-45 degrees) that best lines the given edges up with
/// the grid axes: a length-weighted histogram of edge directions folded into a quarter
/// turn, lightly smoothed, and its peak. Most planned Swedish suburbs sit on one street
/// grid, so turning by this squares most houses up.
pub fn dominant_rotation(edges: impl IntoIterator<Item = (Point, Point)>) -> f64 {
    const BINS: usize = 90;
    let mut histogram = [0.0_f64; BINS];
    for (a, b) in edges {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).sqrt();
        if length < 1.0 {
            continue;
        }
        let degrees = dy.atan2(dx).to_degrees().rem_euclid(90.0);
        histogram[(degrees as usize).min(BINS - 1)] += length;
    }
    let smoothed = |i: usize| {
        histogram[(i + BINS - 1) % BINS] + 2.0 * histogram[i] + histogram[(i + 1) % BINS]
    };
    let peak = (0..BINS)
        .max_by(|a, b| smoothed(*a).total_cmp(&smoothed(*b)))
        .unwrap_or(0);
    if smoothed(peak) <= 0.0 {
        return 0.0;
    }
    let angle = peak as f64 + 0.5;
    // Turn the data back by the edge angle, the short way round.
    let turn = if angle > 45.0 { 90.0 - angle } else { -angle };
    turn.to_radians()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_is_metres() {
        let p = Projection::new(LatLon { lat: 59.0, lon: 18.0 });
        let (x, y) = p.project(LatLon { lat: 59.001, lon: 18.0 });
        assert!(x.abs() < 1e-9);
        assert!((y - 111.19).abs() < 0.1, "{y}");
        let (x, _) = p.project(LatLon { lat: 59.0, lon: 18.001 });
        // Shorter east-west at 59 degrees north: cos(59) ~ 0.515.
        assert!((x - 57.27).abs() < 0.1, "{x}");
    }

    #[test]
    fn frame_puts_north_at_the_top() {
        let frame = Frame {
            rotation: 0.0,
            metres_per_tile: 1.0,
            width: 10,
            height: 10,
        };
        assert_eq!(frame.cell((0.0, 0.0)), Some((5, 5)));
        assert_eq!(frame.cell((0.0, 4.6)), Some((5, 0)));
        assert_eq!(frame.cell((-4.6, -4.6)), Some((0, 9)));
        assert_eq!(frame.cell((0.0, 6.0)), None);
    }

    #[test]
    fn fills_a_square_with_a_hole() {
        let outer = vec![(1.0, 1.0), (6.0, 1.0), (6.0, 6.0), (1.0, 6.0), (1.0, 1.0)];
        let inner = vec![(3.0, 3.0), (4.0, 3.0), (4.0, 4.0), (3.0, 4.0), (3.0, 3.0)];
        let cells = fill_rings(&[outer.clone()], 10, 10);
        assert_eq!(cells.len(), 25);
        let cells = fill_rings(&[outer, inner], 10, 10);
        assert_eq!(cells.len(), 24);
        assert!(!cells.contains(&(3, 3)));
    }

    #[test]
    fn fill_clips_to_the_grid() {
        let ring = vec![(-5.0, -5.0), (3.0, -5.0), (3.0, 3.0), (-5.0, 3.0), (-5.0, -5.0)];
        assert_eq!(fill_rings(&[ring], 10, 10).len(), 9);
    }

    #[test]
    fn thin_diagonal_lines_have_no_gaps() {
        let cells = stroke_line(&[(0.0, 0.0), (8.0, 8.0)], 0.1, 10, 10);
        // Every cell on the diagonal and a 4-connected neighbour for each step.
        for i in 0..7 {
            assert!(cells.contains(&(i, i)));
            assert!(cells.contains(&(i + 1, i)) || cells.contains(&(i, i + 1)));
        }
    }

    #[test]
    fn wide_lines_are_wide() {
        let cells = stroke_line(&[(0.0, 5.0), (10.0, 5.0)], 6.0, 10, 10);
        assert!(cells.contains(&(5, 2)) && cells.contains(&(5, 7)));
        assert!(!cells.contains(&(5, 1)) && !cells.contains(&(5, 8)));
    }

    #[test]
    fn finds_the_street_grid_angle() {
        let turn = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            let rect = [(0.0, 0.0), (10.0, 0.0), (10.0, 6.0), (0.0, 6.0), (0.0, 0.0)];
            let pts: Vec<Point> = rect.iter().map(|(x, y)| (x * c - y * s, x * s + y * c)).collect();
            let edges: Vec<(Point, Point)> = pts.windows(2).map(|w| (w[0], w[1])).collect();
            dominant_rotation(edges).to_degrees()
        };
        assert!((turn(30.0) + 30.0).abs() < 1.0, "{}", turn(30.0));
        assert!((turn(80.0) - 10.0).abs() < 1.0, "{}", turn(80.0));
        assert!(turn(0.0).abs() < 1.0);
        assert_eq!(dominant_rotation([]), 0.0);
    }

    #[test]
    fn ring_area_in_square_metres() {
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0), (0.0, 0.0)];
        assert!((ring_area(&ring) - 12.0).abs() < 1e-9);
    }
}
