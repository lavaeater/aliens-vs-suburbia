use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, EventReader, EventWriter, Query, Res, ResMut, Resource};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::CollisionLayers;
use bevy_xpbd_3d::math::PI;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody, Rotation};
use flagset::{flags, FlagSet};
use pathfinding::grid::Grid;
use crate::enemy::components::general::AlienCounter;
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::{AlienGoal, AlienSpawnPoint, CurrentTile, Floor, Wall};
use crate::general::events::map_events::{LoadMap, SpawnPlayer};
use crate::general::resources::map_resources::MapGraph;
use bevy::math::EulerRot;

flags! {
    pub enum FileFlags: u16 {
        Floor = 1, // 1
        Pickup = 2, // 2
        AlienSpawn = 4, // 4,
        FloorPickup = 3,
        FloorSpawn = 5,
        AlienGoal = 8, // 8
        FloorAlienGoal = 9,
        PlayerSpawn = 16, // 16
        FloorPlayerSpawn = 17,
    }
}

// Max number of flags is... 64, dummy
flags! {
    pub enum TileFlags: u64 {
        Floor = 1, //1
        WallNorth = 2, //2
        WallEast = 4, //4
        WallSouth = 8, //8
        WallWest = 16, //16
        Pickup = 32, //32
        AlienSpawnPoint = 64, //64
        AlienGoal = 128, //128
        PlayerSpawn = 256, //256,
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
} //No data needed now

pub fn load_map_one(
    mut send_event: EventWriter<LoadMap>
) {
    send_event.send(LoadMap {});
}


#[derive(Resource)]
pub struct TileDefinitions {
    pub tile_size: f32,
    pub tile_basis: f32,
    pub tile_unit: f32,
    pub tile_width: f32,
    pub wall_height: f32,
    pub tile_depth: f32,
}

impl TileDefinitions {
    pub fn new(tile_size: f32, tile_basis: f32, wall_basis: f32, tile_depth_basis: f32) -> Self {
        let tile_unit = tile_size / tile_basis;
        let tile_width = tile_basis * tile_unit;
        let wall_height = wall_basis * tile_unit;
        let tile_depth = tile_depth_basis * tile_unit;
        Self {
            tile_size,
            tile_basis,
            tile_unit,
            tile_width,
            wall_height,
            tile_depth,
        }
    }

    pub fn create_floor_collider(&self) -> Collider {
        Collider::cuboid(self.tile_width, self.tile_depth, self.tile_width)
    }

    pub fn get_floor_position(&self, x: i32, y: i32) -> Vec3 {
        Vec3::new(self.tile_width * x as f32, -1.6, self.tile_width * y as f32)
    }

    pub fn create_wall_collider(&self) -> Collider {
        Collider::cuboid(self.tile_width, self.wall_height, self.tile_depth)
    }

    pub fn get_wall_position(&self, x: i32, y: i32, wall_direction: TileFlags) -> Vec3 {
        match wall_direction {
            TileFlags::WallNorth => {
                Vec3::new(self.tile_width * x as f32, -self.wall_height, self.tile_width * y as f32 - self.tile_width / self.tile_size)
            }
            TileFlags::WallEast => {
                Vec3::new(self.tile_width * x as f32 + self.tile_width / self.tile_size - self.tile_size / self.tile_basis, -self.wall_height, self.tile_width * y as f32)
            }
            TileFlags::WallSouth => {
                Vec3::new(self.tile_width * x as f32, -self.wall_height, self.tile_width * y as f32 + self.tile_width / self.tile_size)
            }
            TileFlags::WallWest => {
                Vec3::new(self.tile_width * x as f32 - self.tile_width / self.tile_size, -self.wall_height, self.tile_width * y as f32)
            }
            _ => { panic!("Not a wall direction") }
        }
    }

    pub fn get_wall_rotation(&self, wall_direction: TileFlags) -> Quat {
        match wall_direction {
            TileFlags::WallNorth => { Quat::from_euler(EulerRot::YXZ, 0.0, 0.0, 0.0) }
            TileFlags::WallSouth => { Quat::from_euler(EulerRot::YXZ, 0.0, 0.0, 0.0) }
            TileFlags::WallEast => { Quat::from_euler(EulerRot::YXZ, PI / 2.0, 0.0, 0.0) }
            TileFlags::WallWest => { Quat::from_euler(EulerRot::YXZ, PI / 2.0, 0.0, 0.0) }
            _ => { panic!("Not a wall direction") }
        }
    }
}

pub fn map_loader(
    mut map_graph: ResMut<MapGraph>,
    mut load_map_event_reader: EventReader<LoadMap>,
    mut spawn_player_event_writer: EventWriter<SpawnPlayer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut alien_counter: ResMut<AlienCounter>,
    tile_definitions: Res<TileDefinitions>,
) {
    for _load_map in load_map_event_reader.read() {
        let m = [
            [17, 1, 1, 1, 1, 1, 1, 1, 5],
            [1, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 1, 1, 0, 0, 0, 1, 1, 1],
            [1, 1, 1, 0, 0, 0, 1, 1, 1],
            [1, 1, 1, 0, 0, 0, 1, 1, 1],
            [1, 1, 1, 0, 0, 0, 1, 1, 1],
            [1, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1, 1, 1, 1, 1],
            [9, 1, 1, 1, 1, 1, 1, 1, 1],
        ];
        let rows = m.len();
        let cols = m[0].len();
        map_graph.grid = Grid::new(cols, rows);
        // let m = [
        //     [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        //     [0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        //     [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [0, 1, 1, 0, 0, 1, 1, 3, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [0, 0, 1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [0, 0, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
        //     [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        //     [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        // ];
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
                    map_graph.grid.add_vertex((column, row));
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
                        flag_val |= TileFlags::AlienSpawnPoint;
                    }

                    if *t == 9 {
                        flag_val |= TileFlags::AlienGoal;
                    }

                    if *t == 17 {
                        flag_val |= TileFlags::PlayerSpawn;
                    }
                    tiles.push(MapTile::new(column as i32, row as i32, flag_val));
                }
            }
        }

        //This will make the grid ready for some a* path-finding!
        map_graph.grid.enable_diagonal_mode();

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
                    tile_definitions.create_floor_collider(),
                    Position::from(tile_definitions.get_floor_position(tile.x, tile.y)),
                    CollisionLayers::new([CollisionLayer::Floor], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player])
                ));
            }
            if tile.features.contains(TileFlags::WallEast) { //Change to WallEast
                commands.spawn((
                    Name::from(format!("Wall East {}:{}", tile.x, tile.y)),
                    Wall {},
                    SceneBundle {
                        scene: asset_server.load("wall_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    tile_definitions.create_wall_collider(),
                    Position::from(tile_definitions.get_wall_position(tile.x, tile.y, TileFlags::WallEast)),
                    Rotation::from(tile_definitions.get_wall_rotation(TileFlags::WallEast)),
                    CollisionLayers::new([CollisionLayer::Wall], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                ));
            }
            if tile.features.contains(TileFlags::WallWest) {
                commands.spawn((
                    Name::from(format!("Wall West {}:{}", tile.x, tile.y)),
                    Wall {},
                    SceneBundle {
                        scene: asset_server.load("wall_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(tile_definitions.tile_width, tile_definitions.wall_height, tile_definitions.tile_depth),
                    Position::from(tile_definitions.get_wall_position(tile.x, tile.y, TileFlags::WallWest)),
                    Rotation::from(tile_definitions.get_wall_rotation(TileFlags::WallWest)),
                    CollisionLayers::new([CollisionLayer::Wall], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                ));
            }
            if tile.features.contains(TileFlags::WallSouth) {
                commands.spawn((
                    Name::from(format!("Wall South {}:{}", tile.x, tile.y)),
                    Wall {},
                    SceneBundle {
                        scene: asset_server.load("wall_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(tile_definitions.tile_width, tile_definitions.wall_height, tile_definitions.tile_depth),
                    Position::from(tile_definitions.get_wall_position(tile.x, tile.y, TileFlags::WallSouth)),
                    Rotation::from(tile_definitions.get_wall_rotation(TileFlags::WallSouth)),
                    CollisionLayers::new([CollisionLayer::Wall], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                ));
            }
            if tile.features.contains(TileFlags::WallNorth) {
                commands.spawn((
                    Name::from(format!("Wall North {}:{}", tile.x, tile.y)),
                    Wall {},
                    SceneBundle {
                        scene: asset_server.load("wall_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(tile_definitions.tile_width, tile_definitions.wall_height, tile_definitions.tile_depth),
                    Position::from(tile_definitions.get_wall_position(tile.x, tile.y, TileFlags::WallNorth)),
                    Rotation::from(tile_definitions.get_wall_rotation(TileFlags::WallNorth)),
                    CollisionLayers::new([CollisionLayer::Wall], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                ));
            }

            if tile.features.contains(TileFlags::AlienSpawnPoint) {
                // We set the max aliens in the map, OK?
                alien_counter.max_count = 1;
                commands.spawn((
                    Name::from(format!("Alien Spawn Point{}:{}", tile.x, tile.y)),
                    AlienSpawnPoint::new(30.0),
                    SceneBundle {
                        scene: asset_server.load("player.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(0.5, 0.5, 0.45),
                    Position::from(Vec3::new(tile_definitions.tile_width * tile.x as f32 - 1.0, -tile_definitions.wall_height, tile_definitions.tile_width * tile.y as f32 + 1.0)),
                    CollisionLayers::new([CollisionLayer::AlienSpawnPoint], [CollisionLayer::Player]),
                ));
            }
            if tile.features.contains(TileFlags::AlienGoal) {
                map_graph.goal = (tile.x as usize, tile.y as usize);
                commands.spawn((
                    Name::from(format!("Alien Goal {}:{}", tile.x, tile.y)),
                    AlienGoal::new(tile.x as usize, tile.y as usize), //ooh, we need to handle this in the future...
                    SceneBundle {
                        scene: asset_server.load("player.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(0.5, 0.5, 0.45),
                    Position::from(Vec3::new(tile_definitions.tile_width * tile.x as f32, -tile_definitions.wall_height, tile_definitions.tile_width * tile.y as f32 - tile_definitions.tile_width / 2.0)),
                    CollisionLayers::new([CollisionLayer::AlienGoal], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                ));
            }

            if tile.features.contains(TileFlags::PlayerSpawn) {
                spawn_player_event_writer.send(SpawnPlayer {
                    position: Vec3::new(tile_definitions.tile_width * tile.x as f32 + tile_definitions.tile_width / 2.0, -tile_definitions.wall_height, tile_definitions.tile_width * tile.y as f32 + tile_definitions.tile_width / 2.0),
                });
            }
        }
    }
}

pub fn current_tile_system(
    mut current_tile_query: Query<(&Position, &mut CurrentTile)>
) {
    for (position, mut current_tile) in current_tile_query.iter_mut() {
        current_tile.tile = (((position.0.x / 2.0) as usize), ((position.0.z / 2.0) as usize));
    }
}