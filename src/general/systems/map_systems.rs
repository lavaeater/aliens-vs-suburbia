use bevy::asset::AssetServer;
use bevy::math::{Quat, Vec3};
use bevy::asset::RenderAssetUsages;
use bevy::pbr::StandardMaterial;
use bevy::prelude::{Assets, Color, Commands, Has, Mesh, Mesh3d, MeshMaterial3d, MessageReader, MessageWriter, Name, Query, Res, ResMut, Resource, Transform};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::world_serialization::WorldAssetRoot;
use avian3d::prelude::{Collider, CollisionLayers, Position, RigidBody};
use pathfinding::grid::Grid;
use std::collections::HashSet;
use crate::alien::components::general::AlienCounter;
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::{AlienGoal, AlienSpawnPoint, CurrentTile, Floor, MapModelDefinitions};
use crate::general::events::map_events::{LoadMap, SpawnPlayer};
use crate::general::resources::map_resources::MapGraph;
use crate::settings::resources::GameSettings;
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::assets::assets_plugin::GameAssets;
use crate::map::{BitFlags, MapFeatures};
use crate::building::systems::ToWorldCoordinates;
use crate::player::components::{IsBuildIndicator, IsObstacle};
use crate::general::components::{Health, Indestructible};
use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::towers::components::{TowerSensor, TowerShooter};
use crate::ui::spawn_ui::AddHealthBar;
use avian3d::prelude::Sensor;
use crate::player::events::building_events::{AddTile, RemoveTile};


pub fn load_map_one(mut send_event: MessageWriter<LoadMap>) {
    let text = std::fs::read_to_string("assets/maps/level_01.ron")
        .expect("assets/maps/level_01.ron not found");
    let mut map: crate::general::components::map_components::MapFile =
        ron::from_str(&text).expect("Failed to parse assets/maps/level_01.ron");
    if map.generated {
        map = crate::map::map_generator::generate_suburb_map(map.seed, map.map_width, map.map_height);
    }
    send_event.write(LoadMap { map });
}

pub fn load_map_showcase(mut send_event: MessageWriter<LoadMap>) {
    let map = crate::map::map_generator::generate_showcase_map(42);
    send_event.write(LoadMap { map });
}


#[derive(Resource)]
pub struct TileDefinitions {
    pub tile_size: f32,
    #[allow(dead_code)]
    pub tile_basis: f32,
    pub tile_unit: f32,
    pub tile_width: f32,
    pub wall_height: f32,
    pub tile_depth: f32,
    pub floor_level: f32
}

#[allow(dead_code)]
impl TileDefinitions {
    pub fn new(tile_size: f32,
               tile_basis: f32,
               wall_basis: f32,
               tile_depth_basis: f32) -> Self {
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
            floor_level: -wall_height * 2.0,
        }
    }

    pub fn create_collider(&self, width: f32, height: f32, depth: f32) -> Collider {
        Collider::cuboid(width * self.tile_unit * 2.0, height * self.tile_unit * 2.0, depth * self.tile_unit * 2.0)
    }

    pub fn get_floor_position(&self, x: i32, y: i32) -> Vec3 {
        Vec3::new(self.tile_width * x as f32, self.floor_level, self.tile_width * y as f32)
    }
    
}

fn terrain_color_key(mf: BitFlags<MapFeatures>) -> [u8; 3] {
    if mf.contains(MapFeatures::Water)  { return [38,  90, 179]; }
    if mf.contains(MapFeatures::Mud)    { return [115, 82,  46]; }
    if mf.contains(MapFeatures::Snow)   { return [209, 224, 235]; }
    if mf.contains(MapFeatures::Rock)   { return [ 97,  89,  77]; }
    if mf.contains(MapFeatures::Grass)  { return [ 56, 140,  56]; }
    [140, 133, 122] // Floor (default)
}

#[allow(clippy::too_many_arguments)]
pub fn map_loader(
    mut load_map_event_reader: MessageReader<LoadMap>,
    mut spawn_player_event_writer: MessageWriter<SpawnPlayer>,
    mut commands: Commands,
    mut alien_counter: ResMut<AlienCounter>,
    mut map_graph: ResMut<MapGraph>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    game_assets: Res<GameAssets>,
    tile_defs: Res<TileDefinitions>,
    model_defs: Res<MapModelDefinitions>,
    game_settings: Res<GameSettings>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    mut wave_manager: Option<ResMut<crate::alien::wave_manager::WaveManager>>,
) {
    for load_map in load_map_event_reader.read() {
        let map_file = &load_map.map;
        let raw = &map_file.tiles;
        let rows = raw.len() + 2;
        let cols = raw[0].len() + 2;
        let mut padded = vec![vec![0u64; cols]; rows];
        for (r, row_data) in raw.iter().enumerate() {
            for (c, &v) in row_data.iter().enumerate() {
                padded[r + 1][c + 1] = v;
            }
        }
        let m = &padded;
        map_graph.path_finding_grid = Grid::new(cols, rows);

        // Single pass: build tile sets and handle functional markers.
        let mut floor_set: HashSet<(i32, i32)> = HashSet::new();
        let mut imp_all_set: HashSet<(i32, i32)> = HashSet::new();
        let mut imp_player_set: HashSet<(i32, i32)> = HashSet::new();
        let mut imp_alien_set: HashSet<(i32, i32)> = HashSet::new();

        for (row, cols_data) in m.iter().enumerate() {
            for (col, &raw) in cols_data.iter().enumerate() {
                if raw == 0 {
                    imp_all_set.insert((col as i32, row as i32));
                    continue;
                }
                let mf = BitFlags::<MapFeatures>::from_bits_truncate(raw);
                let for_players = mf.contains(MapFeatures::ImpassableForPlayers);
                let for_enemies = mf.contains(MapFeatures::ImpassableForEnemies);

                floor_set.insert((col as i32, row as i32));
                if !for_enemies { map_graph.path_finding_grid.add_vertex((col, row)); }

                if for_players && for_enemies      { imp_all_set.insert((col as i32, row as i32)); }
                else if for_players                { imp_player_set.insert((col as i32, row as i32)); }
                else if for_enemies                { imp_alien_set.insert((col as i32, row as i32)); }

                if mf.contains(MapFeatures::EnemySpawn) {
                    alien_counter.max_count = 100;
                    commands.spawn((
                        Name::from(format!("Alien Spawn Point {}:{}", col, row)),
                        AlienSpawnPoint::new(2.0),
                        WorldAssetRoot(game_assets.alien_construct.clone()),
                        RigidBody::Static,
                        WindWakerShaderBuilder::default().build(),
                        Collider::cuboid(0.5, 0.5, 0.45),
                        Position::from((col, row).to_world_coords(&tile_defs) + Vec3::new(0.0, -tile_defs.wall_height, 0.0)),
                        CollisionLayers::new([CollisionLayer::AlienSpawnPoint], [CollisionLayer::Player]),
                    ));
                }
                if mf.contains(MapFeatures::EnemyExit) {
                    map_graph.goal = (col, row);
                    commands.spawn((
                        Name::from(format!("Alien Goal {}:{}", col, row)),
                        AlienGoal,
                        WorldAssetRoot(game_assets.alien_construct.clone()),
                        RigidBody::Static,
                        WindWakerShaderBuilder::default().build(),
                        Collider::cuboid(0.5, 0.5, 0.45),
                        Position::from((col, row).to_world_coords(&tile_defs) + Vec3::new(0.0, -tile_defs.wall_height, 0.0)),
                        CollisionLayers::new([CollisionLayer::AlienGoal], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                    ));
                }
                if mf.contains(MapFeatures::PlayerSpawn) {
                    spawn_player_event_writer.write(SpawnPlayer {
                        position: (col, row).to_world_coords(&tile_defs) + Vec3::new(0.0, 1.0, 0.0),
                    });
                }
            }
        }

        // ── Floor colliders (greedy rectangle merge) ──────────────────────────
        {
            let floor_model_def = model_defs.definitions.get("floor").unwrap();
            let mut covered = vec![vec![false; cols]; rows];
            for row in 0..rows {
                for col in 0..cols {
                    if covered[row][col] || !floor_set.contains(&(col as i32, row as i32)) { continue; }
                    let mut max_col = col;
                    while max_col + 1 < cols && floor_set.contains(&((max_col + 1) as i32, row as i32)) && !covered[row][max_col + 1] {
                        max_col += 1;
                    }
                    let mut max_row = row;
                    'extend_floor: loop {
                        if max_row + 1 >= rows { break; }
                        #[allow(clippy::needless_range_loop)]
                        for c in col..=max_col {
                            if !floor_set.contains(&(c as i32, (max_row + 1) as i32)) || covered[max_row + 1][c] { break 'extend_floor; }
                        }
                        max_row += 1;
                    }
                    
                    for r in row..=max_row { for c in col..=max_col { covered[r][c] = true; } }
                    let w = (max_col - col + 1) as f32;
                    let h = (max_row - row + 1) as f32;
                    let center = Vec3::new(
                        tile_defs.tile_width * (col + max_col) as f32 / 2.0,
                        tile_defs.floor_level,
                        tile_defs.tile_width * (row + max_row) as f32 / 2.0,
                    );
                    commands.spawn((
                        Name::from(format!("Floor Collider {}:{} {}x{}", col, row, w as i32, h as i32)),
                        Floor {},
                        floor_model_def.rigid_body,
                        tile_defs.create_collider(floor_model_def.width * w, floor_model_def.height, floor_model_def.depth * h),
                        Position::from(center),
                        floor_model_def.create_collision_layers(),
                        WindWakerShaderBuilder::default().build(),
                    ));
                }
            }
        }

        // ── Floor visual mesh (per-terrain-type coloured quads) ───────────────
        {
            let mut terrain_quads: std::collections::HashMap<[u8;3], (Vec<[f32;3]>, Vec<[f32;3]>, Vec<[f32;2]>, Vec<u32>)> = Default::default();
            let tw = tile_defs.tile_width;
            let y_floor = tile_defs.floor_level;
            for row in 0..rows {
                for col in 0..cols {
                    let raw = m[row][col];
                    if raw == 0 { continue; }
                    let mf = BitFlags::<MapFeatures>::from_bits_truncate(raw);
                    let color_key = terrain_color_key(mf);
                    let entry = terrain_quads.entry(color_key).or_default();
                    let base = entry.0.len() as u32;
                    let (x0, x1) = (tw * (col as f32 - 0.5), tw * (col as f32 + 0.5));
                    let (z0, z1) = (tw * (row as f32 - 0.5), tw * (row as f32 + 0.5));
                    entry.0.extend_from_slice(&[[x0,y_floor,z0],[x1,y_floor,z0],[x1,y_floor,z1],[x0,y_floor,z1]]);
                    entry.1.extend_from_slice(&[[0.0,1.0,0.0];4]);
                    entry.2.extend_from_slice(&[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]);
                    entry.3.extend_from_slice(&[base,base+2,base+1, base,base+3,base+2]);
                }
            }
            for ([r,g,b], (positions, normals, uvs, indices)) in terrain_quads {
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD);
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
                mesh.insert_indices(Indices::U32(indices));
                let base_color = Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
                commands.spawn((
                    Name::from("Floor"),
                    Floor {},
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(materials.add(StandardMaterial { base_color, perceptual_roughness: 1.0, metallic: 0.0, ..Default::default() })),
                ));
            }
        }

        // ── Impassable cube colliders (greedy merge per category) ─────────────
        // Each tile category gets a solid cube: tilewidth × 3.0 tall × tilewidth.
        // Void tiles, tiles marked impassable for both → ImpassableAll (blocks everyone).
        // ImpassableForPlayers only → ImpassablePlayer.  ImpassableForEnemies only → ImpassableAlien.
        let block_half_h = 4.0_f32 / 3.0;
        let block_y = tile_defs.floor_level;
        let tw = tile_defs.tile_width;

        let imp_configs: [(&HashSet<(i32, i32)>, CollisionLayers); 3] = [
            (&imp_all_set,    CollisionLayers::new([CollisionLayer::ImpassableAll],    [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player])),
            (&imp_player_set, CollisionLayers::new([CollisionLayer::ImpassablePlayer], [CollisionLayer::Player])),
            (&imp_alien_set,  CollisionLayers::new([CollisionLayer::ImpassableAlien],  [CollisionLayer::Alien])),
        ];

        for (set, layers) in &imp_configs {
            let mut covered = vec![vec![false; cols]; rows];
            for row in 0..rows {
                for col in 0..cols {
                    if covered[row][col] || !set.contains(&(col as i32, row as i32)) { continue; }
                    let mut max_col = col;
                    while max_col + 1 < cols && set.contains(&((max_col + 1) as i32, row as i32)) && !covered[row][max_col + 1] {
                        max_col += 1;
                    }
                    let mut max_row = row;
                    'extend_imp: loop {
                        if max_row + 1 >= rows { break; }
                        for c in col..=max_col {
                            if !set.contains(&(c as i32, (max_row + 1) as i32)) || covered[max_row + 1][c] { break 'extend_imp; }
                        }
                        max_row += 1;
                    }
                    for r in row..=max_row { for c in col..=max_col { covered[r][c] = true; } }
                    let w = (max_col - col + 1) as f32;
                    let h = (max_row - row + 1) as f32;
                    let center = Vec3::new(tw * (col + max_col) as f32 / 2.0, block_y, tw * (row + max_row) as f32 / 2.0);
                    commands.spawn((
                        RigidBody::Static,
                        Collider::cuboid(tw * w, block_half_h, tw * h),
                        *layers,
                        Transform::from_translation(center),
                        Position::from(center),
                    ));
                }
            }
        }

        // Background plane so the void never shows around the map.
        let map_center_x = cols as f32 * tile_defs.tile_width * 0.5;
        let map_center_z = rows as f32 * tile_defs.tile_width * 0.5;
        let bg_size = (cols.max(rows) as f32 * tile_defs.tile_width * 4.0).max(200.0);
        let bg_y = tile_defs.floor_level - 0.05;
        let bg_mesh = meshes.add(bevy::math::primitives::Rectangle::new(bg_size, bg_size));
        let bg_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.22, 0.18),
            perceptual_roughness: 0.95,
            metallic: 0.0,
            ..Default::default()
        });
        commands.spawn((
            Name::from("BackgroundFloor"),
            Mesh3d(bg_mesh),
            MeshMaterial3d(bg_mat),
            Transform::from_xyz(map_center_x, bg_y, map_center_z)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        ));

        // Spawn editor-placed model defs.
        for placement in &map_file.placements {
            let Ok(text) = std::fs::read_to_string(&placement.def_path) else { continue };
            let Ok(def) = ron::from_str::<AssetDefinition>(&text) else { continue };

            let pos = Vec3::new(
                tile_defs.tile_width * placement.x as f32,
                tile_defs.floor_level + tile_defs.tile_depth,
                tile_defs.tile_width * placement.y as f32,
            );
            let rot = Quat::from_rotation_y(placement.rotation_steps as f32 * std::f32::consts::FRAC_PI_4);
            let scale = Vec3::splat(def.scale);

            let scene_handle = asset_server.load(
                bevy::gltf::GltfAssetLabel::Scene(0).from_asset(def.model_path.clone())
            );

            let tile_coord = (placement.x as usize, placement.y as usize);

            match &def.model_type {
                ModelType::Terrain(props) => {
                    let mut ec = commands.spawn((
                        Name::from(format!("Placement {}:{}", placement.x, placement.y)),
                        WorldAssetRoot(scene_handle),
                        Transform::from_translation(pos).with_rotation(rot).with_scale(scale),
                        CurrentTile { tile: tile_coord },
                        RigidBody::Static,
                    ));
                    if props.blocks_enemies {
                        ec.insert((
                            IsObstacle,
                            tile_defs.create_collider(16.0, 8.0, 16.0),
                            CollisionLayers::new([CollisionLayer::ImpassableAll], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                        ));
                        map_graph.path_finding_grid.remove_vertex(tile_coord);
                    }
                    match props.health {
                        Some(hp) => {
                            let hp_i = hp as i32;
                            ec.insert(Health { health: hp_i, max_health: hp_i });
                        }
                        None => { ec.insert(Indestructible); }
                    }
                }
                ModelType::Tower(props) => {
                    let hp = props.health as i32;
                    let range = props.range;
                    let fire_rate = props.fire_rate_per_minute;
                    let mut ec = commands.spawn((
                        Name::from(format!("Tower {}:{}", placement.x, placement.y)),
                        IsObstacle,
                        WorldAssetRoot(scene_handle),
                        Transform::from_translation(pos).with_rotation(rot).with_scale(scale),
                        tile_defs.create_collider(16.0, 8.0, 16.0),
                        CollisionLayers::new([CollisionLayer::ImpassableAll], [CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                        RigidBody::Static,
                        CurrentTile { tile: tile_coord },
                        Health { health: hp, max_health: hp },
                    ));
                    ec.with_children(|parent| {
                        parent.spawn((
                            Name::from("Sensor"),
                            Collider::cylinder(0.5, range),
                            CollisionLayers::new([CollisionLayer::Sensor], [CollisionLayer::Alien]),
                            Position::from(pos),
                            TowerSensor {},
                            TowerShooter::new(fire_rate),
                            Sensor,
                        ));
                    });
                    map_graph.path_finding_grid.remove_vertex(tile_coord);
                    add_health_bar_mw.write(AddHealthBar { entity: ec.id(), name: "TOWER" });
                }
                ModelType::Item(_) | ModelType::Player(_) | ModelType::Enemy(_) | ModelType::Weapon(_) => {
                    // Items, weapons and decorative enemies just spawn as scenes.
                    commands.spawn((
                        Name::from(format!("Item {}:{}", placement.x, placement.y)),
                        WorldAssetRoot(scene_handle),
                        Transform::from_translation(pos).with_rotation(rot).with_scale(scale),
                    ));
                }
            }
        }

        for dec in &map_file.decorations {
            let pos = Vec3::new(
                tile_defs.tile_width * dec.x as f32,
                tile_defs.floor_level + tile_defs.tile_depth,
                tile_defs.tile_width * dec.y as f32,
            );
            // dec.scale is expressed in player units; multiply by player_unit to get world-unit scale
            let world_scale = dec.scale * game_settings.player_unit;
            commands.spawn((
                Name::from(format!("Decoration {}:{} {}", dec.x, dec.y, dec.model)),
                WorldAssetRoot(asset_server.load(format!("{}#Scene0", dec.model))),
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(dec.rotation_y.to_radians()))
                    .with_scale(bevy::math::Vec3::splat(world_scale)),
            ));
        }

        // Override WaveManager with map-defined waves if any are present.
        if !map_file.waves.is_empty()
            && let Some(ref mut wm) = wave_manager
        {
            use crate::alien::wave_manager::WaveDef as WmWave;
            wm.waves = map_file.waves.iter().enumerate().map(|(i, w)| WmWave {
                alien_count: w.count as i32,
                spawn_rate_per_minute: w.spawn_rate_per_minute,
                delay_before: if i == 0 { 5.0 } else { 15.0 },
            }).collect();
            wm.current_wave = 0;
            wm.wave_timer = 5.0;
            wm.spawning = false;
            wm.spawned_this_wave = 0;
        }
    }
}

pub fn update_current_tile_system(
    mut current_tile_query: Query<(&Position, &mut CurrentTile, Has<IsBuildIndicator>)>,
    tile_definitions: Res<TileDefinitions>,
    mut map_graph: ResMut<MapGraph>,
) {
    map_graph.occupied_tiles.clear();
    for (position, mut current_tile, is_build_indicator) in current_tile_query.iter_mut() {
        current_tile.tile = ((((position.0.x + tile_definitions.tile_width / 2.0) / tile_definitions.tile_size) as usize), (((position.0.z + tile_definitions.tile_width / 2.0) / tile_definitions.tile_size) as usize));
        if !is_build_indicator {
            map_graph.occupied_tiles.insert(current_tile.tile);
        }
    }
}

pub fn remove_tile_from_map(
    mut remove_tile_evr: MessageReader<RemoveTile>,
    mut map_graph: ResMut<MapGraph>,
) {
    for remove_tile_event in remove_tile_evr.read() {
        map_graph.path_finding_grid.remove_vertex(remove_tile_event.0);
    }
}

pub fn add_tile_to_map(
    mut add_tile_evr: MessageReader<AddTile>,
    mut map_graph: ResMut<MapGraph>,
) {
    for add_tile_event in add_tile_evr.read() {
        map_graph.path_finding_grid.add_vertex(add_tile_event.0);
        map_graph.path_reopened = true;
    }
}
