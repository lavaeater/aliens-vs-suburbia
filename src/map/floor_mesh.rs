//! Floor meshes: the walkable ground, one mesh per [`Chunk`] and terrain colour.
//!
//! Pure geometry, no ECS. Tile centres sit at `tile * (col, row)`, like the colliders.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::Mesh;

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

/// Mesh data ready for `Mesh::insert_attribute`.
#[derive(Default, Debug)]
pub struct FloorMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl FloorMesh {
    fn tile(&mut self, (col, row): (i32, i32), tile: f32, y: f32) {
        let base = self.positions.len() as u32;
        let (x0, x1) = (tile * (col as f32 - 0.5), tile * (col as f32 + 0.5));
        let (z0, z1) = (tile * (row as f32 - 0.5), tile * (row as f32 + 0.5));
        self.positions
            .extend_from_slice(&[[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]]);
        self.normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        self.uvs
            .extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
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
                .tile(at, tile, floor_y);
        }
    }
    floors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::streaming::CHUNK_TILES;
    use bevy::math::Vec3;

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

        let floors = build_floors(&tiles, 1.0, 0.0);
        assert_eq!(floors.len(), 3);
        let grey = terrain_color(BitFlags::from_bits_truncate(floor));
        let green = terrain_color(BitFlags::from_bits_truncate(grass));
        assert_eq!(floors[&((0, 0), grey)].positions.len(), 8);
        assert_eq!(floors[&((0, 0), green)].positions.len(), 4);
        assert_eq!(floors[&((1, 0), grey)].indices.len(), 6);
    }

    #[test]
    fn a_floor_tile_is_centred_on_its_grid_point_and_faces_up() {
        let mut mesh = FloorMesh::default();
        mesh.tile((3, 2), 2.0, 0.5);
        let at = |i: usize| Vec3::from(mesh.positions[i]);
        assert!(at(0).distance(Vec3::new(5.0, 0.5, 3.0)) < 1e-6);
        assert!(at(2).distance(Vec3::new(7.0, 0.5, 5.0)) < 1e-6);
        let corner = |i: usize| at(mesh.indices[i] as usize);
        let up = (corner(1) - corner(0)).cross(corner(2) - corner(0));
        assert!(up.y > 0.0, "wound to face up");
    }
}
