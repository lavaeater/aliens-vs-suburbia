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
        Floor,
        WallNorth,
        WallEast,
        WallSouth,
        WallWest,
        Pickup,
        PossibleEncounter
    }
}

/*

 */
pub struct MapTile(FlagSet<TileFlags>);

impl MapTile {
    fn new(flags: impl Into<FlagSet<TileFlags>>) -> MapTile {
        MapTile(flags.into())
    }
}

pub struct MapDef {
    pub x: i32,
    pub y: i32,
    pub tiles: Vec<Vec<MapTile>>,
}

pub fn spawn_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let min_x = -1;
    let max_x = 1;
    let min_y = -1;
    let max_y = 1;
    let ts = (min_x..=max_x).map(|x| (min_y..=max_y).map(|y| {
        let mut flags = MapTile::new(TileFlags::Floor);
        if y == min_y {
            flags.0 ^= TileFlags::WallSouth
        }
        if y == max_y {
            flags.0 ^= TileFlags::WallNorth
        }

        if x == max_x {
            flags.0 ^= TileFlags::WallEast
        }
        if x == min_x {
            flags.0 ^= TileFlags::WallWest
        }
        flags
    }).collect()
    ).collect();

    let map = MapDef {
        x: -5,
        y: -5,
        tiles: ts,
    };

    for (y, row) in map.tiles.iter().enumerate() {
        for (x, tile) in row.iter().enumerate() {
            if tile.0.contains(TileFlags::Floor) {
                commands.spawn((
                    Name::from(format!("Floor {}:{}", x, y)),
                    SceneBundle {
                        scene: asset_server.load("floor_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(2.0, 0.125, 2.0),
                    Position::from(Vec3::new(2.0 * x as f32, -2.0, 2.0 * y as f32)),
                ));
            }
            if tile.0.contains(TileFlags::WallNorth) {
                commands.spawn((
                    Name::from(format!("Wall North {}:{}", x, y)),
                    SceneBundle {
                        scene: asset_server.load("wall_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(2.0, 2.0, 0.01),
                    Position::from(Vec3::new(2.0 * x as f32 + 1.0, -1.1875, 2.0 * y as f32)),
                    Rotation::from(Quat::from_euler(
                        bevy::math::EulerRot::YXZ,
                        PI / 2.0,
                        0.0,
                        0.0,
                    )),
                ));
            }
            // if tile.0.contains(TileFlags::WallSouth) {
            //     commands.spawn((
            //         Name::from(format!("Wall South {}:{}", x, y)),
            //         SceneBundle {
            //             scene: asset_server.load("wall_south.glb#Scene0"),
            //             transform: Transform::from_xyz(2.0 * x as f32 - 1.0, -2.0, 2.0 * y as f32),
            //             ..Default::default()
            //         },
            //         RigidBody::Static,
            //         Collider::cuboid(0.125, 2.0, 2.0),
            //         Position::from(Vec3::new(2.0 * x as f32 - 1.0, -2.0, 2.0 * y as f32)),
            //
            //     ));
            // }
            // if tile.0.contains(TileFlags::WallEast) {
            //     commands.spawn((
            //         Name::from(format!("Wall East {}:{}", x, y)),
            //         SceneBundle {
            //             scene: asset_server.load("wall_east.glb#Scene0"),
            //             ..Default::default()
            //         },
            //         RigidBody::Static,
            //         Collider::cuboid(0.10, 0.10, 0.10),
            //         Position::from(Vec3::new(2.0 * x as f32, -2.0, 2.0 * y as f32)),
            //     ));
            // }
            // if tile.0.contains(TileFlags::WallWest) {
            //     commands.spawn((
            //         Name::from(format!("Wall West {}:{}", x, y)),
            //         SceneBundle {
            //             scene: asset_server.load("wall_west.glb#Scene0"),
            //             ..Default::default()
            //         },
            //         RigidBody::Static,
            //         Collider::cuboid(0.10, 0.10, 0.10),
            //         Position::from(Vec3::new(2.0 * x as f32, -2.0, 2.0 * y as f32)),
            //     ));
            // }
        }
    }
}