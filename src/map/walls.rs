//! Something to see where the map is impassable. `map_loader` gives those tiles
//! colliders; this gives them walls (`wall_mesh` builds the geometry).
//!
//! Three looks, one per kind of blocking:
//!
//! - **Solid** (blocks everyone): opaque masonry with a capped top.
//! - **Blocks players only**: a translucent green membrane, alien stuff the aliens walk
//!   through.
//! - **Blocks aliens only**: a low, translucent amber barrier players step over.
//!
//! A tile that already has a placed model (a house wall from the house tool, a fence)
//! shows that model instead.

use std::collections::HashSet;

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::general::components::map_components::MapFile;
use crate::general::events::map_events::LoadMap;
use crate::general::systems::map_systems::TileDefinitions;
use crate::map::wall_mesh::{WallCap, WallStyle, build_walls};
use crate::map::{BitFlags, MapFeatures};

/// The map's tiles with the one-tile border of void `map_loader` adds, so grid coordinates
/// here match the colliders and the floor (raw tile `(x, y)` is `(x + 1, y + 1)`).
pub fn padded_tiles(map: &MapFile) -> Vec<Vec<u64>> {
    let cols = map.tiles.iter().map(Vec::len).max().unwrap_or(0) + 2;
    let mut padded = vec![vec![0u64; cols]; map.tiles.len() + 2];
    for (out, row) in padded.iter_mut().skip(1).zip(&map.tiles) {
        for (cell, &raw) in out.iter_mut().skip(1).zip(row) {
            *cell = raw;
        }
    }
    padded
}

/// Void tiles walled in by the map, map coordinates `(x, y)`. Void only belongs at the edge
/// of the world: inside the map it shows as a hole into the background, so levels fill it
/// with walls instead. A void counts as the edge if void connects it to the outside.
#[cfg(test)]
pub fn enclosed_voids(map: &MapFile) -> Vec<(usize, usize)> {
    let padded = padded_tiles(map);
    let is_void = |x: usize, y: usize| padded.get(y).and_then(|row| row.get(x)) == Some(&0);
    let mut outside = HashSet::from([(0, 0)]);
    let mut todo = vec![(0usize, 0usize)];
    while let Some((x, y)) = todo.pop() {
        let around = [(x + 1, y), (x.wrapping_sub(1), y), (x, y + 1), (x, y.wrapping_sub(1))];
        for next in around {
            if is_void(next.0, next.1) && outside.insert(next) {
                todo.push(next);
            }
        }
    }
    let mut enclosed = Vec::new();
    for (y, row) in map.tiles.iter().enumerate() {
        for (x, &raw) in row.iter().enumerate() {
            if raw == 0 && !outside.contains(&(x + 1, y + 1)) {
                enclosed.push((x, y));
            }
        }
    }
    enclosed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallKind {
    Solid,
    BlocksPlayers,
    BlocksAliens,
}

impl WallKind {
    pub const ALL: [Self; 3] = [Self::Solid, Self::BlocksPlayers, Self::BlocksAliens];

    /// What kind of wall a tile is. Void is not a wall: it is the edge of the world, and
    /// the background plane shows there.
    pub fn of(raw: u64) -> Option<Self> {
        if raw == 0 {
            return None;
        }
        let f = BitFlags::<MapFeatures>::from_bits_truncate(raw);
        match (
            f.contains(MapFeatures::ImpassableForPlayers),
            f.contains(MapFeatures::ImpassableForEnemies),
        ) {
            (true, true) => Some(Self::Solid),
            (true, false) => Some(Self::BlocksPlayers),
            (false, true) => Some(Self::BlocksAliens),
            (false, false) => None,
        }
    }

    pub const fn style(self) -> WallStyle {
        match self {
            Self::Solid => WallStyle {
                height: 0.9,
                body_color: [0.58, 0.5, 0.44, 1.0],
                cap: Some(WallCap {
                    thickness: 0.08,
                    overhang: 0.04,
                    color: [0.78, 0.75, 0.69, 1.0],
                }),
                jitter: 0.08,
                foot_shade: 0.55,
            },
            Self::BlocksPlayers => WallStyle {
                height: 0.7,
                body_color: [0.4, 1.0, 0.55, 0.35],
                cap: None,
                jitter: 0.0,
                foot_shade: 1.0,
            },
            Self::BlocksAliens => WallStyle {
                height: 0.3,
                body_color: [1.0, 0.65, 0.15, 0.45],
                cap: None,
                jitter: 0.0,
                foot_shade: 1.0,
            },
        }
    }

    fn material(self) -> StandardMaterial {
        match self {
            Self::Solid => StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.92,
                ..default()
            },
            Self::BlocksPlayers | Self::BlocksAliens => {
                let [r, g, b, _] = self.style().body_color;
                StandardMaterial {
                    base_color: Color::WHITE,
                    emissive: LinearRgba::rgb(r, g, b) * 0.6,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                }
            }
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Solid => "Walls",
            Self::BlocksPlayers => "Walls (block players)",
            Self::BlocksAliens => "Walls (block aliens)",
        }
    }
}

pub fn spawn_walls(
    mut loads: MessageReader<LoadMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_defs: Res<TileDefinitions>,
) {
    for load in loads.read() {
        let tiles = padded_tiles(&load.map);
        let kind_at = |(col, row): (i32, i32)| -> Option<WallKind> {
            let raw = *tiles
                .get(usize::try_from(row).ok()?)?
                .get(usize::try_from(col).ok()?)?;
            WallKind::of(raw)
        };
        let modelled: HashSet<(i32, i32)> = load
            .map
            .placements
            .iter()
            .map(|p| (p.x + 1, p.y + 1))
            .collect();

        for kind in WallKind::ALL {
            let cells: HashSet<(i32, i32)> = tiles
                .iter()
                .enumerate()
                .flat_map(|(row, cols)| {
                    cols.iter()
                        .enumerate()
                        .map(move |(col, _)| (col as i32, row as i32))
                })
                .filter(|&c| kind_at(c) == Some(kind) && !modelled.contains(&c))
                .collect();
            if cells.is_empty() {
                continue;
            }
            let style = kind.style();
            // A side is hidden by a neighbour at least as tall: the same kind, or a taller
            // wall of another kind.
            let hides = |c| {
                kind_at(c)
                    .is_some_and(|other| other == kind || other.style().height >= style.height)
            };
            let built = build_walls(
                &cells,
                hides,
                tile_defs.tile_width,
                tile_defs.floor_level,
                &style,
            );
            if built.is_empty() {
                continue;
            }
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, built.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, built.normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, built.colors);
            mesh.insert_indices(Indices::U32(built.indices));

            let mut wall = commands.spawn((
                Name::from(kind.label()),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(kind.material())),
            ));
            if kind != WallKind::Solid {
                wall.insert(NotShadowCaster);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_adds_a_void_border_and_shifts_by_one() {
        let map = MapFile {
            tiles: vec![vec![1, 2], vec![3, 4]],
            ..default()
        };
        let p = padded_tiles(&map);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], vec![0; 4]);
        assert_eq!(p[1], vec![0, 1, 2, 0]);
        assert_eq!(p[2][2], 4);
    }

    #[test]
    fn only_void_cut_off_from_the_edge_is_enclosed() {
        // A hole in the middle, a notch open to the edge, and a short row.
        let map = MapFile {
            tiles: vec![
                vec![9, 9, 9, 9, 9],
                vec![9, 0, 9, 9, 0],
                vec![9, 9, 9, 0, 0],
                vec![9, 9, 9],
            ],
            ..default()
        };
        assert_eq!(enclosed_voids(&map), vec![(1, 1)]);
    }

    #[test]
    fn wall_kinds_follow_the_impassable_flags() {
        let floor = MapFeatures::Floor as u64;
        let players = MapFeatures::ImpassableForPlayers as u64;
        let aliens = MapFeatures::ImpassableForEnemies as u64;
        assert_eq!(WallKind::of(0), None, "void is not a wall");
        assert_eq!(WallKind::of(floor), None);
        assert_eq!(
            WallKind::of(floor | players | aliens),
            Some(WallKind::Solid)
        );
        assert_eq!(WallKind::of(floor | players), Some(WallKind::BlocksPlayers));
        assert_eq!(WallKind::of(floor | aliens), Some(WallKind::BlocksAliens));
    }
}
