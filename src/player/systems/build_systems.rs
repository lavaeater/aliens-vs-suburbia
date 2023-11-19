use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::log::info;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{Commands, Entity, EventReader, EventWriter, Query, Res, ResMut, With, Without};
use bevy::scene::{SceneBundle, SceneInstance, SceneSpawner};
use bevy_xpbd_3d::components::{CollisionLayers, LockedAxes, RigidBody, Rotation};
use bevy_xpbd_3d::prelude::{Collider, Position};
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::{CurrentTile, ModelDefinitions};
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::general::{BuildingIndicator, IsBuilding, IsBuildIndicator, Controller, ControlCommands, IsObstacle};
use crate::player::events::building_events::{EnterBuildMode, ExitBuildMode, ExecuteBuild, RemoveTile, AddTile, ChangeBuildIndicator};


pub fn enter_build_mode(
    mut enter_build_mode_evr: EventReader<EnterBuildMode>,
    mut builder_query: Query<(&CurrentTile, &Rotation), Without<IsBuilding>>,
    asset_server: Res<AssetServer>,
    tile_definitions: Res<TileDefinitions>,
    mut commands: Commands,
) {
    for start_event in enter_build_mode_evr.read() {
        if let Ok((current_tile, rotation)) = builder_query.get_mut(start_event.0) {
            let desired_neighbour_pos =
                rotation
                    .get_neighbour(current_tile.tile)
                    .to_world_coords(&tile_definitions) + Vec3::new(0.0, -tile_definitions.wall_height * 2.0, 0.0);

            let building_indicator = spawn_building_indicator(
                &mut commands,
                &asset_server,
                &desired_neighbour_pos,
                "map/obstacle.glb#Scene0",
                &tile_definitions
            );
            commands.entity(start_event.0).insert(BuildingIndicator(
                building_indicator,
                0));
            commands.entity(start_event.0).insert(IsBuilding {});
        }
    }
}

pub fn spawn_building_indicator(
    mut commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    position: &Vec3,
    file: &'static str,
    tile_definitions: &TileDefinitions,
) -> Entity {
    commands.spawn((
        Name::from("BuildingIndicator"),
        IsBuildIndicator {},
        SceneBundle {
            scene: asset_server.load(file),
            ..Default::default()
        },
        RigidBody::Kinematic,
        tile_definitions.create_collider(16.0, 4.0, 16.0),
        Position::from(*position),
        CollisionLayers::new([CollisionLayer::BuildIndicator], []),
        LockedAxes::new().lock_rotation_x().lock_rotation_z().lock_rotation_y(),
        CurrentTile::default(),
    )).id()
}

pub fn exit_build_mode(
    mut exit_build_mode_evr: EventReader<ExitBuildMode>,
    mut player_build_indicator_query: Query<(&BuildingIndicator, &mut Controller), With<IsBuilding>>,
    mut commands: Commands,
) {
    for stop_event in exit_build_mode_evr.read() {
        if let Ok((bulding_indicator, mut controller)) = player_build_indicator_query.get_mut(stop_event.0) {
            controller.triggers.remove(&ControlCommands::Build);
            commands.entity(bulding_indicator.0).despawn_recursive();
        }
        commands.entity(stop_event.0).remove::<IsBuilding>();
        commands.entity(stop_event.0).remove::<BuildingIndicator>();
    }
}

pub fn remove_tile_from_map(
    mut remove_tile_evr: EventReader<RemoveTile>,
    mut map_graph: ResMut<MapGraph>,
) {
    for remove_tile_event in remove_tile_evr.read() {
        map_graph.path_finding_grid.remove_vertex(remove_tile_event.0);
    }
}

pub fn add_tile_to_map(
    mut add_tile_evr: EventReader<AddTile>,
    mut map_graph: ResMut<MapGraph>,
) {
    for add_tile_event in add_tile_evr.read() {
        map_graph.path_finding_grid.add_vertex(add_tile_event.0);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_build(
    mut commands: Commands,
    mut execute_evr: EventReader<ExecuteBuild>,
    mut exit_build_ew: EventWriter<ExitBuildMode>,
    mut remove_tile_ew: EventWriter<RemoveTile>,
    player_build_indicator_query: Query<&BuildingIndicator>,
    building_indicator: Query<(&Position, &CurrentTile), With<IsBuildIndicator>>,
    asset_server: Res<AssetServer>,
    tile_definitions: Res<TileDefinitions>,
    map_graph: Res<MapGraph>,
    model_defs: Res<ModelDefinitions>
) {
    for execute_event in execute_evr.read() {
        if let Ok(build_indicator) = player_build_indicator_query.get(execute_event.0) {
            if let Ok((position, current_tile)) = building_indicator.get(build_indicator.0) {
                if !map_graph.occupied_tiles.contains(&current_tile.tile) {

                    let current_index = build_indicator.1;
                    let current_key = model_defs.build_indicators[current_index as usize];
                    let model_def = model_defs.definitions.get(current_key).unwrap();

                    commands.spawn((
                        Name::from(current_key),
                        IsObstacle {}, // let this be, for now!
                        SceneBundle {
                            scene: asset_server.load(model_def.file),
                            ..Default::default()
                        },
                        RigidBody::Kinematic,
                        tile_definitions.create_collider(model_def.width, model_def.height, model_def.depth),
                        *position,
                        model_def.create_collision_layers(),
                        CurrentTile::default(),
                    ));
                    remove_tile_ew.send(RemoveTile(current_tile.tile));
                    exit_build_ew.send(ExitBuildMode(execute_event.0));
                }
            }
        }
    }
}

pub fn building_mode(
    builder_query: Query<(&CurrentTile, &Rotation, &BuildingIndicator), With<IsBuilding>>,
    mut building_indicator_query: Query<(&CurrentTile, &Rotation, &mut Position, &SceneInstance), With<IsBuildIndicator>>,
    tile_definitions: Res<TileDefinitions>,
) {
    for (current_tile, rotation, building_indicator) in builder_query.iter() {
        let desired_neighbour = rotation.get_neighbour(current_tile.tile);
        if let Ok((current_tile, rotation, mut position, scene_instance)) = building_indicator_query.get_mut(building_indicator.0) {
            let desired_neighbour_pos = desired_neighbour.to_world_coords(&tile_definitions) + Vec3::new(0.0, -tile_definitions.wall_height, 0.0);
            position.0 = desired_neighbour_pos;
        }
    }
}

pub fn change_build_indicator(
    mut change_build_indicator_evr: EventReader<ChangeBuildIndicator>,
    mut builder_query: Query<(&mut BuildingIndicator, &Position), With<IsBuilding>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    model_defs: Res<ModelDefinitions>,
    tile_defs: Res<TileDefinitions>
) {
    for change_build_event in change_build_indicator_evr.read() {
        if let Ok((mut building_indicator, position)) = builder_query.get_mut(change_build_event.0) {
            let current_index = building_indicator.1;

            building_indicator.1 = current_index + change_build_event.1;

            building_indicator.1 = if building_indicator.1 < 0 {
                model_defs.build_indicators.len() as i32 - 1
            } else if building_indicator.1 >= model_defs.build_indicators.len() as i32 {
                0
            } else {
                building_indicator.1
            };
            let indicator_key = model_defs.build_indicators[building_indicator.1 as usize];
            info!("Changing indicator to {}", indicator_key);

            /*
            We need to destroy the entity and re-create it. Buuummer!
             */

            let p = position.0.clone();
            commands.entity(building_indicator.0).despawn_recursive();
            building_indicator.0 = spawn_building_indicator(
                &mut commands,
                &asset_server,
                &p,
                model_defs
                    .definitions
                    .get(indicator_key)
                    .unwrap()
                    .file,
                &tile_defs
            );
        }
    }
}

pub trait ToGridNeighbour {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize);
}

pub trait ToWorldCoordinates {
    fn to_world_coords(&self, tile_definitions: &Res<TileDefinitions>) -> Vec3;
}

impl ToWorldCoordinates for (usize, usize) {
    fn to_world_coords(&self, tile_definitions: &Res<TileDefinitions>) -> Vec3 {
        Vec3::new(
            tile_definitions.tile_width * self.0 as f32,
            0.0,
            tile_definitions.tile_width * self.1 as f32,
        )
    }
}

impl ToGridNeighbour for Rotation {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize) {
        let n = self.0.
            mul_vec3(Vec3::new(0.0, 0.0, -1.0))
            .xz()
            .normalize();

        let mut angle = n.angle_between(Vec2::new(1.0, 0.0)).to_degrees() as i32;

        angle = if angle.is_negative() { 360 + angle } else { angle };

        let x: i32 = match angle {
            0..=59 => { 1 }
            60..=119 => { 0 }
            120..=239 => { -1 }
            240..=299 => { 0 }
            300..=360 => { 1 }
            _ => { 1 }
        } + current_tile.0 as i32;

        let y: i32 = match angle {
            46..=134 => { -1 }
            135..=224 => { 0 }
            225..=314 => { 1 }
            315..=360 => { 0 }
            _ => { 0 }
        } + current_tile.1 as i32;

        ((if x.is_negative() { 0 } else { x as usize }), if y.is_negative() { 0 } else { y as usize })
    }
}