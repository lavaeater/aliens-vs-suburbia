use bevy::gltf::{Gltf, GltfNode};
use bevy::prelude::*;
use crate::asset_browser::state::{AssetBrowserState, CHARACTER_NODE_PREFIX};
use crate::asset_browser::ui::AssetAnimLabel;
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

    if let Some(path) = state.selected_path().map(|s| s.to_string()) {
        let handle: Handle<Scene> = asset_server.load(
            GltfAssetLabel::Scene(0).from_asset(path.clone()),
        );
        let entity = spawn_viewer_model(&mut commands, handle);
        state.viewer_entity = Some(entity);
        state.reset_anim();
        state.gltf_handle = Some(asset_server.load(path));
    }
}

fn spawn_viewer_model(commands: &mut Commands, handle: Handle<Scene>) -> Entity {
    commands.spawn((
        SceneRoot(handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
        AssetBrowserViewerModel,
        StateMarker,
    )).id()
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

pub fn setup_viewer_animation(
    mut state: ResMut<AssetBrowserState>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_node_assets: Res<Assets<GltfNode>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
    mut anim_players: Query<Entity, With<AnimationPlayer>>,
) {
    let gltf_handle = match state.gltf_handle.clone() {
        Some(h) => h,
        None => return,
    };
    let Some(gltf) = gltf_assets.get(&gltf_handle) else { return };

    // Collect mesh node names from the Gltf hierarchy.
    let mut mesh_nodes: Vec<String> = gltf.named_nodes.iter()
        .filter_map(|(name, handle)| {
            gltf_node_assets.get(handle)
                .filter(|n| n.mesh.is_some())
                .map(|_| name.to_string())
        })
        .collect();
    mesh_nodes.sort();
    state.mesh_nodes = mesh_nodes;
    state.nodes_dirty = true;

    if gltf.animations.is_empty() {
        state.gltf_handle = None;
        return;
    }

    let Some(player_entity) = anim_players.iter_mut().next() else { return };

    let mut names_by_index = vec![String::new(); gltf.animations.len()];
    for (name, handle) in &gltf.named_animations {
        if let Some(idx) = gltf.animations.iter().position(|h| h == handle) {
            names_by_index[idx] = name.to_string();
        }
    }

    let mut graph = AnimationGraph::new();
    let nodes: Vec<AnimationNodeIndex> = gltf.animations.iter()
        .map(|clip| graph.add_clip(clip.clone(), 1.0, graph.root))
        .collect();

    let graph_handle = animation_graphs.add(graph);
    commands.entity(player_entity).insert(AnimationGraphHandle(graph_handle));

    state.anim_count = nodes.len();
    state.anim_node_indices = nodes;
    state.anim_names = names_by_index;
    state.anim_dirty = true;
    state.gltf_handle = None;
}

pub fn apply_node_visibility(
    mut state: ResMut<AssetBrowserState>,
    mut named_entities: Query<(&Name, &mut Visibility)>,
) {
    if !state.nodes_dirty { return; }
    state.nodes_dirty = false;

    for (name, mut vis) in named_entities.iter_mut() {
        if name.starts_with(CHARACTER_NODE_PREFIX) { continue; }
        if state.mesh_nodes.iter().any(|n| n == name.as_str()) {
            *vis = if state.hidden_nodes.contains(name.as_str()) {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
}

pub fn apply_viewer_animation(
    mut state: ResMut<AssetBrowserState>,
    mut anim_players: Query<&mut AnimationPlayer, With<AnimationGraphHandle>>,
    mut anim_label: Query<&mut Text, With<AssetAnimLabel>>,
) {
    if !state.anim_dirty || state.anim_node_indices.is_empty() { return; }

    let idx = state.anim_index;
    let node = state.anim_node_indices[idx];

    let mut played = false;
    for mut player in anim_players.iter_mut() {
        player.play(node).repeat();
        played = true;
    }
    if !played { return; } // AnimationGraphHandle not applied yet — retry next frame
    state.anim_dirty = false;

    let name = state.anim_names.get(idx).filter(|s| !s.is_empty()).map(|s| s.as_str());
    let label_text = match name {
        Some(n) => format!("[{}/{}] {}", idx + 1, state.anim_count, n),
        None => format!("[{}/{}]", idx + 1, state.anim_count),
    };
    if let Ok(mut t) = anim_label.single_mut() {
        **t = label_text;
    }
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
