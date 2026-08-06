use bevy::math::{Quat, Rect, Vec2, Vec3};
use bevy::prelude::{
    Assets, Camera, Camera2d, Camera3d, Commands, Image,
    Name, OrthographicProjection, PerspectiveProjection, Query, Res,
    ResMut, Sprite, Transform, Window, With, default,
};
use bevy::camera::{ImageRenderTarget, Projection, RenderTarget, ScalingMode};
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::camera::visibility::RenderLayers;
use bevy::window::PrimaryWindow;
use std::f32::consts::PI;
use avian3d::interpolation::TransformInterpolation;
use avian3d::prelude::Position;
use crate::camera::components::{CameraOffset, GameCamera, PixelCanvas};
use crate::player::components::Player;
use crate::settings::resources::{GameSettings, ProjectionMode};

#[allow(dead_code)]
const PIXEL_WIDTH: u32 = 480;
#[allow(dead_code)]
const PIXEL_HEIGHT: u32 = 360;
#[allow(dead_code)]
const CANVAS_LAYER: usize = 1;

pub fn spawn_camera(mut commands: Commands) {
  commands.spawn((
    Name::from("Camera"),
    CameraOffset(Vec3::new(2.0, 1.5, 2.0)),
    Camera3d::default(),
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
}

#[allow(dead_code)]
pub fn spawn_pixelated_camera(
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
        PixelCanvas {},
        RenderLayers::layer(CANVAS_LAYER),
    ));
}

#[allow(dead_code)]
pub fn resize_pixel_canvas(
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut canvas_q: Query<&mut Sprite, With<PixelCanvas>>,
) {
    let Ok(window) = window_q.single() else { return; };
    let Ok(mut sprite) = canvas_q.single_mut() else { return; };
    sprite.custom_size = Some(Vec2::new(window.width(), window.height()));
}

pub fn camera_follow(
    mut camera_query: Query<(&mut Transform, &CameraOffset), With<GameCamera>>,
    player_position: Query<&Position, With<Player>>,
) {
    for (mut camera_transform, offset) in camera_query.iter_mut() {
        for player_position in player_position.iter() {
          camera_transform.translation = camera_transform.translation.lerp(player_position.0 + offset.0, 0.9);
            // camera_transform.translation = player_position.0 + offset.0;
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
                near: settings.ortho_near,
                far: settings.ortho_far,
                viewport_origin: Vec2::new(0.5, 0.5),
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: settings.ortho_viewport_height,
                },
                area: Rect::new(-1.0, -1.0, 1.0, 1.0),
                scale: settings.zoom,
            }),
            ProjectionMode::Perspective => Projection::Perspective(PerspectiveProjection {
                fov: settings.persp_fov.clamp(10.0, 170.0).to_radians(),
                near: settings.persp_near,
                far: settings.persp_far,
                ..default()
            }),
        };
    }
}
