//! Firing equipped guns. Hitscan: on the fire input, cast a ray from the weapon's
//! `muzzle` hardpoint along the player's auto-aim and request [`ApplyDamage`] on whatever
//! it hits — the damage pipeline decides whether it lands and tells the gore layer.
//! Muzzle flash + tracer are cheap [`Ephemeral`] visuals.
//!
//! Guns replace throwing while equipped — `throwing` is gated `Without<EquippedWeapon>`
//! and this system requires it, so the same fire button does one or the other.

use avian3d::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::assets::asset_definition::{AmmoKind, Hardpoint, WeaponProps};
use crate::player::ammo::AmmoPouch;
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::components::{CharacterControl, ControlCommand};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::CollisionLayer;
use crate::general::damage::ApplyDamage;
use crate::general::projectiles::{self, ProjectileProps};
use crate::gore::components::{DamageKind, Ephemeral};
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
    /// Pool this gun draws from. `Infinite` skips all magazine bookkeeping.
    pub ammo: AmmoKind,
    pub magazine: u32,
    pub rounds_in_mag: u32,
    pub reload_secs: f32,
    /// Counting down while a reload is in progress.
    pub reloading: Option<Timer>,
    /// Set for launcher-type guns; `None` = hitscan.
    pub projectile: Option<ProjectileProps>,
}

impl Weapon {
    pub fn from_props(props: &WeaponProps, muzzle: Option<Hardpoint>) -> Self {
        let shot_interval = if props.fire_rate_per_minute > 0.0 {
            60.0 / props.fire_rate_per_minute
        } else {
            0.5
        };
        let magazine = if props.ammo == AmmoKind::Infinite { 0 } else { props.magazine.max(1) };
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
            ammo: props.ammo,
            magazine,
            rounds_in_mag: magazine,
            reload_secs: props.reload_secs.max(0.05),
            reloading: None,
            projectile: props.projectile.clone(),
        }
    }

    pub fn uses_ammo(&self) -> bool {
        self.ammo != AmmoKind::Infinite
    }

    /// Whether the trigger can be pulled right now (loaded and not mid-reload).
    pub fn can_fire(&self) -> bool {
        self.reloading.is_none() && (!self.uses_ammo() || self.rounds_in_mag > 0)
    }

    /// Begin a reload if there is something to reload with. Returns whether one started.
    pub fn start_reload(&mut self, pouch_rounds: u32) -> bool {
        if !self.uses_ammo() || self.reloading.is_some() || self.rounds_in_mag >= self.magazine || pouch_rounds == 0 {
            return false;
        }
        self.reloading = Some(Timer::from_seconds(self.reload_secs, TimerMode::Once));
        true
    }

    /// Rounds needed to top the magazine up.
    pub fn missing(&self) -> u32 {
        self.magazine.saturating_sub(self.rounds_in_mag)
    }
}

/// Asks a player's held weapon to reload. Written by the R key / gamepad binding and by
/// `shoot_weapons` on a dry trigger pull.
#[derive(Message, Clone, Copy, Debug)]
pub struct ReloadRequest(pub Entity);

/// Starts reloads on request and finishes the ones in progress, moving rounds from the
/// player's [`AmmoPouch`] into the magazine.
#[allow(clippy::type_complexity)]
pub fn tick_reloads(
    time: Res<Time>,
    mut requests: MessageReader<ReloadRequest>,
    mut players: Query<(Entity, &EquippedWeapon, &mut AmmoPouch), (With<Player>, Without<PlayerDead>)>,
    mut weapons: Query<&mut Weapon>,
    mut anim_ew: MessageWriter<AnimationEvent>,
) {
    for ReloadRequest(player) in requests.read() {
        let Ok((_, equipped, pouch)) = players.get_mut(*player) else { continue };
        let Ok(mut weapon) = weapons.get_mut(equipped.0) else { continue };
        let available = pouch.rounds(weapon.ammo);
        if weapon.start_reload(available) {
            anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, *player, AnimationKey::Reload));
        }
    }

    for (player, equipped, mut pouch) in players.iter_mut() {
        let Ok(mut weapon) = weapons.get_mut(equipped.0) else { continue };
        let Some(timer) = weapon.reloading.as_mut() else { continue };
        timer.tick(time.delta());
        if !timer.is_finished() {
            continue;
        }
        weapon.reloading = None;
        let wanted = weapon.missing();
        let got = pouch.take(weapon.ammo, wanted);
        weapon.rounds_in_mag += got;
        anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, player, AnimationKey::Reload));
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn shoot_weapons(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(Entity, &AutoAim, &CharacterControl, &EquippedWeapon, &Position), (With<Player>, Without<PlayerDead>)>,
    mut weapons: Query<(&mut Weapon, &GlobalTransform)>,
    mut game_mw: MessageWriter<GameTrackingEvent>,
    mut damage_mw: MessageWriter<ApplyDamage>,
    mut reload_mw: MessageWriter<ReloadRequest>,
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
        if !weapon.can_fire() {
            // Dry trigger pull on an empty magazine starts the reload for you.
            if weapon.reloading.is_none() {
                reload_mw.write(ReloadRequest(player));
            }
            continue;
        }
        weapon.cooldown = weapon.shot_interval;
        weapon.recoil = 0.28;
        if weapon.uses_ammo() {
            weapon.rounds_in_mag -= 1;
        }

        // Muzzle world position (fall back to just in front of the player).
        let origin = match &weapon.muzzle {
            Some(m) => weapon_gt.transform_point(Vec3::from(m.translation)),
            None => player_pos.0 + Vec3::Y * 0.35 + aim.0 * 0.3,
        };
        let aim_dir = aim.0.normalize_or(Vec3::X);

        game_mw.write(GameTrackingEvent::ShotFired(player));

        // Launcher-type guns fire a physics projectile instead of a ray.
        if let Some(props) = &weapon.projectile {
            projectiles::launch(&mut commands, &mut meshes, &mut materials, player, origin, aim_dir * props.speed, props);
            spawn_muzzle_flash(&mut commands, &mut meshes, &mut materials, origin);
            continue;
        }

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
                // Whatever it is — alien, wall, tower — `apply_damage` decides if it hurts.
                damage_mw.write(
                    ApplyDamage::at(hit.entity, weapon.damage, DamageKind::Ballistic, point)
                        .from(player)
                        .along(-dir),
                );
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
        let props = WeaponProps { fire_rate_per_minute: 300.0, ..Default::default() }; // 5 shots/sec
        let w = Weapon::from_props(&props, None);
        assert!((w.shot_interval - 0.2).abs() < 1e-6, "300 rpm -> 0.2s between shots");
    }

    #[test]
    fn zero_fire_rate_falls_back_to_a_sane_interval() {
        let props = WeaponProps { fire_rate_per_minute: 0.0, ..Default::default() };
        let w = Weapon::from_props(&props, None);
        assert!(w.shot_interval > 0.0, "must not divide by zero into an infinite fire rate");
    }

    #[test]
    fn pellets_are_clamped_to_at_least_one() {
        let props = WeaponProps { pellets: 0, ..Default::default() };
        let w = Weapon::from_props(&props, None);
        assert_eq!(w.pellets, 1, "a weapon with 0 pellets would never hit anything");
    }
}
