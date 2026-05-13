use bevy::prelude::*;
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::asset_browser::state::AssetBrowserState;
use crate::ui::spawn_ui::StateMarker;

#[derive(Component)]
pub struct AssetBrowserViewerCamera;

#[derive(Component)]
pub struct AssetBrowserViewerModel;

#[derive(Component)]
pub struct AssetBrowserViewerPanel;

pub fn spawn_asset_browser_cameras(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera { order: 1, ..Default::default() },
        StateMarker,
    ));
    let cam = commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.0, 2.5).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        AssetBrowserViewerCamera,
        StateMarker,
    )).id();
    commands.entity(cam).entry::<Camera>().and_modify(|mut c| {
        c.order = 0;
        c.clear_color = ClearColorConfig::Custom(Color::srgb(0.06, 0.06, 0.10));
    });
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

pub fn handle_model_load(
    mut commands: Commands,
    mut state: ResMut<AssetBrowserState>,
    asset_server: Res<AssetServer>,
) {
    if !state.load_requested { return; }
    state.load_requested = false;

    if let Some(old) = state.viewer_entity.take() {
        commands.entity(old).despawn();
    }

    if let Some(path) = state.selected_path() {
        let handle: Handle<Scene> = asset_server.load(
            GltfAssetLabel::Scene(0).from_asset(path.to_string()),
        );
        let entity = spawn_viewer_model(&mut commands, handle, state.toon_shader);
        state.viewer_entity = Some(entity);
    }
}

fn spawn_viewer_model(commands: &mut Commands, handle: Handle<Scene>, toon: bool) -> Entity {
    let mut ec = commands.spawn((
        SceneRoot(handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
        AssetBrowserViewerModel,
        StateMarker,
    ));
    if toon {
        ec.insert(WindWakerShaderBuilder::default().build());
    }
    ec.id()
}

pub fn orbit_viewer(
    mut model_query: Query<&mut Transform, With<AssetBrowserViewerModel>>,
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

pub fn zoom_viewer(
    mut cameras: Query<&mut Transform, With<AssetBrowserViewerCamera>>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
) {
    let mut delta = 0.0f32;
    for ev in scroll.read() {
        delta -= ev.y;
    }
    if delta == 0.0 { return; }
    let Ok(mut transform) = cameras.single_mut() else { return };
    let forward = transform.forward().as_vec3();
    transform.translation += forward * delta * 0.3;
}

pub fn sync_viewer_viewport(
    panels: Query<(&bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform), With<AssetBrowserViewerPanel>>,
    mut cameras: Query<&mut Camera, With<AssetBrowserViewerCamera>>,
    windows: Query<&Window>,
) {
    let Ok((node, transform)) = panels.single() else { return };
    let Ok(mut camera) = cameras.single_mut() else { return };
    let Ok(window) = windows.single() else { return };

    let phys_size = node.size();
    let center = transform.affine().translation;
    let top_left = center - phys_size * 0.5;

    let win_w = window.physical_width();
    let win_h = window.physical_height();
    let x = top_left.x.max(0.0) as u32;
    let y = top_left.y.max(0.0) as u32;
    let w = (phys_size.x as u32).min(win_w.saturating_sub(x));
    let h = (phys_size.y as u32).min(win_h.saturating_sub(y));

    if w == 0 || h == 0 { return; }

    camera.viewport = Some(bevy::camera::Viewport {
        physical_position: bevy::math::UVec2::new(x, y),
        physical_size: bevy::math::UVec2::new(w, h),
        depth: 0.0..1.0,
    });
}
