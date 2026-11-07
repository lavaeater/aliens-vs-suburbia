//! What the retro look should be ([`RetroLook`]), the settings the F12 panel and the
//! hotkeys step through ([`Setting`]), and `retro-settings.ron`, where it is kept between
//! runs (the palette by name, so adding a palette does not shift it). Pure, so tested here.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::Palettes;
use crate::general::systems::lights_systems::{LightingPreset, SkyLightSettings};

pub const SETTINGS_PATH: &str = "retro-settings.ron";
pub const MAX_PIXEL_SIZE: u32 = 8;
const DITHER_STEPS: [f32; 5] = [0.0, 0.2, 0.35, 0.7, 1.0];

/// What the retro look should be. Edited by the hotkeys and the F12 panel; read every frame.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct RetroLook {
    pub enabled: bool,
    /// Screen pixels per texel.
    pub pixel_size: u32,
    /// Index into `Palettes`; 0 is full colour (pixelation only).
    pub palette: usize,
    /// 0 = nearest colour, 1 = full ordered dither between the two nearest.
    pub dither: f32,
    /// Brightness before anything else; dark palettes on a night map want more.
    pub exposure: f32,
    /// Lightness spread around mid grey, 1 = as rendered.
    pub contrast: f32,
    /// Chroma, 1 = as rendered.
    pub saturation: f32,
    /// How far the shadows lean violet (and lift a little), the Giraud night.
    pub shadow_tint: f32,
    /// How far the highlights lean warm.
    pub warmth: f32,
    /// Ink lines on the dark side of every edge in the picture: 0 off, 1 full ink.
    pub ink: f32,
    /// How big a lightness step (OKLab L) gets a line.
    pub ink_threshold: f32,
}

impl Default for RetroLook {
    fn default() -> Self {
        Self {
            enabled: false,
            pixel_size: 3,
            palette: 0,
            dither: 0.7,
            exposure: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            shadow_tint: 0.0,
            warmth: 0.0,
            ink: 0.0,
            ink_threshold: 0.12,
        }
    }
}

/// One row of the panel.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Enabled,
    Palette,
    PixelSize,
    Dither,
    Exposure,
    Contrast,
    Saturation,
    ShadowTint,
    Warmth,
    Ink,
    InkThreshold,
}

impl Setting {
    pub const ALL: [Setting; 11] = [
        Setting::Enabled,
        Setting::Palette,
        Setting::PixelSize,
        Setting::Dither,
        Setting::Exposure,
        Setting::Contrast,
        Setting::Saturation,
        Setting::ShadowTint,
        Setting::Warmth,
        Setting::Ink,
        Setting::InkThreshold,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Setting::Enabled => "Retro",
            Setting::Palette => "Palette",
            Setting::PixelSize => "Pixels",
            Setting::Dither => "Dither",
            Setting::Exposure => "Exposure",
            Setting::Contrast => "Contrast",
            Setting::Saturation => "Colour",
            Setting::ShadowTint => "Violet",
            Setting::Warmth => "Warmth",
            Setting::Ink => "Ink",
            Setting::InkThreshold => "Ink edge",
        }
    }

    /// Move one step left (`-1`) or right (`1`).
    pub fn step(self, look: &mut RetroLook, palettes: &Palettes, dir: i32) {
        let up = dir > 0;
        let nudge = |v: f32, by: f32, lo: f32, hi: f32| {
            let next = if up { v + by } else { v - by };
            // Round off float drift so the labels and the file stay tidy.
            ((next / by).round() * by).clamp(lo, hi)
        };
        match self {
            Setting::Enabled => look.enabled = !look.enabled,
            Setting::Palette => {
                let count = palettes.0.len() + 1;
                look.palette = if up {
                    (look.palette + 1) % count
                } else {
                    (look.palette + count - 1) % count
                };
            }
            Setting::PixelSize => {
                look.pixel_size = if up {
                    (look.pixel_size + 1).min(MAX_PIXEL_SIZE)
                } else {
                    look.pixel_size.saturating_sub(1).max(1)
                };
            }
            Setting::Dither => {
                let at = DITHER_STEPS
                    .iter()
                    .position(|d| (d - look.dither).abs() < 1e-3)
                    .unwrap_or(0);
                let next = if up {
                    (at + 1) % DITHER_STEPS.len()
                } else {
                    (at + DITHER_STEPS.len() - 1) % DITHER_STEPS.len()
                };
                look.dither = DITHER_STEPS.get(next).copied().unwrap_or(0.0);
            }
            Setting::Exposure => look.exposure = nudge(look.exposure, 0.1, 0.2, 4.0),
            Setting::Contrast => look.contrast = nudge(look.contrast, 0.05, 0.5, 2.0),
            Setting::Saturation => look.saturation = nudge(look.saturation, 0.1, 0.0, 2.5),
            Setting::ShadowTint => look.shadow_tint = nudge(look.shadow_tint, 0.1, 0.0, 2.0),
            Setting::Warmth => look.warmth = nudge(look.warmth, 0.1, 0.0, 2.0),
            Setting::Ink => look.ink = nudge(look.ink, 0.25, 0.0, 1.0),
            Setting::InkThreshold => {
                look.ink_threshold = nudge(look.ink_threshold, 0.02, 0.02, 0.5);
            }
        }
    }

    /// The value as the panel shows it.
    pub fn value(self, look: &RetroLook, palettes: &Palettes) -> String {
        match self {
            Setting::Enabled => if look.enabled { "on" } else { "off" }.to_string(),
            Setting::Palette => match palettes.get(look.palette) {
                Some(p) => format!("{} ({})", p.name, p.len()),
                None => "full colour".to_string(),
            },
            Setting::PixelSize => format!("{} px", look.pixel_size),
            Setting::Dither => format!("{:.2}", look.dither),
            Setting::Exposure => format!("{:.1}", look.exposure),
            Setting::Contrast => format!("{:.2}", look.contrast),
            Setting::Saturation => format!("{:.1}", look.saturation),
            Setting::ShadowTint => format!("{:.1}", look.shadow_tint),
            Setting::Warmth => format!("{:.1}", look.warmth),
            Setting::Ink => if look.ink <= 0.0 {
                "off".to_string()
            } else {
                format!("{:.2}", look.ink)
            },
            Setting::InkThreshold => format!("{:.2}", look.ink_threshold),
        }
    }
}

/// A lighting row of the panel: the sky preset and its tuning.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightSetting {
    Preset,
    Height,
    Turn,
    Strength,
    Ambient,
}

impl LightSetting {
    pub const ALL: [LightSetting; 5] = [
        LightSetting::Preset,
        LightSetting::Height,
        LightSetting::Turn,
        LightSetting::Strength,
        LightSetting::Ambient,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LightSetting::Preset => "Sky",
            LightSetting::Height => "Height",
            LightSetting::Turn => "Turn",
            LightSetting::Strength => "Strength",
            LightSetting::Ambient => "Ambient",
        }
    }

    pub fn step(self, preset: &mut LightingPreset, sky: &mut SkyLightSettings, dir: i32) {
        let up = dir > 0;
        let by = |step: f32| if up { step } else { -step };
        match self {
            LightSetting::Preset => {
                *preset = if up { preset.next() } else { preset.previous() };
            }
            LightSetting::Height => {
                sky.elevation_deg = (sky.elevation_deg + by(5.0)).clamp(5.0, 90.0);
            }
            LightSetting::Turn => sky.azimuth_deg = (sky.azimuth_deg + by(15.0)).rem_euclid(360.0),
            LightSetting::Strength => {
                sky.strength = ((sky.strength + by(0.25)) * 4.0).round() / 4.0;
                sky.strength = sky.strength.clamp(0.0, 8.0);
            }
            LightSetting::Ambient => sky.ambient = (sky.ambient + by(25.0)).clamp(0.0, 1000.0),
        }
    }

    pub fn value(self, preset: LightingPreset, sky: &SkyLightSettings) -> String {
        match self {
            LightSetting::Preset => preset.name().to_string(),
            LightSetting::Height => format!("{:.0} deg", sky.elevation_deg),
            LightSetting::Turn => format!("{:.0} deg", sky.azimuth_deg),
            LightSetting::Strength => format!("{:.2}x", sky.strength),
            LightSetting::Ambient => format!("+{:.0}", sky.ambient),
        }
    }
}

/// One line for the on-screen flash when something changes.
pub fn summary(look: &RetroLook, palettes: &Palettes) -> String {
    if !look.enabled {
        return "Retro off   (F12 settings, Ctrl+K on)".to_string();
    }
    format!(
        "{}   {}   dither {}   ink {}   (F12)",
        Setting::Palette.value(look, palettes),
        Setting::PixelSize.value(look, palettes),
        Setting::Dither.value(look, palettes),
        Setting::Ink.value(look, palettes),
    )
}

/// `retro-settings.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct RetroFile {
    pub enabled: bool,
    pub palette: String,
    pub pixel_size: u32,
    pub dither: f32,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub shadow_tint: f32,
    pub warmth: f32,
    pub ink: f32,
    pub ink_threshold: f32,
    /// The sky preset and its tuning (`general::systems::lights_systems`).
    pub light: LightingPreset,
    pub sky: SkyLightSettings,
}

impl Default for RetroFile {
    fn default() -> Self {
        let look = RetroLook {
            enabled: true,
            ..default()
        };
        Self::from_look(&look, &Palettes::default(), super::DEFAULT_PALETTE)
    }
}

impl RetroFile {
    fn from_look(look: &RetroLook, palettes: &Palettes, palette: &str) -> Self {
        Self::with_light(look, palettes, palette, LightingPreset::default(), SkyLightSettings::default())
    }

    fn with_light(
        look: &RetroLook,
        palettes: &Palettes,
        palette: &str,
        light: LightingPreset,
        sky: SkyLightSettings,
    ) -> Self {
        Self {
            enabled: look.enabled,
            palette: palettes
                .get(look.palette)
                .map_or_else(|| palette.to_string(), |p| p.name.clone()),
            pixel_size: look.pixel_size,
            dither: look.dither,
            exposure: look.exposure,
            contrast: look.contrast,
            saturation: look.saturation,
            shadow_tint: look.shadow_tint,
            warmth: look.warmth,
            ink: look.ink,
            ink_threshold: look.ink_threshold,
            light,
            sky,
        }
    }

    #[cfg(test)]
    pub fn of(look: &RetroLook, palettes: &Palettes) -> Self {
        Self::from_look(look, palettes, "")
    }

    pub fn of_all(
        look: &RetroLook,
        palettes: &Palettes,
        light: LightingPreset,
        sky: SkyLightSettings,
    ) -> Self {
        Self::with_light(look, palettes, "", light, sky)
    }

    /// The look this file describes; an unknown palette name falls back to full colour.
    pub fn look(&self, palettes: &Palettes) -> RetroLook {
        RetroLook {
            enabled: self.enabled,
            pixel_size: self.pixel_size.clamp(1, MAX_PIXEL_SIZE),
            palette: palettes.index_of(&self.palette).unwrap_or(0),
            dither: self.dither.clamp(0.0, 1.0),
            exposure: self.exposure,
            contrast: self.contrast,
            saturation: self.saturation,
            shadow_tint: self.shadow_tint,
            warmth: self.warmth,
            ink: self.ink.clamp(0.0, 1.0),
            ink_threshold: self.ink_threshold,
        }
    }

    pub fn load() -> Option<Self> {
        let text = std::fs::read_to_string(SETTINGS_PATH).ok()?;
        match ron::from_str(&text) {
            Ok(file) => Some(file),
            Err(e) => {
                warn!("{SETTINGS_PATH}: {e}");
                None
            }
        }
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(SETTINGS_PATH, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retro::palette::Palette;

    fn palettes() -> Palettes {
        Palettes(
            ["a", "b"]
                .iter()
                .map(|n| Palette {
                    name: n.to_string(),
                    colors: vec![Srgba::BLACK, Srgba::WHITE],
                })
                .collect(),
        )
    }

    #[test]
    fn palette_wraps_both_ways_through_full_colour() {
        let p = palettes();
        let mut look = RetroLook::default();
        Setting::Palette.step(&mut look, &p, -1);
        assert_eq!(look.palette, 2);
        Setting::Palette.step(&mut look, &p, 1);
        assert_eq!(look.palette, 0);
        assert_eq!(Setting::Palette.value(&look, &p), "full colour");
    }

    #[test]
    fn numbers_stay_in_range_and_on_their_steps() {
        let p = palettes();
        let mut look = RetroLook::default();
        for _ in 0..30 {
            Setting::Ink.step(&mut look, &p, 1);
            Setting::PixelSize.step(&mut look, &p, -1);
            Setting::Exposure.step(&mut look, &p, 1);
        }
        assert_eq!(look.ink, 1.0);
        assert_eq!(look.pixel_size, 1);
        assert_eq!(Setting::Exposure.value(&look, &p), "4.0");
        Setting::Exposure.step(&mut look, &p, -1);
        assert_eq!(Setting::Exposure.value(&look, &p), "3.9");
    }

    #[test]
    fn dither_cycles_its_steps() {
        let p = palettes();
        let mut look = RetroLook::default();
        Setting::Dither.step(&mut look, &p, 1);
        assert_eq!(look.dither, 1.0);
        Setting::Dither.step(&mut look, &p, 1);
        assert_eq!(look.dither, 0.0);
        Setting::Dither.step(&mut look, &p, -1);
        assert_eq!(look.dither, 1.0);
    }

    #[test]
    fn the_file_keeps_the_palette_by_name() {
        let p = palettes();
        let look = RetroLook {
            palette: 2,
            ink: 0.5,
            ..default()
        };
        let file = RetroFile::of(&look, &p);
        assert_eq!(file.palette, "b");
        let text = ron::to_string(&file).unwrap_or_default();
        let back: RetroFile = ron::from_str(&text).unwrap_or_default();
        assert_eq!(back.look(&p), look);
        let shuffled = Palettes(p.0.into_iter().rev().collect());
        assert_eq!(back.look(&shuffled).palette, 1);
    }

    #[test]
    fn the_light_rows_wrap_and_clamp() {
        let mut preset = LightingPreset::Night;
        let mut sky = SkyLightSettings::default();
        LightSetting::Preset.step(&mut preset, &mut sky, -1);
        assert_eq!(preset, LightingPreset::Day);
        LightSetting::Turn.step(&mut preset, &mut sky, 1);
        assert_eq!(LightSetting::Turn.value(preset, &sky), "0 deg");
        for _ in 0..30 {
            LightSetting::Height.step(&mut preset, &mut sky, 1);
            LightSetting::Strength.step(&mut preset, &mut sky, -1);
        }
        assert_eq!(sky.elevation_deg, 90.0);
        assert_eq!(sky.strength, 0.0);
    }

    #[test]
    fn a_partial_file_fills_in_the_defaults() {
        let file: RetroFile = ron::from_str("(palette: \"a\", ink: 0.25)").unwrap_or_default();
        assert_eq!(file.ink, 0.25);
        assert_eq!(file.pixel_size, 3);
        assert!(file.enabled);
    }
}
