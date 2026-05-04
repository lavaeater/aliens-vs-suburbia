use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use image::{DynamicImage, GenericImage, GenericImageView, RgbaImage};
use image::imageops;

use crate::character_creator::config::{CharacterConfig, ComposedSpriteSheet};
use crate::sprite_billboard::components::{
    IDLE_FRAMES, SHEET_H, SHEET_W, TILE_PX, WALK_FRAMES, DIR_DOWN,
};

// ── Internal helpers ─────────────────────────────────────────────────────────

fn load_rgba(path: &str) -> Option<RgbaImage> {
    match image::open(path) {
        Ok(img) => Some(img.to_rgba8()),
        Err(e) => {
            warn!("LPC layer not found: {path} — {e}");
            None
        }
    }
}

/// Alpha-composite `src` on top of `dst` in-place.
fn overlay(dst: &mut RgbaImage, src: &RgbaImage) {
    imageops::overlay(dst, src, 0, 0);
}

/// Composite all layers for one animation into a single RGBA image.
/// Returns None if the base body layer cannot be loaded.
fn composite_anim(config: &CharacterConfig, anim_file: &str) -> Option<RgbaImage> {
    let base_path = format!("{}/{}", config.body_path(), anim_file);
    let mut canvas = load_rgba(&base_path)?;

    for layer_path in &config.extra_layers {
        let path = format!("{}/{}", layer_path, anim_file);
        if let Some(layer) = load_rgba(&path) {
            if layer.dimensions() == canvas.dimensions() {
                overlay(&mut canvas, &layer);
            } else {
                warn!("Layer size mismatch: {path}");
            }
        }
    }
    Some(canvas)
}

/// Build the combined 704×256 billboard sheet (idle cols 0-1, walk cols 2-10).
fn build_combined_sheet(idle: &RgbaImage, walk: &RgbaImage) -> RgbaImage {
    let w = SHEET_W;
    let h = SHEET_H;
    let mut sheet = RgbaImage::new(w, h);

    // Copy idle (128×256) into left portion.
    let idle_w = (IDLE_FRAMES * TILE_PX) as u32;
    for y in 0..h {
        for x in 0..idle_w {
            if x < idle.width() && y < idle.height() {
                sheet.put_pixel(x, y, *idle.get_pixel(x, y));
            }
        }
    }

    // Copy walk (576×256) starting at x=128.
    let walk_offset = idle_w;
    let walk_w = (WALK_FRAMES * TILE_PX) as u32;
    for y in 0..h {
        for x in 0..walk_w {
            if x < walk.width() && y < walk.height() {
                sheet.put_pixel(walk_offset + x, y, *walk.get_pixel(x, y));
            }
        }
    }
    sheet
}

/// Extract a single 64×64 portrait tile: idle animation, facing down (row DIR_DOWN, frame 0).
fn extract_portrait(idle: &RgbaImage) -> RgbaImage {
    let tile = TILE_PX as u32;
    let y_off = DIR_DOWN * tile;
    let sub = DynamicImage::ImageRgba8(idle.clone())
        .crop_imm(0, y_off, tile, tile)
        .to_rgba8();
    sub
}

fn to_bevy_image(rgba: RgbaImage) -> Image {
    let (w, h) = rgba.dimensions();
    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

// ── Public system ─────────────────────────────────────────────────────────────

/// Runs whenever `CharacterConfig` changes; recomposites and updates the sheet handles.
pub fn recompose_character_system(
    config: Res<CharacterConfig>,
    mut sheet: ResMut<ComposedSpriteSheet>,
    mut images: ResMut<Assets<Image>>,
) {
    if !config.is_changed() { return; }

    let Some(idle) = composite_anim(&config, "idle.png") else {
        warn!("Failed to composite idle animation — skipping.");
        return;
    };
    let Some(walk) = composite_anim(&config, "walk.png") else {
        warn!("Failed to composite walk animation — skipping.");
        return;
    };

    let portrait = extract_portrait(&idle);
    let combined = build_combined_sheet(&idle, &walk);

    // Replace or insert handles.
    if let Some(h) = &sheet.billboard_handle {
        if let Some(img) = images.get_mut(h) {
            *img = to_bevy_image(combined);
        } else {
            sheet.billboard_handle = Some(images.add(to_bevy_image(combined)));
        }
    } else {
        sheet.billboard_handle = Some(images.add(to_bevy_image(combined)));
    }

    if let Some(h) = &sheet.portrait_handle {
        if let Some(img) = images.get_mut(h) {
            *img = to_bevy_image(portrait);
        } else {
            sheet.portrait_handle = Some(images.add(to_bevy_image(portrait)));
        }
    } else {
        sheet.portrait_handle = Some(images.add(to_bevy_image(portrait)));
    }
}
