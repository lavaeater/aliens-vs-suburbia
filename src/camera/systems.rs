use bevy::math::{Quat, Rect, Vec2, Vec3};
use bevy::prelude::{
    AlphaMode, Assets, Camera, Camera2d, Camera3d, Children, Color, Commands, Entity, Image,
    MeshMaterial3d, Name, OrthographicProjection, PerspectiveProjection, Query, Res,
    ResMut, Sprite, StandardMaterial, Transform, Window, With, Without, default,
};
use bevy::camera::{ImageRenderTarget, Projection, RenderTarget, ScalingMode};
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::camera::visibility::RenderLayers;
use bevy::scene::{SceneInstance, SceneSpawner};
use bevy::window::PrimaryWindow;
use std::f32::consts::PI;
use avian3d::interpolation::TransformInterpolation;
use avian3d::prelude::Position;
use crate::camera::components::{CameraOffset, GameCamera, PixelCanvas};
use crate::general::systems::map_systems::{WallMaterials, WallOccluder};
use crate::player::components::Player;
use crate::settings::resources::{GameSettings, ProjectionMode};

const PIXEL_WIDTH: u32 = 480;
const PIXEL_HEIGHT: u32 = 270;
const CANVAS_LAYER: usize = 1;

pub fn spawn_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
) {
    let size = Extent3d {
        width: PIXEL_WIDTH,
        height: PIXEL_HEIGHT,
        depth_or_array_layers: 1,
    };

    let mut render_texture = Image {
        texture_descriptor: bevy::render::render_resource::TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        sampler: ImageSampler::nearest(),
        ..default()
    };
    render_texture.resize(size);

    let render_texture_handle = images.add(render_texture);

    // 3D game camera — renders to the low-res texture
    commands.spawn((
        Name::from("Camera"),
        CameraOffset(Vec3::new(2.0, 1.5, 2.0)),
        Camera3d::default(),
        Camera { order: -1, ..default() },
        RenderTarget::Image(ImageRenderTarget {
            handle: render_texture_handle.clone(),
            scale_factor: 1.0,
        }),
        Projection::Orthographic(OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            viewport_origin: Vec2::new(0.5, 0.5),
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 2.0 },
            area: Rect::new(-1.0, -1.0, 1.0, 1.0),
            scale: 2.0,
        }),
        Transform {
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        TransformInterpolation,
        GameCamera {},
    ));

    let window_size = window_q.single()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));

    // 2D canvas camera — upscales the pixel texture to screen
    commands.spawn((
        Name::from("PixelCanvasCamera"),
        Camera2d,
        Camera { order: 0, ..default() },
        RenderLayers::layer(CANVAS_LAYER),
    ));

    // Fullscreen sprite showing the low-res render texture
    commands.spawn((
        Name::from("PixelCanvas"),
        Sprite {
            image: render_texture_handle,
            custom_size: Some(window_size),
            ..default()
        },
        Transform::default(),
        PixelCanvas,
        RenderLayers::layer(CANVAS_LAYER),
    ));
}

pub fn resize_pixel_canvas(
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut canvas_q: Query<&mut Sprite, With<PixelCanvas>>,
) {
    let Ok(window) = window_q.single() else { return; };
    let Ok(mut sprite) = canvas_q.single_mut() else { return; };
    sprite.custom_size = Some(Vec2::new(window.width(), window.height()));
}

fn collect_descendants(entity: Entity, children_q: &Query<&Children>, out: &mut Vec<Entity>) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            out.push(*child);
            collect_descendants(*child, children_q, out);
        }
    }
}

/// After each wall's scene is ready, clone its mesh materials with AlphaMode::Blend
/// so we can fade alpha at runtime without affecting shared materials.
pub fn init_wall_materials(
    mut walls: Query<(Entity, &SceneInstance, &mut WallMaterials), With<WallOccluder>>,
    scene_spawner: Res<SceneSpawner>,
    children_q: Query<&Children>,
    mut mat_q: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, scene_instance, mut wall_mats) in &mut walls {
        if wall_mats.initialized { continue; }
        if !scene_spawner.instance_is_ready(**scene_instance) { continue; }

        let mut descendants = Vec::new();
        collect_descendants(entity, &children_q, &mut descendants);

        for desc in descendants {
            if let Ok(mut mat_handle) = mat_q.get_mut(desc) {
                if let Some(cloned) = materials.get(&mat_handle.0).cloned() {
                    let mut new_mat = cloned;
                    new_mat.alpha_mode = AlphaMode::Blend;
                    let handle = materials.add(new_mat);
                    wall_mats.handles.push(handle.clone());
                    mat_handle.0 = handle;
                }
            }
        }
        wall_mats.initialized = true;
    }
}

/// Fade walls that lie between the camera and the player to semi-transparent.
/// Walls not on the camera→player line are restored to full opacity.
pub fn wall_occlusion_system(
    camera_q: Query<&Transform, With<GameCamera>>,
    player_q: Query<&Position, With<Player>>,
    walls: Query<(&Transform, &WallMaterials), (With<WallOccluder>, Without<GameCamera>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(cam_transform) = camera_q.single() else { return; };
    let Ok(player_pos) = player_q.single() else { return; };

    let cam = Vec2::new(cam_transform.translation.x, cam_transform.translation.z);
    let player = Vec2::new(player_pos.x, player_pos.z);
    let seg = player - cam;
    let seg_len_sq = seg.length_squared();

    for (wall_transform, wall_mats) in &walls {
        if !wall_mats.initialized { continue; }

        let w = Vec2::new(wall_transform.translation.x, wall_transform.translation.z);
        let t = if seg_len_sq > 0.0 {
            ((w - cam).dot(seg) / seg_len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let closest = cam + seg * t;
        let dist = (w - closest).length();
        let alpha = if dist < 0.55 && t > 0.05 && t < 0.95 {
            0.15_f32
        } else {
            1.0_f32
        };
        for handle in &wall_mats.handles {
            if let Some(mat) = materials.get_mut(handle) {
                let c = mat.base_color.to_srgba();
                mat.base_color = Color::srgba(c.red, c.green, c.blue, alpha);
            }
        }
    }
}

pub fn camera_follow(
    mut camera_query: Query<(&mut Transform, &CameraOffset), With<GameCamera>>,
    player_position: Query<&Position, With<Player>>,
) {
    for (mut camera_transform, offset) in camera_query.iter_mut() {
        for player_position in player_position.iter() {
            camera_transform.translation = player_position.0 + offset.0;
            camera_transform.look_at(player_position.0, Vec3::Y);
        }
    }
}

pub fn apply_camera_settings(
    settings: Res<GameSettings>,
    mut camera_query: Query<(&mut Projection, &mut Transform, &mut CameraOffset), With<GameCamera>>,
) {

    let pitch_rad = settings.pitch_degrees.to_radians();
    let yaw_rad = settings.yaw_degrees.to_radians();
    let offset_dist = settings.zoom * 0.75;
    let offset_y = -pitch_rad.sin() * offset_dist;
    let offset_xz = pitch_rad.cos() * offset_dist;

    for (mut proj, mut _transform, mut offset) in &mut camera_query {
        offset.0 = Vec3::new(yaw_rad.sin() * offset_xz, offset_y, yaw_rad.cos() * offset_xz);

        *proj = match settings.projection {
            ProjectionMode::Orthographic => Projection::Orthographic(OrthographicProjection {
                near: -1000.0,
                far: 1000.0,
                viewport_origin: Vec2::new(0.5, 0.5),
                scaling_mode: ScalingMode::FixedVertical { viewport_height: 2.0 },
                area: Rect::new(-1.0, -1.0, 1.0, 1.0),
                scale: settings.zoom,
            }),
            ProjectionMode::Perspective => Projection::Perspective(PerspectiveProjection {
                fov: (settings.zoom).clamp(10.0, 120.0).to_radians(),
                near: 0.1,
                far: 1000.0,
                ..default()
            }),
        };
    }
}
