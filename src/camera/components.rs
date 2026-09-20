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

/// Decaying screen shake, fed by explosions. `camera_follow` adds a small jitter scaled
/// by `amplitude` to the camera position each frame.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CameraShake {
    pub amplitude: f32,
}

impl CameraShake {
    /// Add trauma; stacking blasts saturate rather than fly off.
    pub fn add(&mut self, amount: f32) {
        self.amplitude = (self.amplitude + amount).min(1.0);
    }

    /// Decay towards zero and return this frame's jitter offset.
    pub fn tick(&mut self, dt: f32, t: f32) -> Vec3 {
        if self.amplitude <= 0.0 {
            return Vec3::ZERO;
        }
        // Trauma^2 for a snappy start and gentle tail (Squirrel Eiserloh's trick).
        let strength = self.amplitude * self.amplitude * 0.35;
        self.amplitude = (self.amplitude - dt * 1.8).max(0.0);
        Vec3::new((t * 37.0).sin(), (t * 43.0).cos(), (t * 29.0).sin()) * strength
    }
}

#[allow(dead_code)]
#[derive(Component)]
pub struct PixelCanvas;
