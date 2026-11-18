//! Terraced houses (Swedish *radhus*), drawn from the real outlines an OSM import keeps
//! (`MapFile::buildings`) instead of the wall blocks their tiles would get.
//!
//! The model is the 1969-70 rows of the `gis` campaign's town: houses about 8 m wide and
//! 11 m deep, joined three to seven in a row, with a low single-pitch (skillion) roof,
//! rendered concrete ("puts") and wooden panelling, in one of two looks:
//!
//! - **red**: light grey render below, red board-and-batten above, light grey dividing
//!   walls between the houses;
//! - **brown**: cream render, a tall dark brown board band under the roof edge, a
//!   chimney, and a little gabled porch over the front door with a white fenced patio.
//!
//! and one of two sections, picked from the ground:
//!
//! - **souterrain**: the path side is the upper floor, the ground falls away behind and the
//!   lower floor opens onto the garden;
//! - **two storeys**: on level ground, the ground floor opens onto the path.
//!
//! Pure planning and geometry, no ECS: `streaming` turns [`build_houses`] into chunk
//! meshes, `map_systems::map_loader` gives each house a collider along its outline
//! ([`footprints`]), and `walls::plan_walls` leaves out the tiles a house stands on
//! ([`covered_tiles`]). The tiles stay walls for the pathfinding.

use std::collections::{HashMap, HashSet};

use bevy::math::{Vec2, Vec3};

use super::streaming::{Chunk, chunk_of};
use super::wall_mesh::WallMesh;
use crate::general::components::map_components::{BuildingKind, BuildingOutline, RoadLine};

/// One storey, floor to floor, in world units (a player is about 0.5 tall).
pub const STOREY: f32 = 0.75;
/// How much higher the roof is at the front than at the back.
const ROOF_RISE: f32 = 0.1;
const ROOF_THICKNESS: f32 = 0.07;
/// Roof overhang at the front and back, and at the gable of a row's end house.
const EAVE: f32 = 0.14;
const GABLE_EAVE: f32 = 0.06;
/// Ground this much lower behind a row than in front of it makes it a souterrain row.
pub const SOUTERRAIN_DROP: f32 = 0.25;
/// Corners this close (in tiles) are the same corner: the houses share a wall.
const SHARED_CORNER: f32 = 0.2;
/// Front garden depth, back garden depth.
const FRONT_YARD: f32 = 0.6;
const BACK_YARD: f32 = 1.3;

type Color = [f32; 4];
const TRIM: Color = [0.92, 0.92, 0.89, 1.0];
const GLASS: Color = [0.1, 0.12, 0.16, 1.0];
const ROOF: Color = [0.16, 0.16, 0.17, 1.0];
/// The seams between the asphalt sheets, and how far apart they are.
const ROOF_SEAM: Color = [0.24, 0.24, 0.25, 1.0];
const SEAM_SPACING: f32 = 0.3;
const GUTTER: Color = [0.07, 0.07, 0.08, 1.0];
const HEDGE: Color = [0.16, 0.3, 0.14, 1.0];
const FLAGSTONE: Color = [0.55, 0.54, 0.5, 1.0];
const CHIMNEY: Color = [0.2, 0.2, 0.22, 1.0];
const LAWN: Color = [0.3, 0.42, 0.2, 1.0];
const RETAINING_WALL: Color = [0.5, 0.49, 0.46, 1.0];

/// A row's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Red,
    Brown,
}

impl Scheme {
    /// Lower storey, upper storey, the band under the roof (`None`: none), the fascia, the
    /// dividing walls, the front door.
    const fn colors(self) -> SchemeColors {
        match self {
            Self::Red => SchemeColors {
                lower: [0.76, 0.77, 0.76, 1.0],
                upper: [0.5, 0.12, 0.1, 1.0],
                band: None,
                fascia: [0.46, 0.11, 0.09, 1.0],
                divider: [0.8, 0.8, 0.79, 1.0],
                door: TRIM,
                fence: [0.88, 0.88, 0.85, 1.0],
            },
            Self::Brown => SchemeColors {
                lower: [0.86, 0.83, 0.74, 1.0],
                upper: [0.86, 0.83, 0.74, 1.0],
                band: Some([0.27, 0.18, 0.13, 1.0]),
                fascia: [0.24, 0.16, 0.12, 1.0],
                divider: [0.82, 0.79, 0.7, 1.0],
                door: [0.42, 0.24, 0.13, 1.0],
                fence: [0.9, 0.9, 0.88, 1.0],
            },
        }
    }
}

struct SchemeColors {
    lower: Color,
    upper: Color,
    band: Option<Color>,
    fascia: Color,
    divider: Color,
    door: Color,
    fence: Color,
}

/// Which way the ground goes under a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// The path side is the upper floor; the lower floor opens onto the garden.
    Souterrain,
    /// Two storeys on level ground, the ground floor on the path.
    TwoStorey,
}

/// A building's outline as a rectangle in world space (xz).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub centre: Vec2,
    /// Unit vectors along the two sides: `a` is `size.x` long, `b` is `size.y`.
    pub a: Vec2,
    pub b: Vec2,
    pub size: Vec2,
}

impl Footprint {
    /// The smallest rectangle round the points, its sides along one of their edges.
    pub fn of(points: &[Vec2]) -> Option<Self> {
        let mut best: Option<(f32, Self)> = None;
        for (i, &p) in points.iter().enumerate() {
            let q = points.get((i + 1) % points.len()).copied()?;
            let a = (q - p).normalize_or_zero();
            if a == Vec2::ZERO {
                continue;
            }
            let b = a.perp();
            let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
            for &r in points {
                let local = Vec2::new(r.dot(a), r.dot(b));
                lo = lo.min(local);
                hi = hi.max(local);
            }
            let size = hi - lo;
            let mid = (lo + hi) / 2.0;
            let rect = Self {
                centre: a * mid.x + b * mid.y,
                a,
                b,
                size,
            };
            if best.is_none_or(|(area, _)| size.x * size.y < area) {
                best = Some((size.x * size.y, rect));
            }
        }
        best.map(|(_, rect)| rect)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        let d = p - self.centre;
        d.dot(self.a).abs() <= self.size.x / 2.0 && d.dot(self.b).abs() <= self.size.y / 2.0
    }

    /// Its rotation about +Y for a box whose local x runs along `a` and z along `b`.
    pub fn yaw(&self) -> f32 {
        // Local +X goes to (cos, -sin) in xz under a yaw of `angle`.
        (-self.a.y).atan2(self.a.x)
    }
}

/// Each building's footprint in world space, with the buildings' map coordinates
/// (`tile` world units each, one tile in from the padded grid).
pub fn footprints(buildings: &[BuildingOutline], tile: f32) -> Vec<Option<Footprint>> {
    buildings
        .iter()
        .map(|b| {
            let points: Vec<Vec2> = b
                .outline
                .iter()
                .map(|&[x, y]| Vec2::new(tile * (x + 1.0), tile * (y + 1.0)))
                .collect();
            Footprint::of(&points)
        })
        .collect()
}

/// The padded-grid tiles whose centre a building stands on: they are drawn as the house.
pub fn covered_tiles(buildings: &[BuildingOutline]) -> HashSet<(i32, i32)> {
    let mut covered = HashSet::new();
    for footprint in footprints(buildings, 1.0).into_iter().flatten() {
        let reach = footprint.size.length() / 2.0 + 1.0;
        let (lo, hi) = (footprint.centre - reach, footprint.centre + reach);
        for row in lo.y.floor() as i32..=hi.y.ceil() as i32 {
            for col in lo.x.floor() as i32..=hi.x.ceil() as i32 {
                if footprint.contains(Vec2::new(col as f32, row as f32)) {
                    covered.insert((col, row));
                }
            }
        }
    }
    covered
}

/// One house of a row, ready to build.
#[derive(Clone, Debug, PartialEq)]
pub struct House {
    pub centre: Vec2,
    /// Along the row, and out of the front (the path side).
    pub along: Vec2,
    pub out: Vec2,
    pub width: f32,
    pub depth: f32,
    /// World height of the path-side floor (the front door's threshold).
    pub floor: f32,
    /// The lowest ground under it: the walls reach down to here.
    pub foot: f32,
    pub section: Section,
    pub scheme: Scheme,
    /// No neighbour on its own left / right (seen from the front): a gable end.
    pub end_left: bool,
    pub end_right: bool,
    /// Its layout mirrored (rows alternate).
    pub mirrored: bool,
}

/// What the planner needs to know about the world.
pub struct Ground<'a> {
    /// World height of the ground at a point.
    pub height: &'a dyn Fn(Vec2) -> f32,
    pub tile: f32,
}

/// Plan the houses: rows from shared corners, each row's section, front and look.
pub fn plan_houses(
    buildings: &[BuildingOutline],
    roads: &[RoadLine],
    ground: &Ground,
) -> Vec<House> {
    let tile = ground.tile;
    let prints: Vec<(usize, Footprint)> = footprints(buildings, tile)
        .into_iter()
        .enumerate()
        .filter_map(|(i, f)| f.map(|f| (i, f)))
        .filter(|(i, _)| {
            buildings
                .get(*i)
                .is_some_and(|b| b.kind == BuildingKind::Terrace)
        })
        .collect();
    let corners: Vec<Vec<Vec2>> = prints
        .iter()
        .map(|(i, _)| {
            buildings.get(*i).map_or_else(Vec::new, |b| {
                b.outline
                    .iter()
                    .map(|&[x, y]| Vec2::new(tile * (x + 1.0), tile * (y + 1.0)))
                    .collect()
            })
        })
        .collect();

    // Neighbours share a corner; rows are what neighbours join.
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); prints.len()];
    let near = SHARED_CORNER * tile;
    for i in 0..prints.len() {
        for j in i + 1..prints.len() {
            let (Some(ci), Some(cj)) = (corners.get(i), corners.get(j)) else {
                continue;
            };
            let touch = ci.iter().any(|p| cj.iter().any(|q| p.distance(*q) < near));
            if touch {
                if let Some(n) = neighbours.get_mut(i) {
                    n.push(j);
                }
                if let Some(n) = neighbours.get_mut(j) {
                    n.push(i);
                }
            }
        }
    }
    let mut row_of = vec![usize::MAX; prints.len()];
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for start in 0..prints.len() {
        if row_of.get(start) != Some(&usize::MAX) {
            continue;
        }
        let id = rows.len();
        let mut members = vec![start];
        if let Some(r) = row_of.get_mut(start) {
            *r = id;
        }
        let mut next = 0;
        while let Some(&at) = members.get(next) {
            next += 1;
            for &n in neighbours.get(at).into_iter().flatten() {
                if row_of.get(n) == Some(&usize::MAX) {
                    if let Some(r) = row_of.get_mut(n) {
                        *r = id;
                    }
                    members.push(n);
                }
            }
        }
        rows.push(members);
    }

    let mut houses = Vec::new();
    for members in &rows {
        let Some(&(_, first)) = members.first().and_then(|&m| prints.get(m)) else {
            continue;
        };
        // The row runs along the side facing a neighbour, or the house's shorter side.
        let row_axis = members
            .iter()
            .find_map(|&m| {
                let (_, here) = prints.get(m)?;
                let &n = neighbours.get(m)?.first()?;
                let (_, there) = prints.get(n)?;
                Some((there.centre - here.centre).normalize_or_zero())
            })
            .unwrap_or(if first.size.x <= first.size.y {
                first.a
            } else {
                first.b
            });
        let across = row_axis.perp();

        // Each house's sides, its depth axis pointing `across`.
        let shaped: Vec<(Footprint, Vec2, Vec2, f32, f32)> = members
            .iter()
            .filter_map(|&m| prints.get(m).map(|(_, f)| *f))
            .map(|f| {
                let (along, width, depth) = if f.a.dot(row_axis).abs() >= f.b.dot(row_axis).abs() {
                    (f.a, f.size.x, f.size.y)
                } else {
                    (f.b, f.size.y, f.size.x)
                };
                let along = if along.dot(row_axis) < 0.0 {
                    -along
                } else {
                    along
                };
                (f, along, along.perp(), width, depth)
            })
            .map(|(f, along, out, w, d)| {
                let out = if out.dot(across) < 0.0 { -out } else { out };
                (f, along, out, w, d)
            })
            .collect();

        // Which side is the front: the high side of a slope, else the one nearer a path.
        let side_height = |sign: f32| -> f32 {
            let samples: Vec<f32> = shaped
                .iter()
                .map(|(f, _, out, _, d)| {
                    (ground.height)(f.centre + *out * sign * (d / 2.0 + 0.6 * tile))
                })
                .collect();
            samples.iter().sum::<f32>() / samples.len().max(1) as f32
        };
        let (plus, minus) = (side_height(1.0), side_height(-1.0));
        let drop = plus - minus;
        let (section, sign) = if drop.abs() >= SOUTERRAIN_DROP {
            (Section::Souterrain, if drop > 0.0 { 1.0 } else { -1.0 })
        } else {
            let road_distance = |sign: f32| -> f32 {
                shaped
                    .iter()
                    .map(|(f, _, out, _, d)| {
                        distance_to_roads(f.centre + *out * sign * d / 2.0, roads, tile)
                    })
                    .fold(f32::MAX, f32::min)
            };
            let sign = if road_distance(1.0) <= road_distance(-1.0) {
                1.0
            } else {
                -1.0
            };
            (Section::TwoStorey, sign)
        };
        let scheme = if row_hash(first.centre) % 2 == 0 {
            Scheme::Red
        } else {
            Scheme::Brown
        };

        // Order along the row to alternate the layouts and find the ends.
        let mut order: Vec<usize> = (0..shaped.len()).collect();
        order.sort_by(|&x, &y| {
            let at = |k: usize| shaped.get(k).map_or(0.0, |s| s.0.centre.dot(row_axis));
            at(x).total_cmp(&at(y))
        });
        for (rank, &k) in order.iter().enumerate() {
            let Some(&(f, along, out, width, depth)) = shaped.get(k) else {
                continue;
            };
            let out = out * sign;
            // Seen from the front (looking along -out), right is (out.y, -out.x).
            let right = Vec2::new(out.y, -out.x);
            let along = if along.dot(right) < 0.0 {
                -along
            } else {
                along
            };
            let has_side = |dir: f32| {
                shaped.iter().any(|(g, ..)| {
                    let d = g.centre - f.centre;
                    d.length() > 0.1 && d.length() < width * 1.6 && d.dot(along) * dir > 0.0
                })
            };
            let corners_of = |f: &Footprint| {
                [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .map(|(x, y)| f.centre + f.a * x * f.size.x / 2.0 + f.b * y * f.size.y / 2.0)
            };
            let under: Vec<f32> = corners_of(&f)
                .iter()
                .chain(std::iter::once(&f.centre))
                .map(|p| (ground.height)(*p))
                .collect();
            let lowest = under.iter().copied().fold(f32::MAX, f32::min);
            let highest = under.iter().copied().fold(f32::MIN, f32::max);
            let floor = match section {
                // The path-side ground, a little above the highest of it.
                Section::Souterrain => {
                    let front = [-0.4, 0.0, 0.4].map(|t| {
                        (ground.height)(f.centre + out * (depth / 2.0) + along * width * t)
                    });
                    front.iter().copied().fold(f32::MIN, f32::max) + 0.03
                }
                Section::TwoStorey => highest + 0.03,
            };
            houses.push(House {
                centre: f.centre,
                along,
                out,
                width,
                depth,
                floor,
                foot: lowest.min(floor) - 0.05,
                section,
                scheme,
                end_left: !has_side(-1.0),
                end_right: !has_side(1.0),
                mirrored: rank % 2 == 1,
            });
        }
    }
    houses
}

/// A stable small hash of a position, to pick a row's look.
fn row_hash(p: Vec2) -> u32 {
    let x = (p.x * 10.0).round() as i32 as u32;
    let y = (p.y * 10.0).round() as i32 as u32;
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE3D);
    h ^ (h >> 13)
}

/// Distance from a world point to the nearest road centreline, in world units.
fn distance_to_roads(p: Vec2, roads: &[RoadLine], tile: f32) -> f32 {
    let world = |q: &[f32; 2]| Vec2::new(tile * (q[0] + 1.0), tile * (q[1] + 1.0));
    roads
        .iter()
        .flat_map(|r| r.points.windows(2))
        .filter_map(|w| Some((world(w.first()?), world(w.get(1)?))))
        .map(|(a, b)| {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f32::MAX, f32::min)
}

/// Is a garden piece free: on no other building and off the roads.
pub struct Clearance<'a> {
    pub prints: &'a [Footprint],
    pub roads: &'a [RoadLine],
    pub tile: f32,
}

impl Clearance<'_> {
    fn clear(&self, p: Vec2, own: Vec2) -> bool {
        let on_road = self.roads.iter().any(|r| {
            distance_to_roads(p, std::slice::from_ref(r), self.tile)
                < r.width * self.tile / 2.0 + 0.15
        });
        let on_building = self
            .prints
            .iter()
            .any(|f| f.centre.distance(own) > 0.05 && f.contains(p));
        !on_road && !on_building
    }
}

/// A house in its own space: x along the row (from the front, left to right), y up
/// (world), z out of the front.
struct Frame {
    origin: Vec2,
    along: Vec2,
    out: Vec2,
}

impl Frame {
    fn at(&self, x: f32, y: f32, z: f32) -> Vec3 {
        let p = self.origin + self.along * x + self.out * z;
        Vec3::new(p.x, y, p.y)
    }

    fn dir(&self, x: f32, y: f32, z: f32) -> Vec3 {
        let p = self.along * x + self.out * z;
        Vec3::new(p.x, y, p.y).normalize_or_zero()
    }

    fn flat(&self, x: f32, z: f32) -> Vec2 {
        self.origin + self.along * x + self.out * z
    }
}

/// A quad from four corners in order round it, facing `normal`.
fn quad(mesh: &mut WallMesh, corners: [Vec3; 4], normal: Vec3, color: Color) {
    let Ok(base) = u32::try_from(mesh.positions.len()) else {
        return;
    };
    for c in corners {
        mesh.positions.push(c.to_array());
        mesh.normals.push(normal.to_array());
        mesh.colors.push(color);
    }
    let [a, b, c, _] = corners;
    let facing = (b - a).cross(c - a).dot(normal) >= 0.0;
    let order: [u32; 6] = if facing {
        [0, 1, 2, 0, 2, 3]
    } else {
        [0, 2, 1, 0, 3, 2]
    };
    mesh.indices.extend(order.iter().map(|i| base + i));
}

/// A box in a house's space, all sides but the bottom.
fn cuboid(mesh: &mut WallMesh, frame: &Frame, lo: Vec3, hi: Vec3, color: Color) {
    let p = |x: f32, y: f32, z: f32| frame.at(x, y, z);
    let shade = |c: Color, k: f32| [c[0] * k, c[1] * k, c[2] * k, c[3]];
    quad(
        mesh,
        [
            p(lo.x, hi.y, lo.z),
            p(hi.x, hi.y, lo.z),
            p(hi.x, hi.y, hi.z),
            p(lo.x, hi.y, hi.z),
        ],
        Vec3::Y,
        color,
    );
    let side = shade(color, 0.92);
    quad(
        mesh,
        [
            p(lo.x, lo.y, hi.z),
            p(hi.x, lo.y, hi.z),
            p(hi.x, hi.y, hi.z),
            p(lo.x, hi.y, hi.z),
        ],
        frame.dir(0.0, 0.0, 1.0),
        side,
    );
    quad(
        mesh,
        [
            p(lo.x, lo.y, lo.z),
            p(hi.x, lo.y, lo.z),
            p(hi.x, hi.y, lo.z),
            p(lo.x, hi.y, lo.z),
        ],
        frame.dir(0.0, 0.0, -1.0),
        side,
    );
    quad(
        mesh,
        [
            p(hi.x, lo.y, lo.z),
            p(hi.x, lo.y, hi.z),
            p(hi.x, hi.y, hi.z),
            p(hi.x, hi.y, lo.z),
        ],
        frame.dir(1.0, 0.0, 0.0),
        side,
    );
    quad(
        mesh,
        [
            p(lo.x, lo.y, lo.z),
            p(lo.x, lo.y, hi.z),
            p(lo.x, hi.y, hi.z),
            p(lo.x, hi.y, lo.z),
        ],
        frame.dir(-1.0, 0.0, 0.0),
        side,
    );
}

/// A window or door on a facade.
#[derive(Clone, Copy)]
enum Opening {
    /// Centre across the facade (fraction of the width, from its left seen from outside),
    /// width, sill above the storey's floor, height.
    Window(f32, f32, f32, f32),
    Door(f32),
}

const DOOR_WIDTH: f32 = 0.26;
const DOOR_HEIGHT: f32 = 0.55;
const FRAME: f32 = 0.035;

/// Which facade: the front (`z = depth / 2`) or the back.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    Front,
    Back,
}

struct HouseBuilder<'a> {
    mesh: &'a mut WallMesh,
    frame: Frame,
    house: &'a House,
    colors: SchemeColors,
}

impl HouseBuilder<'_> {
    fn half(&self) -> (f32, f32) {
        (self.house.width / 2.0, self.house.depth / 2.0)
    }

    /// The lower storey's floor; the upper one's is a storey up.
    fn base(&self) -> f32 {
        match self.house.section {
            Section::Souterrain => self.house.floor - STOREY,
            Section::TwoStorey => self.house.floor,
        }
    }

    fn top(&self) -> f32 {
        self.base() + 2.0 * STOREY
    }

    /// The local x of a point `u` (fraction from the left, seen from outside) on a facade.
    fn facade_x(&self, face: Face, u: f32) -> f32 {
        let (hw, _) = self.half();
        let u = if self.house.mirrored { 1.0 - u } else { u };
        let x = -hw + u * self.house.width;
        // From behind, left and right swap.
        if face == Face::Front { x } else { -x }
    }

    fn face_z(&self, face: Face, offset: f32) -> f32 {
        let (_, hd) = self.half();
        if face == Face::Front {
            hd + offset
        } else {
            -hd - offset
        }
    }

    fn face_normal(&self, face: Face) -> Vec3 {
        self.frame
            .dir(0.0, 0.0, if face == Face::Front { 1.0 } else { -1.0 })
    }

    /// A flat rectangle on a facade, `offset` out from the wall.
    /// `[x0, x1, y0, y1]`.
    fn panel(&mut self, face: Face, [x0, x1, y0, y1]: [f32; 4], offset: f32, color: Color) {
        let z = self.face_z(face, offset);
        let corners = [
            self.frame.at(x0, y0, z),
            self.frame.at(x1, y0, z),
            self.frame.at(x1, y1, z),
            self.frame.at(x0, y1, z),
        ];
        let normal = self.face_normal(face);
        quad(self.mesh, corners, normal, color);
    }

    /// The walls: front and back in bands (plinth, lower storey, upper storey, the brown
    /// band), the gable walls at a row's end or where a neighbour stands lower.
    fn walls(&mut self) {
        let (hw, hd) = self.half();
        let (foot, base, top) = (self.house.foot, self.base(), self.top());
        let plinth = shade(self.colors.lower, 0.8);
        let mut bands = vec![
            (foot, base, plinth),
            (base, base + STOREY, self.colors.lower),
        ];
        match self.colors.band {
            Some(band) => {
                bands.push((base + STOREY, top - 0.3, self.colors.upper));
                bands.push((top - 0.3, top, band));
            }
            None => bands.push((base + STOREY, top, self.colors.upper)),
        }
        for face in [Face::Front, Face::Back] {
            for &(y0, y1, color) in &bands {
                if y1 > y0 {
                    self.panel(face, [-hw, hw, y0, y1], 0.0, color);
                }
            }
        }
        // Gable walls, both sides always: a neighbour hides its own side, and where rows
        // step up a slope the higher house's shows above the lower one.
        for sx in [-1.0_f32, 1.0] {
            let x = sx * hw;
            let normal = self.frame.dir(sx, 0.0, 0.0);
            for &(y0, y1, color) in &bands {
                if y1 > y0 {
                    let corners = [
                        self.frame.at(x, y0, -hd),
                        self.frame.at(x, y0, hd),
                        self.frame.at(x, y1, hd),
                        self.frame.at(x, y1, -hd),
                    ];
                    quad(self.mesh, corners, normal, shade(color, 0.95));
                }
            }
        }
        // Dividing walls stand a little proud between the houses; each house draws the
        // one on its right.
        let divider = self.colors.divider;
        if !self.house.end_right {
            for face in [Face::Front, Face::Back] {
                let x = hw;
                let (z0, z1) = match face {
                    Face::Front => (hd - 0.01, hd + 0.05),
                    Face::Back => (-hd - 0.05, -hd + 0.01),
                };
                cuboid(
                    self.mesh,
                    &self.frame,
                    Vec3::new(x - 0.07, foot, z0),
                    Vec3::new(x + 0.07, top + 0.02, z1),
                    divider,
                );
            }
        }
    }

    /// The skillion roof: high at the front, the fascia round it, a gutter at the back.
    fn roof(&mut self) {
        let (hw, hd) = self.half();
        let top = self.top();
        let left = -hw - if self.house.end_left { GABLE_EAVE } else { 0.0 };
        let right = hw
            + if self.house.end_right {
                GABLE_EAVE
            } else {
                0.0
            };
        let (z0, z1) = (-hd - EAVE, hd + EAVE);
        let y_at = |z: f32| top + ROOF_RISE * (z + hd) / (2.0 * hd);
        let (b0, b1) = (y_at(z0), y_at(z1));
        let f = &self.frame;
        let up = Vec3::new(0.0, 1.0, 0.0);
        let slope = (f.at(0.0, b1, z1) - f.at(0.0, b0, z0)).normalize_or_zero();
        let normal = f.dir(1.0, 0.0, 0.0).cross(slope).normalize_or_zero();
        let normal = if normal.dot(up) < 0.0 {
            -normal
        } else {
            normal
        };
        let t = ROOF_THICKNESS;
        quad(
            self.mesh,
            [
                f.at(left, b0 + t, z0),
                f.at(right, b0 + t, z0),
                f.at(right, b1 + t, z1),
                f.at(left, b1 + t, z1),
            ],
            normal,
            ROOF,
        );
        // Seams down the slope between the sheets, just proud of the roof.
        let lift = normal * 0.004;
        let mut x = left + SEAM_SPACING;
        while x < right - 0.05 {
            let seam = [
                f.at(x - 0.012, b0 + t, z0) + lift,
                f.at(x + 0.012, b0 + t, z0) + lift,
                f.at(x + 0.012, b1 + t, z1) + lift,
                f.at(x - 0.012, b1 + t, z1) + lift,
            ];
            quad(self.mesh, seam, normal, ROOF_SEAM);
            x += SEAM_SPACING;
        }
        // The dividing wall's cap across the roof, marking each house off from above.
        if !self.house.end_right {
            let cap = self.colors.divider;
            let (c0, c1) = (b0 + t + 0.035, b1 + t + 0.035);
            let side = |sx: f32| {
                [
                    f.at(hw + sx * 0.05, b0 - 0.02, z0),
                    f.at(hw + sx * 0.05, b1 - 0.02, z1),
                    f.at(hw + sx * 0.05, c1, z1),
                    f.at(hw + sx * 0.05, c0, z0),
                ]
            };
            quad(self.mesh, side(1.0), f.dir(1.0, 0.0, 0.0), shade(cap, 0.9));
            quad(
                self.mesh,
                side(-1.0),
                f.dir(-1.0, 0.0, 0.0),
                shade(cap, 0.9),
            );
            let top = [
                f.at(hw - 0.05, c0, z0),
                f.at(hw + 0.05, c0, z0),
                f.at(hw + 0.05, c1, z1),
                f.at(hw - 0.05, c1, z1),
            ];
            quad(self.mesh, top, normal, cap);
        }
        // The fascia: front, back and the two ends.
        let fascia = self.colors.fascia;
        let front = [
            f.at(left, b1 - 0.02, z1),
            f.at(right, b1 - 0.02, z1),
            f.at(right, b1 + t, z1),
            f.at(left, b1 + t, z1),
        ];
        quad(self.mesh, front, f.dir(0.0, 0.0, 1.0), fascia);
        let back = [
            f.at(left, b0 - 0.02, z0),
            f.at(right, b0 - 0.02, z0),
            f.at(right, b0 + t, z0),
            f.at(left, b0 + t, z0),
        ];
        quad(self.mesh, back, f.dir(0.0, 0.0, -1.0), fascia);
        for (x, sx) in [(left, -1.0_f32), (right, 1.0)] {
            let side = [
                f.at(x, b0 - 0.02, z0),
                f.at(x, b1 - 0.02, z1),
                f.at(x, b1 + t, z1),
                f.at(x, b0 + t, z0),
            ];
            quad(self.mesh, side, f.dir(sx, 0.0, 0.0), shade(fascia, 0.9));
        }
        // The soffit, seen from low down (the intro's copter).
        quad(
            self.mesh,
            [
                f.at(left, b0 - 0.02, z0),
                f.at(right, b0 - 0.02, z0),
                f.at(right, b1 - 0.02, z1),
                f.at(left, b1 - 0.02, z1),
            ],
            -normal,
            shade(fascia, 0.6),
        );
        // A gutter along the low edge.
        cuboid(
            self.mesh,
            &self.frame,
            Vec3::new(left, b0 - 0.07, z0 - 0.04),
            Vec3::new(right, b0 - 0.01, z0),
            GUTTER,
        );
        if self.house.scheme == Scheme::Brown {
            let x = self.facade_x(Face::Front, 0.22);
            let z = hd - 0.6;
            cuboid(
                self.mesh,
                &self.frame,
                Vec3::new(x - 0.12, y_at(z) - 0.05, z - 0.12),
                Vec3::new(x + 0.12, y_at(z) + 0.42, z + 0.12),
                CHIMNEY,
            );
        }
    }

    fn opening(&mut self, face: Face, storey: usize, opening: Opening) {
        let floor = self.base() + STOREY * storey as f32;
        let (u, width, sill, height, fill) = match opening {
            Opening::Window(u, w, sill, h) => (u, w, sill, h, GLASS),
            Opening::Door(u) => (u, DOOR_WIDTH, 0.0, DOOR_HEIGHT, self.colors.door),
        };
        let x = self.facade_x(face, u);
        let (x0, x1) = (x - width / 2.0, x + width / 2.0);
        let (y0, y1) = (floor + sill, floor + sill + height);
        self.panel(
            face,
            [x0 - FRAME, x1 + FRAME, y0 - FRAME, y1 + FRAME],
            0.012,
            TRIM,
        );
        self.panel(face, [x0, x1, y0, y1], 0.02, fill);
        if let Opening::Window(_, w, ..) = opening
            && w > 0.45
        {
            // Mullions split a wide window in three.
            for k in [1.0_f32, 2.0] {
                let mx = x0 + w * k / 3.0;
                self.panel(face, [mx - 0.015, mx + 0.015, y0, y1], 0.024, TRIM);
            }
        }
    }

    fn openings(&mut self) {
        let (path_storey, garden_storey) = match self.house.section {
            Section::Souterrain => (1, 0),
            Section::TwoStorey => (0, 0),
        };
        // The path side: the door and two windows.
        self.opening(Face::Front, path_storey, Opening::Door(0.42));
        self.opening(
            Face::Front,
            path_storey,
            Opening::Window(0.17, 0.3, 0.22, 0.33),
        );
        self.opening(
            Face::Front,
            path_storey,
            Opening::Window(0.72, 0.42, 0.22, 0.33),
        );
        if self.house.section == Section::TwoStorey {
            for u in [0.2, 0.5, 0.8] {
                self.opening(Face::Front, 1, Opening::Window(u, 0.32, 0.25, 0.32));
            }
        }
        // The garden side: a wide window, the door, a window below; three above.
        self.opening(
            Face::Back,
            garden_storey,
            Opening::Window(0.25, 0.62, 0.18, 0.36),
        );
        self.opening(Face::Back, garden_storey, Opening::Door(0.56));
        self.opening(
            Face::Back,
            garden_storey,
            Opening::Window(0.8, 0.3, 0.18, 0.36),
        );
        for u in [0.2, 0.5, 0.8] {
            self.opening(Face::Back, 1, Opening::Window(u, 0.32, 0.25, 0.32));
        }
        // The gable at a row's end: a window upstairs, a small one low on a souterrain.
        let (hw, _) = self.half();
        for (sx, end) in [(-1.0_f32, self.house.end_left), (1.0, self.house.end_right)] {
            if !end {
                continue;
            }
            let upstairs = self.base() + STOREY;
            self.gable_window(sx * hw, -0.15, upstairs + 0.25, 0.28, 0.32);
            if self.house.section == Section::Souterrain {
                self.gable_window(sx * hw, -0.9, self.base() + 0.4, 0.22, 0.14);
            }
        }
        // A little flat canopy over the garden door.
        let x = self.facade_x(Face::Back, 0.56);
        let y = self.base() + STOREY * garden_storey as f32 + DOOR_HEIGHT + 0.06;
        let z = self.face_z(Face::Back, 0.0);
        cuboid(
            self.mesh,
            &self.frame,
            Vec3::new(x - 0.22, y, z - 0.22),
            Vec3::new(x + 0.22, y + 0.03, z),
            TRIM,
        );
    }

    /// A window on the gable wall at local `x` (the house's side), centred at local `z`.
    fn gable_window(&mut self, x: f32, z: f32, y0: f32, width: f32, height: f32) {
        let sx = x.signum();
        let normal = self.frame.dir(sx, 0.0, 0.0);
        let f = &self.frame;
        let rect = |off: f32, z0: f32, z1: f32, y0: f32, y1: f32| {
            let x = x + sx * off;
            [
                f.at(x, y0, z0),
                f.at(x, y0, z1),
                f.at(x, y1, z1),
                f.at(x, y1, z0),
            ]
        };
        let (z0, z1) = (z - width / 2.0, z + width / 2.0);
        let frame = rect(
            0.012,
            z0 - FRAME,
            z1 + FRAME,
            y0 - FRAME,
            y0 + height + FRAME,
        );
        let glass = rect(0.02, z0, z1, y0, y0 + height);
        quad(self.mesh, frame, normal, TRIM);
        quad(self.mesh, glass, normal, GLASS);
    }

    /// The brown rows' gabled porch over the front door.
    fn porch(&mut self) {
        if self.house.scheme != Scheme::Brown {
            return;
        }
        let storey = usize::from(self.house.section == Section::Souterrain);
        let floor = self.base() + STOREY * storey as f32;
        let x = self.facade_x(Face::Front, 0.42);
        let z = self.face_z(Face::Front, 0.0);
        let (half_w, deep, eave) = (0.24, 0.32, floor + DOOR_HEIGHT + 0.1);
        let wood = self.colors.band.unwrap_or(ROOF);
        // The gable: a triangle of brown boarding on white bargeboards, a little roof.
        for sx in [-1.0_f32, 1.0] {
            cuboid(
                self.mesh,
                &self.frame,
                Vec3::new(x + sx * half_w - 0.025, floor, z + deep - 0.05),
                Vec3::new(x + sx * half_w + 0.025, eave, z + deep),
                TRIM,
            );
        }
        let f = &self.frame;
        let ridge = eave + 0.16;
        let gable = [
            f.at(x - half_w - 0.03, eave, z + deep + 0.01),
            f.at(x + half_w + 0.03, eave, z + deep + 0.01),
            f.at(x, ridge, z + deep + 0.01),
            f.at(x, ridge, z + deep + 0.01),
        ];
        quad(self.mesh, gable, f.dir(0.0, 0.0, 1.0), wood);
        for sx in [-1.0_f32, 1.0] {
            let outer = x + sx * (half_w + 0.06);
            let pitch = [
                f.at(x, ridge + 0.03, z),
                f.at(x, ridge + 0.03, z + deep + 0.06),
                f.at(outer, eave - 0.02, z + deep + 0.06),
                f.at(outer, eave - 0.02, z),
            ];
            let up = f.dir(sx * 0.16, half_w + 0.06, 0.0);
            quad(self.mesh, pitch, up, ROOF);
            let barge = [
                f.at(x, ridge + 0.03, z + deep + 0.065),
                f.at(outer, eave - 0.02, z + deep + 0.065),
                f.at(outer, eave - 0.08, z + deep + 0.065),
                f.at(x, ridge - 0.03, z + deep + 0.065),
            ];
            quad(self.mesh, barge, f.dir(0.0, 0.0, 1.0), TRIM);
        }
    }

    /// Hedge, path and patio in front; fences between the gardens behind.
    fn gardens(&mut self, ground: &dyn Fn(Vec2) -> f32, clear: &Clearance) {
        let (hw, hd) = self.half();
        let own = self.house.centre;
        let door_x = self.facade_x(Face::Front, 0.42);
        let fence = self.colors.fence;
        let path_y = |b: &Self, x: f32, z: f32| ground(b.frame.flat(x, z));
        // A low hedge along the front garden's edge, with a gap for the path.
        let edge = hd + FRONT_YARD;
        let mut x = -hw;
        while x < hw - 0.01 {
            let x1 = (x + 0.45).min(hw);
            let mid = f32::midpoint(x, x1);
            let in_gap = (mid - door_x).abs() < 0.3;
            let spot = self.frame.flat(mid, edge);
            if !in_gap && clear.clear(spot, own) {
                let y = path_y(self, mid, edge);
                cuboid(
                    self.mesh,
                    &self.frame,
                    Vec3::new(x + 0.02, y - 0.1, edge - 0.07),
                    Vec3::new(x1 - 0.02, y + 0.17, edge + 0.07),
                    HEDGE,
                );
            }
            x = x1;
        }
        // Flagstones from the gap to the door.
        let mut z = hd + 0.05;
        while z < edge {
            let spot = self.frame.flat(door_x, z + 0.1);
            if clear.clear(spot, own) {
                let y = path_y(self, door_x, z + 0.1);
                cuboid(
                    self.mesh,
                    &self.frame,
                    Vec3::new(door_x - 0.16, y - 0.05, z),
                    Vec3::new(door_x + 0.16, y + 0.02, z + 0.17),
                    FLAGSTONE,
                );
            }
            z += 0.21;
        }
        // The brown rows' white fenced patio beside the door.
        if self.house.scheme == Scheme::Brown {
            let side = if self.house.mirrored { 1.0 } else { -1.0 };
            let x0 = door_x + side * 0.3;
            let x1 = door_x + side * 0.85;
            let spot = self.frame.flat(f32::midpoint(x0, x1), hd + 0.4);
            if clear.clear(spot, own) {
                let y = path_y(self, f32::midpoint(x0, x1), hd + 0.4);
                // Along the front of the patio, then back to the wall at its outer end.
                cuboid(
                    self.mesh,
                    &self.frame,
                    Vec3::new(x0.min(x1), y - 0.08, hd + 0.4),
                    Vec3::new(x0.max(x1), y + 0.24, hd + 0.44),
                    fence,
                );
                cuboid(
                    self.mesh,
                    &self.frame,
                    Vec3::new(x1 - 0.02, y - 0.08, hd),
                    Vec3::new(x1 + 0.02, y + 0.24, hd + 0.44),
                    fence,
                );
            }
        }
        // Behind a souterrain house the garden is a terrace at the lower floor, held up
        // by a stone wall where the ground falls away, as deep as the ground allows.
        let base = self.base();
        let terrace = if self.house.section == Section::Souterrain {
            [BACK_YARD, 0.9, 0.5].into_iter().find(|&yard| {
                [(-hw, -hd - yard), (hw, -hd - yard), (0.0, -hd - yard / 2.0)]
                    .iter()
                    .all(|&(x, z)| clear.clear(self.frame.flat(x * 0.95, z), own))
            })
        } else {
            None
        };
        let yard = terrace.unwrap_or(BACK_YARD);
        if let Some(yard) = terrace {
            let low = [(-hw, -hd), (hw, -hd), (-hw, -hd - yard), (hw, -hd - yard)]
                .map(|(x, z)| path_y(self, x, z))
                .into_iter()
                .fold(f32::MAX, f32::min);
            if low < base - 0.04 {
                cuboid(
                    self.mesh,
                    &self.frame,
                    Vec3::new(-hw, low - 0.05, -hd - yard),
                    Vec3::new(hw, base - 0.01, -hd),
                    RETAINING_WALL,
                );
                let f = &self.frame;
                let lawn = [
                    f.at(-hw, base, -hd - yard + 0.06),
                    f.at(hw, base, -hd - yard + 0.06),
                    f.at(hw, base, -hd),
                    f.at(-hw, base, -hd),
                ];
                quad(self.mesh, lawn, Vec3::Y, LAWN);
            }
        }
        let garden_y = |b: &Self, x: f32, z: f32| {
            let y = path_y(b, x, z);
            if terrace.is_some() { y.max(base) } else { y }
        };
        // Fences between the back gardens, on the right-hand dividing line, in steps.
        if !self.house.end_right {
            let x = hw;
            let mut z = -hd - 0.05;
            while z > -hd - yard {
                let z1 = (z - 0.4).max(-hd - yard);
                let mid = f32::midpoint(z, z1);
                let spot = self.frame.flat(x, mid);
                if clear.clear(spot, own) {
                    let y = garden_y(self, x, mid);
                    cuboid(
                        self.mesh,
                        &self.frame,
                        Vec3::new(x - 0.02, y - 0.1, z1),
                        Vec3::new(x + 0.02, y + 0.3, z),
                        fence,
                    );
                }
                z = z1;
            }
        }
    }
}

const fn shade(c: Color, k: f32) -> Color {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// Build one house into `mesh`.
pub fn build_house(
    mesh: &mut WallMesh,
    house: &House,
    ground: &dyn Fn(Vec2) -> f32,
    clear: &Clearance,
) {
    let mut builder = HouseBuilder {
        mesh,
        frame: Frame {
            origin: house.centre,
            along: house.along,
            out: house.out,
        },
        house,
        colors: house.scheme.colors(),
    };
    builder.walls();
    builder.roof();
    builder.openings();
    builder.porch();
    builder.gardens(ground, clear);
}

/// Every house, one mesh per [`Chunk`] (by the house's middle).
pub fn build_houses(
    buildings: &[BuildingOutline],
    roads: &[RoadLine],
    ground: &Ground,
) -> Vec<(Chunk, WallMesh)> {
    let houses = plan_houses(buildings, roads, ground);
    let prints: Vec<Footprint> = footprints(buildings, ground.tile)
        .into_iter()
        .flatten()
        .collect();
    let clear = Clearance {
        prints: &prints,
        roads,
        tile: ground.tile,
    };
    let mut chunks: HashMap<Chunk, WallMesh> = HashMap::new();
    for house in &houses {
        let tile = (
            (house.centre.x / ground.tile).round() as i32,
            (house.centre.y / ground.tile).round() as i32,
        );
        let mesh = chunks.entry(chunk_of(tile)).or_default();
        build_house(mesh, house, ground.height, &clear);
    }
    chunks.into_iter().filter(|(_, m)| !m.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::RoadKind;

    /// A row of `n` houses 2.6 wide and 3.6 deep along x, at y 10..13.6 (map coordinates).
    fn row(n: usize) -> Vec<BuildingOutline> {
        (0..n)
            .map(|i| {
                let x = 10.0 + 2.6 * i as f32;
                BuildingOutline {
                    kind: BuildingKind::Terrace,
                    storeys: 2,
                    outline: vec![[x, 10.0], [x + 2.6, 10.0], [x + 2.6, 13.6], [x, 13.6]],
                }
            })
            .collect()
    }

    fn flat() -> impl Fn(Vec2) -> f32 {
        |_| 0.0
    }

    #[test]
    fn a_footprint_finds_the_rectangle_at_its_angle() {
        let turn = Vec2::from_angle(0.4);
        let points: Vec<Vec2> = [(0.0, 0.0), (3.0, 0.0), (3.0, 2.0), (0.0, 2.0)]
            .map(|(x, y)| turn.rotate(Vec2::new(x, y)))
            .to_vec();
        let f = Footprint::of(&points).unwrap();
        let mut size = [f.size.x, f.size.y];
        size.sort_by(f32::total_cmp);
        assert!((size[0] - 2.0).abs() < 1e-4 && (size[1] - 3.0).abs() < 1e-4);
        assert!(f.contains(turn.rotate(Vec2::new(1.5, 1.0))));
        assert!(!f.contains(turn.rotate(Vec2::new(3.5, 1.0))));
    }

    #[test]
    fn a_footprints_yaw_turns_local_x_onto_its_first_side() {
        let f = Footprint::of(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 3.0),
            Vec2::new(-1.0, 2.0),
        ])
        .unwrap();
        let turned = bevy::math::Quat::from_rotation_y(f.yaw()) * Vec3::X;
        assert!(Vec2::new(turned.x, turned.z).distance(f.a) < 1e-4);
    }

    #[test]
    fn covered_tiles_are_those_whose_centre_is_inside() {
        let covered = covered_tiles(&row(1));
        // Map 10..12.6 x 10..13.6 is padded 11..13.6 x 11..14.6: centres 11..13 x 11..14.
        assert!(covered.contains(&(11, 11)) && covered.contains(&(13, 14)));
        assert!(!covered.contains(&(14, 11)) && !covered.contains(&(10, 11)));
        assert_eq!(covered.len(), 12);
    }

    #[test]
    fn a_row_is_found_with_its_ends_and_alternating_layouts() {
        let height = flat();
        let ground = Ground {
            height: &height,
            tile: 1.0,
        };
        let houses = plan_houses(&row(4), &[], &ground);
        assert_eq!(houses.len(), 4);
        assert!(houses.iter().all(|h| (h.width - 2.6).abs() < 1e-3));
        assert!(houses.iter().all(|h| (h.depth - 3.6).abs() < 1e-3));
        assert_eq!(houses.iter().filter(|h| h.end_left).count(), 1);
        assert_eq!(houses.iter().filter(|h| h.end_right).count(), 1);
        assert_eq!(houses.iter().filter(|h| h.mirrored).count(), 2);
        assert!(houses.iter().all(|h| h.scheme == houses[0].scheme));
        assert!(houses.iter().all(|h| h.section == Section::TwoStorey));
    }

    #[test]
    fn on_level_ground_the_front_faces_the_path() {
        let height = flat();
        let ground = Ground {
            height: &height,
            tile: 1.0,
        };
        // A path north of the row (smaller y).
        let path = RoadLine {
            kind: RoadKind::Path,
            width: 0.6,
            points: vec![[5.0, 8.0], [30.0, 8.0]],
        };
        let houses = plan_houses(&row(3), &[path], &ground);
        assert!(
            houses.iter().all(|h| h.out.y < -0.99),
            "{:?}",
            houses[0].out
        );
    }

    #[test]
    fn on_a_slope_the_front_is_uphill_and_the_lower_floor_opens_behind() {
        // The ground falls towards +y (south): the front faces north, uphill.
        let height = |p: Vec2| -0.5 * p.y;
        let ground = Ground {
            height: &height,
            tile: 1.0,
        };
        let houses = plan_houses(&row(3), &[], &ground);
        assert!(houses.iter().all(|h| h.section == Section::Souterrain));
        assert!(houses.iter().all(|h| h.out.y < -0.99));
        // The path-side floor sits on the uphill ground, the walls reach the downhill foot.
        let h = &houses[0];
        let front = height(h.centre + h.out * h.depth / 2.0);
        assert!((h.floor - front).abs() < 0.2);
        assert!(h.foot < h.floor - STOREY);
    }

    #[test]
    fn a_house_mesh_is_whole_and_faces_out() {
        let height = flat();
        let ground = Ground {
            height: &height,
            tile: 1.0,
        };
        let buildings = row(3);
        let built = build_houses(&buildings, &[], &ground);
        assert!(!built.is_empty());
        for (_, mesh) in &built {
            let n = mesh.positions.len();
            assert_eq!(mesh.normals.len(), n);
            assert_eq!(mesh.colors.len(), n);
            assert!(mesh.indices.iter().all(|&i| (i as usize) < n));
            assert_eq!(mesh.indices.len() % 3, 0);
            for tri in mesh.indices.chunks(3) {
                let p = |k: usize| Vec3::from(mesh.positions[tri[k] as usize]);
                let normal = Vec3::from(mesh.normals[tri[0] as usize]);
                let geo = (p(1) - p(0)).cross(p(2) - p(0));
                assert!(
                    geo.dot(normal) >= -1e-6,
                    "a triangle faces away from its normal"
                );
            }
        }
        // The roof is the highest thing: two storeys and the skillion's rise.
        let top = built
            .iter()
            .flat_map(|(_, m)| m.positions.iter().map(|p| p[1]))
            .fold(f32::MIN, f32::max);
        assert!(top > 2.0 * STOREY && top < 2.0 * STOREY + 0.8, "{top}");
    }

    #[test]
    fn gardens_keep_off_the_roads() {
        let height = flat();
        let ground = Ground {
            height: &height,
            tile: 1.0,
        };
        // A wide road right along the front.
        let road = RoadLine {
            kind: RoadKind::Street,
            width: 2.0,
            points: vec![[0.0, 9.6], [40.0, 9.6]],
        };
        let prints: Vec<Footprint> = footprints(&row(1), 1.0).into_iter().flatten().collect();
        let clear = Clearance {
            prints: &prints,
            roads: std::slice::from_ref(&road),
            tile: 1.0,
        };
        let houses = plan_houses(&row(1), std::slice::from_ref(&road), &ground);
        let house = &houses[0];
        let edge = house.centre + house.out * (house.depth / 2.0 + FRONT_YARD);
        assert!(!clear.clear(edge, house.centre));
        let behind = house.centre - house.out * (house.depth / 2.0 + 1.0);
        assert!(clear.clear(behind, house.centre));
    }
}

#[cfg(test)]
mod survey {
    use super::*;

    #[test]
    #[ignore = "prints the gis town's rows"]
    fn survey_the_gis_town() {
        let map =
            crate::map::text_format::read_map_file("assets/campaigns/gis/world/world.map").unwrap();
        let g = crate::map::ground::GroundHeight::new(&map, 1.0);
        let height = |p: Vec2| g.height(p.x, p.y);
        let ground = Ground {
            height: &height,
            tile: 1.0,
        };
        let houses = plan_houses(&map.buildings, &map.roads, &ground);
        let sou = houses
            .iter()
            .filter(|h| h.section == Section::Souterrain)
            .count();
        let red = houses.iter().filter(|h| h.scheme == Scheme::Red).count();
        println!("houses {} souterrain {sou} red {red}", houses.len());
        let mut drops: Vec<f32> = houses
            .iter()
            .map(|h| {
                let f = height(h.centre + h.out * (h.depth / 2.0 + 0.6));
                let b = height(h.centre - h.out * (h.depth / 2.0 + 0.6));
                f - b
            })
            .collect();
        drops.sort_by(f32::total_cmp);
        let q = |k: f32| drops[((drops.len() - 1) as f32 * k) as usize];
        println!(
            "front-back drop quartiles {:.2} {:.2} {:.2} {:.2} {:.2}",
            q(0.0),
            q(0.25),
            q(0.5),
            q(0.75),
            q(1.0)
        );
        let ends = houses.iter().filter(|h| h.end_left && h.end_right).count();
        println!("lone houses {ends}");
    }
}
