use bevy::math::Vec3;
use bevy::prelude::{Component, Resource};
use bevy::reflect::Reflect;

#[derive(Component)]
pub struct GameCamera {}

#[derive(Component, Reflect)]
pub struct CameraOffset(pub Vec3);

/// Something the camera keeps in view. Every player carries one; the camera aims at the
/// weighted centroid of all of them and zooms out until they all fit.
#[derive(Component, Reflect)]
pub struct CameraTarget {
    pub weight: f32,
}

impl Default for CameraTarget {
    fn default() -> Self {
        Self { weight: 1.0 }
    }
}

/// The smoothed point the camera is looking at, plus how far the furthest target is from
/// it. One place for the HUD, abilities ("all aliens on screen") and a future split
/// screen to read instead of each re-deriving it from the players.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraFocus {
    pub center: Vec3,
    /// Distance from `center` to the furthest [`CameraTarget`].
    pub radius: f32,
    /// Current zoom-out factor (1.0 = the configured zoom, >1 = pulled back to fit).
    pub fit: f32,
    /// True once at least one target has been seen, so the first frame snaps instead of
    /// lerping in from the origin.
    pub primed: bool,
}

impl Default for CameraFocus {
    fn default() -> Self {
        Self { center: Vec3::ZERO, radius: 0.0, fit: 1.0, primed: false }
    }
}

#[allow(dead_code)]
#[derive(Component)]
pub struct PixelCanvas;
