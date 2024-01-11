// ! This example demonstrates how to create a custom mesh,
// ! assign a custom UV mapping for a custom texture,
// ! and how to change the UV mapping at run-time.
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::utils::petgraph::visit::Walker;
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
    let cube_mesh_handle: Handle<Mesh> = meshes.add(create_cube_mesh());

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
    let mut height = 0.0;
    let min_x = -1.0;
    let min_z = -1.0;
    let max_x = 1.0;
    let max_z = 1.0;

    /*
    We could probably create some kind of cool mesh-generating thing pretty simply by just
    moving along a grid and changing the height every so slightly of all the vertices. We
    just have to remember the indices and stuff, just like before. It's not that hard, is it?

    0---1
    |f1 |
    3---2

    4---5
    |f2 |
    7---6


    this is trivial stuff, mate, check the winding order.
    0, 3, 1, 1, 3, 2,
f1 = 0
f2 = f1 + stride 4


    I want to declare a plane
    Ah, we just introduce the concept of STRIDE
    Every square on the plane is four values and the

     */

    let rows = 4;
    let columns = 5;
    let total_squares = rows * columns;
    let square_size = 0.5;
    let vertices = (0..total_squares).flat_map(|i| {
        let x = i % rows;
        let z = i / rows;
        let x = x as f32;
        let z = z as f32;
        let y = 0.0;
        height += 0.1;
        vec![
            [x, y, z],
            [x + square_size, y, z],
            [x + square_size, y, z + square_size],
            [x, y, z + square_size],
        ]
    }).collect::<Vec<[f32; 3]>>();


    let face_map = [0, 3, 1, 1, 3, 2];
    let indices = (0..total_squares).step_by(4).flat_map(|i|
        [face_map[0] + i, face_map[1] + i, face_map[2] + i, face_map[3] + i, face_map[4] + i, face_map[5] + i]
    ).collect::<Vec<u32>>();

    let normals = (0..total_squares).flat_map(|_i| {
        vec![
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
    }).collect::<Vec<[f32; 3]>>();

    let uvs = (0..total_squares).step_by(4).flat_map(|i| {
        vec![
            [0.0, 0.25], [0.0, 0.0], [1.0, 0.0], [1.0, 0.25],
        ]
    }).collect::<Vec<[f32; 2]>>();


    Mesh::new(PrimitiveTopology::TriangleList)
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vertices
            // Each array is an [x, y, z] coordinate in local space.
            // Meshes always rotate around their local [0, 0, 0] when a rotation is applied to their Transform.
            // By centering our mesh around the origin, rotating the mesh preserves its center of mass.
            // vec![
            //     // top (facing towards +y)
            //     [min_x, height, min_z], // vertex with index 0
            //     [max_x, height, min_z], // vertex with index 1
            //     [max_x, height, max_z], // etc. until 23
            //     [min_x, height, max_z],
            //     // // bottom   (-y)
            //     // [-bottom_side, -top_side, -bottom_side],
            //     // [bottom_side, -top_side, -bottom_side],
            //     // [bottom_side, -top_side, bottom_side],
            //     // [-bottom_side, -top_side, bottom_side],
            //     // // right    (+x)
            //     // [top_side, -top_side, -top_side],
            //     // [top_side, -top_side, top_side],
            //     // [top_side, top_side, top_side], // This vertex is at the same position as vertex with index 2, but they'll have different UV and normal
            //     // [top_side, top_side, -top_side],
            //     // // left     (-x)
            //     // [-top_side, -top_side, -top_side],
            //     // [-top_side, -top_side, top_side],
            //     // [-top_side, top_side, top_side],
            //     // [-top_side, top_side, -top_side],
            //     // // back     (+z)
            //     // [-top_side, -top_side, top_side],
            //     // [-top_side, top_side, top_side],
            //     // [top_side, top_side, top_side],
            //     // [top_side, -top_side, top_side],
            //     // // forward  (-z)
            //     // [-top_side, -top_side, -top_side],
            //     // [-top_side, top_side, -top_side],
            //     // [top_side, top_side, -top_side],
            //     // [top_side, -top_side, -top_side],
            // ],
        )
        // Set-up UV coordinated to point to the upper (V < 0.5), "dirt+grass" part of the texture.
        // Take a look at the custom image (assets/textures/array_texture.png)
        // so the UV coords will make more sense
        // Note: (0.0, 0.0) = Top-Left in UV mapping, (1.0, 1.0) = Bottom-Right in UV mapping
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            uvs,
            // vec![
            //     // Assigning the UV coords for the top side.
            //     [0.0, 0.25], [0.0, 0.0], [1.0, 0.0], [1.0, 0.25],
            //     // Assigning the UV coords for the bottom side.
            //     // [0.0, 0.45], [0.0, 0.25], [1.0, 0.25], [1.0, 0.45],
            //     // // Assigning the UV coords for the right side.
            //     // [1.0, 0.45], [0.0, 0.45], [0.0, 0.2], [1.0, 0.2],
            //     // // Assigning the UV coords for the left side.
            //     // [1.0, 0.45], [0.0, 0.45], [0.0, 0.2], [1.0, 0.2],
            //     // // Assigning the UV coords for the back side.
            //     // [0.0, 0.45], [0.0, 0.2], [1.0, 0.2], [1.0, 0.45],
            //     // // Assigning the UV coords for the forward side.
            //     // [0.0, 0.45], [0.0, 0.2], [1.0, 0.2], [1.0, 0.45],
            // ],
        )
        // For meshes with flat shading, normals are orthogonal (pointing out) from the direction of
        // the surface.
        // Normals are required for correct lighting calculations.
        // Each array represents a normalized vector, which length should be equal to 1.0.
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            normals
            // vec![
            //     // Normals for the top side (towards +y)
            //     [0.0, 1.0, 0.0],
            //     [0.0, 1.0, 0.0],
            //     [0.0, 1.0, 0.0],
            //     [0.0, 1.0, 0.0],
            //     // // Normals for the bottom side (towards -y)
            //     // [0.0, -1.0, 0.0],
            //     // [0.0, -1.0, 0.0],
            //     // [0.0, -1.0, 0.0],
            //     // [0.0, -1.0, 0.0],
            //     // // Normals for the right side (towards +x)
            //     // [1.0, 0.0, 0.0],
            //     // [1.0, 0.0, 0.0],
            //     // [1.0, 0.0, 0.0],
            //     // [1.0, 0.0, 0.0],
            //     // // Normals for the left side (towards -x)
            //     // [-1.0, 0.0, 0.0],
            //     // [-1.0, 0.0, 0.0],
            //     // [-1.0, 0.0, 0.0],
            //     // [-1.0, 0.0, 0.0],
            //     // // Normals for the back side (towards +z)
            //     // [0.0, 0.0, 1.0],
            //     // [0.0, 0.0, 1.0],
            //     // [0.0, 0.0, 1.0],
            //     // [0.0, 0.0, 1.0],
            //     // // Normals for the forward side (towards -z)
            //     // [0.0, 0.0, -1.0],
            //     // [0.0, 0.0, -1.0],
            //     // [0.0, 0.0, -1.0],
            //     // [0.0, 0.0, -1.0],
            // ],
        )
        // Create the triangles out of the 24 vertices we created.
        // To construct a square, we need 2 triangles, therefore 12 triangles in total.
        // To construct a triangle, we need the indices of its 3 defined vertices, adding them one
        // by one, in a counter-clockwise order (relative to the position of the viewer, the order
        // should appear counter-clockwise from the front of the triangle, in this case from outside the cube).
        // Read more about how to correctly build a mesh manually in the Bevy documentation of a Mesh,
        // further examples and the implementation of the built-in shapes.
        .with_indices(Some(Indices::U32(
            indices
            // vec![
            // 0, 3, 1, 1, 3, 2, // triangles making up the top (+y) facing side.
            // 4, 5, 7, 5, 6, 7, // bottom (-y)
            // 8, 11, 9, 9, 11, 10, // right (+x)
            // 12, 13, 15, 13, 14, 15, // left (-x)
            // 16, 19, 17, 17, 19, 18, // back (+z)
            // 20, 21, 23, 21, 22, 23, // forward (-z)
        // ]
        )))
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