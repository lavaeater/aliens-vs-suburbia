//! Roads as roads: each [`RoadLine`] of the map drawn as ribbons over the ground, off the
//! tile grid. A street is a pavement ribbon with an asphalt one on top, a major road also
//! gets a dashed centre line, a path is a gravel strip. Ribbons are mitred at their bends
//! and end in round caps, so where roads meet their asphalt (all one material, lifted the
//! same) runs together into a junction; the dashes stop short of every road end.
//!
//! Only the look: the tiles under a road say where one can walk. Pure geometry, no ECS,
//! split per [`Chunk`] like the floor so it streams with it.

use std::collections::HashMap;

use bevy::prelude::Vec2;

use crate::general::components::map_components::{RoadKind, RoadLine};
use crate::map::floor_mesh::{FloorMesh, TerrainColor};
use crate::map::ground::GroundHeight;
use crate::map::streaming::{Chunk, chunk_of};

/// One look a road is drawn with, in the order they stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoadLayer {
    Pavement,
    Gravel,
    Asphalt,
    Marking,
}

impl RoadLayer {
    pub fn color(self) -> TerrainColor {
        match self {
            Self::Pavement => [132, 129, 122],
            Self::Gravel => [150, 132, 104],
            Self::Asphalt => [56, 58, 64],
            Self::Marking => [226, 220, 198],
        }
    }

    /// Height over the floor, in tiles: each layer above the one under it.
    fn lift(self) -> f32 {
        match self {
            Self::Pavement => 0.010,
            Self::Gravel => 0.012,
            Self::Asphalt => 0.020,
            Self::Marking => 0.030,
        }
    }
}

/// How wide the pavement is on each side, as a share of the carriageway.
const PAVEMENT: f32 = 0.22;
/// The centre line: dash length, gap and line width, as shares of the carriageway.
const DASH: f32 = 0.55;
const GAP: f32 = 0.55;
const LINE: f32 = 0.05;
/// A narrow path still shows, in tiles.
const MIN_PATH: f32 = 0.35;
/// Corners in a round cap.
const CAP_STEPS: usize = 10;
/// Longest piece of ribbon, in tiles, so it follows the hills (and chunks stay small).
const MAX_PIECE: f32 = 1.0;
/// A sharp bend's mitre is cut off at this many half-widths.
const MITRE_LIMIT: f32 = 2.5;

/// Every road of the map as meshes per chunk and layer. `tile` is the world size of a tile,
/// `floor_y` the floor's height; points are map coordinates (one tile in from the padded
/// grid, like decorations).
pub fn build_roads(
    roads: &[RoadLine],
    tile: f32,
    floor_y: f32,
    ground: &GroundHeight,
) -> HashMap<(Chunk, RoadLayer), FloorMesh> {
    let mut out: HashMap<(Chunk, RoadLayer), FloorMesh> = HashMap::new();
    let world = |p: &[f32; 2]| Vec2::new(tile * (p[0] + 1.0), tile * (p[1] + 1.0));
    let lines: Vec<Vec<Vec2>> = roads
        .iter()
        .map(|r| dedup(r.points.iter().map(world).collect()))
        .collect();
    let ends: Vec<Vec2> = lines
        .iter()
        .filter(|l| l.len() > 1)
        .flat_map(|l| [l.first().copied(), l.last().copied()])
        .flatten()
        .collect();

    let mut sink = Sink {
        out: &mut out,
        tile,
        floor_y,
        ground,
    };
    for (road, line) in roads.iter().zip(&lines) {
        if line.len() < 2 {
            continue;
        }
        let line = densify(line, MAX_PIECE * tile);
        let width = road.width.max(0.0) * tile;
        match road.kind {
            RoadKind::Major | RoadKind::Street => {
                sink.ribbon(&line, width * (0.5 + PAVEMENT), RoadLayer::Pavement);
                sink.ribbon(&line, width / 2.0, RoadLayer::Asphalt);
                if road.kind == RoadKind::Major {
                    sink.dashes(&line, width, &ends);
                }
            }
            RoadKind::Service => sink.ribbon(&line, width / 2.0, RoadLayer::Asphalt),
            RoadKind::Path => {
                sink.ribbon(&line, width.max(MIN_PATH * tile) / 2.0, RoadLayer::Gravel);
            }
        }
    }
    out.retain(|_, mesh| !mesh.is_empty());
    out
}

/// Drop points on top of the one before.
fn dedup(points: Vec<Vec2>) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_none_or(|last| last.distance(p) > 1e-4) {
            out.push(p);
        }
    }
    out
}

/// Put points along each segment so none is longer than `step`.
fn densify(points: &[Vec2], step: f32) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(points.len());
    for pair in points.windows(2) {
        let &[a, b] = pair else { continue };
        let pieces = (a.distance(b) / step.max(1e-3)).ceil().max(1.0) as usize;
        for i in 0..pieces {
            out.push(a.lerp(b, i as f32 / pieces as f32));
        }
    }
    out.extend(points.last());
    out
}

/// The left-hand normal of `a -> b`.
fn left(a: Vec2, b: Vec2) -> Vec2 {
    let d = (b - a).normalize_or_zero();
    Vec2::new(-d.y, d.x)
}

struct Sink<'a> {
    out: &'a mut HashMap<(Chunk, RoadLayer), FloorMesh>,
    tile: f32,
    floor_y: f32,
    ground: &'a GroundHeight,
}

impl Sink<'_> {
    fn mesh(&mut self, at: Vec2, layer: RoadLayer) -> &mut FloorMesh {
        let tile = (
            (at.x / self.tile).round() as i32,
            (at.y / self.tile).round() as i32,
        );
        self.out.entry((chunk_of(tile), layer)).or_default()
    }

    fn quad(&mut self, corners: [Vec2; 4], layer: RoadLayer) {
        let middle = (corners[0] + corners[1] + corners[2] + corners[3]) / 4.0;
        let lift = self.floor_y + layer.lift() * self.tile;
        let ground = self.ground;
        let mesh = self.mesh(middle, layer);
        let [a, b, c, d] = corners.map(|p| mesh.vertex(p.x, p.y, lift, ground));
        mesh.triangle(a, b, c);
        mesh.triangle(a, c, d);
    }

    fn disc(&mut self, centre: Vec2, radius: f32, layer: RoadLayer) {
        let lift = self.floor_y + layer.lift() * self.tile;
        let ground = self.ground;
        let mesh = self.mesh(centre, layer);
        let middle = mesh.vertex(centre.x, centre.y, lift, ground);
        let rim: Vec<u32> = (0..CAP_STEPS)
            .map(|i| {
                let angle = i as f32 / CAP_STEPS as f32 * std::f32::consts::TAU;
                let p = centre + Vec2::from_angle(angle) * radius;
                mesh.vertex(p.x, p.y, lift, ground)
            })
            .collect();
        for (i, &a) in rim.iter().enumerate() {
            let b = rim.get(i + 1).or(rim.first()).copied().unwrap_or(a);
            mesh.triangle(middle, a, b);
        }
    }

    /// A strip `half` either side of the line, mitred at the bends, round at the ends.
    fn ribbon(&mut self, line: &[Vec2], half: f32, layer: RoadLayer) {
        if half <= 0.0 {
            return;
        }
        let offsets: Vec<Vec2> = (0..line.len())
            .map(|i| {
                let here = line.get(i).copied().unwrap_or_default();
                let before = i.checked_sub(1).and_then(|j| line.get(j)).copied();
                let after = line.get(i + 1).copied();
                let n_in = before.map(|b| left(b, here));
                let n_out = after.map(|a| left(here, a));
                let normal = match (n_in, n_out) {
                    (Some(a), Some(b)) => {
                        let mitre = (a + b).normalize_or(b);
                        mitre / mitre.dot(b).max(1.0 / MITRE_LIMIT)
                    }
                    (Some(n), None) | (None, Some(n)) => n,
                    (None, None) => Vec2::ZERO,
                };
                normal * half
            })
            .collect();
        for i in 0..line.len().saturating_sub(1) {
            let (Some(&a), Some(&b), Some(&oa), Some(&ob)) =
                (line.get(i), line.get(i + 1), offsets.get(i), offsets.get(i + 1))
            else {
                continue;
            };
            self.quad([a + oa, b + ob, b - ob, a - oa], layer);
        }
        for end in [line.first(), line.last()].into_iter().flatten() {
            self.disc(*end, half, layer);
        }
    }

    /// A dashed line down the middle, stopping a road's width short of any road's end
    /// (where it meets another road, or a cul-de-sac).
    fn dashes(&mut self, line: &[Vec2], width: f32, ends: &[Vec2]) {
        let (dash, gap, half) = (DASH * width, GAP * width, LINE * width / 2.0);
        let clear = |p: Vec2| ends.iter().all(|e| e.distance(p) > width);
        let mut walked = 0.0;
        for pair in line.windows(2) {
            let &[a, b] = pair else { continue };
            let length = a.distance(b);
            if length <= 0.0 {
                continue;
            }
            let normal = left(a, b) * half;
            let dir = (b - a) / length;
            // Dashes start every dash + gap along the whole road; draw the part of each in
            // this segment.
            let period = dash + gap;
            let mut start = (walked / period).floor() * period;
            while start < walked + length {
                let from = (start - walked).max(0.0);
                let to = (start + dash - walked).min(length);
                if to > from {
                    let (p, q) = (a + dir * from, a + dir * to);
                    if clear(p) && clear(q) {
                        self.quad([p + normal, q + normal, q - normal, p - normal], RoadLayer::Marking);
                    }
                }
                start += period;
            }
            walked += length;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn road(kind: RoadKind, points: &[[f32; 2]]) -> RoadLine {
        RoadLine {
            kind,
            width: 2.0,
            points: points.to_vec(),
        }
    }

    fn triangles(meshes: &HashMap<(Chunk, RoadLayer), FloorMesh>, layer: RoadLayer) -> usize {
        meshes
            .iter()
            .filter(|((_, l), _)| *l == layer)
            .map(|(_, m)| m.parts().1.len() / 3)
            .sum()
    }

    #[test]
    fn a_street_is_pavement_under_asphalt_and_a_path_is_gravel() {
        let roads = [
            road(RoadKind::Street, &[[2.0, 2.0], [12.0, 2.0]]),
            road(RoadKind::Path, &[[2.0, 8.0], [5.0, 8.0]]),
        ];
        let meshes = build_roads(&roads, 1.0, 0.0, &GroundHeight::default());
        assert!(triangles(&meshes, RoadLayer::Pavement) > 0);
        assert!(triangles(&meshes, RoadLayer::Asphalt) > 0);
        assert!(triangles(&meshes, RoadLayer::Gravel) > 0);
        assert_eq!(triangles(&meshes, RoadLayer::Marking), 0, "streets have no centre line");
    }

    #[test]
    fn every_triangle_faces_up_and_the_asphalt_is_as_wide_as_the_road() {
        let roads = [road(RoadKind::Service, &[[0.0, 0.0], [6.0, 3.0], [6.0, 9.0]])];
        let meshes = build_roads(&roads, 2.0, 0.0, &GroundHeight::default());
        for mesh in meshes.values() {
            let (positions, indices) = mesh.parts();
            for tri in indices.as_chunks::<3>().0 {
                let p = |i: u32| positions[i as usize];
                let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
                let (ab, ac) = ([b[0] - a[0], b[2] - a[2]], [c[0] - a[0], c[2] - a[2]]);
                assert!(ab[1] * ac[0] - ab[0] * ac[1] >= -1e-4, "faces down: {tri:?}");
            }
            // Width 2 tiles of 2 world units: everything within 2 of the centreline.
            let line = [Vec2::new(2.0, 2.0), Vec2::new(14.0, 8.0), Vec2::new(14.0, 20.0)];
            for p in &positions {
                let q = Vec2::new(p[0], p[2]);
                let d = line
                    .windows(2)
                    .map(|s| {
                        let t = ((q - s[0]).dot(s[1] - s[0]) / s[0].distance_squared(s[1])).clamp(0.0, 1.0);
                        q.distance(s[0].lerp(s[1], t))
                    })
                    .fold(f32::MAX, f32::min);
                assert!(d <= 2.0 * MITRE_LIMIT + 1e-3, "{q} is {d} off the road");
            }
        }
    }

    #[test]
    fn dashes_stop_short_of_junctions() {
        // A major road with a side street meeting it in the middle.
        let roads = [
            road(RoadKind::Major, &[[0.0, 10.0], [40.0, 10.0]]),
            road(RoadKind::Street, &[[20.0, 10.0], [20.0, 30.0]]),
        ];
        let meshes = build_roads(&roads, 1.0, 0.0, &GroundHeight::default());
        let marks: Vec<[f32; 3]> = meshes
            .iter()
            .filter(|((_, l), _)| *l == RoadLayer::Marking)
            .flat_map(|(_, m)| m.parts().0)
            .collect();
        assert!(!marks.is_empty());
        // Map (20, 10) is world (21, 11); the road is 2 wide.
        let junction = Vec2::new(21.0, 11.0);
        assert!(marks.iter().all(|p| Vec2::new(p[0], p[2]).distance(junction) > 2.0 - 1e-3));
    }

    #[test]
    fn long_roads_split_over_the_chunks_they_cross() {
        let far = crate::map::streaming::CHUNK_TILES as f32 * 2.5;
        let roads = [road(RoadKind::Service, &[[0.0, 0.0], [far, 0.0]])];
        let meshes = build_roads(&roads, 1.0, 0.0, &GroundHeight::default());
        assert_eq!(meshes.len(), 3);
    }
}
