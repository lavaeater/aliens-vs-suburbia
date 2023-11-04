use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::math::PI;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody, Rotation};
use flagset::{flags, Flags, FlagSet};

flags! {
    enum TileFlags: u64 {
        Floor, //1
        WallNorth, //2
        WallEast, //4
        WallSouth, //8
        WallWest, //16
        Pickup, //32
        PossibleEncounter //64
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

    let min_x = -1;
    let max_x = 1;
    let min_y = -1;
    let max_y = 1;
    let ts = (min_x..=max_x).flat_map(|x| (min_y..=max_y).map(|y| {
        let mut flags = MapTile::new(x,y,TileFlags::Floor);
        if y == min_y {
            flags.features ^= TileFlags::WallSouth
        }
        if y == max_y {
            flags.features ^= TileFlags::WallNorth
        }

        if x == max_x {
            flags.features ^= TileFlags::WallEast
        }
        if x == min_x {
            flags.features ^= TileFlags::WallWest
        }
        flags
    }).collect::<Vec<MapTile>>()).collect();

    let map = MapDef {
        x: 0,
        y: 0,
        tiles: ts,
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
        if tile.features.contains(TileFlags::WallNorth) {
            commands.spawn((
                Name::from(format!("Wall North {}:{}", tile.x, tile.y)),
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
        if tile.features.contains(TileFlags::WallSouth) {
            commands.spawn((
                Name::from(format!("Wall South {}:{}", tile.x, tile.y)),
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
        if tile.features.contains(TileFlags::WallEast) {
            commands.spawn((
                Name::from(format!("Wall East {}:{}", tile.x, tile.y)),
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
        if tile.features.contains(TileFlags::WallEast) {
            commands.spawn((
                Name::from(format!("Wall East {}:{}", tile.x, tile.y)),
                SceneBundle {
                    scene: asset_server.load("wall_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Static,
                Collider::cuboid(tile_width, wall_height, tile_depth),
                Position::from(Vec3::new(tile_width * tile.x as f32, -wall_height, tile_width * tile.y as f32 - tile_width * 2.5)),
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