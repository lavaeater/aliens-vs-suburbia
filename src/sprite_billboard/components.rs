use bevy::prelude::*;

// ── LPC sprite sheet layout ──────────────────────────────────────────────────
// Combined sheet: idle frames (cols 0-1) then walk frames (cols 2-10)
// 4 direction rows, 11 total columns → 704 × 256 px at 64×64 per tile.

pub const TILE_PX: u32 = 64;
pub const SHEET_W: u32 = 704;  // (2 + 9) * 64
pub const SHEET_H: u32 = 256;  // 4 dirs * 64
pub const IDLE_FRAMES: u32 = 2;
pub const WALK_FRAMES: u32 = 9;
pub const IDLE_FPS: f32 = 4.0;
pub const WALK_FPS: f32 = 9.0;

/// Direction row indices (LPC standard: up, left, down, right).
pub const DIR_UP: u32 = 0;
pub const DIR_LEFT: u32 = 1;
pub const DIR_DOWN: u32 = 2;
pub const DIR_RIGHT: u32 = 3;

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component, Default, PartialEq, Clone, Copy)]
pub enum SpriteAnim {
    #[default]
    Idle,
    Walk,
}

/// Attached to the billboard child entity of the player.
#[derive(Component)]
pub struct SpriteBillboard {
    pub anim: SpriteAnim,
    pub dir: u32,
    pub frame: u32,
    pub frame_timer: f32,
}

impl Default for SpriteBillboard {
    fn default() -> Self {
        Self {
            anim: SpriteAnim::Idle,
            dir: DIR_DOWN,
            frame: 0,
            frame_timer: 0.0,
        }
    }
}

impl SpriteBillboard {
    /// Compute the UV rect (offset_u, offset_v, tile_w_uv, tile_h_uv) for the current frame.
    pub fn uv_rect(&self) -> Vec4 {
        let tw = TILE_PX as f32 / SHEET_W as f32;
        let th = TILE_PX as f32 / SHEET_H as f32;
        let col = match self.anim {
            SpriteAnim::Idle => self.frame,
            SpriteAnim::Walk => IDLE_FRAMES + self.frame,
        };
        Vec4::new(col as f32 * tw, self.dir as f32 * th, tw, th)
    }
}

/// Resource holding the mesh handle used for all billboard quads (shared).
#[derive(Resource)]
pub struct BillboardMeshHandle(pub Handle<Mesh>);
