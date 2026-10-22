//! Muzzle flashes. `shoot_weapons` only reports a shot ([`MuzzleFlash`]); what that
//! looks like is decided here, by [`MuzzleFlashSettings::style`]:
//!
//! - **Light**: a warm point light at the muzzle hardpoint that blinks on at full
//!   strength and dies away in a few hundredths of a second, lighting the shooter, the
//!   floor and the walls around them. Nothing is drawn; you see what it lights.
//! - **Sparks**: a particle burst out of the barrel along the aim
//!   (`assets/fx/muzzle_sparks.ron`, through `crate::fx`).
//! - **Both** (default): the light, with the sparks as its visible core.
//!
//! `Ctrl+M` cycles them in game for comparison.
//!
//! The light is parented to the weapon at the muzzle's local position, so it rides along
//! with a moving, recoiling gun for its short life instead of hanging where the shot was.

use bevy::prelude::*;

use crate::fx::{FxKind, SpawnFx};

/// A shot was fired: flash at the muzzle.
#[derive(Message, Clone, Copy, Debug)]
pub struct MuzzleFlash {
    /// The weapon entity and the muzzle in its local frame, when the gun has a muzzle
    /// hardpoint. The light is parented there.
    pub weapon: Option<(Entity, Vec3)>,
    /// The muzzle in world space, for the sparks and for guns without a hardpoint.
    pub at: Vec3,
    /// Which way the shot went, for the sparks.
    pub dir: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MuzzleFlashStyle {
    Light,
    Sparks,
    #[default]
    Both,
}

impl MuzzleFlashStyle {
    pub const fn next(self) -> Self {
        match self {
            Self::Light => Self::Sparks,
            Self::Sparks => Self::Both,
            Self::Both => Self::Light,
        }
    }

    pub const fn light(self) -> bool {
        matches!(self, Self::Light | Self::Both)
    }

    pub const fn sparks(self) -> bool {
        matches!(self, Self::Sparks | Self::Both)
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MuzzleFlashSettings {
    pub style: MuzzleFlashStyle,
    pub color: Color,
    /// Peak brightness, lumens. A street lamp is 80 000 (`map::map_lights`).
    pub intensity: f32,
    /// How far the flash reaches, metres (a tile is one).
    pub range: f32,
    /// Seconds from full strength to dark.
    pub duration: f32,
}

impl Default for MuzzleFlashSettings {
    fn default() -> Self {
        Self {
            style: MuzzleFlashStyle::default(),
            color: Color::srgb(1.0, 0.75, 0.4),
            intensity: 300_000.0,
            range: 5.0,
            duration: 0.06,
        }
    }
}

/// A muzzle flash's light, dying away.
#[derive(Component)]
pub struct MuzzleFlashLight {
    timer: Timer,
    peak: f32,
}

/// How much of the peak is left `t` (0..1) through the flash. Squared, so the light is
/// gone almost as soon as it came: a flash, not a fade.
pub fn flash_falloff(t: f32) -> f32 {
    let left = (1.0 - t).clamp(0.0, 1.0);
    left * left
}

pub struct MuzzleFlashPlugin;

impl Plugin for MuzzleFlashPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MuzzleFlash>()
            .init_resource::<MuzzleFlashSettings>()
            .add_systems(
                Update,
                (
                    cycle_muzzle_flash_style,
                    spawn_muzzle_flashes,
                    fade_muzzle_flash_lights,
                )
                    .run_if(in_state(crate::game_state::GameState::InGame)),
            );
    }
}

/// `Ctrl+M`: next style.
fn cycle_muzzle_flash_style(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<MuzzleFlashSettings>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::KeyM) {
        settings.style = settings.style.next();
        info!("muzzle flash: {:?}", settings.style);
    }
}

fn spawn_muzzle_flashes(
    mut flashes: MessageReader<MuzzleFlash>,
    settings: Res<MuzzleFlashSettings>,
    mut commands: Commands,
    mut fx: MessageWriter<SpawnFx>,
) {
    for flash in flashes.read() {
        if settings.style.light() {
            let light = (
                Name::from("Muzzle Flash"),
                MuzzleFlashLight {
                    timer: Timer::from_seconds(settings.duration, TimerMode::Once),
                    peak: settings.intensity,
                },
                PointLight {
                    color: settings.color,
                    intensity: settings.intensity,
                    range: settings.range,
                    // Dozens a second on full auto: shadows would cost more than they show.
                    shadow_maps_enabled: false,
                    ..default()
                },
            );
            match flash.weapon {
                Some((weapon, local)) => {
                    commands.spawn((light, Transform::from_translation(local), ChildOf(weapon)));
                }
                None => {
                    commands.spawn((light, Transform::from_translation(flash.at)));
                }
            }
        }
        if settings.style.sparks() {
            fx.write(SpawnFx::at(FxKind::MuzzleSparks, flash.at).along(flash.dir));
        }
    }
}

fn fade_muzzle_flash_lights(
    time: Res<Time>,
    mut commands: Commands,
    mut lights: Query<(Entity, &mut MuzzleFlashLight, &mut PointLight)>,
) {
    for (entity, mut flash, mut light) in &mut lights {
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            commands.entity(entity).despawn();
        } else {
            light.intensity = flash.peak * flash_falloff(flash.timer.fraction());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flash_starts_at_full_and_ends_dark() {
        assert!((flash_falloff(0.0) - 1.0).abs() < 1e-6);
        assert!(flash_falloff(1.0).abs() < 1e-6);
        assert!(flash_falloff(0.5) < 0.5, "drops faster than a linear fade");
    }

    #[test]
    fn styles_cycle_and_say_what_they_draw() {
        let s = MuzzleFlashStyle::default();
        assert_eq!(s, MuzzleFlashStyle::Both);
        assert!(s.light() && s.sparks());
        assert!(s.next().light() && !s.next().sparks());
        assert!(s.next().next().sparks() && !s.next().next().light());
        assert_eq!(s.next().next().next(), s);
    }

    #[test]
    fn a_flash_light_dies_and_is_removed() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(Update, fade_muzzle_flash_lights);
        let light = app
            .world_mut()
            .spawn((
                MuzzleFlashLight {
                    timer: Timer::from_seconds(0.06, TimerMode::Once),
                    peak: 100.0,
                },
                PointLight::default(),
            ))
            .id();
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }
        let dimmed = app
            .world()
            .entity(light)
            .get::<PointLight>()
            .unwrap()
            .intensity;
        assert!(dimmed < 100.0 && dimmed > 0.0, "fading: {dimmed}");
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(50));
        app.update();
        assert!(
            app.world().get_entity(light).is_err(),
            "gone once it has played out"
        );
    }
}
