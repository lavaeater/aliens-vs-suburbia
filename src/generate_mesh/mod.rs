
use std::io::Error;
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::components::{Collider, RigidBody};
use image::{ImageBuffer, Luma};
use noise::{Fbm, Perlin};
use crate::game_state::GameState;
use crate::general::components::map_components::CoolDown;
use crate::towers::systems::BallBundle;

#[derive(Component)]
struct CustomUV;

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<BallTimer>()
            .init_resource::<MapBuilder>()
            .add_systems(OnEnter(GameState::Mesh), setup_mesh)
            .add_plugins(WorldInspectorPlugin::new())
            .add_systems(Update, (mesh_input_handler, spawn_balls).run_if(in_state(GameState::Mesh)));
    }
}

#[derive(Resource, Default)]
pub struct BallTimer {
    pub drop_balls: bool,
    pub time: f32,
}

impl CoolDown for BallTimer {
    fn cool_down(&mut self, delta_time: f32) -> bool {
        self.time -= delta_time;
        if self.time < 0.0 {
            self.time = 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Resource)]
pub struct MapBuilder {
    pub noise_function: Fbm::<Perlin>,
}

impl Default for MapBuilder {
    fn default() -> Self {
        Self {
            noise_function: Fbm::<Perlin>::new(0)
        }
    }
}

fn setup_mesh(
    mut commands: Commands,
    asset_server: ResMut<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    perlin_noise_resource: Res<MapBuilder>
) {
    // Import the custom texture.
    let custom_texture_handle: Handle<Image> = asset_server.load("textures/array_texture.png");
    // Create and save a handle to the mesh.
    let actual_mesh = load_terrain_mesh(perlin_noise_resource).unwrap();
    let cube_mesh_handle: Handle<Mesh> = meshes.add(actual_mesh.clone());

    // Render the mesh with the custom texture using a PbrBundle, add the marker.
    commands.spawn((
        RigidBody::Static,
        Collider::trimesh_from_mesh(&actual_mesh).unwrap(),
        PbrBundle {
            mesh: cube_mesh_handle,
            material: materials.add(StandardMaterial {
                base_color_texture: Some(custom_texture_handle),
                ..default()
            }),
            ..default()
        },
        CustomUV,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(1.8, 1.8, 1.8).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn(Camera3dBundle {
        transform: camera_and_light_transform,
        ..default()
    });

    // Light up the scene.
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1000.0,
            range: 100.0,
            ..default()
        },
        transform: camera_and_light_transform,
        ..default()
    });

    // Text to describe the controls.
    commands.spawn(
        TextBundle::from_section(
            "Controls:\nSpace: Change UVs\nX/Y/Z: Rotate\nR: Reset orientation",
            TextStyle {
                font_size: 20.0,
                ..default()
            },
        )
            .with_style(Style {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                ..default()
            }),
    );
}

fn spawn_balls(
    mut commands: Commands,
    asset_server: ResMut<AssetServer>,
    mut ball_timer: ResMut<BallTimer>,
    time: Res<Time>,
) {
    if ball_timer.drop_balls && ball_timer.cool_down(time.delta_seconds()) {
        commands.spawn(
            BallBundle::new(Vec3::new(1.0, 1.5, 0.0), Vec3::new(0.01, 0.0, 0.01), &asset_server)
        );
    }
}

// System to receive input from the user,
// check out examples/input/ for more examples about user input.
fn mesh_input_handler(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<CustomUV>>,
    time: Res<Time>,
    mut ball_timer: ResMut<BallTimer>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        ball_timer.drop_balls = !ball_timer.drop_balls;
        // let mesh_handle = mesh_query.get_single().expect("Query not successful");
        // let mesh = meshes.get_mut(mesh_handle).unwrap();
        // toggle_texture(mesh);
    }
    if keyboard_input.pressed(KeyCode::X) {
        for mut transform in &mut query {
            transform.rotate_x(time.delta_seconds() / 1.2);
        }
    }
    if keyboard_input.pressed(KeyCode::Y) {
        for mut transform in &mut query {
            transform.rotate_y(time.delta_seconds() / 1.2);
        }
    }
    if keyboard_input.pressed(KeyCode::Z) {
        for mut transform in &mut query {
            transform.rotate_z(time.delta_seconds() / 1.2);
        }
    }
    if keyboard_input.pressed(KeyCode::R) {
        for mut transform in &mut query {
            transform.look_to(Vec3::NEG_Z, Vec3::Y);
        }
    }

    if keyboard_input.pressed(KeyCode::Up) {
        for mut transform in &mut query {
            transform.translation.z += time.delta_seconds() * 2.0;
        }
    }
    if keyboard_input.pressed(KeyCode::Down) {
        for mut transform in &mut query {
            transform.translation.z -= time.delta_seconds() * 2.0;
        }
    }

    if keyboard_input.pressed(KeyCode::Left) {
        for mut transform in &mut query {
            transform.translation.x += time.delta_seconds() * 2.0;
        }
    }
    if keyboard_input.pressed(KeyCode::Right) {
        for mut transform in &mut query {
            transform.translation.x -= time.delta_seconds() * 2.0;
        }
    }
}

fn sample_vertex_height(cy: i32, cx: i32, heightmap: &ImageBuffer<Luma<u16>, Vec::<u16>>) -> f32 {
    let mut cnt = 0;
    let mut height = 0.0;

    for dy in [-1, 0].iter() {
        for dx in [-1, 0].iter() {
            let sy = cy + dy;
            let sx = cx + dx;
            if sy < 0
                || sx < 0
                || sy >= heightmap.height() as i32
                || sx >= heightmap.width() as i32 {
                continue;
            } else {
                height += heightmap.get_pixel(
                    sx as u32, sy as u32).0[0] as f32 * 1.0f32 / std::u16::MAX as f32;
                cnt += 1;
            }
        }
    }

    height / cnt as f32
}

fn load_terrain_mesh(perlin_noise_resource: Res<MapBuilder>) -> Result<Mesh, Error> {
    let filename = "assets/terrain_3.png";

    let side_length = 0.125f32;
    let max_height = 0.5f32;
    let terrain_bitmap = image::io::Reader::open(filename).unwrap().decode().unwrap();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    let heightmap = terrain_bitmap.as_luma16().unwrap();

    let mut vertices: Vec::<[f32; 3]> = Vec::new();
    let mut normals: Vec::<[f32; 3]> = Vec::new();
    let mut indices: Vec::<u32> = Vec::new();

    let vertex_number = ((heightmap.height() + 1) *
        (heightmap.width() + 1)) as usize;

    vertices.resize(vertex_number, [0.0f32, 0.0f32, 0.0f32]);
    normals.resize(vertex_number, [0.0f32, 1.0f32, 0.0f32]);
    let mut uvs = vec![[0.0, 0.25]; vertices.len()];

    let mut vertex_index = 0;
    for cy in 0..(heightmap.height() as i32 + 1) {
        for cx in 0..(heightmap.width() as i32 + 1) {
            let height = sample_vertex_height(cy, cx, heightmap);
            // println!("sampled height at y={:>3} x={:>3}  = {:>4}", cy, cx, height);



            vertices[vertex_index] = [cx as f32 * side_length,
                height * max_height,
                // (perlin_noise_resource.noise_function.get([cx as f64, cy as f64]) as f32),
                cy as f32 * side_length];

            //[0.0, 0.25], [0.0, 0.0], [1.0, 0.0], [1.0, 0.25]

            let mod_x = cx % 2;
            let mod_y = cy % 2;
            match (mod_x, mod_y)
            {
                (0, 0) => { uvs[vertex_index] = [0.0, 0.25] }
                (1, 0) => { uvs[vertex_index] = [0.0, 0.0] }
                (0, 1) => { uvs[vertex_index] = [1.0, 0.0] }
                (_, _) => { uvs[vertex_index] = [1.0, 0.25] }
            }

            vertex_index += 1;
        }
    }

    let grid_height = heightmap.height() + 1;
    let grid_width = heightmap.width() + 1;

    for cy in 0..(heightmap.height()) {
        for cx in 0..(heightmap.width()) {
            indices.extend([
                cy * grid_width + cx,
                (cy + 1) * grid_width + cx + 1,
                cy * grid_width + cx + 1,
            ].iter());
            indices.extend([
                cy * grid_width + cx,
                (cy + 1) * grid_width + cx,
                (cy + 1) * grid_width + cx + 1,
            ].iter());
        }
    }

    // for i in 0..(indices.len()/3) {
    //     println!("triangle {:03}: {} {} {} ",
    //         i, indices[i*3], indices[i*3+1], indices[i*3+2])
    // }

    // println!(" {} {} ", indices.len() / 3, 2  * heightmap.height() * (heightmap.width()));

    assert!(indices.len() as u32 / 3 == 2 * heightmap.height() * (heightmap.width()));


    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(vertices));
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(normals));
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        VertexAttributeValues::Float32x2(uvs));
    mesh.set_indices(Some(Indices::U32(indices)));


    Ok(mesh)
}