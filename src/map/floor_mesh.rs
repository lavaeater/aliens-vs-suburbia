//! Floor meshes: the walkable ground, one mesh per [`Chunk`] and terrain colour.
//!
//! Pure geometry, no ECS. Tile centres sit at `tile * (col, row)`, like the colliders.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{Mesh, Vec2, Vec3, Vec3Swizzles};

use crate::map::ground::GroundHeight;
use crate::map::streaming::{Chunk, chunk_of};
use crate::map::{BitFlags, MapFeatures};

/// A terrain's colour, as sRGB bytes.
pub type TerrainColor = [u8; 3];

fn terrain_color(tile: BitFlags<MapFeatures>) -> TerrainColor {
    if tile.contains(MapFeatures::Water) {
        return [38, 90, 179];
    }
    if tile.contains(MapFeatures::Mud) {
        return [115, 82, 46];
    }
    if tile.contains(MapFeatures::Snow) {
        return [209, 224, 235];
    }
    if tile.contains(MapFeatures::Rock) {
        return [97, 89, 77];
    }
    if tile.contains(MapFeatures::Grass) {
        return [56, 140, 56];
    }
    [140, 133, 122] // Floor (default)
}

/// The way the hills are lit, baked in: the camera looks nearly straight down, so a hill's
/// shape shows only in its light, and at night there is hardly any. Pointing downhill towards
/// the north west is bright, away from it dark, and flat ground is untouched.
const HILL_LIGHT: Vec2 = Vec2::new(-0.71, -0.71);
/// How much brighter a slope gets per unit of its normal leaning towards [`HILL_LIGHT`].
const HILL_CONTRAST: f32 = 8.0;

/// How much darker the lowest ground and lighter the highest is than the middle: valleys
/// sink and hilltops lift, which reads even where the slope light doesn't (a broad rise, a
/// plateau).
const ALTITUDE_CONTRAST: f32 = 0.15;

fn hill_shade(normal: Vec3) -> [f32; 4] {
    let lean = normal.xz().dot(HILL_LIGHT);
    let shade = (1.0_f32 + lean * HILL_CONTRAST).clamp(0.4, 1.6);
    [shade, shade, shade, 1.0]
}

/// [`hill_shade`], times the altitude tint: `height` between `range`'s lowest (darker by
/// [`ALTITUDE_CONTRAST`]) and highest (lighter by as much). Flat ground is untouched.
fn ground_shade(normal: Vec3, height: f32, (lowest, highest): (f32, f32)) -> [f32; 4] {
    let [slope, _, _, _] = hill_shade(normal);
    let span = highest - lowest;
    let altitude = if span > f32::EPSILON {
        let at = ((height - lowest) / span).clamp(0.0, 1.0);
        1.0 + ALTITUDE_CONTRAST * (2.0 * at - 1.0)
    } else {
        1.0
    };
    let shade = slope * altitude;
    [shade, shade, shade, 1.0]
}

/// Mesh data ready for `Mesh::insert_attribute`.
#[derive(Default, Debug)]
pub struct FloorMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl FloorMesh {
    /// A tile, its corners on the ground: neighbours share their heights, so no seams.
    fn tile(&mut self, (col, row): (i32, i32), tile: f32, y: f32, ground: &GroundHeight) {
        let base = self.positions.len() as u32;
        let (x0, x1) = (tile * (col as f32 - 0.5), tile * (col as f32 + 0.5));
        let (z0, z1) = (tile * (row as f32 - 0.5), tile * (row as f32 + 0.5));
        for (x, z) in [(x0, z0), (x1, z0), (x1, z1), (x0, z1)] {
            let height = ground.height(x, z);
            self.positions.push([x, y + height, z]);
            let normal = ground.normal(x, z);
            self.normals.push(normal.to_array());
            self.colors.push(ground_shade(normal, height, ground.range()));
        }
        self.uvs
            .extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    /// A loose vertex on the ground at world `(x, z)`, `lift` above the floor; its index.
    pub(crate) fn vertex(&mut self, x: f32, z: f32, lift: f32, ground: &GroundHeight) -> u32 {
        let index = self.positions.len() as u32;
        let height = ground.height(x, z);
        self.positions.push([x, lift + height, z]);
        let normal = ground.normal(x, z);
        self.normals.push(normal.to_array());
        self.colors.push(ground_shade(normal, height, ground.range()));
        self.uvs.push([x, z]);
        index
    }

    /// A triangle of earlier vertices, turned to face up whichever way they were given.
    pub(crate) fn triangle(&mut self, a: u32, b: u32, c: u32) {
        let at = |i: u32| {
            self.positions
                .get(i as usize)
                .map_or(Vec2::ZERO, |p| Vec2::new(p[0], p[2]))
        };
        let (ab, ac) = (at(b) - at(a), at(c) - at(a));
        // The y of (ab x ac) in 3D, with Vec2 holding (x, z).
        if ab.y * ac.x - ab.x * ac.y >= 0.0 {
            self.indices.extend_from_slice(&[a, b, c]);
        } else {
            self.indices.extend_from_slice(&[a, c, b]);
        }
    }

    /// Positions and indices, for tests.
    #[cfg(test)]
    pub(crate) fn parts(&self) -> (Vec<[f32; 3]>, Vec<u32>) {
        (self.positions.clone(), self.indices.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

/// The floor of a padded tile grid. Void has none.
pub fn build_floors(
    tiles: &[Vec<u64>],
    tile: f32,
    floor_y: f32,
    ground: &GroundHeight,
) -> HashMap<(Chunk, TerrainColor), FloorMesh> {
    let mut floors: HashMap<(Chunk, TerrainColor), FloorMesh> = HashMap::new();
    for (row, cols) in tiles.iter().enumerate() {
        for (col, &raw) in cols.iter().enumerate() {
            if raw == 0 {
                continue;
            }
            let at = (col as i32, row as i32);
            let color = terrain_color(BitFlags::<MapFeatures>::from_bits_truncate(raw));
            floors
                .entry((chunk_of(at), color))
                .or_default()
                .tile(at, tile, floor_y, ground);
        }
    }
    floors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::streaming::CHUNK_TILES;

    #[test]
    fn floors_split_by_chunk_and_colour_and_skip_void() {
        let floor = MapFeatures::Floor as u64;
        let grass = floor | MapFeatures::Grass as u64;
        let far = CHUNK_TILES as usize;
        let mut tiles = vec![vec![0; far + 1]; 1];
        tiles[0][0] = floor;
        tiles[0][1] = floor;
        tiles[0][2] = grass;
        tiles[0][far] = floor;

        let floors = build_floors(&tiles, 1.0, 0.0, &GroundHeight::default());
        assert_eq!(floors.len(), 3);
        let grey = terrain_color(BitFlags::from_bits_truncate(floor));
        let green = terrain_color(BitFlags::from_bits_truncate(grass));
        assert_eq!(floors[&((0, 0), grey)].positions.len(), 8);
        assert_eq!(floors[&((0, 0), green)].positions.len(), 4);
        assert_eq!(floors[&((1, 0), grey)].indices.len(), 6);
    }

    #[test]
    fn hill_shade_leaves_flat_ground_alone() {
        assert_eq!(hill_shade(Vec3::Y), [1.0; 4]);
    }

    #[test]
    fn hill_shade_lights_one_side_of_a_hill_and_darkens_the_other() {
        let north_west = Vec3::new(-0.1, 1.0, -0.1).normalize();
        let south_east = Vec3::new(0.1, 1.0, 0.1).normalize();
        assert!(hill_shade(north_west)[0] > 1.2);
        assert!(hill_shade(south_east)[0] < 0.8);
    }

    #[test]
    fn higher_ground_is_lighter_and_lower_darker() {
        let range = (0.0, 4.0);
        let low = ground_shade(Vec3::Y, 0.0, range)[0];
        let middle = ground_shade(Vec3::Y, 2.0, range)[0];
        let high = ground_shade(Vec3::Y, 4.0, range)[0];
        assert!((middle - 1.0).abs() < 1e-6);
        assert!((low - (1.0 - ALTITUDE_CONTRAST)).abs() < 1e-6);
        assert!((high - (1.0 + ALTITUDE_CONTRAST)).abs() < 1e-6);
        assert_eq!(ground_shade(Vec3::Y, 3.0, (0.0, 0.0)), [1.0; 4], "flat: no tint");
    }

    #[test]
    fn neighbouring_tiles_meet_on_a_slope() {
        let ground = GroundHeight::new(
            &crate::general::components::map_components::MapFile {
                elevation: vec![vec![0, 100, 200, 300]; 4],
                ..Default::default()
            },
            1.0,
        );
        let mut mesh = FloorMesh::default();
        mesh.tile((2, 2), 1.0, 0.0, &ground);
        mesh.tile((3, 2), 1.0, 0.0, &ground);
        // The first tile's east corners are the second's west corners.
        assert_eq!(mesh.positions[1], mesh.positions[4]);
        assert_eq!(mesh.positions[2], mesh.positions[7]);
        assert_eq!(mesh.normals[1], mesh.normals[4]);
        assert!(
            mesh.positions[1][1] > mesh.positions[0][1],
            "rises eastward"
        );
        assert!(mesh.normals[0][0] < 0.0, "faces away from the rise");
    }

    #[test]
    fn a_floor_tile_is_centred_on_its_grid_point_and_faces_up() {
        let mut mesh = FloorMesh::default();
        mesh.tile((3, 2), 2.0, 0.5, &GroundHeight::default());
        let at = |i: usize| Vec3::from(mesh.positions[i]);
        assert!(at(0).distance(Vec3::new(5.0, 0.5, 3.0)) < 1e-6);
        assert!(at(2).distance(Vec3::new(7.0, 0.5, 5.0)) < 1e-6);
        let corner = |i: usize| at(mesh.indices[i] as usize);
        let up = (corner(1) - corner(0)).cross(corner(2) - corner(0));
        assert!(up.y > 0.0, "wound to face up");
    }
}
