use std::io::Error;
// ! This example demonstrates how to create a custom mesh,
// ! assign a custom UV mapping for a custom texture,
// ! and how to change the UV mapping at run-time.
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use image::{ImageBuffer, Luma};
use crate::game_state::GameState;

// Define a "marker" component to mark the custom mesh. Marker components are often used in Bevy for
// filtering entities in queries with With, they're usually not queried directly since they don't contain information within them.
#[derive(Component)]
struct CustomUV;

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnEnter(GameState::Mesh), setup_mesh)
            .add_plugins(WorldIynspectorPlugin::new())
            .add_systems(Update, (mesh_input_handler).run_if(in_state(GameState::Mesh)));
    }
}

fn setup_mesh(
    mut commands: Commands,
    asset_server: ResMut<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Import the custom texture.
    let custom_texture_handle: Handle<Image> = asset_server.load("textures/array_texture.png");
    // Create and save a handle to the mesh.
    let cube_mesh_handle: Handle<Mesh> = meshes.add(load_terrain_mesh().unwrap());

    // Render the mesh with the custom texture using a PbrBundle, add the marker.
    commands.spawn((
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

// System to receive input from the user,
// check out examples/input/ for more examples about user input.
fn mesh_input_handler(
    keyboard_input: Res<Input<KeyCode>>,
    mesh_query: Query<&Handle<Mesh>, With<CustomUV>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<&mut Transform, With<CustomUV>>,
    time: Res<Time>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        let mesh_handle = mesh_query.get_single().expect("Query not successful");
        let mesh = meshes.get_mut(mesh_handle).unwrap();
        toggle_texture(mesh);
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
}

#[rustfmt::skip]
fn create_cube_mesh() -> Mesh {
    let height = 0.0;

    let rows = 2;
    let columns = 2;

    let quad_side = 2;
    let row_points = rows * quad_side;
    let column_points = columns * quad_side;
    let total_points = row_points * column_points;
    let quad_count = total_points / (quad_side * quad_side);

    let square_size = 1.0;
    /*
    1-2-3
    4-5-6
    */

    let vertices = (0..row_points).flat_map(|r| {
        (0..column_points).map(|c| {
            let x = c as f32 * square_size;
            let z = r as f32 * square_size;
            [x, height, z]
        }).collect::<Vec<[f32; 3]>>()
    }).collect::<Vec<[f32; 3]>>();

    // let vertices = (0..total_points).map(|i| {
    //     let x = i % column_points;
    //     let z = i / column_points + x;
    //     /*
    //     if x is even, then it is left
    //     if x is odd, then it is  right
    //
    //     if z is even, then it is top
    //     if z is odd, then it is bottom
    //      */
    //     let x = x as f32 * square_size;
    //     let z = z as f32 * square_size;
    //     [x, height, z]
    // }).collect::<Vec<[f32; 3]>>();

    let new_uvs = (0..quad_count).flat_map(|_i| {
        vec![
            [0.0, 0.25], [0.0, 0.0], [1.0, 0.0], [1.0, 0.25],
        ]
    }).collect::<Vec<[f32; 2]>>();

    /*
    No, they are PER VERTEX - every point has a normal.
    That "Makes sense" - we don't have triangles etc, we have a bunch of points.
    The amount of normals equals the amount of vertices.
    BUT the normal is the SAME for all vertices in a certain triangle
    Hence we can use a range instead and stride by 3

    0-1
    | |
    3-2

    0: 0-1, 0-3
    3: 3-0, 3-1
    1: 1-0, 1-3

    1: 1-3,1-2
    3: 3-1,3-2
    2: 2-1,2-3

    So, our basic theory is we have some structures that
    1. define triangles in triangle_one and triangle_two
    2. define the corners of triangles in triangles_one and triangles_two

    using these we can loop over triangle_one indexes and for each one
    of those calculate the surface normal for a triangle that has the
    point as the first point and the other two points as the other two points
    of the triangle to calculate for.

    This surface normal should always become the same, shouldn't it?
    The triangle is the same, bro. So what I had already done should suffice?

    But it is the shared points that are interesting. What do we do about
    them, eh?

     */

    /*
    0---1    4---5
    |i1 |    |i2 |
    3---2    7---6

    this is trivial stuff, mate, check the winding order.
    0, 3, 1, 1, 3, 2,
    4, 7, 5, 5, 7, 6,

    f1 = 0
    f2 = f1 + stride 4

     */

    let face_map = [0, 2, 1, 1, 2, 3];
    let stride = 4;

    /*
    Aha.

    What we WANT to do is to calculate the positions for our other corners
    and from those positions calculate the indices for the corners in the
    triangles that we want!

    So, a given index gives a given x and y, these in turn give us x2 and y2,
    x3 and y3 for the corners and from these we can calculate the indices, right?

    This time we need to stride forth!

    Nah, it is still the quad, bro
     */

    let tr_one:[[u32;2]; 3] = [[0,0], [0,1], [1,0]];
    let tr_two:[[u32;2]; 3] = [[1,0], [0,1], [1,1]];

    let indices = (0..row_points as u32).step_by(2).flat_map(|r| {
        (0..column_points as u32).step_by(2).flat_map(|c| {
            let p1 = [c + tr_one[0][0],r + tr_one[0][1]];
            let p2 = [c + tr_one[1][0],r + tr_one[1][1]];
            let p3 = [c + tr_one[2][0],r + tr_one[2][1]];

            let q1 = [c + tr_two[0][0],r + tr_two[0][1]];
            let q2 = [c + tr_two[1][0],r + tr_two[1][1]];
            let q3 = [c + tr_two[2][0],r + tr_two[2][1]];
            [
                position_to_index(p1[0], p1[1], column_points),
                position_to_index(p2[0], p2[1], column_points),
                position_to_index(p3[0], p3[1], column_points),
                position_to_index(q1[0], q1[1], column_points),
                position_to_index(q2[0], q2[1], column_points),
                position_to_index(q3[0], q3[1], column_points),
            ]


        }).collect::<Vec<u32>>()
    }
    ).collect::<Vec<u32>>();


    // let indices = (0..quad_count as u32).flat_map(|i|
    //     [
    //         face_map[0] + i * stride,
    //         face_map[1] + i * stride,
    //         face_map[2] + i * stride,
    //         face_map[3] + i * stride,
    //         face_map[4] + i * stride,
    //         face_map[5] + i * stride]
    // ).collect::<Vec<u32>>();

    Mesh::new(PrimitiveTopology::TriangleList)
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vertices,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            new_uvs,
        )
        .with_indices(Some(Indices::U32(
            indices
        )))
        .with_duplicated_vertices()
        .with_computed_flat_normals()
}

fn position_to_index(x: u32, y: u32, width: u32) -> u32 {
    y * width + x
}

// Function that changes the UV mapping of the mesh, to apply the other texture.
fn toggle_texture(mesh_to_change: &mut Mesh) {
    // Get a mutable reference to the values of the UV attribute, so we can iterate over it.
    let uv_attribute = mesh_to_change.attribute_mut(Mesh::ATTRIBUTE_UV_0).unwrap();
    // The format of the UV coordinates should be Float32x2.
    let VertexAttributeValues::Float32x2(uv_attribute) = uv_attribute else {
        panic!("Unexpected vertex format, expected Float32x2.");
    };

    // Iterate over the UV coordinates, and change them as we want.
    for uv_coord in uv_attribute.iter_mut() {
        // If the UV coordinate points to the upper, "dirt+grass" part of the texture...
        if (uv_coord[1] + 0.5) < 1.0 {
            // ... point to the equivalent lower, "sand+water" part instead,
            uv_coord[1] += 0.5;
        } else {
            // else, point back to the upper, "dirt+grass" part.
            uv_coord[1] -= 0.5;
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
            if    sy < 0
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

fn load_terrain_mesh() -> Result<Mesh, Error> {
    let filename = "assets/terrain.png";

    let side_length = 1f32;
    let max_height = 1f32;
    let terrain_bitmap = image::io::Reader::open(filename).unwrap().decode().unwrap();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    let heightmap = terrain_bitmap.as_luma16().unwrap();

    let mut vertices : Vec::<[f32; 3]> = Vec::new();
    let mut normals : Vec::<[f32; 3]> = Vec::new();
    let mut indices : Vec::<u32> = Vec::new();

    let vertex_number = ( (heightmap.height() + 1) *
        (heightmap.width() + 1) ) as usize;

    vertices.resize(vertex_number, [0.0f32, 0.0f32, 0.0f32]);
    normals.resize(vertex_number, [0.0f32, 1.0f32, 0.0f32]);
    let uvs = vec![[0.0, 0.0]; vertices.len()];


    let mut vertex_index = 0;
    for cy in 0..(heightmap.height() as i32 +1) {
        for cx in 0..(heightmap.width() as i32 +1) {
            let height = sample_vertex_height(cy, cx, heightmap);
            // println!("sampled height at y={:>3} x={:>3}  = {:>4}", cy, cx, height);

            vertices[vertex_index] = [cx as f32 * side_length,
                height * max_height,
                cy as f32 * side_length];
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

    assert!(indices.len() as u32 /  3 == 2  * heightmap.height() * (heightmap.width()) );


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