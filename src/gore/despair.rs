//! The despair pass. Mood, not mechanic: as the danger rises the world curdles —
//! colors drain and turn sickly, the light dims, a haze creeps in, and (when a player
//! is bleeding out) a heartbeat thuds. Everything rides the `danger` measure the music
//! system already computes and smooths (`MusicMoods`), so audio and image swell together.
//!
//! All visual channels are asset-free (post-processing + light tuning). The heartbeat is
//! silent until you drop a `heartbeat*.wav` in `assets/sfx/` (see `sfx.rs`).

use bevy::light::GlobalAmbientLight;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::view::ColorGrading;

use crate::camera::components::GameCamera;
use crate::general::components::Health;
use crate::gore::sfx::{PlaySfx, SfxKind};
use crate::music::game_music_plugin::MusicMoods;
use crate::player::components::{Player, PlayerDead};

/// How strong the despair look is allowed to get (0 = off, 1 = full dread). Lets you
/// dial the whole effect down without touching the per-channel curves below.
#[derive(Resource)]
pub struct DespairSettings {
    pub max_strength: f32,
    /// Ambient brightness at calm vs. full dread.
    pub ambient_calm: f32,
    pub ambient_dread: f32,
}

impl Default for DespairSettings {
    fn default() -> Self {
        Self {
            max_strength: 1.0,
            // spawn_lights sets 300 at calm; we crush it toward a gloom.
            ambient_calm: 300.0,
            ambient_dread: 90.0,
        }
    }
}

/// Linear blend.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Drive the color grade, ambient light and fog off the smoothed `danger` measure.
/// The camera may not carry `ColorGrading`/`DistanceFog` yet (it's spawned on entering
/// the game), so we insert them on first sighting, then mutate in place after.
#[allow(clippy::type_complexity)]
pub fn apply_despair(
    mut commands: Commands,
    settings: Res<DespairSettings>,
    moods: Option<Res<MusicMoods>>,
    ambient: Option<ResMut<GlobalAmbientLight>>,
    mut cameras: Query<(Entity, Option<&mut ColorGrading>, Option<&mut DistanceFog>), With<GameCamera>>,
) {
    let danger = moods.map(|m| m.danger).unwrap_or(0.0).clamp(0.0, 1.0);
    let d = danger * settings.max_strength;

    // ── Ambient light: the room dims as dread rises. ──────────────────────────
    if let Some(mut ambient) = ambient {
        ambient.brightness = lerp(settings.ambient_calm, settings.ambient_dread, d);
    }

    // ── Camera color grade + fog. ─────────────────────────────────────────────
    let grade = despair_grade(d);
    let fog = despair_fog(d);
    for (entity, color_grading, distance_fog) in cameras.iter_mut() {
        match color_grading {
            Some(mut cg) => *cg = grade.clone(),
            None => {
                commands.entity(entity).insert(grade.clone());
            }
        }
        match distance_fog {
            Some(mut f) => *f = fog.clone(),
            None => {
                commands.entity(entity).insert(fog.clone());
            }
        }
    }
}

/// The color grade for a given dread level `d` (0..1): drain saturation, cool/sicken the
/// hue, crush exposure. At `d = 0` this is a no-op grade.
fn despair_grade(d: f32) -> ColorGrading {
    let mut grade = ColorGrading::default();
    // Desaturate toward a sodium-lit gray; never fully monochrome.
    grade.global.post_saturation = lerp(1.0, 0.35, d);
    // Slight green tint (negative = greener) for a sickly cast.
    grade.global.tint = lerp(0.0, -0.08, d);
    // A touch warmer, like failing streetlights.
    grade.global.temperature = lerp(0.0, 0.12, d);
    // Pull the exposure down so dread reads as gloom.
    grade.global.exposure = lerp(0.0, -0.7, d);
    // Crush the shadows a little for contrast.
    grade.shadows.contrast = lerp(1.0, 1.15, d);
    grade
}

/// The haze for a given dread level. Alpha on the color fades the whole effect in, so at
/// `d = 0` the fog is invisible regardless of density.
fn despair_fog(d: f32) -> DistanceFog {
    DistanceFog {
        color: Color::srgba(0.12, 0.13, 0.11, d * 0.9),
        directional_light_color: Color::NONE,
        directional_light_exponent: 8.0,
        falloff: FogFalloff::Exponential { density: lerp(0.0, 0.035, d) },
    }
}

/// Paces the heartbeat: faster as the most-hurt living player nears death.
#[derive(Resource)]
pub struct Heartbeat {
    timer: Timer,
}

impl Default for Heartbeat {
    fn default() -> Self {
        // Repeating; we reset the duration each beat from current health.
        Self { timer: Timer::from_seconds(1.0, TimerMode::Repeating) }
    }
}

/// Below this health fraction a living player's heart starts pounding.
const HEARTBEAT_THRESHOLD: f32 = 0.35;

/// Thud a heartbeat when a living player is bleeding out, quickening toward death.
/// Silent until a `heartbeat*.wav` exists (see `sfx.rs`); the pacing logic runs regardless.
pub fn despair_heartbeat(
    time: Res<Time>,
    mut heart: ResMut<Heartbeat>,
    players: Query<&Health, (With<Player>, Without<PlayerDead>)>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    // Lowest health fraction among the living.
    let lowest = players
        .iter()
        .filter(|h| h.max_health > 0 && h.health > 0)
        .map(|h| h.health as f32 / h.max_health as f32)
        .fold(f32::INFINITY, f32::min);

    if !lowest.is_finite() || lowest >= HEARTBEAT_THRESHOLD {
        heart.timer.reset();
        return;
    }

    // Map health fraction (0..threshold) to a beat interval: ~1.1s calm-ish down to ~0.4s.
    let t = (lowest / HEARTBEAT_THRESHOLD).clamp(0.0, 1.0);
    let interval = lerp(0.4, 1.1, t);
    heart.timer.tick(time.delta());
    if heart.timer.just_finished() {
        heart.timer.set_duration(std::time::Duration::from_secs_f32(interval));
        sfx.write(PlaySfx { kind: SfxKind::Heartbeat, gain_db: -4.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::{despair_fog, despair_grade, lerp};
    use bevy::color::Alpha;

    #[test]
    fn zero_dread_is_a_neutral_grade() {
        let g = despair_grade(0.0);
        assert_eq!(g.global.post_saturation, 1.0, "no desaturation when calm");
        assert_eq!(g.global.exposure, 0.0);
        assert_eq!(despair_fog(0.0).color.alpha(), 0.0, "fog invisible when calm");
    }

    #[test]
    fn full_dread_desaturates_dims_and_hazes() {
        let g = despair_grade(1.0);
        assert!(g.global.post_saturation < 0.5, "colors drain toward gray");
        assert!(g.global.exposure < 0.0, "the world darkens");
        assert!(despair_fog(1.0).color.alpha() > 0.5, "haze is visible");
    }

    #[test]
    fn lerp_is_monotonic_between_endpoints() {
        assert_eq!(lerp(2.0, 4.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 4.0, 1.0), 4.0);
        assert_eq!(lerp(2.0, 4.0, 0.5), 3.0);
    }
}
