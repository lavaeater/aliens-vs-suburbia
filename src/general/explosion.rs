//! Things that go boom.
//!
//! An [`Explode`] message names a point, a radius and a peak. [`explosion_system`] finds
//! every `Health` inside the radius, checks a wall is not in the way, and writes an
//! `ApplyDamage` scaled by [`falloff`] plus a shove along the blast direction. The visual
//! is a particle burst (`assets/fx/explosion.ron`: fireball, sparks, debris, smoke);
//! `Fire` explosions also leave a `FireField`. Writers: grenades and projectile weapons (`projectiles`), Bombardment,
//! and anything carrying [`ExplodesOnDeath`].

use crate::physics::prelude::{LinearVelocity, Position, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::camera::components::CameraShake;
use crate::fx::{FxKind, SpawnFx};
use crate::general::components::{CollisionLayer, Health};
use crate::general::damage::ApplyDamage;
use crate::gore::components::DamageKind;
use crate::gore::fire::SpawnFire;
use crate::gore::sfx::{PlaySfx, SfxKind};

/// Authoring-side description of a blast, embedded in weapon/terrain defs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplosionProps {
    pub radius: f32,
    /// Damage at the centre; falls off quadratically to zero at `radius`.
    pub damage: i32,
    /// Peak velocity change (world units/s) applied at the centre.
    #[serde(default = "default_impulse")]
    pub impulse: f32,
    /// Leave burning ground behind.
    #[serde(default)]
    pub fire: bool,
}

const fn default_impulse() -> f32 {
    8.0
}

impl Default for ExplosionProps {
    fn default() -> Self {
        Self {
            radius: 2.5,
            damage: 80,
            impulse: default_impulse(),
            fire: false,
        }
    }
}

#[derive(Message, Debug, Clone)]
pub struct Explode {
    pub position: Vec3,
    pub props: ExplosionProps,
    /// Who set it off; credited for kills and checked against the damage rules.
    pub source: Option<Entity>,
}

/// An entity that detonates when its health hits zero (barrels, gas tanks).
#[derive(Component, Debug, Clone)]
pub struct ExplodesOnDeath(pub ExplosionProps);

/// Marker so a dying barrel only blows once.
#[derive(Component)]
pub struct Exploded;

/// 1 at the centre, 0 at the edge, quadratic in between (so the outer half of the
/// radius does much less than the inner half).
pub fn falloff(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let t = (distance / radius).clamp(0.0, 1.0);
    1.0 - t * t
}

/// Everything one blast does to one target, given a way to ask "is there a wall
/// between these two points". Pure so the maths is testable without physics.
pub fn blast_effect(
    center: Vec3,
    target: Vec3,
    props: &ExplosionProps,
    blocked: impl Fn(Vec3, Vec3) -> bool,
) -> Option<(i32, Vec3)> {
    let delta = target - center;
    let distance = delta.length();
    if distance > props.radius {
        return None;
    }
    if blocked(center, target) {
        return None;
    }
    let f = falloff(distance, props.radius);
    let damage = (props.damage as f32 * f).round() as i32;
    // Straight up when the target sits on the centre, otherwise away and a little up so
    // things visibly jump rather than slide.
    let dir = (delta.normalize_or(Vec3::Y) + Vec3::Y * 0.5).normalize_or(Vec3::Y);
    Some((damage, dir * props.impulse * f))
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn explosion_system(
    mut explosions: MessageReader<Explode>,
    spatial: SpatialQuery,
    mut targets: Query<(Entity, &Position, Option<&mut LinearVelocity>), With<Health>>,
    mut damage_mw: MessageWriter<ApplyDamage>,
    mut fire_mw: MessageWriter<SpawnFire>,
    mut sfx_mw: MessageWriter<PlaySfx>,
    mut shake: ResMut<CameraShake>,
    mut fx_mw: MessageWriter<SpawnFx>,
) {
    let walls = SpatialQueryFilter::from_mask([CollisionLayer::ImpassableAll]);
    for explosion in explosions.read() {
        let center = explosion.position;
        let props = &explosion.props;

        let blocked = |from: Vec3, to: Vec3| {
            let delta = to - from;
            let Ok(dir) = Dir3::new(delta) else {
                return false;
            };
            // Anything solid short of the target shields it.
            spatial
                .cast_ray(from, dir, (delta.length() - 0.2).max(0.0), true, &walls)
                .is_some()
        };

        for (entity, pos, velocity) in targets.iter_mut() {
            let target = pos.0 + Vec3::Y * 0.4;
            let Some((damage, shove)) = blast_effect(center, target, props, blocked) else {
                continue;
            };
            let kind = if props.fire {
                DamageKind::Fire
            } else {
                DamageKind::Explosive
            };
            let mut req = ApplyDamage::at(entity, damage, kind, target).along(target - center);
            if let Some(source) = explosion.source {
                req = req.from(source);
            }
            damage_mw.write(req);
            if let Some(mut v) = velocity {
                v.0 += shove;
            }
        }

        if props.fire {
            fire_mw.write(SpawnFire {
                position: center,
                radius: props.radius * 0.8,
                duration: 5.0,
                dps: 40.0,
            });
        }
        sfx_mw.write(PlaySfx {
            kind: SfxKind::Fire,
            gain_db: 4.0,
        });
        shake.add(0.35 * (props.radius / 2.5).clamp(0.3, 2.0));
        fx_mw.write(SpawnFx::at(FxKind::Explosion, center));
    }
}

/// Barrels and the like: blow up the frame their health hits zero. Runs before the
/// systems that despawn dead things.
pub fn explode_on_death(
    mut commands: Commands,
    dying: Query<(Entity, &Health, &ExplodesOnDeath, &Position), Without<Exploded>>,
    mut explode_mw: MessageWriter<Explode>,
) {
    for (entity, health, props, pos) in dying.iter() {
        if !health.is_dead() {
            continue;
        }
        commands.entity(entity).try_insert(Exploded);
        explode_mw.write(Explode {
            position: pos.0,
            props: props.0.clone(),
            source: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props() -> ExplosionProps {
        ExplosionProps {
            radius: 4.0,
            damage: 100,
            impulse: 10.0,
            fire: false,
        }
    }

    #[test]
    fn falloff_is_full_at_centre_and_zero_at_the_edge() {
        assert_eq!(falloff(0.0, 4.0), 1.0);
        assert_eq!(falloff(4.0, 4.0), 0.0);
        assert!(
            (falloff(2.0, 4.0) - 0.75).abs() < 1e-6,
            "quadratic: half way keeps 75%"
        );
        assert_eq!(falloff(1.0, 0.0), 0.0);
    }

    #[test]
    fn outside_the_radius_nothing_happens() {
        assert!(
            blast_effect(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), &props(), |_, _| false).is_none()
        );
    }

    #[test]
    fn damage_and_shove_scale_with_distance() {
        let (near_dmg, near_shove) =
            blast_effect(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), &props(), |_, _| false).unwrap();
        let (far_dmg, far_shove) =
            blast_effect(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0), &props(), |_, _| false).unwrap();
        assert!(near_dmg > far_dmg, "{near_dmg} vs {far_dmg}");
        assert!(near_shove.length() > far_shove.length());
        assert!(near_shove.x > 0.0, "pushed away from the centre");
        assert!(near_shove.y > 0.0, "and a little up");
    }

    #[test]
    fn walls_shield_targets() {
        assert!(
            blast_effect(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), &props(), |_, _| true).is_none()
        );
    }
}
