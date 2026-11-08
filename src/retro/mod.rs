//! The retro look (`docs/retro-look.md`): the game camera renders into a small image that
//! a second camera draws to the window, scaled up by a whole number with nearest
//! filtering, through `RetroCanvasMaterial`, which maps every texel to a palette with
//! ordered dithering.
//!
//! What keeps it steady: `snap_retro_camera` puts the game camera on its own texel grid
//! every frame and moves the picture on screen by what the snap took away, so pixels stay
//! glued to the world while the camera still glides; the dither pattern is offset by the
//! camera's texel so it stays on the world too.
//!
//! **F12** opens the settings panel (`panel.rs`), every setting with left/right buttons;
//! any change flashes a summary on screen and is saved to `retro-settings.ron`
//! (`look.rs`). Hotkeys: **Ctrl+K** on/off, **Ctrl+J** next palette (Shift: previous; the
//! first is full colour), **Ctrl+H** dither strength, **Ctrl+U / Ctrl+I** smaller / bigger
//! pixels. Starts as the file says ([`DEFAULT_PALETTE`] without one); `--no-retro` starts
//! with it off, `--palette <name>` picks another palette, `--retro-shots <dir>` saves a
//! screenshot per palette and quits.
//! Off in the character editor (its camera is clipped to a pane).

pub mod look;
pub mod palette;
mod panel;
mod shots;
pub mod snap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, ImageRenderTarget, Projection, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy::transform::TransformSystems;
use bevy::window::{PrimaryWindow, WindowRef};

use crate::camera::components::GameCamera;
use crate::character_editor::state::EditorSession;
pub use look::RetroLook;
use look::{RetroFile, Setting};
use palette::{MAX_COLORS, Palette};
use snap::{CanvasLayout, snap_to_texels};

/// The render layer only the canvas camera and its quad live on.
const CANVAS_LAYER: usize = 31;
const SHADER: &str = "shaders/retro_canvas.wesl";
/// The palette the game starts with, unless `--palette` names another.
pub const DEFAULT_PALETTE: &str = "moebius-64";

/// Every `.hex` palette in `assets/palettes`, sorted. `RetroLook::palette` 0 is "none",
/// so palette `i` is `list[i - 1]`.
#[derive(Resource, Default)]
pub struct Palettes(pub Vec<Palette>);

impl Palettes {
    pub fn get(&self, index: usize) -> Option<&Palette> {
        index.checked_sub(1).and_then(|i| self.0.get(i))
    }

    pub fn name(&self, index: usize) -> &str {
        self.get(index).map_or("full colour", |p| p.name.as_str())
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.0.iter().position(|p| p.name == name).map(|i| i + 1)
    }
}

/// Where the picture is this frame, for anything that turns window positions into world
/// rays (mouse aim). `None` while the retro look is off.
#[derive(Resource, Default)]
pub struct RetroView(pub Option<CanvasLayout>);

impl RetroView {
    /// A window cursor position as the game camera's viewport position.
    pub fn cursor_to_viewport(&self, cursor: Vec2, window: &Window) -> Vec2 {
        match &self.0 {
            Some(layout) => layout.window_to_image(cursor, window.size()),
            None => cursor,
        }
    }
}

/// The pieces the retro look has spawned and the image the game camera renders to.
#[derive(Resource, Default)]
struct RetroRig {
    image: Option<Handle<Image>>,
    material: Option<Handle<RetroCanvasMaterial>>,
    quad: Option<Entity>,
    canvas_camera: Option<Entity>,
    size: UVec2,
}

/// The canvas camera and quad.
#[derive(Component)]
struct RetroCanvasPart;

/// On the game camera while it renders into the retro image.
#[derive(Component)]
struct RetroCamera;

#[derive(Clone, ShaderType)]
struct RetroParams {
    image_size: Vec2,
    pattern_offset: Vec2,
    dither: f32,
    exposure: f32,
    palette_len: u32,
    _pad: u32,
    contrast: f32,
    saturation: f32,
    shadow_tint: f32,
    warmth: f32,
    ink: f32,
    ink_threshold: f32,
    _pad2: f32,
    _pad3: f32,
    palette_lab: [Vec4; MAX_COLORS],
    palette_linear: [Vec4; MAX_COLORS],
}

impl Default for RetroParams {
    fn default() -> Self {
        Self {
            image_size: Vec2::ONE,
            pattern_offset: Vec2::ZERO,
            dither: 0.0,
            exposure: 1.0,
            palette_len: 0,
            _pad: 0,
            contrast: 1.0,
            saturation: 1.0,
            shadow_tint: 0.0,
            warmth: 0.0,
            ink: 0.0,
            ink_threshold: 0.1,
            _pad2: 0.0,
            _pad3: 0.0,
            palette_lab: [Vec4::ZERO; MAX_COLORS],
            palette_linear: [Vec4::ZERO; MAX_COLORS],
        }
    }
}

/// Draws the low-res scene on the window quad: palette, dither, nothing else.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct RetroCanvasMaterial {
    #[uniform(0)]
    params: RetroParams,
    #[texture(1)]
    scene: Handle<Image>,
}

impl Material2d for RetroCanvasMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct RetroPlugin {
    /// `--no-retro`: start with the look off whatever the settings file says.
    pub force_off: bool,
    pub palette: Option<String>,
    /// `--retro-shots <dir>`: screenshot every palette and quit (`shots.rs`).
    pub shots: Option<std::path::PathBuf>,
}

impl Plugin for RetroPlugin {
    fn build(&self, app: &mut App) {
        let palettes = Palettes(palette::load_all());
        let file = RetroFile::load().unwrap_or_default();
        let mut look = file.look(&palettes);
        app.insert_resource(file.light).insert_resource(file.sky);
        if self.force_off {
            // Off for this run only: saves keep what the file said.
            app.insert_resource(look::SavedEnabled(look.enabled));
            look.enabled = false;
        }
        if let Some(name) = &self.palette {
            if let Some(i) = palettes.index_of(name) {
                look.palette = i;
            } else {
                warn!("no palette named {name} in {}", palette::PALETTE_DIR);
            }
        }
        if let Some(dir) = &self.shots {
            shots::plugin(app, dir.clone());
        }
        app.add_plugins(Material2dPlugin::<RetroCanvasMaterial>::default())
            .insert_resource(palettes)
            .insert_resource(look)
            .init_resource::<RetroView>()
            .init_resource::<RetroRig>()
            .add_systems(Update, (retro_keys, sync_retro_rig).chain())
            .add_plugins(panel::plugin)
            .add_systems(
                PostUpdate,
                snap_retro_camera
                    .after(crate::camera::systems::camera_follow)
                    .before(TransformSystems::Propagate),
            );
    }
}

fn retro_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut look: ResMut<RetroLook>,
    palettes: Res<Palettes>,
) {
    if !keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        return;
    }
    let back = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        -1
    } else {
        1
    };
    for (key, setting, dir) in [
        (KeyCode::KeyK, Setting::Enabled, 1),
        (KeyCode::KeyJ, Setting::Palette, back),
        (KeyCode::KeyH, Setting::Dither, 1),
        (KeyCode::KeyU, Setting::PixelSize, -1),
        (KeyCode::KeyI, Setting::PixelSize, 1),
    ] {
        if keys.just_pressed(key) {
            setting.step(&mut look, &palettes, dir);
        }
    }
}

fn new_target_image(size: UVec2) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::nearest();
    image
}

/// Bring the rig in line with `RetroLook`: render the game camera into an image of the
/// right size and show it through the canvas, or put everything back.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_retro_rig(
    mut commands: Commands,
    look: Res<RetroLook>,
    editor: Option<Res<EditorSession>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<
        (Entity, &mut Camera, &mut RenderTarget, Has<RetroCamera>),
        With<GameCamera>,
    >,
    mut rig: ResMut<RetroRig>,
    mut view: ResMut<RetroView>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<RetroCanvasMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    parts: Query<(), With<RetroCanvasPart>>,
) {
    let physical = windows.single().map_or(UVec2::ZERO, Window::physical_size);
    let want =
        look.enabled && editor.is_none() && !cameras.is_empty() && physical.x > 0 && physical.y > 0;

    if !want {
        for (entity, mut camera, mut target, retro) in &mut cameras {
            if retro {
                *target = RenderTarget::Window(WindowRef::Primary);
                camera.order = 0;
                commands.entity(entity).remove::<RetroCamera>();
            }
        }
        for entity in [rig.quad.take(), rig.canvas_camera.take()]
            .into_iter()
            .flatten()
        {
            commands.entity(entity).try_despawn();
        }
        *rig = RetroRig::default();
        view.0 = None;
        return;
    }

    let size = CanvasLayout::image_size(physical, look.pixel_size);
    let image = match rig.image.clone() {
        Some(image) if rig.size == size => image,
        _ => {
            let image = images.add(new_target_image(size));
            rig.image = Some(image.clone());
            rig.size = size;
            if let Some(mut material) = rig.material.as_ref().and_then(|m| materials.get_mut(m)) {
                material.scene = image.clone();
            }
            image
        }
    };
    let material = if let Some(material) = rig.material.clone().filter(|m| materials.contains(m)) {
        material
    } else {
        let material = materials.add(RetroCanvasMaterial {
            params: RetroParams::default(),
            scene: image.clone(),
        });
        rig.material = Some(material.clone());
        material
    };

    for (entity, mut camera, mut target, retro) in &mut cameras {
        let on_image = matches!(&*target, RenderTarget::Image(t) if t.handle == image);
        if !on_image {
            *target = RenderTarget::Image(ImageRenderTarget {
                handle: image.clone(),
                scale_factor: 1.0,
            });
        }
        camera.order = -1;
        if !retro {
            commands.entity(entity).insert(RetroCamera);
        }
    }

    // The canvas pieces are not tied to a state; check they still exist.
    let alive = |e: Option<Entity>| e.is_some_and(|e| parts.contains(e));
    if !alive(rig.canvas_camera) {
        rig.canvas_camera = Some(
            commands
                .spawn((
                    Name::new("Retro Canvas Camera"),
                    RetroCanvasPart,
                    Camera2d,
                    Camera {
                        order: 0,
                        clear_color: ClearColorConfig::Custom(Color::BLACK),
                        ..default()
                    },
                    Tonemapping::None,
                    DebandDither::Disabled,
                    RenderLayers::layer(CANVAS_LAYER),
                ))
                .id(),
        );
    }
    if !alive(rig.quad) {
        rig.quad = Some(
            commands
                .spawn((
                    Name::new("Retro Canvas"),
                    RetroCanvasPart,
                    Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                    MeshMaterial2d(material),
                    Transform::default(),
                    RenderLayers::layer(CANVAS_LAYER),
                ))
                .id(),
        );
    }
}

type RetroGameCamera = (With<GameCamera>, With<RetroCamera>);

/// Snap the game camera to its texel grid, move the picture by the remainder and hand
/// the shader this frame's palette and pattern offset.
#[allow(clippy::too_many_arguments)]
fn snap_retro_camera(
    look: Res<RetroLook>,
    palettes: Res<Palettes>,
    rig: Res<RetroRig>,
    mut view: ResMut<RetroView>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &Projection), RetroGameCamera>,
    mut quads: Query<&mut Transform, Without<GameCamera>>,
    mut materials: ResMut<Assets<RetroCanvasMaterial>>,
) {
    let (Some(quad), Some(material)) = (rig.quad, rig.material.as_ref()) else {
        return;
    };
    let Ok(window) = windows.single() else { return };
    let Ok((mut cam, projection)) = cameras.single_mut() else {
        return;
    };
    let image = rig.size.as_vec2();

    // World units per texel; only an orthographic camera can be snapped.
    let texel = match projection {
        Projection::Orthographic(ortho) => match ortho.scaling_mode {
            ScalingMode::FixedVertical { viewport_height } => {
                viewport_height * ortho.scale / image.y
            }
            _ => 0.0,
        },
        _ => 0.0,
    };
    let snapped = snap_to_texels(cam.translation, cam.rotation, texel);
    cam.translation = snapped.translation;

    let texel_on_screen = look.pixel_size.max(1) as f32 / window.scale_factor();
    let layout = CanvasLayout {
        image: rig.size,
        texel_on_screen,
        shift: CanvasLayout::shift_for(snapped.remainder, texel_on_screen),
    };
    if let Ok(mut tf) = quads.get_mut(quad) {
        tf.translation = layout.shift.extend(0.0);
        tf.scale = (image * texel_on_screen).extend(1.0);
    }
    view.0 = Some(layout);

    let Some(mut material) = materials.get_mut(material) else {
        return;
    };
    let params = &mut material.params;
    params.image_size = image;
    // Image rows run down, the camera's texel index up.
    params.pattern_offset = Vec2::new(
        snapped.texel.x.rem_euclid(8) as f32,
        (-snapped.texel.y).rem_euclid(8) as f32,
    );
    params.dither = look.dither;
    params.exposure = look.exposure;
    params.contrast = look.contrast;
    params.saturation = look.saturation;
    params.shadow_tint = look.shadow_tint;
    params.warmth = look.warmth;
    params.ink = look.ink;
    params.ink_threshold = look.ink_threshold;
    match palettes.get(look.palette) {
        Some(p) => {
            let (lab, linear) = p.shader_arrays();
            params.palette_len = p.len() as u32;
            params.palette_lab = lab;
            params.palette_linear = linear;
        }
        None => params.palette_len = 0,
    }
}
