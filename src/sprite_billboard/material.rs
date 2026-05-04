use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// UV rect: (offset_u, offset_v, tile_width_uv, tile_height_uv) in [0..1] space.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SpriteBillboardMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub sprite_sheet: Handle<Image>,
    #[uniform(2)]
    pub uv_rect: Vec4,
}

impl Material for SpriteBillboardMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sprite_billboard.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
