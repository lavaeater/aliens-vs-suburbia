use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, Res};
use bevy::scene::SceneBundle;

use bevy_xpbd_3d::math::PI;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody, Rotation};
use flagset::{flags, FlagSet};
use crate::general::components::{Floor, HittableTarget, Wall};

flags! {
    enum FileFlags: u16 {
        Floor = 1, // 1
        Pickup = 2, // 2
        PossibleEncounter = 4, // 4,
        FloorPickup = 3,
        FloorPossibleEncounter = 5,
    }
}

flags! {
    enum TileFlags: u64 {
        Floor = 1, //1
        WallNorth = 2, //2
        WallEast = 4, //4
        WallSouth = 8, //8
        WallWest = 16, //16
        Pickup = 32, //32
        PossibleEncounter = 64, //64
        WallNorthEast = (TileFlags::WallNorth | TileFlags::WallEast).bits(),
        WallEastSouth = (TileFlags::WallEast | TileFlags::WallSouth).bits(),
        WallSouthWest = (TileFlags::WallSouth | TileFlags::WallWest).bits(),
        WallWestNorth = (TileFlags::WallWest | TileFlags::WallNorth).bits(),
        WallNorthEastSouth = (TileFlags::WallNorthEast | TileFlags::WallSouth).bits(),
        WallEastSouthWest = (TileFlags::WallEastSouth | TileFlags::WallWest).bits(),
        WallSouthWestNorth = (TileFlags::WallSouthWest | TileFlags::WallNorth).bits(),
        WallWestNorthEast = (TileFlags::WallWestNorth | TileFlags::WallEast).bits(),
        WallWestEast = (TileFlags::WallWest | TileFlags::WallEast).bits(),
        WallNorthSouth = (TileFlags::WallNorth | TileFlags::WallSouth).bits(),
    }
}

/*

 */
pub struct MapTile {
    features: FlagSet<TileFlags>,
    x: i32,
    y: i32,
}

impl MapTile {
    fn new(x: i32, y: i32, features: impl Into<FlagSet<TileFlags>>) -> MapTile {
        MapTile { features: features.into(), x, y }
    }
}

pub struct MapDef {
    pub x: i32,
    pub y: i32,
    pub tiles: Vec<MapTile>,
}

pub fn spawn_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let tile_size = 2.0;
    let tile_unit = tile_size / 32.0;
    let tile_width = 32.0 * tile_unit;
    let wall_height = 19.0 * tile_unit;
    let tile_depth = 1.0 * tile_unit;
    let m = [
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 1, 1, 3, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        [1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ];
    let checks = [
        [-1, 0],
        [1, 0],
        [0, -1],
        [0, 1]
    ];
    let mut tiles: Vec<MapTile> = Vec::new();
    for (row, rows) in m.iter().enumerate() {
        for (column, t) in rows.iter().enumerate() {
            let mut flag_val: FlagSet<TileFlags> = TileFlags::Floor.into();
            //Fix border tiles
            if *t != 0 {
                if column == 0 {
                    flag_val |= TileFlags::WallWest;
                }
                if column == rows.len() - 1 {
                    flag_val |= TileFlags::WallEast;
                }
                if row == 0 {
                    flag_val |= TileFlags::WallNorth;
                }
                if row == m.len() - 1 {
                    flag_val |= TileFlags::WallSouth;
                }

                //Check neighbours
                for check in checks.iter() {
                    let column_check = column as i32 + check[0];
                    let row_check = row as i32 + check[1];
                    if column_check >= 0 && column_check < rows.len() as i32 {
                        if column_check < column as i32 && m[row][column_check as usize] == 0 {
                            flag_val |= TileFlags::WallWest;
                        }
                        if column_check > column as i32 && m[row][column_check as usize] == 0 {
                            flag_val |= TileFlags::WallEast;
                        }
                    }
                    if row_check >= 0 && row_check < m.len() as i32 {
                        if row_check < row as i32 && m[row_check as usize][column] == 0 {
                            flag_val |= TileFlags::WallNorth;
                        }
                        if row_check > row as i32 && m[row_check as usize][column] == 0 {
                            flag_val |= TileFlags::WallSouth;
                        }
                    }
                }
                if *t == 3 {
                    flag_val |= TileFlags::Pickup;
                }

                if *t == 5 {
                    flag_val |= TileFlags::PossibleEncounter;
                }
                tiles.push(MapTile::new(column as i32, row as i32, flag_val));
            }
        }
    }
    let map = MapDef {
        x: 0,
        y: 0,
        tiles,
    };

    for tile in map.tiles.iter() {
        if tile.features.contains(TileFlags::Floor) {
            commands.spawn((
                Name::from(format!("Floor {}:{}", tile.x, tile.y)),
                Floor {},
                SceneBundle {
                    scene: asset_server.load("floor_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Static,
                Collider::cuboid(tile_width, tile_depth, tile_width),
                Position::from(Vec3::new(tile_width * tile.x as f32, -2.0, tile_width * tile.y as f32)),
            ));
        }
        if tile.features.contains(TileFlags::WallEast) { //Change to WallEast
            commands.spawn((
                Name::from(format!("Wall East {}:{}", tile.x, tile.y)),
                Wall {},
                HittableTarget {},
                SceneBundle {
                    scene: asset_server.load("wall_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Static,
                Collider::cuboid(tile_width, wall_height, tile_depth),
                Position::from(Vec3::new(tile_width * tile.x as f32 + tile_width / 2.0 - 2.0 / 16.0, -wall_height, tile_width * tile.y as f32)),
                Rotation::from(Quat::from_euler(
                    bevy::math::EulerRot::YXZ,
                    PI / 2.0,
                    0.0,
                    0.0,
                )),
            ));
        }
        if tile.features.contains(TileFlags::WallWest) {
            commands.spawn((
                Name::from(format!("Wall West {}:{}", tile.x, tile.y)),
                Wall {},
                HittableTarget {},
                SceneBundle {
                    scene: asset_server.load("wall_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Static,
                Collider::cuboid(tile_width, wall_height, tile_depth),
                Position::from(Vec3::new(tile_width * tile.x as f32 - tile_width / 2.0, -wall_height, tile_width * tile.y as f32)),
                Rotation::from(Quat::from_euler(
                    bevy::math::EulerRot::YXZ,
                    PI / 2.0,
                    0.0,
                    0.0,
                )),
            ));
        }
        if tile.features.contains(TileFlags::WallSouth) {
            commands.spawn((
                Name::from(format!("Wall South {}:{}", tile.x, tile.y)),
                Wall {},
                HittableTarget {},
                SceneBundle {
                    scene: asset_server.load("wall_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Static,
                Collider::cuboid(tile_width, wall_height, tile_depth),
                Position::from(Vec3::new(tile_width * tile.x as f32, -wall_height, tile_width * tile.y as f32 + tile_width / 2.0)),
                Rotation::from(Quat::from_euler(
                    bevy::math::EulerRot::YXZ,
                    0.0,
                    0.0,
                    0.0,
                )),
            ));
        }
        if tile.features.contains(TileFlags::WallNorth) {
            commands.spawn((
                Name::from(format!("Wall North {}:{}", tile.x, tile.y)),
                Wall {},
                HittableTarget {},
                SceneBundle {
                    scene: asset_server.load("wall_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Static,
                Collider::cuboid(tile_width, wall_height, tile_depth),
                Position::from(Vec3::new(tile_width * tile.x as f32, -wall_height, tile_width * tile.y as f32 - tile_width / 2.0)),
                Rotation::from(Quat::from_euler(
                    bevy::math::EulerRot::YXZ,
                    0.0,
                    0.0,
                    0.0,
                )),
            ));
        }
    }
}