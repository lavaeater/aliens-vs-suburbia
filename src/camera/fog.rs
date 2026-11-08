//! Fog: Bevy's `DistanceFog` on the game camera, set so it reads as **depth haze**. The
//! camera is orthographic and looks down at the floor from a few metres off, so how far a
//! point is from the camera is mostly how low it is: valleys sink into the haze and
//! hilltops stand clear of it (and the top of the screen, a little further off, is a
//! touch hazier). The fog starts just past the ground under the camera's focus and keeps
//! that place when the camera zooms out to fit everyone.
//!
//! `gore::despair` owns the camera's `DistanceFog`: this module only works out
//! [`SceneFog`], which despair blends its dread haze into.
//!
//! Its colour follows the lighting preset (a blue-black night, a pale grey day).
//! **Ctrl+O** cycles Off / Haze / Thick; default Haze, or `--fog <off|haze|thick>`.

use bevy::prelude::*;

use super::components::{CameraFocus, CameraOffset, GameCamera};
use crate::general::systems::lights_systems::LightingPreset;

/// How thick the fog is.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FogLevel {
    Off,
    #[default]
    Haze,
    Thick,
}

impl FogLevel {
    const fn next(self) -> Self {
        match self {
            Self::Off => Self::Haze,
            Self::Haze => Self::Thick,
            Self::Thick => Self::Off,
        }
    }

    /// Metres past the focus where the fog starts, how many more until it is full, and
    /// how full that is (the colour's alpha).
    const fn shape(self) -> Option<(f32, f32, f32)> {
        match self {
            Self::Off => None,
            Self::Haze => Some((0.6, 9.0, 0.75)),
            Self::Thick => Some((0.0, 4.5, 0.92)),
        }
    }

    /// `off`, `haze` or `thick` (`--fog <level>`).
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Off, Self::Haze, Self::Thick]
            .into_iter()
            .find(|l| l.label() == name)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Haze => "haze",
            Self::Thick => "thick",
        }
    }
}

/// The fog's colour under a lighting preset.
pub fn fog_color(preset: LightingPreset) -> Color {
    match preset {
        LightingPreset::Night => Color::srgb(0.03, 0.04, 0.08),
        LightingPreset::Dark => Color::srgb(0.01, 0.01, 0.02),
        LightingPreset::Day => Color::srgb(0.68, 0.72, 0.76),
    }
}

/// The fog for a camera `distance` metres from the ground it centres on.
pub fn fog_for(level: FogLevel, preset: LightingPreset, distance: f32) -> Option<DistanceFog> {
    let (start, depth, strength) = level.shape()?;
    Some(DistanceFog {
        color: fog_color(preset).with_alpha(strength),
        falloff: FogFalloff::Linear {
            start: distance + start,
            end: distance + start + depth,
        },
        ..default()
    })
}

pub fn cycle_fog(keys: Res<ButtonInput<KeyCode>>, mut level: ResMut<FogLevel>) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::KeyO) {
        *level = level.next();
        info!("fog: {}", level.label());
    }
}

/// The fog the scene has before the mood gets at it. `gore::despair` puts it on the
/// camera, blended with its own dread haze (it owns the camera's `DistanceFog`).
#[derive(Resource, Debug, Clone, Default)]
pub struct SceneFog(pub Option<DistanceFog>);

/// Keep the scene's fog where the camera's focus is.
pub fn apply_fog(
    level: Res<FogLevel>,
    preset: Res<LightingPreset>,
    focus: Res<CameraFocus>,
    cameras: Query<&CameraOffset, With<GameCamera>>,
    mut scene: ResMut<SceneFog>,
) {
    let Some(offset) = cameras.iter().next() else {
        return;
    };
    let distance = offset.0.length() * focus.fit.max(0.01);
    scene.0 = fog_for(*level, *preset, distance);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fog_starts_past_the_focus_and_follows_the_zoom() {
        let near = fog_for(FogLevel::Haze, LightingPreset::Night, 3.0).expect("haze");
        let far = fog_for(FogLevel::Haze, LightingPreset::Night, 6.0).expect("haze");
        let start = |f: &DistanceFog| match f.falloff {
            FogFalloff::Linear { start, .. } => start,
            _ => f32::NAN,
        };
        assert!(start(&near) > 3.0);
        assert!((start(&far) - start(&near) - 3.0).abs() < 1e-5);
        assert!(fog_for(FogLevel::Off, LightingPreset::Day, 3.0).is_none());
    }

    #[test]
    fn thick_fog_is_thicker() {
        let haze = FogLevel::Haze.shape().expect("haze");
        let thick = FogLevel::Thick.shape().expect("thick");
        assert!(thick.1 < haze.1 && thick.2 > haze.2);
        assert_eq!(FogLevel::Thick.next(), FogLevel::Off);
    }
}

