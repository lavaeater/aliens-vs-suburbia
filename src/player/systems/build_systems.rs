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
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::general::{BuildingIndicator, IsBuilding};
use crate::player::events::building_events::{StartBuilding, StopBuilding};


pub fn start_build(
    mut start_evr: EventReader<StartBuilding>,
    mut commands: Commands,
) {
    for start_event in start_evr.read() {
        commands.entity(start_event.0).insert(IsBuilding {});
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
    mut builder_query: Query<(&CurrentTile, &Rotation, &Position, Has<BuildingIndicator>), With<IsBuilding>>,
    map_graph: Res<MapGraph>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_definitions: Res<TileDefinitions>,
) {
    for (current_tile, rotation, position, has_indicator) in builder_query.iter_mut() {
        if has_indicator {
            if map_graph.grid.has_vertex(desired_neighbour) {}
        } else {
            let entity_commands = commands.spawn((
                Name::from("BuildingIndicator"),
                SceneBundle {
                    scene: asset_server.load("floor_fab.glb#Scene0"),
                    ..Default::default()
                },
                RigidBody::Kinematic,
                tile_definitions.create_floor_collider(),
                Position::from(Vec3::new(tile_width * tile.x as f32, -2.0, tile_width * tile.y as f32)),
                CollisionLayers::new([CollisionLayer::BuildIndicator], []),
                CurrentTile::default(),
            ));
        }

    }
}

trait ToGridNeighbour {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize);
}

/// This is a trait that allows us to convert a Vec2 to a grid neighbour. vec has to be normalized
impl ToGridNeighbour for Rotation {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize) {
        let n = self.0.
            mul_vec3(Vec3::new(0.0, 0.0, -1.0))
            .xz()
            .normalize();
        let x = if n.x.is_zero() { 0 } else { n.x.signum() as i32 } + current_tile.0 as i32;
        let y = if n.y.is_zero() { 0 } else { n.y.signum() as i32 } + current_tile.1 as i32;
        ((if x.is_negative() { 0 } else {x as usize}), if y.is_negative() { 0 } else {y as usize})
    }
}