//! What the meshes are drawn with: Bevy's `StandardMaterial`, the flat toon shader
//! (`bevy_wind_waker_shader::flat`) or `bevy_psx`'s PlayStation material, by
//! [`LookMode::shading`], switched at any time.
//!
//! Every mesh keeps the `StandardMaterial` it was given in [`BaseMaterial`] (recorded on
//! every insert of one), and is drawn with a toon or PSX copy of it: one copy per base
//! material, kept in step when the base is edited (fades, tints) and retuned when the
//! panel changes. So code that fades or tints a mesh reads its handle from
//! `BaseMaterial`, never from `MeshMaterial3d<StandardMaterial>`, which a toon or PSX mesh
//! does not have, and swaps a mesh's material by inserting a new
//! `MeshMaterial3d<StandardMaterial>`.
//!
//! Standard everywhere but a match (menus, editors, the character editor's sandbox).
//!
//! `bevy_psx`'s own plugin is not used: it spawns an upscale camera at startup and converts
//! every material for good. We load its material shader ourselves (`psx_material.wesl`,
//! copied from the pinned rev) under the handle its material asks for, and the retro
//! canvas does the low-res picture.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::pbr::{ExtendedMaterial, MaterialPlugin};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_psx::{BlendMode, ColorSpace, DitherPattern, PsxMaterial, PsxMaterialExtension};
use bevy_wind_waker_shader::flat::{
    FlatExtendedMaterial, FlatShader, FlatShaderBuilder, FlatShaderPlugin,
};

use super::Palettes;
use super::mode::{LookMode, PsxLook, PsxPattern, Shading, ToonLook};
use crate::character_editor::state::EditorSession;
use crate::game_state::GameState;

/// `bevy_psx`'s `PSX_MATERIAL_SHADER_HANDLE`, which its material's shaders point at.
const PSX_SHADER: Handle<Shader> = uuid_handle!("12345678-1234-5678-90ab-cdef01234567");
/// `bevy_psx` takes at most this many palette colours.
const PSX_PALETTE_MAX: usize = 256;

/// The `StandardMaterial` this mesh was given, whatever it is drawn with now.
#[derive(Component, Clone, Debug)]
pub struct BaseMaterial(pub Handle<StandardMaterial>);

/// The shading in force and the copies made for it, by base material.
#[derive(Resource, Default)]
struct Shaded {
    now: Shading,
    toon: HashMap<AssetId<StandardMaterial>, Handle<FlatExtendedMaterial>>,
    psx: HashMap<AssetId<StandardMaterial>, Handle<PsxMaterial>>,
}

pub fn plugin(app: &mut App) {
    load_internal_asset!(app, PSX_SHADER, "psx_material.wesl", Shader::from_wesl);
    // `default()`, not `global()`: only the material and its shader; we do the swapping.
    app.add_plugins((
        FlatShaderPlugin::default(),
        MaterialPlugin::<PsxMaterial>::default(),
    ))
    .init_resource::<Shaded>()
    .add_observer(remember_base)
    .add_systems(
        PostUpdate,
        (
            follow_base_edits,
            retune_toon.run_if(resource_changed::<ToonLook>),
            retune_psx.run_if(resource_changed::<PsxLook>),
            apply_shading,
        )
            .chain(),
    );
}

fn remember_base(
    insert: On<Insert<MeshMaterial3d<StandardMaterial>>>,
    materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut commands: Commands,
) {
    let entity = insert.event_target();
    if let Ok(material) = materials.get(entity) {
        commands
            .entity(entity)
            .try_insert(BaseMaterial(material.0.clone()));
    }
}

fn toon_extension(toon: &ToonLook) -> FlatShader {
    FlatShaderBuilder::default()
        .shadow_tint(toon.shadow_tint().into())
        .highlight_tint(toon.highlight_tint().into())
        .color_levels(toon.levels)
        .dark_level(toon.dark_level)
        .light_tint(toon.lamp_hue)
        .build()
}

fn tune_psx(ext: &mut PsxMaterialExtension, psx: &PsxLook, palettes: &Palettes) {
    ext.set_snap_enabled(psx.snap > 0.0);
    ext.snap_amount = psx.snap.max(1.0);
    ext.set_quantize_enabled(psx.colours > 0);
    ext.quantize_steps = psx.colours.max(2);
    let palette = psx
        .palette
        .as_deref()
        .and_then(|name| palettes.index_of(name))
        .and_then(|i| palettes.get(i));
    match palette {
        Some(p) => {
            ext.set_use_palette(true);
            let colours = p.colors.iter().take(PSX_PALETTE_MAX);
            ext.palette_size = colours.len() as u32;
            for (slot, c) in ext.palette_colors.iter_mut().zip(colours) {
                // The material matches the lit colour, which is linear.
                let l = LinearRgba::from(*c);
                *slot = Vec3::new(l.red, l.green, l.blue);
            }
        }
        None => ext.set_use_palette(false),
    }
    ext.set_dither_enabled(psx.dither > 0.0);
    ext.dither_strength = psx.dither;
    ext.set_dither_pattern(match psx.pattern {
        PsxPattern::Bayer4 => DitherPattern::Bayer4x4,
        PsxPattern::Bayer8 => DitherPattern::Bayer8x8,
        PsxPattern::BlueNoise => DitherPattern::BlueNoise,
        PsxPattern::Random => DitherPattern::Random,
    });
    ext.set_color_space(ColorSpace::RGB);
    ext.set_error_diffusion(false);
    ext.set_blend_mode(BlendMode::Replace);
    ext.blend_factor = 1.0;
}

/// A base material edited (a fade, a tint) is copied into its toon and PSX versions; one
/// dropped takes its copies with it.
fn follow_base_edits(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    mut shaded: ResMut<Shaded>,
    standard: Res<Assets<StandardMaterial>>,
    mut toon: ResMut<Assets<FlatExtendedMaterial>>,
    mut psx: ResMut<Assets<PsxMaterial>>,
) {
    for event in events.read() {
        match *event {
            AssetEvent::Modified { id } => {
                let Some(base) = standard.get(id) else {
                    continue;
                };
                if let Some(mut m) = shaded.toon.get(&id).and_then(|h| toon.get_mut(h)) {
                    m.base = base.clone();
                }
                if let Some(mut m) = shaded.psx.get(&id).and_then(|h| psx.get_mut(h)) {
                    m.base = base.clone();
                }
            }
            AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
                shaded.toon.remove(&id);
                shaded.psx.remove(&id);
            }
            _ => {}
        }
    }
}

fn retune_toon(toon: Res<ToonLook>, mut materials: ResMut<Assets<FlatExtendedMaterial>>) {
    let ext = toon_extension(&toon);
    for (_, m) in materials.iter_mut() {
        m.extension.shadow_tint = ext.shadow_tint;
        m.extension.highlight_tint = ext.highlight_tint;
        m.extension.color_levels = ext.color_levels;
        m.extension.dark_level = ext.dark_level;
        m.extension.light_tint = ext.light_tint;
    }
}

fn retune_psx(
    psx: Res<PsxLook>,
    palettes: Res<Palettes>,
    mut materials: ResMut<Assets<PsxMaterial>>,
) {
    for (_, m) in materials.iter_mut() {
        tune_psx(&mut m.extension, &psx, &palettes);
    }
}

/// Put every mesh on the material the look wants: all of them when the shading changes,
/// otherwise only the ones that just got a `StandardMaterial`. A mesh whose base has not
/// loaded yet keeps it and is tried again next frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_shading(
    mut commands: Commands,
    mode: Res<LookMode>,
    state: Res<State<GameState>>,
    editor: Option<Res<EditorSession>>,
    toon_look: Res<ToonLook>,
    psx_look: Res<PsxLook>,
    palettes: Res<Palettes>,
    mut shaded: ResMut<Shaded>,
    standard: Res<Assets<StandardMaterial>>,
    mut toon: ResMut<Assets<FlatExtendedMaterial>>,
    mut psx: ResMut<Assets<PsxMaterial>>,
    all: Query<(Entity, &BaseMaterial)>,
    fresh: Query<(Entity, &BaseMaterial), With<MeshMaterial3d<StandardMaterial>>>,
) {
    let want = if *state.get() == GameState::InGame && editor.is_none() {
        mode.shading()
    } else {
        Shading::Standard
    };
    let meshes: Vec<(Entity, Handle<StandardMaterial>)> = if want != shaded.now {
        shaded.now = want;
        all.iter().map(|(e, b)| (e, b.0.clone())).collect()
    } else if want == Shading::Standard {
        return;
    } else {
        fresh.iter().map(|(e, b)| (e, b.0.clone())).collect()
    };

    for (entity, base) in meshes {
        let mut e = commands.entity(entity);
        match want {
            Shading::Standard => {
                e.try_remove::<(
                    MeshMaterial3d<FlatExtendedMaterial>,
                    MeshMaterial3d<PsxMaterial>,
                )>()
                .try_insert(MeshMaterial3d(base));
            }
            Shading::Toon => {
                let handle = if let Some(h) = shaded.toon.get(&base.id()) {
                    h.clone()
                } else {
                    let Some(material) = standard.get(&base) else {
                        continue;
                    };
                    let h = toon.add(ExtendedMaterial {
                        base: material.clone(),
                        extension: toon_extension(&toon_look),
                    });
                    shaded.toon.insert(base.id(), h.clone());
                    h
                };
                e.try_remove::<(
                    MeshMaterial3d<StandardMaterial>,
                    MeshMaterial3d<PsxMaterial>,
                )>()
                .try_insert(MeshMaterial3d(handle));
            }
            Shading::Psx => {
                let handle = if let Some(h) = shaded.psx.get(&base.id()) {
                    h.clone()
                } else {
                    let Some(material) = standard.get(&base) else {
                        continue;
                    };
                    let mut extension = PsxMaterialExtension::default();
                    tune_psx(&mut extension, &psx_look, &palettes);
                    let h = psx.add(ExtendedMaterial {
                        base: material.clone(),
                        extension,
                    });
                    shaded.psx.insert(base.id(), h.clone());
                    h
                };
                e.try_remove::<(
                    MeshMaterial3d<StandardMaterial>,
                    MeshMaterial3d<FlatExtendedMaterial>,
                )>()
                .try_insert(MeshMaterial3d(handle));
            }
        }
    }
}
