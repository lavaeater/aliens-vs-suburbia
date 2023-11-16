use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{Commands, EventReader, Has, Query, Res, With};
use bevy::scene::SceneBundle;
use bevy::utils::petgraph::matrix_graph::Zero;
use bevy_xpbd_3d::components::{Collider, CollisionLayers, RigidBody, Rotation};
use bevy_xpbd_3d::prelude::Position;
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::CurrentTile;
use crate::general::resources::map_resources::MapGraph;
use crate::player::components::general::{BuildingIndicator, IsBuilding};
use crate::player::events::building_events::{StartBuilding, StopBuilding};


pub fn start_build(
    mut start_evr: EventReader<StartBuilding>,
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    for start_event in start_evr.read() {
        commands.entity(start_event.0).insert(IsBuilding {});
        // let entity_commands = commands.spawn((
        //     Name::from("BuildingIndicator"),
        //     SceneBundle {
        //         scene: asset_server.load("floor_fab.glb#Scene0"),
        //         ..Default::default()
        //     },
        //     RigidBody::Kinematic,
        //     Collider::cuboid(tile_width, tile_depth, tile_width),
        //     Position::from(Vec3::new(tile_width * tile.x as f32, -2.0, tile_width * tile.y as f32)),
        //     CollisionLayers::new([CollisionLayer::BuildIndicator], []),
        //     CurrentTile::default(),
        // ));
    }
}

pub fn stop_build(
    mut stop_evr: EventReader<StopBuilding>,
    mut commands: Commands,
) {
    for start_event in stop_evr.read() {
        commands.entity(start_event.0).remove::<IsBuilding>();
    }
}

pub fn building_mode(
    mut builder_query: Query<(&CurrentTile, &Rotation, &Position), With<IsBuilding>>,
    map_graph: Res<MapGraph>,
    mut commands: Commands,
) {
    for (current_tile, rotation, position) in builder_query.iter_mut() {
        let neighbours = map_graph.grid.neighbours(current_tile.tile);
        /*
        Figure out how to "point" att one of the neighbours.
         */
        let player_direction =
            rotation.0.
                mul_vec3(Vec3::new(0.0, 0.0, -1.0))
                .xz()
                .normalize()
                .to_grid_neighbour();

        let desired_neighbour = ((current_tile.tile.0 as i32 + player_direction.0) as usize, (current_tile.tile.1 as i32 + player_direction.1) as usize);

         if map_graph.grid.has_vertex(desired_neighbour) {

         }
    }
}

trait ToGridNeighbour {
    fn to_grid_neighbour(&self) -> (i32, i32);
}

/// This is a trait that allows us to convert a Vec2 to a grid neighbour. vec has to be normalized
impl ToGridNeighbour for Vec2 {
    fn to_grid_neighbour(&self) -> (i32, i32) {
        let n = (*self).normalize();
        let x = if n.x.is_zero() { 0 } else { n.x.signum() as i32 };
        let y = if n.y.is_zero() { 0 } else { n.y.signum() as i32 };
        (x, y)
    }
}