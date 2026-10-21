//! Wall meshes for impassable tiles: what the colliders in `map_loader` look like.
//!
//! Pure geometry, no ECS: given the set of tiles in one wall category, build one merged
//! mesh for all of them. Only the faces that can be seen are emitted -- the side between
//! two neighbouring wall tiles is never drawn -- so a long wall is a handful of quads per
//! tile instead of a box each, and a whole map is one draw call per category.
//!
//! A wall is a body with an optional cap: a slightly wider, lighter slab on top that
//! overhangs only on the exposed sides, so runs of wall read as one continuous coping
//! rather than a row of blocks. Colour is baked into the vertices (the material is white):
//! each tile gets a small brightness jitter so masonry does not look like one extruded
//! slab, and the sides darken toward the floor, a cheap stand-in for ambient occlusion
//! that still reads once ambient light is gone.

use std::collections::HashSet;

/// One wall category's look.
#[derive(Clone, Copy, Debug)]
pub struct WallStyle {
    /// Total height above the floor, cap included.
    pub height: f32,
    pub body_color: [f32; 4],
    pub cap: Option<WallCap>,
    /// How far each tile's brightness may stray, e.g. `0.08` for +/- 8%.
    pub jitter: f32,
    /// Brightness of the side faces at the floor, relative to their top.
    pub foot_shade: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WallCap {
    pub thickness: f32,
    /// How far the cap sticks out past the body on an exposed side.
    pub overhang: f32,
    pub color: [f32; 4],
}

/// Mesh data ready for `Mesh::insert_attribute`.
#[derive(Default, Debug)]
pub struct WallMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl WallMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// A quad whose corners `c` run counter-clockwise seen from the side `normal` points
    /// to, which is the front face in Bevy's convention.
    fn quad(&mut self, c: [[f32; 3]; 4], normal: [f32; 3], colors: [[f32; 4]; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&c);
        self.normals.extend_from_slice(&[normal; 4]);
        self.colors.extend_from_slice(&colors);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// The four sides of a tile. Rows grow toward +Z.
#[derive(Clone, Copy)]
enum Side {
    East,
    West,
    South,
    North,
}

const SIDES: [Side; 4] = [Side::East, Side::West, Side::South, Side::North];

impl Side {
    /// The neighbouring tile on this side.
    const fn step(self) -> (i32, i32) {
        match self {
            Self::East => (1, 0),
            Self::West => (-1, 0),
            Self::South => (0, 1),
            Self::North => (0, -1),
        }
    }

    const fn normal(self) -> [f32; 3] {
        match self {
            Self::East => [1.0, 0.0, 0.0],
            Self::West => [-1.0, 0.0, 0.0],
            Self::South => [0.0, 0.0, 1.0],
            Self::North => [0.0, 0.0, -1.0],
        }
    }
}

/// A stable per-tile number in `[-1, 1]`, so the masonry jitter is the same every load.
fn tile_noise(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 32767.5 - 1.0
}

fn shade(color: [f32; 4], k: f32) -> [f32; 4] {
    [color[0] * k, color[1] * k, color[2] * k, color[3]]
}

/// The vertical face on `side` of the box `[x0, x1] x [y0, y1] x [z0, z1]`, wound to face
/// outward. Bottom corners first, so the per-vertex colours can shade the foot.
fn side_face(
    side: Side,
    (x0, x1): (f32, f32),
    (y0, y1): (f32, f32),
    (z0, z1): (f32, f32),
) -> [[f32; 3]; 4] {
    match side {
        Side::East => [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
        Side::West => [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        Side::South => [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        Side::North => [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
    }
}

/// The horizontal face on top of a footprint, facing up.
fn top_face((x0, x1): (f32, f32), y: f32, (z0, z1): (f32, f32)) -> [[f32; 3]; 4] {
    [[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]]
}

/// Build the walls for `cells` (grid `(col, row)`, tile centres at `tile * (col, row)` like
/// the floor). `hides(neighbour)` says whether a neighbouring tile covers the side facing
/// it -- the same category, or a taller wall -- so that side is skipped.
pub fn build_walls(
    cells: &HashSet<(i32, i32)>,
    hides: impl Fn((i32, i32)) -> bool,
    tile: f32,
    floor_y: f32,
    style: &WallStyle,
) -> WallMesh {
    let mut mesh = WallMesh::default();
    let half = tile * 0.5;
    let cap_thickness = style.cap.map_or(0.0, |c| c.thickness.min(style.height));
    let body_top = floor_y + style.height - cap_thickness;
    let top = floor_y + style.height;

    // Sorted so the output does not depend on the set's iteration order.
    let mut sorted: Vec<(i32, i32)> = cells.iter().copied().collect();
    sorted.sort_unstable();

    for (col, row) in sorted {
        let (cx, cz) = (tile * col as f32, tile * row as f32);
        let (xs, zs) = ((cx - half, cx + half), (cz - half, cz + half));
        let k = 1.0 + style.jitter * tile_noise(col, row);
        let body = shade(style.body_color, k);
        let foot = shade(body, style.foot_shade);
        let exposed = SIDES.map(|side| {
            let (dx, dy) = side.step();
            !hides((col + dx, row + dy))
        });

        for (side, open) in SIDES.into_iter().zip(exposed) {
            if open {
                mesh.quad(
                    side_face(side, xs, (floor_y, body_top), zs),
                    side.normal(),
                    [foot, foot, body, body],
                );
            }
        }

        let Some(cap) = style.cap else {
            mesh.quad(top_face(xs, top, zs), [0.0, 1.0, 0.0], [body; 4]);
            continue;
        };
        // The cap's footprint grows by the overhang on each exposed side.
        let [east, west, south, north] = exposed.map(|open| if open { cap.overhang } else { 0.0 });
        let cap_xs = (xs.0 - west, xs.1 + east);
        let cap_zs = (zs.0 - north, zs.1 + south);
        let lit = shade(cap.color, 1.0 + style.jitter * 0.5 * tile_noise(row, col));
        mesh.quad(top_face(cap_xs, top, cap_zs), [0.0, 1.0, 0.0], [lit; 4]);
        let rim = shade(lit, 0.85);
        for (side, open) in SIDES.into_iter().zip(exposed) {
            if open {
                mesh.quad(
                    side_face(side, cap_xs, (body_top, top), cap_zs),
                    side.normal(),
                    [rim; 4],
                );
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: WallStyle = WallStyle {
        height: 1.0,
        body_color: [0.5, 0.5, 0.5, 1.0],
        cap: None,
        jitter: 0.0,
        foot_shade: 0.5,
    };

    fn walls(cells: &[(i32, i32)], style: &WallStyle) -> WallMesh {
        let set: HashSet<(i32, i32)> = cells.iter().copied().collect();
        build_walls(&set, |c| set.contains(&c), 1.0, 0.0, style)
    }

    fn quads(mesh: &WallMesh) -> usize {
        mesh.indices.len() / 6
    }

    #[test]
    fn a_lone_tile_is_a_box_without_a_bottom() {
        assert_eq!(quads(&walls(&[(0, 0)], &STYLE)), 5);
    }

    #[test]
    fn the_side_between_two_wall_tiles_is_not_drawn() {
        // Two boxes would be 10 quads; the shared side goes on both, leaving 8.
        assert_eq!(quads(&walls(&[(0, 0), (1, 0)], &STYLE)), 8);
    }

    #[test]
    fn every_triangle_faces_the_way_its_normal_says() {
        let style = WallStyle {
            cap: Some(WallCap {
                thickness: 0.1,
                overhang: 0.05,
                color: [0.8, 0.8, 0.8, 1.0],
            }),
            ..STYLE
        };
        // An L, so outer and inner corners are both covered.
        let mesh = walls(&[(0, 0), (1, 0), (1, 1)], &style);
        assert!(!mesh.is_empty());
        for tri in mesh.indices.chunks(3) {
            let at = |i: u32| bevy::math::Vec3::from(mesh.positions[i as usize]);
            let corners = [at(tri[0]), at(tri[1]), at(tri[2])];
            let normal = bevy::math::Vec3::from(mesh.normals[tri[0] as usize]);
            let winding = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            assert!(
                winding.dot(normal) > 0.0,
                "counter-clockwise from the outside: {corners:?} normal={normal}"
            );
        }
    }

    #[test]
    fn the_cap_overhangs_only_where_the_wall_is_exposed() {
        let style = WallStyle {
            cap: Some(WallCap {
                thickness: 0.1,
                overhang: 0.05,
                color: [0.8, 0.8, 0.8, 1.0],
            }),
            ..STYLE
        };
        let mesh = walls(&[(0, 0), (1, 0)], &style);
        let max_x = mesh.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let max_z = mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!((max_x - 1.55).abs() < 1e-5, "sticks out past the run's end");
        assert!((max_z - 0.55).abs() < 1e-5, "and along its sides");
        // No cap edge in the middle of the run, where the two tiles meet.
        assert!(
            !mesh
                .positions
                .iter()
                .any(|p| (p[0] - 0.45).abs() < 1e-5 || (p[0] - 0.55).abs() < 1e-5),
            "the join between tiles has no overhang"
        );
    }

    #[test]
    fn a_hidden_side_is_skipped_even_against_another_category() {
        let set: HashSet<(i32, i32)> = [(0, 0)].into_iter().collect();
        // Pretend a taller wall stands to the east.
        let mesh = build_walls(&set, |c| c == (1, 0), 1.0, 0.0, &STYLE);
        assert_eq!(quads(&mesh), 4);
    }

    #[test]
    fn the_jitter_is_stable_and_bounded() {
        for (x, y) in [(0, 0), (5, -3), (1000, 77)] {
            let n = tile_noise(x, y);
            assert!((-1.0..=1.0).contains(&n));
            assert!((n - tile_noise(x, y)).abs() < f32::EPSILON);
        }
    }
}
