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



    I want to declare a plane
    Ah, we just introduce the concept of STRIDE
    Every square on the plane is four values and the

    Height can only change in the connection between two squares, so we can just


     */

    /*
    The below structures are reasoning about the map as a collection of SQUARES
    which is what it is, but a terrain is a collection of vertices connected by triangles,
    we need to figure out how to connect them all instead, that is what this is all about.

    This can be accomplished by utilizing the stride, again.

     */

    let quad_side = 2;
    let row_points = 2 * quad_side;
    let column_points = 2 * quad_side;
    let total_points = row_points * column_points;
    let quad_count = total_points / (quad_side * quad_side);

    let square_size = 1.0;
    /*
    1-2-3
    4-5-6
    */
    let new_vertices = (0..total_points).map(|i| {
        let x = i / row_points;
        let z = i % row_points;
        /*
        if x is even, then it is left
        if x is odd, then it is  right

        if z is even, then it is top
        if z is odd, then it is bottom
         */
        let x = x as f32;
        let z = z as f32;
        [x, height, z]
    }).collect::<Vec<[f32; 3]>>();

    let new_uvs = (0..quad_count).flat_map(|_i| {
        vec![
            [0.0, 0.25], [0.0, 0.0], [1.0, 0.0], [1.0, 0.25],
        ]
    }).collect::<Vec<[f32; 2]>>();

    /*
    Ah, normals are PER FACE OF A QUAD; I assume, what the fuck is going on?
    No, they are PER VERTEX - every point has a normal.
    That "Makes sense" - we don't have triangles etc, we have a bunch of points.
    The amount of normals equals the amount of vertices.
    BUT the normal is the SAME for all vertices in a certain triangle
    Hence we can use a range instead and stride by 3

    0-1
    | |
    3-2

     */

    let tr_1 = [0, 3, 1];
    let tr_2 = [1, 3, 2];

    /*
    We should do it PER point and then simply use the indexes for a triangle with
    its... something
     */
    let new_normals = (0..quad_count).flat_map(|index| {
        /*
        index here is... what?
        quad_index, so from quadindex we calculate the
        four normals for this quad's vertices... is that
        enough then? Sure is, we won't return, will we?
         */
        let normal_1 =
            calculate_surface_normal(
                new_vertices[index + tr_1[0]],
                new_vertices[index + tr_1[1]],
                new_vertices[index + tr_1[2]],
            );
        let normal_2 = calculate_surface_normal(
            new_vertices[index + tr_2[0]],
            new_vertices[index + tr_2[1]],
            new_vertices[index + tr_2[2]],
        );
        vec![
            normal_1,
            normal_1,
            normal_2,
            normal_2,
        ]
    }).collect::<Vec<[f32; 3]>>();


    let mut y = 0.0;
    let vertices = (0..quad_count).flat_map(|i| {
        let x = i / row_points;
        let z = i % row_points;
        let x = x as f32;
        let z = z as f32;
        y += 0.25;
        vec![
            [x, y, z], //top left
            [x + square_size, y, z], //top right
            [x + square_size, y, z + square_size], //bottom right
            [x, y, z + square_size], //bottom left
        ]
    }).collect::<Vec<[f32; 3]>>();

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

    let face_map = [0, 3, 1, 1, 3, 2];
    let stride = 4;
    let indices = (0..quad_count).flat_map(|i|
        [face_map[0] + i * stride, face_map[1] + i * stride, face_map[2] + i * stride, face_map[3] + i * stride, face_map[4] + i * stride, face_map[5] + i * stride]
    ).collect::<Vec<u32>>();

    /*
    We also need to calculate the normals, bro.
     */

    let normals = (0..quad_count).flat_map(|_i| {
        vec![
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
    }).collect::<Vec<[f32; 3]>>();

    let uvs = (0..quad_count).flat_map(|_i| {
        vec![
            [0.0, 0.25], [0.0, 0.0], [1.0, 0.0], [1.0, 0.25],
        ]
    }).collect::<Vec<[f32; 2]>>();


    Mesh::new(PrimitiveTopology::TriangleList)
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vertices,
        )
        // Set-up UV coordinated to point to the upper (V < 0.5), "dirt+grass" part of the texture.
        // Take a look at the custom image (assets/textures/array_texture.png)
        // so the UV coords will make more sense
        // Note: (0.0, 0.0) = Top-Left in UV mapping, (1.0, 1.0) = Bottom-Right in UV mapping
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            uvs,
        )
        // For meshes with flat shading, normals are orthogonal (pointing out) from the direction of
        // the surface.
        // Normals are required for correct lighting calculations.
        // Each array represents a normalized vector, which length should be equal to 1.0.
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            normals,
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
        )))
}

fn calculate_surface_normal(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]) -> [f32; 3] {
    let u = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
    let v = [p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]];
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
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