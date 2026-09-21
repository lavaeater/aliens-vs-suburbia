use crate::camera::components::{CameraFocus, CameraOffset, CameraShake, CameraTarget, GameCamera, PixelCanvas};
use crate::player::components::PlayerDead;
use crate::settings::resources::{GameSettings, ProjectionMode};
use avian3d::interpolation::TransformInterpolation;
use avian3d::prelude::Position;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ImageRenderTarget, Projection, RenderTarget, ScalingMode};
use bevy::image::ImageSampler;
use bevy::math::{Quat, Rect, Vec2, Vec3};
use bevy::prelude::{
    Assets, Camera, Camera2d, Camera3d, Commands, Has, Image, Name, OrthographicProjection,
    PerspectiveProjection, Query, Res, ResMut, Sprite, Time, Transform, Window, With, default,
};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::PrimaryWindow;
use std::f32::consts::PI;

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
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 2.0,
            },
            area: Rect::new(-1.0, -1.0, 1.0, 1.0),
            scale: 2.0,
        }),
        Transform {
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        // TransformInterpolation,
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
        Camera {
            order: -1,
            ..default()
        },
        RenderTarget::Image(ImageRenderTarget {
            handle: render_texture_handle.clone(),
            scale_factor: 1.0,
        }),
        Projection::Orthographic(OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            viewport_origin: Vec2::new(0.5, 0.5),
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 2.0,
            },
            area: Rect::new(-1.0, -1.0, 1.0, 1.0),
            scale: 2.0,
        }),
        TransformInterpolation,
        Transform {
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        GameCamera {},
    ));

    let window_size = window_q
        .single()
        .map_or(Vec2::new(1280.0, 720.0), |w| Vec2::new(w.width(), w.height()));

    // 2D canvas camera — upscales the pixel texture to screen
    commands.spawn((
        Name::from("PixelCanvasCamera"),
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
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
    let Ok(window) = window_q.single() else {
        return;
    };
    let Ok(mut sprite) = canvas_q.single_mut() else {
        return;
    };
    sprite.custom_size = Some(Vec2::new(window.width(), window.height()));
}

/// Aim the camera at the weighted centroid of every [`CameraTarget`] and pull back until
/// they all fit. Writes the smoothed result into [`CameraFocus`] for other systems.
///
/// Runs after physics writeback so `Position` is this frame's. The centroid and the fit
/// factor are both exponentially smoothed so a player joining, dying or sprinting off does
/// not snap the view.
#[allow(clippy::type_complexity)]
pub fn camera_follow(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut focus: ResMut<CameraFocus>,
    mut shake: ResMut<CameraShake>,
    mut camera_query: Query<(&mut Transform, &mut Projection, &CameraOffset), With<GameCamera>>,
    targets: Query<(&Position, &CameraTarget, Has<PlayerDead>)>,
) {
    let Some((raw_center, radius)) = focus_of(targets.iter().map(|(p, t, dead)| {
        // A downed player still matters, just less: the living ones need the room.
        (p.0, if dead { t.weight * 0.5 } else { t.weight })
    })) else {
        return;
    };

    let wanted_fit = fit_factor(
        radius,
        settings.fit_margin,
        settings.zoom,
        settings.ortho_viewport_height,
        settings.pitch_degrees,
        settings.fit_zoom_max,
    );

    if focus.primed {
        let t = 1.0 - (-settings.focus_smoothing * time.delta_secs()).exp();
        focus.center = focus.center.lerp(raw_center, t);
        focus.fit += (wanted_fit - focus.fit) * t;
    } else {
        focus.center = raw_center;
        focus.fit = wanted_fit;
        focus.primed = true;
    }
    focus.radius = radius;
    let jitter = shake.tick(time.delta_secs(), time.elapsed_secs());

    for (mut camera_transform, mut projection, offset) in camera_query.iter_mut() {
        camera_transform.translation = focus.center + offset.0 * focus.fit;
        camera_transform.look_at(focus.center, Vec3::Y);
        camera_transform.translation += jitter;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = settings.zoom * focus.fit;
        }
    }
}

/// Weighted centroid of the targets and the distance from it to the furthest one.
/// `None` when there are no targets (or all weights are zero).
pub fn focus_of(targets: impl IntoIterator<Item = (Vec3, f32)>) -> Option<(Vec3, f32)> {
    let targets: Vec<(Vec3, f32)> = targets.into_iter().filter(|(_, w)| *w > 0.0).collect();
    let total: f32 = targets.iter().map(|(_, w)| w).sum();
    if total <= 0.0 {
        return None;
    }
    let center = targets.iter().map(|(p, w)| *p * *w).sum::<Vec3>() / total;
    let radius = targets
        .iter()
        .map(|(p, _)| p.distance(center))
        .fold(0.0_f32, f32::max);
    Some((center, radius))
}

/// How far to pull the camera back (multiplier on `zoom`, >= 1) so a circle of `radius`
/// plus `margin` on the ground fits vertically on screen.
///
/// The ground's depth axis is foreshortened by `sin(pitch)` at the isometric angle, so
/// that is the tight direction; landscape windows are wider than tall, so horizontal is
/// never the limit.
pub fn fit_factor(radius: f32, margin: f32, zoom: f32, viewport_height: f32, pitch_degrees: f32, max: f32) -> f32 {
    let foreshorten = pitch_degrees.to_radians().sin().abs().max(0.1);
    let needed_scale = 2.0 * (radius + margin) / (viewport_height.max(0.01) * foreshorten);
    (needed_scale / zoom.max(0.01)).clamp(1.0, max.max(1.0))
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
        offset.0 = Vec3::new(
            yaw_rad.sin() * offset_xz,
            offset_y,
            yaw_rad.cos() * offset_xz,
        );

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

#[cfg(test)]
mod tests {
    use super::{fit_factor, focus_of};
    use bevy::math::Vec3;

    #[test]
    fn one_target_is_the_focus_with_zero_radius() {
        let (c, r) = focus_of([(Vec3::new(3.0, 0.0, -2.0), 1.0)]).unwrap();
        assert_eq!(c, Vec3::new(3.0, 0.0, -2.0));
        assert_eq!(r, 0.0);
    }

    #[test]
    fn equal_weights_average_and_radius_reaches_the_furthest() {
        let (c, r) = focus_of([(Vec3::ZERO, 1.0), (Vec3::new(4.0, 0.0, 0.0), 1.0)]).unwrap();
        assert_eq!(c, Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(r, 2.0);
    }

    #[test]
    fn a_heavier_target_pulls_the_centre_towards_it() {
        let (c, _) = focus_of([(Vec3::ZERO, 3.0), (Vec3::new(4.0, 0.0, 0.0), 1.0)]).unwrap();
        assert_eq!(c, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn no_targets_or_zero_weight_gives_nothing() {
        assert!(focus_of([]).is_none());
        assert!(focus_of([(Vec3::ONE, 0.0)]).is_none());
    }

    #[test]
    fn fit_never_zooms_in_and_is_capped() {
        assert_eq!(fit_factor(0.0, 3.0, 8.0, 2.0, -45.0, 3.0), 1.0);
        assert_eq!(fit_factor(1000.0, 3.0, 8.0, 2.0, -45.0, 3.0), 3.0);
    }

    #[test]
    fn fit_grows_with_spread() {
        let near = fit_factor(4.0, 3.0, 8.0, 2.0, -45.0, 10.0);
        let far = fit_factor(12.0, 3.0, 8.0, 2.0, -45.0, 10.0);
        assert!(far > near, "{far} should exceed {near}");
    }
}
