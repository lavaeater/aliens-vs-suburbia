use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, Res};
use bevy::scene::SceneBundle;

use bevy_xpbd_3d::math::PI;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody, Rotation};
use flagset::{flags, FlagSet};

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

    // let min_y = -1;
    // let max_y = 1;
    // let min_x = -1;
    // let max_x = 1;
    // let ts = (min_y..=max_y).flat_map(|y| (min_x..=max_x).map(|x| {
    //     let mut flags = MapTile::new(x, y, TileFlags::Floor);
    //     if x == min_x {
    //         flags.features ^= TileFlags::WallWest
    //     }
    //     if x == max_x {
    //         flags.features ^= TileFlags::WallEast
    //     }
    //     if y == max_y {
    //         flags.features ^= TileFlags::WallSouth
    //     }
    //     if y == min_y {
    //         flags.features ^= TileFlags::WallNorth // Change to WallNorth
    //     }
    //     flags
    // }).collect::<Vec<MapTile>>()).collect();

    let m = [
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ];
    let verbatim_string_map = r#"
    11111111100000000
    00000001100000000
    00011111111110000
    00011113111110000
    00011111111110000
    00011511111110000
    00011100011110000
    00011100011110000
    00011100011110000
    00000000000000000
    00000000000000000
    "#;

    let other_tiles = vec![
        MapTile::new(0, 0, TileFlags::Floor | TileFlags::WallWestNorthEast),
        MapTile::new(0, 1, TileFlags::Floor | TileFlags::WallWestEast),
        MapTile::new(0, 2, TileFlags::Floor | TileFlags::WallSouthWest),
        MapTile::new(1, 2, TileFlags::Floor | TileFlags::WallNorthSouth),
        MapTile::new(2, 2, TileFlags::Floor | TileFlags::WallNorthSouth),
        MapTile::new(3, 2, TileFlags::Floor | TileFlags::WallNorthEast),
        MapTile::new(3, 3, TileFlags::Floor | TileFlags::WallWestEast),
        MapTile::new(3, 3, TileFlags::Floor | TileFlags::WallWestEast),
        MapTile::new(3, 3, TileFlags::Floor | TileFlags::WallEastSouthWest)];

    let map = MapDef {
        x: 0,
        y: 0,
        tiles: other_tiles,
    };

    for tile in map.tiles.iter() {
        if tile.features.contains(TileFlags::Floor) {
            commands.spawn((
                Name::from(format!("Floor {}:{}", tile.x, tile.y)),
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