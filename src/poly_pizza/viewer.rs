use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::poly_pizza::state::PolyPizzaState;
use crate::ui::spawn_ui::StateMarker;

#[derive(Component)]
pub struct ViewerModel;

#[derive(Component)]
pub struct ViewerCamera;

pub fn spawn_polypizza_cameras(mut commands: Commands) {
    // UI camera (renders on top of the 3D viewer)
    commands.spawn((
        Camera2d::default(),
        IsDefaultUiCamera,
        Camera { order: 1, ..Default::default() },
        StateMarker,
    ));
    spawn_viewer_camera_inner(&mut commands);
}

fn spawn_viewer_camera_inner(commands: &mut Commands) {
    // Don't provide a separate Camera component — Camera3d sets up its own with the render graph.
    // We modify it via entry after spawn to set order and clear color.
    let cam = commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.0, 4.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        ViewerCamera,
        StateMarker,
    )).id();
    commands.entity(cam).entry::<Camera>().and_modify(|mut c| {
        c.order = 0;
        c.clear_color = ClearColorConfig::Custom(Color::srgb(0.06, 0.06, 0.10));
    });
    // Ambient light so the model is visible
    commands.spawn((
        AmbientLight {
            color: Color::WHITE,
            brightness: 800.0,
            affects_lightmapped_meshes: false,
        },
        StateMarker,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        StateMarker,
    ));
}

pub fn handle_viewer_load(
    mut commands: Commands,
    mut state: ResMut<PolyPizzaState>,
    asset_server: Res<AssetServer>,
) {
    if !state.viewer_needs_load { return; }
    let Some(model) = state.selected_model.clone() else { return; };
    state.viewer_needs_load = false;

    // Despawn old viewer entity
    if let Some(old) = state.viewer_entity.take() {
        commands.entity(old).despawn();
    }

    let cache_path = state.glb_cache_path(&model.id);
    if cache_path.exists() {
        // Already cached — load directly
        let asset_path = state.glb_asset_path(&model.id);
        let handle = asset_server.load(asset_path);
        let entity = spawn_viewer_model(&mut commands, handle, state.toon_shader);
        state.viewer_entity = Some(entity);
    } else {
        // Need to download — signal bridge
        state.viewer_downloading = true;
        // The actual ApiRequest::DownloadGlb is sent by handle_viewer_download_trigger
    }
}

pub fn spawn_viewer_model(
    commands: &mut Commands,
    scene_handle: Handle<Scene>,
    toon: bool,
) -> Entity {
    let mut ec = commands.spawn((
        SceneRoot(scene_handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
        ViewerModel,
        StateMarker,
    ));
    if toon {
        ec.insert(WindWakerShaderBuilder::default().build());
    }
    ec.id()
}

pub fn orbit_viewer(
    mut model_query: Query<&mut Transform, With<ViewerModel>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
) {
    let dragging = mouse_button.pressed(MouseButton::Left);
    for motion in mouse_motion.read() {
        if !dragging { continue; }
        for mut transform in model_query.iter_mut() {
            transform.rotate_y(motion.delta.x * 0.012);
            let right = transform.right().as_vec3();
            transform.rotate_axis(Dir3::new_unchecked(right.normalize()), motion.delta.y * 0.008);
        }
    }
}

pub fn handle_toon_toggle(
    mut commands: Commands,
    mut state: ResMut<PolyPizzaState>,
    asset_server: Res<AssetServer>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyT) { return; }
    state.toon_shader = !state.toon_shader;

    let Some(model) = state.selected_model.clone() else { return; };
    if let Some(old) = state.viewer_entity.take() {
        commands.entity(old).despawn();
    }
    let cache_path = state.glb_cache_path(&model.id);
    if cache_path.exists() {
        let handle = asset_server.load(state.glb_asset_path(&model.id));
        let entity = spawn_viewer_model(&mut commands, handle, state.toon_shader);
        state.viewer_entity = Some(entity);
    }
}
