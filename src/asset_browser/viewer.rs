use bevy::gltf::{Gltf, GltfNode};
use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use crate::animation::animation_plugin::get_child_with_component_recursive;
use crate::asset_browser::state::{AssetBrowserState, CHARACTER_NODE_PREFIX};
use crate::asset_browser::ui::{AssetAnimLabel, HeightDisplay};
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
        // Pre-populate hidden_nodes and anim_mapping from existing definition.
        state.load_definition();
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
    child_query: Query<&Children>,
    mut anim_players: Query<&mut AnimationPlayer>,
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
    state.nodes_ui_dirty = true;

    let Some(viewer_entity) = state.viewer_entity else { return };

    // Build graph with whatever clips the model itself has (may be empty).
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

    // Find the AnimationPlayer in the hierarchy, or insert one on the viewer root
    // (needed for models that have no embedded animations so external clips can play).
    let player_entity = get_child_with_component_recursive(viewer_entity, &child_query, &anim_players)
        .unwrap_or_else(|| {
            commands.entity(viewer_entity).insert(AnimationPlayer::default());
            viewer_entity
        });

    if let Ok(mut player) = anim_players.get_mut(player_entity) {
        player.stop_all();
    }
    commands.entity(player_entity).insert(AnimationGraphHandle(graph_handle.clone()));

    state.anim_player_entity = Some(player_entity);
    state.viewer_graph_handle = Some(graph_handle);
    state.anim_count = nodes.len();
    state.anim_node_indices = nodes;
    state.anim_names = names_by_index;
    state.anim_dirty = !gltf.animations.is_empty(); // only auto-play if model has clips
    state.mapping_dirty = true;
    state.gltf_handle = None;
}

/// Load extra animation GltFs when animation_sources changes.
pub fn load_extra_animation_sources(
    mut state: ResMut<AssetBrowserState>,
    asset_server: Res<AssetServer>,
) {
    if !state.sources_dirty { return; }
    state.sources_dirty = false;

    // Reload all handles from scratch to keep them in sync with the source list.
    state.extra_gltf_handles = state.animation_sources.iter()
        .map(|path| asset_server.load(path.clone()))
        .collect();
}

/// When extra GltFs finish loading, append their clips to the viewer's animation graph.
pub fn merge_extra_anim_clips(
    mut state: ResMut<AssetBrowserState>,
    gltf_assets: Res<Assets<Gltf>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
    mut last_merged_count: Local<usize>,
) {
    // Only run after the main model graph has been built.
    let Some(graph_handle) = state.viewer_graph_handle.clone() else { return };
    let Some(graph) = animation_graphs.get_mut(&graph_handle) else { return };

    let mut any_new = false;

    // Collect new clips outside of state borrow.
    struct NewClip { stem: String, name: String, handle: Handle<AnimationClip> }
    let mut new_clips: Vec<NewClip> = Vec::new();

    for (idx, handle) in state.extra_gltf_handles.iter().enumerate() {
        let Some(gltf) = gltf_assets.get(handle) else { continue };
        let stem = state.animation_sources.get(idx)
            .map(|p| std::path::Path::new(p)
                .file_stem().and_then(|s| s.to_str()).unwrap_or("ext").to_string())
            .unwrap_or_else(|| format!("ext{idx}"));
        for (name, clip_handle) in &gltf.named_animations {
            let prefixed = format!("{stem}|{name}");
            if !state.anim_names.contains(&prefixed) {
                new_clips.push(NewClip { stem: stem.clone(), name: name.to_string(), handle: clip_handle.clone() });
            }
        }
    }

    for clip in new_clips {
        let prefixed = format!("{}|{}", clip.stem, clip.name);
        let node_idx = graph.add_clip(clip.handle, 1.0, graph.root);
        state.anim_names.push(prefixed);
        state.anim_node_indices.push(node_idx);
        state.anim_count += 1;
        any_new = true;
    }

    if any_new {
        // Reinstall the updated graph on the player entity.
        if let Some(player_entity) = state.anim_player_entity {
            commands.entity(player_entity).insert(AnimationGraphHandle(graph_handle));
        }
        state.mapping_dirty = true;
    }

    let _ = last_merged_count; // suppress unused warning
}

/// Walk the viewer model hierarchy and accumulate world-space Y extents from all Aabb components.
/// Waits several frames after load so Bevy has time to attach Aabb to all mesh children, then
/// measures once at scale=1.0 and locks the result.
pub fn compute_model_height(
    mut state: ResMut<AssetBrowserState>,
    aabb_q: Query<(&Aabb, &GlobalTransform)>,
    children_q: Query<&Children>,
) {
    if state.model_raw_height > 0.0 { return; } // already measured
    let Some(viewer_entity) = state.viewer_entity else { return };

    // Count frames to let Bevy populate Aabb on all spawned mesh children.
    const SETTLE_FRAMES: u32 = 8;
    state.aabb_settle_frames += 1;
    if state.aabb_settle_frames < SETTLE_FRAMES { return; }

    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut found = false;
    collect_aabbs(viewer_entity, &children_q, &aabb_q, &mut min_y, &mut max_y, &mut found);

    if !found || max_y <= min_y { return; }

    // Model is always at scale=1.0 here (apply_viewer_scale hasn't run yet).
    state.model_raw_height = max_y - min_y;
    if let Some(stored_scale) = state.pending_scale.take() {
        state.target_height_m = stored_scale * state.model_raw_height;
    }
    state.height_dirty = true;
}

fn collect_aabbs(
    entity: Entity,
    children_q: &Query<&Children>,
    aabb_q: &Query<(&Aabb, &GlobalTransform)>,
    min_y: &mut f32,
    max_y: &mut f32,
    found: &mut bool,
) {
    if let Ok((aabb, gt)) = aabb_q.get(entity) {
        let center_y = gt.translation().y + aabb.center.y;
        *min_y = min_y.min(center_y - aabb.half_extents.y);
        *max_y = max_y.max(center_y + aabb.half_extents.y);
        *found = true;
    }
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            collect_aabbs(child, children_q, aabb_q, min_y, max_y, found);
        }
    }
}

/// Apply computed_scale() to the viewer model Transform and update the height label.
/// Also repositions the camera so the model fills the viewport at a comfortable distance.
pub fn apply_viewer_scale(
    mut state: ResMut<AssetBrowserState>,
    mut model_q: Query<&mut Transform, With<AssetBrowserViewerModel>>,
    mut camera_q: Query<&mut Transform, (With<AssetBrowserViewerCamera>, Without<AssetBrowserViewerModel>)>,
    mut label_q: Query<&mut Text, With<HeightDisplay>>,
) {
    if !state.height_dirty || state.model_raw_height <= 0.0 { return; }
    state.height_dirty = false;

    let scale = state.computed_scale();
    if let Ok(mut t) = model_q.single_mut() {
        t.scale = Vec3::splat(scale);
    }
    if let Ok(mut t) = label_q.single_mut() {
        **t = format!("{:.2} m  (x{:.4})", state.target_height_m, scale);
    }

    // Fit camera to the model. Runs on load (once AABB is measured) and on user height changes.
    // Uses vertical FOV of 60 degrees (Bevy default perspective).
    let displayed_height = state.model_raw_height * scale;
    let center_y = displayed_height * 0.5;
    let half_fov = std::f32::consts::PI / 6.0; // 30 degrees
    let distance = (displayed_height * 0.5) / half_fov.tan() * 1.4;
    if let Ok(mut cam) = camera_q.single_mut() {
        cam.translation = Vec3::new(0.0, center_y, distance);
        *cam = cam.looking_at(Vec3::new(0.0, center_y, 0.0), Vec3::Y);
    }
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
    mut anim_players: Query<&mut AnimationPlayer>,
    mut anim_label: Query<&mut Text, With<AssetAnimLabel>>,
) {
    if !state.anim_dirty || state.anim_node_indices.is_empty() { return; }

    let Some(player_entity) = state.anim_player_entity else { return };
    let Ok(mut player) = anim_players.get_mut(player_entity) else { return };

    let idx = state.anim_index;
    let node = state.anim_node_indices[idx];
    player.stop_all();
    player.play(node).repeat();
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
