//! The looks the F12 panel switches between ([`LookMode`]) and the settings of the two
//! that shade the scene themselves: the flat toon shader ([`ToonLook`],
//! `bevy_wind_waker_shader::flat`) and PlayStation 1 materials ([`PsxLook`], `bevy_psx`).
//! Which materials a mode puts on the meshes is `shading.rs`; which picture it draws on the
//! window is the retro rig (`mod.rs`). Pure, so tested here.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::Palettes;
use super::look::{MAX_PIXEL_SIZE, RetroLook};

/// The whole look, one row in the panel and Ctrl+K.
#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LookMode {
    /// Bevy's own shading straight to the window.
    Plain,
    /// Our palette, dither, grade and ink over a low-res picture (`RetroLook`).
    #[default]
    Retro,
    /// The flat toon shader on every mesh, full resolution.
    Toon,
    /// The toon shader under the retro canvas.
    ToonRetro,
    /// PlayStation 1 materials (vertex snapping, colour steps) in a low-res picture.
    Psx,
}

/// What the meshes are drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Shading {
    #[default]
    Standard,
    Toon,
    Psx,
}

/// What the window shows of the game camera's picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Canvas {
    /// The camera draws to the window itself.
    Off,
    /// The retro canvas with every `RetroLook` setting.
    Retro,
    /// The retro canvas as a plain low-res upscale (`PsxLook::pixel_size`), nothing else.
    Psx,
}

impl LookMode {
    pub const ALL: [LookMode; 5] = [
        LookMode::Plain,
        LookMode::Retro,
        LookMode::Toon,
        LookMode::ToonRetro,
        LookMode::Psx,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LookMode::Plain => "Plain",
            LookMode::Retro => "Retro",
            LookMode::Toon => "Toon",
            LookMode::ToonRetro => "Toon + retro",
            LookMode::Psx => "PSX",
        }
    }

    /// `plain`, `retro`, `toon`, `toon-retro` or `psx` (`--look <name>`).
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| {
            m.name().to_lowercase().replace(" + ", "-") == name.to_lowercase()
        })
    }

    pub fn shading(self) -> Shading {
        match self {
            LookMode::Plain | LookMode::Retro => Shading::Standard,
            LookMode::Toon | LookMode::ToonRetro => Shading::Toon,
            LookMode::Psx => Shading::Psx,
        }
    }

    pub fn canvas(self) -> Canvas {
        match self {
            LookMode::Plain | LookMode::Toon => Canvas::Off,
            LookMode::Retro | LookMode::ToonRetro => Canvas::Retro,
            LookMode::Psx => Canvas::Psx,
        }
    }

    /// The next (`dir > 0`) or previous mode, wrapping.
    pub fn step(self, dir: i32) -> Self {
        let at = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        wrap(&Self::ALL, at, dir).unwrap_or(self)
    }

    /// The canvas settings this mode draws with, `None` for no canvas.
    pub fn canvas_look(self, retro: &RetroLook, psx: &PsxLook) -> Option<RetroLook> {
        match self.canvas() {
            Canvas::Off => None,
            Canvas::Retro => Some(retro.clone()),
            // Full colour, no dither, no grade, no ink: the PSX materials do the colour.
            Canvas::Psx => Some(RetroLook {
                pixel_size: psx.pixel_size,
                palette: 0,
                dither: 0.0,
                ..RetroLook::default()
            }),
        }
    }
}

/// Step `v` by `by` within `lo..=hi`, rounded onto the step so the labels and the file stay
/// tidy.
fn nudge(v: f32, by: f32, dir: i32, lo: f32, hi: f32) -> f32 {
    let next = if dir > 0 { v + by } else { v - by };
    ((next / by).round() * by).clamp(lo, hi)
}

/// The entry after (`dir > 0`) or before index `at` of `items`, wrapping.
fn wrap<T: Copy>(items: &[T], at: usize, dir: i32) -> Option<T> {
    let n = items.len().max(1);
    let next = if dir > 0 {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    };
    items.get(next).copied()
}

/// The next or previous entry of `steps` from the one nearest `v`, wrapping.
fn cycle<T: Copy + Into<f64>>(steps: &[T], current: T, dir: i32) -> T {
    let v: f64 = current.into();
    let at = steps
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let (a, b): (f64, f64) = ((**a).into(), (**b).into());
            (a - v).abs().total_cmp(&(b - v).abs())
        })
        .map_or(0, |(i, _)| i);
    wrap(steps, at, dir).unwrap_or(current)
}

fn off_or(v: f32, text: String) -> String {
    if v <= 0.0 { "off".to_string() } else { text }
}

// ---------------------------------------------------------------------------
// Toon
// ---------------------------------------------------------------------------

/// Posterize steps; 0 is off.
const LEVEL_STEPS: [f32; 9] = [0.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0];

/// The flat toon shader's settings: the model's own colours times a shadow or a light
/// tint, picked by the light level through the shader's band texture.
#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct ToonLook {
    /// Brightness of the shadow side, 0 black to 1 no shadow.
    pub shadow: f32,
    /// How far the shadow leans blue-violet.
    pub shadow_cool: f32,
    /// Brightness of the lit side, 1 as textured.
    pub light: f32,
    /// How far the lit side leans warm.
    pub light_warm: f32,
    /// Colour levels per channel (posterize), 0 off.
    pub levels: f32,
    /// The light level that counts as fully lit; below it the surface bands down to
    /// black, so unlit ground at night is dark. 0 off (never darker than the shadow).
    pub dark_level: f32,
    /// How much a coloured lamp tints what it hits, 0 brightness only, 1 its full hue.
    pub lamp_hue: f32,
}

impl Default for ToonLook {
    fn default() -> Self {
        // What `main.rs` had when the shader was last on.
        Self {
            shadow: 0.5,
            shadow_cool: 0.0,
            light: 1.0,
            light_warm: 0.0,
            levels: 16.0,
            dark_level: 0.25,
            lamp_hue: 0.8,
        }
    }
}

impl ToonLook {
    pub fn shadow_tint(&self) -> LinearRgba {
        let cool = Vec3::ONE.lerp(Vec3::new(0.75, 0.8, 1.25), self.shadow_cool);
        let c = cool * self.shadow;
        Color::srgb(c.x, c.y, c.z).into()
    }

    pub fn highlight_tint(&self) -> LinearRgba {
        let warm = Vec3::ONE.lerp(Vec3::new(1.15, 1.0, 0.8), self.light_warm);
        let c = warm * self.light;
        Color::srgb(c.x, c.y, c.z).into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToonSetting {
    Shadow,
    ShadowCool,
    Light,
    LightWarm,
    Levels,
    DarkLevel,
    LampHue,
}

impl ToonSetting {
    pub const ALL: [ToonSetting; 7] = [
        ToonSetting::Shadow,
        ToonSetting::ShadowCool,
        ToonSetting::Light,
        ToonSetting::LightWarm,
        ToonSetting::Levels,
        ToonSetting::DarkLevel,
        ToonSetting::LampHue,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ToonSetting::Shadow => "Shadow",
            ToonSetting::ShadowCool => "Cool",
            ToonSetting::Light => "Light",
            ToonSetting::LightWarm => "Warm",
            ToonSetting::Levels => "Levels",
            ToonSetting::DarkLevel => "Dark at",
            ToonSetting::LampHue => "Lamp hue",
        }
    }

    pub fn step(self, toon: &mut ToonLook, dir: i32) {
        match self {
            ToonSetting::Shadow => toon.shadow = nudge(toon.shadow, 0.05, dir, 0.0, 1.0),
            ToonSetting::ShadowCool => {
                toon.shadow_cool = nudge(toon.shadow_cool, 0.1, dir, 0.0, 1.0);
            }
            ToonSetting::Light => toon.light = nudge(toon.light, 0.05, dir, 0.5, 2.0),
            ToonSetting::LightWarm => toon.light_warm = nudge(toon.light_warm, 0.1, dir, 0.0, 1.0),
            ToonSetting::Levels => toon.levels = cycle(&LEVEL_STEPS, toon.levels, dir),
            ToonSetting::DarkLevel => toon.dark_level = nudge(toon.dark_level, 0.05, dir, 0.0, 2.0),
            ToonSetting::LampHue => toon.lamp_hue = nudge(toon.lamp_hue, 0.1, dir, 0.0, 1.0),
        }
    }

    pub fn value(self, toon: &ToonLook) -> String {
        match self {
            ToonSetting::Shadow => format!("{:.2}", toon.shadow),
            ToonSetting::ShadowCool => off_or(toon.shadow_cool, format!("{:.1}", toon.shadow_cool)),
            ToonSetting::Light => format!("{:.2}", toon.light),
            ToonSetting::LightWarm => off_or(toon.light_warm, format!("{:.1}", toon.light_warm)),
            ToonSetting::Levels => off_or(toon.levels, format!("{:.0}", toon.levels)),
            ToonSetting::DarkLevel => off_or(toon.dark_level, format!("{:.2}", toon.dark_level)),
            ToonSetting::LampHue => off_or(toon.lamp_hue, format!("{:.1}", toon.lamp_hue)),
        }
    }
}

// ---------------------------------------------------------------------------
// PSX
// ---------------------------------------------------------------------------

/// Vertex snap grids; 0 is off, fewer is coarser (more wobble).
const SNAP_STEPS: [f32; 10] = [
    0.0, 32.0, 48.0, 64.0, 96.0, 128.0, 192.0, 256.0, 384.0, 512.0,
];
/// Colour steps per channel; 0 is off.
const COLOUR_STEPS: [u32; 7] = [0, 4, 8, 16, 24, 32, 64];

/// `bevy_psx`'s ordered dither patterns, by name in the file.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PsxPattern {
    Bayer4,
    #[default]
    Bayer8,
    BlueNoise,
    Random,
}

impl PsxPattern {
    const ALL: [PsxPattern; 4] = [
        PsxPattern::Bayer4,
        PsxPattern::Bayer8,
        PsxPattern::BlueNoise,
        PsxPattern::Random,
    ];

    fn name(self) -> &'static str {
        match self {
            PsxPattern::Bayer4 => "Bayer 4x4",
            PsxPattern::Bayer8 => "Bayer 8x8",
            PsxPattern::BlueNoise => "blue noise",
            PsxPattern::Random => "random",
        }
    }
}

/// The PSX look: `bevy_psx`'s material on every mesh and a low-res picture.
#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PsxLook {
    /// Screen pixels per texel of the low-res picture.
    pub pixel_size: u32,
    /// Vertex snap grid, 0 off; lower wobbles more.
    pub snap: f32,
    /// Colour steps per channel, 0 off.
    pub colours: u32,
    /// Ordered dither between the steps (or palette colours), 0 off.
    pub dither: f32,
    pub pattern: PsxPattern,
    /// One of our palettes (`assets/palettes`) by name; `None` for the colour steps only.
    pub palette: Option<String>,
}

impl Default for PsxLook {
    fn default() -> Self {
        // What `--psx` had: wobble, 15-bit style steps, a light dither, no palette (its
        // own default palette turned a night level black and red).
        Self {
            pixel_size: 4,
            snap: 128.0,
            colours: 32,
            dither: 0.05,
            pattern: PsxPattern::Bayer8,
            palette: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsxSetting {
    Pixels,
    Snap,
    Colours,
    Palette,
    Dither,
    Pattern,
}

impl PsxSetting {
    pub const ALL: [PsxSetting; 6] = [
        PsxSetting::Pixels,
        PsxSetting::Snap,
        PsxSetting::Colours,
        PsxSetting::Palette,
        PsxSetting::Dither,
        PsxSetting::Pattern,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PsxSetting::Pixels => "Pixels",
            PsxSetting::Snap => "Wobble",
            PsxSetting::Colours => "Colours",
            PsxSetting::Palette => "Palette",
            PsxSetting::Dither => "Dither",
            PsxSetting::Pattern => "Pattern",
        }
    }

    pub fn step(self, psx: &mut PsxLook, palettes: &Palettes, dir: i32) {
        match self {
            PsxSetting::Pixels => {
                psx.pixel_size = if dir > 0 {
                    (psx.pixel_size + 1).min(MAX_PIXEL_SIZE)
                } else {
                    psx.pixel_size.saturating_sub(1).max(1)
                };
            }
            PsxSetting::Snap => psx.snap = cycle(&SNAP_STEPS, psx.snap, dir),
            PsxSetting::Colours => psx.colours = cycle(&COLOUR_STEPS, psx.colours, dir),
            PsxSetting::Palette => {
                let count = palettes.0.len() + 1;
                let at = psx
                    .palette
                    .as_deref()
                    .and_then(|name| palettes.index_of(name))
                    .unwrap_or(0);
                let next = if dir > 0 {
                    (at + 1) % count
                } else {
                    (at + count - 1) % count
                };
                psx.palette = palettes.get(next).map(|p| p.name.clone());
            }
            PsxSetting::Dither => psx.dither = nudge(psx.dither, 0.05, dir, 0.0, 1.0),
            PsxSetting::Pattern => {
                let at = PsxPattern::ALL
                    .iter()
                    .position(|p| *p == psx.pattern)
                    .unwrap_or(0);
                psx.pattern = wrap(&PsxPattern::ALL, at, dir).unwrap_or(psx.pattern);
            }
        }
    }

    pub fn value(self, psx: &PsxLook) -> String {
        match self {
            PsxSetting::Pixels => format!("{} px", psx.pixel_size),
            PsxSetting::Snap => off_or(psx.snap, format!("grid {:.0}", psx.snap)),
            PsxSetting::Colours => {
                if psx.colours == 0 {
                    "off".to_string()
                } else {
                    format!("{} steps", psx.colours)
                }
            }
            PsxSetting::Palette => psx.palette.clone().unwrap_or_else(|| "none".to_string()),
            PsxSetting::Dither => off_or(psx.dither, format!("{:.2}", psx.dither)),
            PsxSetting::Pattern => psx.pattern.name().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Outlines
// ---------------------------------------------------------------------------

/// The black outline round every model (`bevy_mod_outline`, `auto_outline_scenes`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct OutlineLook {
    /// In screen pixels; 0 is off.
    pub width: f32,
}

impl Default for OutlineLook {
    fn default() -> Self {
        Self { width: 2.0 }
    }
}

impl OutlineLook {
    pub fn step(&mut self, dir: i32) {
        self.width = nudge(self.width, 0.5, dir, 0.0, 8.0);
    }

    pub fn value(&self) -> String {
        off_or(self.width, format!("{:.1} px", self.width))
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
    fn modes_wrap_both_ways() {
        assert_eq!(LookMode::Plain.step(-1), LookMode::Psx);
        assert_eq!(LookMode::Psx.step(1), LookMode::Plain);
        let mut m = LookMode::Retro;
        for _ in 0..LookMode::ALL.len() {
            m = m.step(1);
        }
        assert_eq!(m, LookMode::Retro);
    }

    #[test]
    fn modes_go_by_their_flag_names() {
        let names = ["plain", "retro", "toon", "toon-retro", "psx"];
        for (name, mode) in names.into_iter().zip(LookMode::ALL) {
            assert_eq!(LookMode::from_name(name), Some(mode));
        }
        assert_eq!(LookMode::from_name("sepia"), None);
    }

    #[test]
    fn each_mode_has_its_shading_and_canvas() {
        let retro = RetroLook {
            palette: 2,
            pixel_size: 2,
            ..default()
        };
        let psx = PsxLook::default();
        assert_eq!(LookMode::Plain.canvas_look(&retro, &psx), None);
        assert_eq!(LookMode::Toon.canvas_look(&retro, &psx), None);
        assert_eq!(
            LookMode::ToonRetro.canvas_look(&retro, &psx),
            Some(retro.clone())
        );
        let canvas = LookMode::Psx.canvas_look(&retro, &psx).unwrap_or_default();
        assert_eq!(
            (canvas.palette, canvas.pixel_size, canvas.dither),
            (0, 4, 0.0)
        );
        assert_eq!(LookMode::ToonRetro.shading(), Shading::Toon);
        assert_eq!(LookMode::Psx.shading(), Shading::Psx);
        assert_eq!(LookMode::Retro.shading(), Shading::Standard);
    }

    #[test]
    fn toon_steps_stay_in_range() {
        let mut toon = ToonLook::default();
        for _ in 0..40 {
            ToonSetting::Shadow.step(&mut toon, 1);
            ToonSetting::DarkLevel.step(&mut toon, -1);
        }
        assert_eq!(toon.shadow, 1.0);
        assert_eq!(ToonSetting::DarkLevel.value(&toon), "off");
        ToonSetting::Levels.step(&mut toon, 1);
        assert_eq!(toon.levels, 24.0);
        toon.levels = 32.0;
        ToonSetting::Levels.step(&mut toon, 1);
        assert_eq!(ToonSetting::Levels.value(&toon), "off");
    }

    #[test]
    fn neutral_toon_tints_are_grey_and_white() {
        let toon = ToonLook::default();
        assert_eq!(toon.highlight_tint(), LinearRgba::WHITE);
        let s = toon.shadow_tint();
        assert!((s.red - s.blue).abs() < 1e-6 && s.red < 0.5);
        let cool = ToonLook {
            shadow_cool: 1.0,
            ..toon
        };
        assert!(cool.shadow_tint().blue > cool.shadow_tint().red);
    }

    #[test]
    fn psx_palette_cycles_through_none_by_name() {
        let p = palettes();
        let mut psx = PsxLook::default();
        PsxSetting::Palette.step(&mut psx, &p, 1);
        assert_eq!(psx.palette.as_deref(), Some("a"));
        PsxSetting::Palette.step(&mut psx, &p, -1);
        assert_eq!(psx.palette, None);
        PsxSetting::Palette.step(&mut psx, &p, -1);
        assert_eq!(PsxSetting::Palette.value(&psx), "b");
        PsxSetting::Snap.step(&mut psx, &p, 1);
        assert_eq!(psx.snap, 192.0);
        PsxSetting::Pattern.step(&mut psx, &p, -1);
        assert_eq!(psx.pattern, PsxPattern::Bayer4);
    }

    #[test]
    fn outline_width_stops_at_off() {
        let mut o = OutlineLook::default();
        for _ in 0..10 {
            o.step(-1);
        }
        assert_eq!(o.value(), "off");
    }
}
