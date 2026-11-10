//! Scene lighting: no ambient light, a sky light (moon by default), and the lights placed
//! around the map (`map::map_lights`) and carried by players (`player::systems::flashlight`)
//! doing the actual work.
//!
//! `Ctrl+L` cycles [`LightingPreset`] so the looks can be compared in place: moonlit night,
//! pitch dark (placed lights only) and the old daylight (ambient + sun). The character
//! editor starts in daylight, since you cannot place a hardpoint in the dark.
//!
//! [`SkyLightSettings`] tunes on top of the preset: where the sky light comes from, how
//! strong it is, and extra ambient. The retro panel (F12, `retro::panel`) edits both and
//! keeps them in `retro-settings.ron`.

use bevy::light::{CascadeShadowConfigBuilder, DirectionalLight, GlobalAmbientLight};
use bevy::prelude::*;

use crate::character_editor::state::EditorSession;

/// What lights the scene besides the placed lights.
#[derive(
    Resource, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub enum LightingPreset {
    /// No ambient, a faint blue moon for silhouettes.
    #[default]
    Night,
    /// No ambient, no sky: only the lights on the map and the flashlights.
    Dark,
    /// The old look: ambient plus a sun.
    Day,
}

impl LightingPreset {
    pub const fn next(self) -> Self {
        match self {
            Self::Night => Self::Dark,
            Self::Dark => Self::Day,
            Self::Day => Self::Night,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Night => Self::Day,
            Self::Dark => Self::Night,
            Self::Day => Self::Dark,
        }
    }

    /// One word, for the panel.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Night => "moon",
            Self::Dark => "none",
            Self::Day => "sun",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Night => "night (moon + placed lights)",
            Self::Dark => "dark (placed lights only)",
            Self::Day => "day (ambient + sun)",
        }
    }

    /// Ambient brightness. `gore::despair` dims from this as the danger rises.
    pub const fn ambient(self) -> f32 {
        match self {
            Self::Night | Self::Dark => 0.0,
            Self::Day => 300.0,
        }
    }

    /// The sky light's illuminance (lux) and colour.
    fn sky(self) -> (f32, Color) {
        match self {
            Self::Night => (1200.0, Color::srgb(0.72, 0.8, 1.0)),
            Self::Dark => (0.0, Color::WHITE),
            Self::Day => (5000.0, Color::WHITE),
        }
    }

    /// Whether the sky light casts shadows. Only the sun does: the moon's cascades cost more
    /// than any other single light (`docs/performance.md`) for shadows too faint to read.
    const fn sky_shadows(self) -> bool {
        matches!(self, Self::Day)
    }
}

/// Tuning on top of [`LightingPreset`]: the sky light's direction and strength, and
/// ambient added to the preset's.
#[derive(Resource, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SkyLightSettings {
    /// Degrees above the horizon the light comes from (90 = straight down).
    pub elevation_deg: f32,
    /// Degrees round the compass, 0 = shining towards -Z, growing clockwise from above.
    pub azimuth_deg: f32,
    /// Multiplies the preset's illuminance.
    pub strength: f32,
    /// Ambient brightness added to the preset's (Day has 300).
    pub ambient: f32,
}

impl Default for SkyLightSettings {
    fn default() -> Self {
        // Where the light has always been: the old Euler (-0.5, 0.2, 0.4).
        Self {
            elevation_deg: 30.0,
            azimuth_deg: 345.0,
            strength: 1.0,
            ambient: 0.0,
        }
    }
}

impl SkyLightSettings {
    /// Which way the light shines.
    pub fn direction(&self) -> Vec3 {
        let el = self.elevation_deg.clamp(1.0, 89.9).to_radians();
        let az = self.azimuth_deg.to_radians();
        Vec3::new(az.sin() * el.cos(), -el.sin(), -az.cos() * el.cos())
    }

    pub fn transform(&self) -> Transform {
        Transform::default().looking_to(self.direction(), Vec3::Y)
    }

    /// The ambient brightness for `preset` with the extra added.
    pub fn ambient_for(&self, preset: LightingPreset) -> f32 {
        preset.ambient() + self.ambient.max(0.0)
    }
}

/// The one directional light: the moon at night, the sun by day.
#[derive(Component)]
pub struct SkyLight;

pub fn spawn_lights(
    mut commands: Commands,
    editor: Option<Res<EditorSession>>,
    mut preset: ResMut<LightingPreset>,
    sky: Res<SkyLightSettings>,
) {
    if editor.is_some() {
        *preset = LightingPreset::Day;
    }
    let (illuminance, color) = preset.sky();
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: sky.ambient_for(*preset),
        ..default()
    });
    commands.spawn((
        Name::from("Sky Light"),
        SkyLight,
        DirectionalLight {
            illuminance: illuminance * sky.strength,
            color,
            shadow_maps_enabled: preset.sky_shadows(),
            ..default()
        },
        sky.transform(),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
    ));
}

/// Other screens (asset browser, showcase) light their own scenes and expect the default
/// ambient, so leaving the match puts it back.
pub fn restore_ambient(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight::default());
}

/// `Ctrl+L`: next preset.
pub fn cycle_lighting_preset(keys: Res<ButtonInput<KeyCode>>, mut preset: ResMut<LightingPreset>) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::KeyL) {
        *preset = preset.next();
        info!("lighting: {}", preset.label());
    }
}

/// Push a changed preset or sky tuning into the sky light and the ambient light.
pub fn apply_lighting_preset(
    preset: Res<LightingPreset>,
    settings: Res<SkyLightSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sky: Query<(&mut DirectionalLight, &mut Transform), With<SkyLight>>,
) {
    if !preset.is_changed() && !settings.is_changed() {
        return;
    }
    ambient.brightness = settings.ambient_for(*preset);
    let (illuminance, color) = preset.sky();
    for (mut light, mut transform) in &mut sky {
        light.illuminance = illuminance * settings.strength;
        light.color = color;
        light.shadow_maps_enabled = preset.sky_shadows();
        *transform = settings.transform();
    }
}

/// Makes a light breathe: a flame's flicker or a slow pulse. `base` is the intensity it
/// varies around.
#[derive(Component, Clone, Copy, Debug)]
pub struct LightAnimation {
    pub base: f32,
    pub style: LightStyle,
}

#[derive(Clone, Copy, Debug)]
pub enum LightStyle {
    /// Irregular, fast, small dips: a flame or a failing bulb. `seed` keeps neighbours
    /// out of step.
    Flicker { seed: f32 },
    /// A smooth swell, `speed` cycles per second, dipping to `1 - depth` of the base.
    Pulse { speed: f32, depth: f32 },
}

impl LightStyle {
    /// The intensity factor at time `t` seconds, in `(0, 1]`.
    pub fn factor(self, t: f32) -> f32 {
        match self {
            Self::Flicker { seed } => {
                // Three incommensurate sines read as noise without a noise texture.
                let t = t + seed * 17.0;
                let n = (t * 11.0).sin() * 0.5 + (t * 23.7).sin() * 0.3 + (t * 5.3).sin() * 0.2;
                0.85 + 0.15 * n
            }
            Self::Pulse { speed, depth } => {
                let wave = 0.5 + 0.5 * (t * speed * std::f32::consts::TAU).sin();
                1.0 - depth * wave
            }
        }
    }
}

pub fn animate_lights(
    time: Res<Time>,
    mut lights: Query<(
        &LightAnimation,
        Option<&mut PointLight>,
        Option<&mut SpotLight>,
    )>,
) {
    let t = time.elapsed_secs();
    for (animation, point, spot) in &mut lights {
        let intensity = animation.base * animation.style.factor(t);
        if let Some(mut point) = point {
            point.intensity = intensity;
        }
        if let Some(mut spot) = spot {
            spot.intensity = intensity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SkyLightSettings;

    #[test]
    fn the_default_sky_comes_from_where_it_always_did() {
        let old = Quat::from_euler(EulerRot::XYZ, -0.5, 0.2, 0.4) * Vec3::NEG_Z;
        let new = SkyLightSettings::default().transform().rotation * Vec3::NEG_Z;
        assert!(old.angle_between(new) < 0.05, "{old} vs {new}");
    }

    #[test]
    fn straight_down_still_has_a_direction() {
        let sky = SkyLightSettings {
            elevation_deg: 90.0,
            ..default()
        };
        let dir = sky.transform().rotation * Vec3::NEG_Z;
        assert!(dir.is_finite() && dir.y < -0.99);
    }

    #[test]
    fn only_the_sun_casts_shadows() {
        use super::LightingPreset::{Dark, Day, Night};
        assert!(Day.sky_shadows());
        assert!(!Night.sky_shadows());
        assert!(!Dark.sky_shadows());
    }

    use super::*;

    #[test]
    fn presets_cycle_through_all_three() {
        let mut p = LightingPreset::default();
        let mut seen = vec![p];
        for _ in 0..2 {
            p = p.next();
            seen.push(p);
        }
        assert_eq!(p.next(), LightingPreset::default());
        assert_eq!(seen.len(), 3);
        assert!(seen.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn night_and_dark_have_no_ambient() {
        assert_eq!(LightingPreset::Night.ambient(), 0.0);
        assert_eq!(LightingPreset::Dark.ambient(), 0.0);
    }

    #[test]
    fn animations_stay_positive_and_never_exceed_the_base() {
        for style in [
            LightStyle::Flicker { seed: 0.3 },
            LightStyle::Pulse {
                speed: 0.7,
                depth: 0.6,
            },
        ] {
            for i in 0..500 {
                let f = style.factor(i as f32 * 0.013);
                assert!(f > 0.0 && f <= 1.0 + 1e-6, "{style:?} at {i}: {f}");
            }
        }
    }
}
