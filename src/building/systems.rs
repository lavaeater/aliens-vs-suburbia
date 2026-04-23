use bevy::asset::AssetServer;
use bevy::log::info;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{AlphaMode, Assets, Children, Color, Commands, Component, Entity, MeshMaterial3d, MessageReader, MessageWriter, Name, Query, Res, ResMut, StandardMaterial, With, Without};
use bevy::scene::{SceneRoot, SceneInstance, SceneSpawner};
use avian3d::prelude::{Collider, CollisionLayers, LockedAxes, Position, RigidBody, Rotation, Sensor};
use crate::control::components::{ControlCommand, CharacterControl};
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::{CurrentTile, MapModelDefinitions};
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::{BuildingIndicator, IsBuildIndicator, IsBuilding, IsObstacle};
use crate::player::events::building_events::{ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode, RemoveTile};
use crate::towers::components::{TowerSensor, TowerShooter};
use crate::towers::events::BuildTower;
use crate::ui::spawn_ui::AddHealthBar;

/// Tracks materials cloned from the indicator model's scene children so they can be tinted.
#[derive(Component, Default)]
pub struct BuildIndicatorTint {
    handles: Vec<bevy::asset::Handle<StandardMaterial>>,
    initialized: bool,
}

pub fn enter_build_mode(
    mut enter_build_mode_evr: MessageReader<EnterBuildMode>,
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
                &tile_definitions,
            );
            commands.entity(start_event.0).insert(BuildingIndicator(building_indicator, 0));
            commands.entity(start_event.0).insert(IsBuilding {});
        }
    }
}

pub fn spawn_building_indicator(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    position: &Vec3,
    file: &'static str,
    tile_definitions: &TileDefinitions,
) -> Entity {
    commands.spawn((
        Name::from("BuildingIndicator"),
        IsBuildIndicator {},
        BuildIndicatorTint::default(),
        SceneRoot(asset_server.load(file)),
        RigidBody::Kinematic,
        tile_definitions.create_collider(16.0, 4.0, 16.0),
        Position::from(*position),
        CollisionLayers::new([CollisionLayer::BuildIndicator], [CollisionLayer::Floor; 0]),
        LockedAxes::new().lock_rotation_x().lock_rotation_z().lock_rotation_y(),
        CurrentTile::default(),
    )).id()
}

fn collect_descendants(entity: Entity, children_q: &Query<&Children>, out: &mut Vec<Entity>) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            out.push(*child);
            collect_descendants(*child, children_q, out);
        }
    }
}

/// On first frame after the scene is ready, clone each mesh's material with alpha blending
/// and a green tint. Stores the handles so `update_build_indicator_tint` can change the color.
pub fn init_build_indicator_tint(
    mut indicators: Query<(Entity, &SceneInstance, &mut BuildIndicatorTint), With<IsBuildIndicator>>,
    scene_spawner: Res<SceneSpawner>,
    children_q: Query<&Children>,
    mut mat_q: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, scene_instance, mut tint) in &mut indicators {
        if tint.initialized { continue; }
        if !scene_spawner.instance_is_ready(scene_instance.0) { continue; }

        let mut descendants = Vec::new();
        collect_descendants(entity, &children_q, &mut descendants);

        for desc in descendants {
            if let Ok(mut mat_handle) = mat_q.get_mut(desc) {
                if let Some(new_mat) = materials.get(&mat_handle.0).cloned() {
                    let mut tinted = new_mat;
                    tinted.alpha_mode = AlphaMode::Blend;
                    tinted.base_color = Color::srgba(0.2, 1.0, 0.2, 0.55);
                    let handle = materials.add(tinted);
                    tint.handles.push(handle.clone());
                    mat_handle.0 = handle;
                }
            }
        }
        tint.initialized = true;
    }
}

/// Each frame, update tint color to green (free) or red (occupied).
pub fn update_build_indicator_tint(
    indicators: Query<(&CurrentTile, &BuildIndicatorTint), With<IsBuildIndicator>>,
    map_graph: Res<MapGraph>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (tile, tint) in &indicators {
        if !tint.initialized { continue; }
        let color = if map_graph.occupied_tiles.contains(&tile.tile) {
            Color::srgba(1.0, 0.2, 0.2, 0.55)
        } else {
            Color::srgba(0.2, 1.0, 0.2, 0.55)
        };
        for handle in &tint.handles {
            if let Some(mat) = materials.get_mut(handle) {
                mat.base_color = color;
            }
        }
    }
}

pub fn exit_build_mode(
    mut exit_build_mode_evr: MessageReader<ExitBuildMode>,
    mut player_build_indicator_query: Query<(&BuildingIndicator, &mut CharacterControl), With<IsBuilding>>,
    mut commands: Commands,
) {
    for stop_event in exit_build_mode_evr.read() {
        if let Ok((bulding_indicator, mut controller)) = player_build_indicator_query.get_mut(stop_event.0) {
            controller.triggers.remove(&ControlCommand::Build);
            commands.entity(bulding_indicator.0).despawn();
        }
        commands.entity(stop_event.0).remove::<IsBuilding>();
        commands.entity(stop_event.0).remove::<BuildingIndicator>();
    }
}

pub fn execute_build(
    mut execute_evr: MessageReader<ExecuteBuild>,
    mut remove_tile_mw: MessageWriter<RemoveTile>,
    player_build_indicator_query: Query<&BuildingIndicator>,
    building_indicator: Query<(&Position, &CurrentTile), With<IsBuildIndicator>>,
    map_graph: Res<MapGraph>,
    model_defs: Res<MapModelDefinitions>,
    mut build_tower_mw: MessageWriter<BuildTower>,
) {
    for execute_event in execute_evr.read() {
        if let Ok(build_indicator) = player_build_indicator_query.get(execute_event.0) {
            if let Ok((position, current_tile)) = building_indicator.get(build_indicator.0) {
                if !map_graph.occupied_tiles.contains(&current_tile.tile) {

                    let current_index = build_indicator.1;
                    let current_key = model_defs.build_indicators[current_index as usize];
                    build_tower_mw.write(BuildTower {
                        position: position.0,
                        model_definition_key: current_key,
                    });

                    remove_tile_mw.write(RemoveTile(current_tile.tile));
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
        if let Ok((_, _, mut position, _)) = building_indicator_query.get_mut(building_indicator.0) {
            let desired_neighbour_pos = desired_neighbour.to_world_coords(&tile_definitions) + Vec3::new(0.0, -tile_definitions.wall_height, 0.0);
            position.0 = desired_neighbour_pos;
        }
    }
}

pub fn change_build_indicator(
    mut change_build_indicator_evr: MessageReader<ChangeBuildIndicator>,
    mut builder_query: Query<(&mut BuildingIndicator, &Position), With<IsBuilding>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    model_defs: Res<MapModelDefinitions>,
    tile_defs: Res<TileDefinitions>,
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

            let p = position.0;
            commands.entity(building_indicator.0).despawn();
            building_indicator.0 = spawn_building_indicator(
                &mut commands,
                &asset_server,
                &p,
                model_defs
                    .definitions
                    .get(indicator_key)
                    .unwrap()
                    .file,
                &tile_defs,
            );
        }
    }
}

pub trait ToGridNeighbour {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize);
}

pub trait ToWorldCoordinates {
    fn to_world_coords(&self, tile_definitions: &TileDefinitions) -> Vec3;
}

impl ToWorldCoordinates for (usize, usize) {
    fn to_world_coords(&self, tile_definitions: &TileDefinitions) -> Vec3 {
        Vec3::new(
            tile_definitions.tile_width * self.0 as f32,
            0.0,
            tile_definitions.tile_width * self.1 as f32,
        )
    }
}

impl ToGridNeighbour for Rotation {
    fn get_neighbour(&self, current_tile: (usize, usize)) -> (usize, usize) {
        let n = self.0
            .mul_vec3(Vec3::new(0.0, 0.0, -1.0))
            .xz()
            .normalize();

        let mut angle = n.angle_to(Vec2::new(1.0, 0.0)).to_degrees() as i32;

        angle = if angle.is_negative() { 360 + angle } else { angle };

        let x: i32 = match angle {
            0..=59 => 1,
            60..=119 => 0,
            120..=239 => -1,
            240..=299 => 0,
            300..=360 => 1,
            _ => 1,
        } + current_tile.0 as i32;

        let y: i32 = match angle {
            46..=134 => -1,
            135..=224 => 0,
            225..=314 => 1,
            315..=360 => 0,
            _ => 0,
        } + current_tile.1 as i32;

        ((if x.is_negative() { 0 } else { x as usize }), if y.is_negative() { 0 } else { y as usize })
    }
}

pub fn build_tower_system(
    mut build_tower_mr: MessageReader<BuildTower>,
    mut commands: Commands,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    asset_server: Res<AssetServer>,
    model_defs: Res<MapModelDefinitions>,
    tile_defs: Res<TileDefinitions>,
) {
    for build_tower in build_tower_mr.read() {
        let model_def = model_defs.definitions.get(build_tower.model_definition_key).unwrap();
        let mut ec = commands.spawn((
            Name::from(model_def.name),
            IsObstacle {},
            SceneRoot(asset_server.load(model_def.file)),
            model_def.rigid_body,
            tile_defs.create_collider(model_def.width, model_def.height, model_def.depth),
            Position::from(build_tower.position),
            model_def.create_collision_layers(),
            CurrentTile::default(),
            Health::default(),
        ));

        if build_tower.model_definition_key == "tower" {
            ec.with_children(|parent| {
                parent.spawn((
                    Name::from("Sensor"),
                    Collider::cylinder(0.5, 2.0),
                    CollisionLayers::new([CollisionLayer::Sensor], [CollisionLayer::Alien]),
                    Position::from(build_tower.position),
                    TowerSensor {},
                    TowerShooter::new(20.0),
                    Sensor,
                ));
            });
        }

        let id = ec.id();
        add_health_bar_mw.write(AddHealthBar {
            entity: id,
            name: "OBSTACLE",
        });
    }
}
