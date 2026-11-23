//! Every row of the F12 panel ([`Row`]) over everything it edits ([`LookParams`]). The
//! panel's buttons and the Ctrl hotkeys both step a row and write [`LookChanged`], which
//! flashes the row's new value on screen.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::Palettes;
use super::look::{LightSetting, RetroLook, Setting};
use super::mode::{
    Canvas, LookMode, OutlineLook, PsxLook, PsxSetting, Shading, ToonLook, ToonSetting,
};
use crate::camera::fog::FogLevel;
use crate::general::systems::lights_systems::{LightingPreset, SkyLightSettings};
use crate::gore::despair::DespairSettings;
use crate::map::grass::GrassOn;

/// Settings of the scene that show in every look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneSetting {
    Outline,
    Fog,
    Dread,
    Grass,
}

impl SceneSetting {
    pub const ALL: [SceneSetting; 4] = [
        SceneSetting::Outline,
        SceneSetting::Fog,
        SceneSetting::Dread,
        SceneSetting::Grass,
    ];

    fn name(self) -> &'static str {
        match self {
            SceneSetting::Outline => "Outline",
            SceneSetting::Fog => "Fog",
            SceneSetting::Dread => "Dread",
            SceneSetting::Grass => "Grass",
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Mode,
    Retro(Setting),
    Toon(ToonSetting),
    Psx(PsxSetting),
    Scene(SceneSetting),
    Light(LightSetting),
}

/// A row stepped from the panel or a hotkey.
#[derive(Message, Clone, Copy, Debug)]
pub struct LookChanged(pub Row);

#[derive(SystemParam)]
pub struct LookParams<'w> {
    pub mode: ResMut<'w, LookMode>,
    pub retro: ResMut<'w, RetroLook>,
    pub toon: ResMut<'w, ToonLook>,
    pub psx: ResMut<'w, PsxLook>,
    pub outline: ResMut<'w, OutlineLook>,
    pub fog: Option<ResMut<'w, FogLevel>>,
    pub despair: Option<ResMut<'w, DespairSettings>>,
    pub grass: Option<ResMut<'w, GrassOn>>,
    pub preset: ResMut<'w, LightingPreset>,
    pub sky: ResMut<'w, SkyLightSettings>,
    pub palettes: Res<'w, Palettes>,
}

impl Row {
    pub fn name(self) -> &'static str {
        match self {
            Row::Mode => "Look",
            Row::Retro(s) => s.name(),
            Row::Toon(s) => s.name(),
            Row::Psx(s) => s.name(),
            Row::Scene(s) => s.name(),
            Row::Light(s) => s.name(),
        }
    }

    /// Whether the row applies to `mode` (the panel hides the others).
    pub fn shown_in(self, mode: LookMode) -> bool {
        match self {
            Row::Retro(_) => mode.canvas() == Canvas::Retro,
            Row::Toon(_) => mode.shading() == Shading::Toon,
            Row::Psx(_) => mode == LookMode::Psx,
            Row::Mode | Row::Scene(_) | Row::Light(_) => true,
        }
    }

    pub fn step(self, p: &mut LookParams, dir: i32) {
        match self {
            Row::Mode => *p.mode = p.mode.step(dir),
            Row::Retro(s) => s.step(&mut p.retro, &p.palettes, dir),
            Row::Toon(s) => s.step(&mut p.toon, dir),
            Row::Psx(s) => s.step(&mut p.psx, &p.palettes, dir),
            Row::Light(s) => s.step(&mut p.preset, &mut p.sky, dir),
            Row::Scene(SceneSetting::Outline) => p.outline.step(dir),
            Row::Scene(SceneSetting::Fog) => {
                if let Some(fog) = p.fog.as_mut() {
                    **fog = if dir > 0 {
                        fog.next()
                    } else {
                        fog.next().next()
                    };
                }
            }
            Row::Scene(SceneSetting::Dread) => {
                if let Some(d) = p.despair.as_mut() {
                    let by = if dir > 0 { 0.1 } else { -0.1 };
                    d.max_strength = ((d.max_strength + by) * 10.0).round().clamp(0.0, 10.0) / 10.0;
                }
            }
            Row::Scene(SceneSetting::Grass) => {
                if let Some(grass) = p.grass.as_mut() {
                    grass.0 = !grass.0;
                }
            }
        }
    }

    pub fn value(self, p: &LookParams) -> String {
        match self {
            Row::Mode => p.mode.name().to_string(),
            Row::Retro(s) => s.value(&p.retro, &p.palettes),
            Row::Toon(s) => s.value(&p.toon),
            Row::Psx(s) => s.value(&p.psx),
            Row::Light(s) => s.value(*p.preset, &p.sky),
            Row::Scene(SceneSetting::Outline) => p.outline.value(),
            Row::Scene(SceneSetting::Fog) => {
                p.fog.as_deref().map_or("n/a", |f| f.label()).to_string()
            }
            Row::Scene(SceneSetting::Dread) => {
                p.despair.as_deref().map_or("n/a".to_string(), |d| {
                    if d.max_strength <= 0.0 {
                        "off".to_string()
                    } else {
                        format!("{:.1}", d.max_strength)
                    }
                })
            }
            Row::Scene(SceneSetting::Grass) => p
                .grass
                .as_deref()
                .map_or("n/a", |g| if g.0 { "on" } else { "off" })
                .to_string(),
        }
    }
}
