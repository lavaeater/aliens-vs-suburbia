use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{Commands, EventReader, Query, Res, With};
use bevy::scene::{SceneBundle, SceneInstance};
use bevy_xpbd_3d::components::{CollisionLayers, LockedAxes, RigidBody, Rotation};
use bevy_xpbd_3d::prelude::Position;
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::CurrentTile;
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::general::{BuildingIndicator, IsBuilding, IsBuildIndicator};
use crate::player::events::building_events::{StartBuilding, StopBuilding};


pub fn start_build_mode(
    mut start_evr: EventReader<StartBuilding>,
    mut builder_query: Query<(&CurrentTile, &Rotation)>,
    asset_server: Res<AssetServer>,
    tile_definitions: Res<TileDefinitions>,
    mut commands: Commands,
) {
    for start_event in start_evr.read() {
        if let Ok((current_tile, rotation)) = builder_query.get_mut(start_event.0) {
            let desired_neighbour_pos =
                rotation
                    .get_neighbour(current_tile.tile)
                    .to_world_coords(&tile_definitions) + Vec3::new(0.0, -tile_definitions.wall_height + 0.1, 0.0);

            let building_indicator = commands.spawn((
                Name::from("BuildingIndicator"),
                IsBuildIndicator {},
                SceneBundle {
                    scene: asset_server.load("floor_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Kinematic,
                tile_definitions.create_floor_collider(),
                Position::from(desired_neighbour_pos),
                CollisionLayers::new([CollisionLayer::BuildIndicator], []),
                LockedAxes::new().lock_rotation_x().lock_rotation_z().lock_rotation_y(),
                CurrentTile::default(),
            )).id();

            commands.entity(start_event.0).insert(BuildingIndicator(building_indicator));
            commands.entity(start_event.0).insert(IsBuilding {});
        }
    }
}

pub fn stop_build_mode(
    mut stop_evr: EventReader<StopBuilding>,
    player_build_indicator_query: Query<&BuildingIndicator>,
    mut commands: Commands,
) {
    for stop_event in stop_evr.read() {
        if let Ok(bulding_indicator) = player_build_indicator_query.get(stop_event.0) {
            commands.entity(bulding_indicator.0).despawn_recursive();
        }
        commands.entity(stop_event.0).remove::<IsBuilding>();
        commands.entity(stop_event.0).remove::<BuildingIndicator>();
    }
}

pub fn building_mode(
    builder_query: Query<(&CurrentTile, &Rotation, &BuildingIndicator), With<IsBuilding>>,
    mut building_indicator_query: Query<(&CurrentTile, &Rotation, &mut Position, &SceneInstance), With<IsBuildIndicator>>,
    map_graph: Res<MapGraph>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_definitions: Res<TileDefinitions>,
) {
    for (current_tile, rotation, building_indicator) in builder_query.iter() {
        let desired_neighbour = rotation.get_neighbour(current_tile.tile);
        // if map_graph.grid.has_vertex(desired_neighbour) {
        //     //Color should be green
        // } else {
        //     //Color should be red
        // }
        if let Ok((current_tile, rotation, mut position, scene_instance)) = building_indicator_query.get_mut(building_indicator.0) {
            let desired_neighbour_pos = desired_neighbour.to_world_coords(&tile_definitions) + Vec3::new(0.0, -tile_definitions.wall_height + 0.1 , 0.0);
            position.0 = desired_neighbour_pos;
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

/// This is a trait that allows us to convert a Vec2 to a grid neighbour. vec has to be normalized
impl ToGridNeighbour for Rotation {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize) {
        let n = self.0.
            mul_vec3(Vec3::new(0.0, 0.0, -1.0))
            .xz()
            .normalize();

        let mut angle = n.angle_between(Vec2::new(1.0, 0.0)).to_degrees() as i32;

        angle = if angle.is_negative() { 360 + angle } else { angle };

        let x:i32 = match angle {
            0..=59 => { 1 }
            60..=119 => { 0 }
            120..=239 => { -1 }
            240..=299 => { 0 }
            300..=360 => { 1 }
            _ => { 1 }
        } + current_tile.0 as i32;

        let y:i32 = match angle {
            46..=134 => { -1 }
            135..=224 => { 0 }
            225..=314 => { 1 }
            315..=360 => { 0 }
            _ => { 0 }
        } + current_tile.1 as i32;

        ((if x.is_negative() { 0 } else { x as usize }), if y.is_negative() { 0 } else { y as usize })
    }
}