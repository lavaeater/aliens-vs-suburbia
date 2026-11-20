//! Burning: a creature on fire. Fire damage every quarter second (through
//! `ApplyDamage`, credited to whoever lit it), flames and a flickering light on the
//! body, and now and then the fire jumps to a creature it touches.
//!
//! The flames are their own entity ([`BurningFx`]) following the body rather than a
//! child of it, so when the creature dies or the fire goes out they stop emitting and
//! the last flames die out in place instead of popping.

use bevy::prelude::*;
use bevy_sprinkles::prelude::Particles3d;

use super::Afflict;
use crate::alien::components::general::Alien;
use crate::fx::{FxAssets, StopFx};
use crate::general::components::Health;
use crate::general::damage::ApplyDamage;
use crate::general::systems::lights_systems::{LightAnimation, LightStyle};
use crate::gore::components::DamageKind;
use crate::infestation::neighbours::Neighbour;
use crate::physics::prelude::Position;
use crate::player::components::Player;

/// How often a burning creature takes its damage.
const TICK: f32 = 0.25;
/// Closer than this (metres, centre to centre) and the fire may jump across.
const SPREAD_REACH: f32 = 1.0;
/// Chance per tick that the fire jumps to a creature in reach.
const SPREAD_CHANCE: f32 = 0.2;
/// A fire that jumps keeps this much of its heat and of what it has left to burn, so a
/// crowd catches in a dwindling chain rather than for ever.
const SPREAD_KEEPS: f32 = 0.6;
/// Below this many seconds left the fire no longer jumps.
const SPREAD_MIN_SECS: f32 = 1.2;
/// At most this many burning bodies carry a light (each is a light the renderer pays for).
const MAX_LIGHTS: usize = 12;
const LIGHT_LUMENS: f32 = 9_000.0;
/// The flames' last particles live this long (`fire.ron`'s embers).
const FLAME_LINGER: f32 = 1.2;
/// The body flames are `fire.ron` at this size.
const FLAME_SCALE: f32 = 0.4;

/// On fire.
#[derive(Component, Debug)]
pub struct Burning {
    pub dps: f32,
    left: Timer,
    tick: Timer,
    /// Damage owed but not yet dealt, so a low dps does not round to nothing.
    owed: f32,
    pub source: Option<Entity>,
}

impl Burning {
    pub fn new(dps: f32, secs: f32, source: Option<Entity>) -> Self {
        Self {
            dps,
            left: Timer::from_seconds(secs, TimerMode::Once),
            tick: Timer::from_seconds(TICK, TimerMode::Repeating),
            owed: 0.0,
            source,
        }
    }

    /// Set alight again: the hotter fire and the longer burn win.
    pub fn refresh(&mut self, dps: f32, secs: f32, source: Option<Entity>) {
        self.dps = self.dps.max(dps);
        if secs > self.secs_left() {
            self.left = Timer::from_seconds(secs, TimerMode::Once);
        }
        if source.is_some() {
            self.source = source;
        }
    }

    pub fn secs_left(&self) -> f32 {
        self.left.remaining_secs()
    }

    /// Advance by `dt`; the whole damage points due now.
    fn advance(&mut self, dt: std::time::Duration) -> i32 {
        self.left.tick(dt);
        self.tick.tick(dt);
        let ticks = self.tick.times_finished_this_tick();
        if ticks == 0 {
            return 0;
        }
        self.owed += self.dps * TICK * ticks as f32;
        let whole = self.owed.floor();
        self.owed -= whole;
        whole as i32
    }
}

/// Burn: damage on each tick, and put the fire out when it has run its time.
pub fn tick_burning(
    time: Res<Time>,
    mut commands: Commands,
    mut burning: Query<(Entity, &mut Burning, &Position, &Health)>,
    mut damage_mw: MessageWriter<ApplyDamage>,
) {
    for (entity, mut burning, position, health) in &mut burning {
        let amount = burning.advance(time.delta());
        if amount > 0 && health.health > 0 {
            let mut hit = ApplyDamage::at(entity, amount, DamageKind::Fire, position.0 + Vec3::Y * 0.5);
            hit.source = burning.source;
            damage_mw.write(hit);
        }
        if burning.left.is_finished() {
            commands.entity(entity).remove::<Burning>();
        }
    }
}

/// Cheap deterministic noise in `[0, 1)` from two entities and a counter.
fn roll(a: Entity, b: Entity, n: u32) -> f32 {
    let mut x = (a.to_bits() as u32)
        .wrapping_mul(0x9E37_79B9)
        ^ (b.to_bits() as u32).wrapping_mul(0x85EB_CA6B)
        ^ n.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Burning creatures set the ones they brush against alight, now and then.
#[allow(clippy::type_complexity)]
pub fn spread_burning(
    burning: Query<(Entity, &Burning, &Position)>,
    others: Query<(Entity, &Position), (Or<(With<Alien>, With<Neighbour>)>, Without<Burning>)>,
    mut afflict_mw: MessageWriter<Afflict>,
    mut count: Local<u32>,
) {
    for (from, fire, at) in &burning {
        if !fire.tick.just_finished() || fire.secs_left() < SPREAD_MIN_SECS {
            continue;
        }
        *count = count.wrapping_add(1);
        for (to, pos) in &others {
            if pos.0.xz().distance_squared(at.0.xz()) > SPREAD_REACH * SPREAD_REACH {
                continue;
            }
            if roll(from, to, *count) < SPREAD_CHANCE {
                afflict_mw.write(
                    Afflict::burning(
                        to,
                        fire.dps * SPREAD_KEEPS,
                        fire.secs_left() * SPREAD_KEEPS,
                    )
                    .from(fire.source),
                );
            }
        }
    }
}

/// The flames (and maybe a light) following a burning body.
#[derive(Component)]
pub struct BurningFx {
    owner: Entity,
}

/// Light up whoever just caught fire.
pub fn show_burning(
    mut commands: Commands,
    caught: Query<(Entity, &Position), Added<Burning>>,
    lit: Query<(), (With<BurningFx>, With<PointLight>)>,
    fx: Option<Res<FxAssets>>,
) {
    let Some(fx) = fx else { return };
    let mut lights = lit.iter().count();
    for (owner, position) in &caught {
        let mut flames = commands.spawn((
            Name::new("Burning"),
            BurningFx { owner },
            Particles3d(fx.fire.clone()),
            Transform::from_translation(position.0).with_scale(Vec3::splat(FLAME_SCALE)),
        ));
        if lights < MAX_LIGHTS {
            lights += 1;
            flames.insert((
                PointLight {
                    color: Color::srgb(1.0, 0.55, 0.2),
                    intensity: LIGHT_LUMENS,
                    range: 3.5,
                    shadow_maps_enabled: false,
                    ..default()
                },
                LightAnimation {
                    base: LIGHT_LUMENS,
                    style: LightStyle::Flicker {
                        seed: (owner.index_u32() % 97) as f32 * 0.37,
                    },
                },
            ));
        }
    }
}

/// Keep the flames on the body; when the fire is out (or the body gone), let them die.
pub fn follow_burning(
    mut commands: Commands,
    mut flames: Query<(Entity, &BurningFx, &mut Transform), Without<StopFx>>,
    bodies: Query<(&Position, Has<Burning>, Has<Player>)>,
) {
    for (entity, fx, mut transform) in &mut flames {
        match bodies.get(fx.owner) {
            Ok((position, true, player)) => {
                // A player's flames sit a little lower: the camera looks down on them.
                let lift = if player { 0.3 } else { 0.4 };
                transform.translation = position.0 + Vec3::Y * lift;
            }
            _ => {
                commands
                    .entity(entity)
                    .remove::<(PointLight, LightAnimation, BurningFx)>()
                    .insert(StopFx::after(FLAME_LINGER));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_low_heat_still_adds_up() {
        // 2 dps is half a point per tick: nothing, then one, then nothing, then one.
        let mut b = Burning::new(2.0, 10.0, None);
        let dealt: i32 = (0..8)
            .map(|_| b.advance(Duration::from_millis(250)))
            .sum();
        assert_eq!(dealt, 4, "two seconds at 2 dps");
    }

    #[test]
    fn a_refresh_keeps_the_hotter_and_the_longer() {
        let mut b = Burning::new(10.0, 2.0, None);
        b.refresh(5.0, 6.0, None);
        assert!((b.dps - 10.0).abs() < f32::EPSILON);
        assert!((b.secs_left() - 6.0).abs() < 1e-4);
        b.refresh(20.0, 1.0, None);
        assert!((b.dps - 20.0).abs() < f32::EPSILON);
        assert!((b.secs_left() - 6.0).abs() < 1e-4);
    }

    #[test]
    fn the_spread_roll_is_spread_out() {
        let a = Entity::from_bits(5);
        let hits = (0..1000)
            .filter(|&n| roll(a, Entity::from_bits(9), n) < SPREAD_CHANCE)
            .count();
        assert!((120..280).contains(&hits), "{hits} of 1000 at 20%");
    }
}
