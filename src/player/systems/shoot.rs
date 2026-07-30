//! Firing equipped guns. Hitscan: on the fire input, cast a ray from the weapon's
//! `muzzle` hardpoint along the player's auto-aim, apply damage to whatever it hits,
//! and emit [`DamageDealt`] so the gore layer sprays blood. Muzzle flash + tracer are
//! cheap [`Ephemeral`] visuals.
//!
//! Guns replace throwing while equipped — `throwing` is gated `Without<EquippedWeapon>`
//! and this system requires it, so the same fire button does one or the other.

use avian3d::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::alien::components::general::{Alien, AlienCounter};
use crate::assets::asset_definition::{Hardpoint, WeaponProps};
use crate::control::components::{CharacterControl, ControlCommand};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::{CollisionLayer, Health};
use crate::gore::components::{DamageDealt, DamageKind, Ephemeral};
use crate::player::components::{AutoAim, Player, PlayerDead};
use crate::player::systems::equip::EquippedWeapon;

/// Runtime combat state of an equipped weapon, resolved from its `WeaponProps` at
/// equip time and carried on the weapon entity.
#[derive(Component)]
pub struct Weapon {
    /// Where bullets leave from (weapon-local frame). `None` falls back to the player.
    pub muzzle: Option<Hardpoint>,
    pub damage: i32,
    /// Seconds between shots (derived from fire rate).
    pub shot_interval: f32,
    /// Counts down to the next allowed shot.
    pub cooldown: f32,
    pub range: f32,
    pub spread_deg: f32,
    pub pellets: u32,
    pub auto: bool,
    /// Current recoil kick in radians; applied and decayed by `keep_weapons_snapped`.
    pub recoil: f32,
    /// Whether the fire input was held last frame (for semi-auto edge detection).
    pub was_firing: bool,
}

impl Weapon {
    pub fn from_props(props: &WeaponProps, muzzle: Option<Hardpoint>) -> Self {
        let shot_interval = if props.fire_rate_per_minute > 0.0 {
            60.0 / props.fire_rate_per_minute
        } else {
            0.5
        };
        Self {
            muzzle,
            damage: props.damage,
            shot_interval,
            cooldown: 0.0,
            range: props.range,
            spread_deg: props.spread_deg,
            pellets: props.pellets.max(1),
            auto: props.auto,
            recoil: 0.0,
            was_firing: false,
        }
    }
}

/// Tiny xorshift RNG for shot spread — deterministic per call, no dep.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// In [-1, 1].
    fn signed(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

#[allow(clippy::too_many_arguments)]
pub fn shoot_weapons(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(Entity, &AutoAim, &CharacterControl, &EquippedWeapon, &Position), (With<Player>, Without<PlayerDead>)>,
    mut weapons: Query<(&mut Weapon, &GlobalTransform)>,
    mut targets: Query<(&mut Health, Has<Alien>)>,
    mut alien_counter: ResMut<AlienCounter>,
    mut game_mw: MessageWriter<GameTrackingEvent>,
    mut damage_mw: MessageWriter<DamageDealt>,
    mut rng_seed: Local<u32>,
) {
    let dt = time.delta_secs();

    // Only aliens and world geometry stop bullets — never the shooter.
    let filter = SpatialQueryFilter::from_mask([
        CollisionLayer::Alien,
        CollisionLayer::ImpassableAll,
    ]);

    for (player, aim, control, equipped, player_pos) in players.iter() {
        let Ok((mut weapon, weapon_gt)) = weapons.get_mut(equipped.0) else { continue };

        if weapon.cooldown > 0.0 {
            weapon.cooldown -= dt;
        }

        let firing = control.triggers.contains(&ControlCommand::Throw);
        // Semi-auto needs a fresh press; full-auto fires while held.
        let trigger = firing && (weapon.auto || !weapon.was_firing);
        weapon.was_firing = firing;

        if !trigger || weapon.cooldown > 0.0 {
            continue;
        }
        weapon.cooldown = weapon.shot_interval;
        weapon.recoil = 0.28;

        // Muzzle world position (fall back to just in front of the player).
        let origin = match &weapon.muzzle {
            Some(m) => weapon_gt.transform_point(Vec3::from(m.translation)),
            None => player_pos.0 + Vec3::Y * 0.35 + aim.0 * 0.3,
        };
        let aim_dir = aim.0.normalize_or(Vec3::X);

        game_mw.write(GameTrackingEvent::ShotFired(player));

        *rng_seed = rng_seed.wrapping_add(0x9E3779B9);
        let mut rng = Rng(*rng_seed ^ player.to_bits() as u32);

        for _ in 0..weapon.pellets {
            // Perturb the aim inside the spread cone.
            let spread = weapon.spread_deg.to_radians();
            let jitter = Vec3::new(rng.signed(), rng.signed() * 0.4, rng.signed()) * spread;
            let dir = (aim_dir + jitter).normalize_or(aim_dir);
            let Ok(dir3) = Dir3::new(dir) else { continue };

            let end = if let Some(hit) = spatial.cast_ray(origin, dir3, weapon.range, true, &filter) {
                let point = origin + dir * hit.distance;
                if let Ok((mut health, is_alien)) = targets.get_mut(hit.entity) {
                    health.health -= weapon.damage;
                    let lethal = health.health <= 0;
                    damage_mw.write(DamageDealt {
                        target: hit.entity,
                        position: point,
                        normal: -dir,
                        amount: weapon.damage,
                        kind: DamageKind::Ballistic,
                        lethal,
                    });
                    game_mw.write(GameTrackingEvent::ShotHit(hit.entity));
                    if lethal && is_alien {
                        game_mw.write(GameTrackingEvent::AlienKilled(hit.entity));
                        alien_counter.count -= 1;
                    }
                }
                point
            } else {
                origin + dir * weapon.range
            };

            spawn_tracer(&mut commands, &mut meshes, &mut materials, origin, end);
        }

        spawn_muzzle_flash(&mut commands, &mut meshes, &mut materials, origin);
    }
}

/// A brief thin bright streak from muzzle to impact.
fn spawn_tracer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    from: Vec3,
    to: Vec3,
) {
    let delta = to - from;
    let len = delta.length();
    if len < 0.01 {
        return;
    }
    let mid = from + delta * 0.5;
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.5),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(mat),
        Transform::from_translation(mid)
            .looking_to(delta / len, Vec3::Y)
            .with_scale(Vec3::new(0.02, 0.02, len)),
        Ephemeral::new(0.05),
    ));
}

/// A tiny bright puff at the muzzle.
fn spawn_muzzle_flash(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
) {
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.06))),
        MeshMaterial3d(mat),
        Transform::from_translation(at),
        Ephemeral::new(0.06).with_grow(1.8).base_scale(Vec3::ONE),
    ));
}

#[cfg(test)]
mod tests {
    use super::Weapon;
    use crate::assets::asset_definition::WeaponProps;

    #[test]
    fn shot_interval_is_derived_from_fire_rate() {
        let mut props = WeaponProps::default();
        props.fire_rate_per_minute = 300.0; // 5 shots/sec
        let w = Weapon::from_props(&props, None);
        assert!((w.shot_interval - 0.2).abs() < 1e-6, "300 rpm -> 0.2s between shots");
    }

    #[test]
    fn zero_fire_rate_falls_back_to_a_sane_interval() {
        let mut props = WeaponProps::default();
        props.fire_rate_per_minute = 0.0;
        let w = Weapon::from_props(&props, None);
        assert!(w.shot_interval > 0.0, "must not divide by zero into an infinite fire rate");
    }

    #[test]
    fn pellets_are_clamped_to_at_least_one() {
        let mut props = WeaponProps::default();
        props.pellets = 0;
        let w = Weapon::from_props(&props, None);
        assert_eq!(w.pellets, 1, "a weapon with 0 pellets would never hit anything");
    }
}
