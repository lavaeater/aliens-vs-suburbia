//! Flamethrowers: a gun whose def has `flame: Some(..)` (`WeaponProps::flame`). Each shot
//! `shoot_weapons` fires is a [`FlameBurst`] instead of a ray: everything in a short cone
//! in front of the muzzle that the walls don't shield takes the gun's `damage` as Fire
//! and is set alight (`status::Afflict`, so resistances, reactions and friendly fire all
//! apply), and every so often burning fuel drips where the stream ends, a small fire
//! field. The stream itself is one looping particle effect per gun (`fx/flame_jet.ron`)
//! with a flickering light, riding the muzzle while the trigger is held.

use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use bevy_sprinkles::prelude::Particles3d;
use serde::{Deserialize, Serialize};

use crate::alien::components::general::Alien;
use crate::fx::{FxAssets, StopFx, facing};
use crate::game_state::GameState;
use crate::general::components::{CollisionLayer, Health};
use crate::general::damage::ApplyDamage;
use crate::general::systems::lights_systems::{LightAnimation, LightStyle};
use crate::gore::components::DamageKind;
use crate::gore::fire::SpawnFire;
use crate::infestation::neighbours::Neighbour;
use crate::physics::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use crate::player::components::Player;
use crate::player::systems::shoot::shoot_weapons;
use crate::status::Afflict;

/// A flamethrower's fire, in its weapon def (`flame: Some((..))`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FlameProps {
    /// How far the stream reaches, metres.
    #[serde(default = "default_reach")]
    pub reach: f32,
    /// Half-angle of the cone, degrees.
    #[serde(default = "default_cone")]
    pub cone_deg: f32,
    /// What catching fire from it does: damage per second...
    #[serde(default = "default_burn_dps")]
    pub burn_dps: f32,
    /// ...for this long.
    #[serde(default = "default_burn_secs")]
    pub burn_secs: f32,
    /// Seconds between drips of burning fuel at the end of the stream; 0 = none.
    #[serde(default = "default_drip_every")]
    pub drip_every: f32,
    #[serde(default = "default_drip_radius")]
    pub drip_radius: f32,
    #[serde(default = "default_drip_secs")]
    pub drip_secs: f32,
    #[serde(default = "default_drip_dps")]
    pub drip_dps: f32,
}

const fn default_reach() -> f32 {
    5.5
}
const fn default_cone() -> f32 {
    12.0
}
const fn default_burn_dps() -> f32 {
    10.0
}
const fn default_burn_secs() -> f32 {
    4.0
}
const fn default_drip_every() -> f32 {
    0.4
}
const fn default_drip_radius() -> f32 {
    0.7
}
const fn default_drip_secs() -> f32 {
    3.0
}
const fn default_drip_dps() -> f32 {
    15.0
}

impl Default for FlameProps {
    fn default() -> Self {
        Self {
            reach: default_reach(),
            cone_deg: default_cone(),
            burn_dps: default_burn_dps(),
            burn_secs: default_burn_secs(),
            drip_every: default_drip_every(),
            drip_radius: default_drip_radius(),
            drip_secs: default_drip_secs(),
            drip_dps: default_drip_dps(),
        }
    }
}

/// One shot of a flamethrower.
#[derive(Message, Clone, Copy, Debug)]
pub struct FlameBurst {
    pub source: Entity,
    pub weapon: Entity,
    /// The muzzle in the weapon's own frame, for the stream to ride; `None` = at `origin`.
    pub muzzle: Option<Vec3>,
    pub origin: Vec3,
    pub dir: Vec3,
    pub damage: i32,
    pub props: FlameProps,
}

/// A body this wide (metres either side of its centre) is caught by the edge of the cone.
const BODY_HALF_WIDTH: f32 = 0.35;
/// The stream goes out this long after the last burst.
const JET_IDLE: f32 = 0.15;
/// The stream's last particles live this long (`flame_jet.ron`).
const JET_LINGER: f32 = 0.9;
const JET_LUMENS: f32 = 30_000.0;

/// Whether `target` is in a flame cone from `origin` along `dir` (on the floor plane).
pub fn in_cone(origin: Vec3, dir: Vec3, reach: f32, cone_deg: f32, target: Vec3) -> bool {
    let Some(dir) = dir.xz().try_normalize() else {
        return false;
    };
    let to = target.xz() - origin.xz();
    let along = to.dot(dir);
    if along <= 0.0 || along > reach {
        return false;
    }
    let side = (to - dir * along).length();
    side <= along * cone_deg.to_radians().tan() + BODY_HALF_WIDTH
}

/// Burn what is in the stream, and drip fire where it ends.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn burn_in_the_stream(
    time: Res<Time>,
    mut bursts: MessageReader<FlameBurst>,
    spatial: SpatialQuery,
    targets: Query<
        (Entity, &Position),
        (With<Health>, Or<(With<Alien>, With<Neighbour>, With<Player>)>),
    >,
    mut damage_mw: MessageWriter<ApplyDamage>,
    mut afflict_mw: MessageWriter<Afflict>,
    mut fire_mw: MessageWriter<SpawnFire>,
    mut last_drip: Local<HashMap<Entity, f32>>,
) {
    let walls = SpatialQueryFilter::from_mask([CollisionLayer::ImpassableAll]);
    let now = time.elapsed_secs();
    for burst in bursts.read() {
        let props = burst.props;
        let flat = Vec3::new(burst.dir.x, 0.0, burst.dir.z).normalize_or(Vec3::X);
        let Ok(flat_dir) = Dir3::new(flat) else {
            continue;
        };
        // Walls cut the stream short.
        let reach = spatial
            .cast_ray(burst.origin, flat_dir, props.reach, true, &walls)
            .map_or(props.reach, |hit| hit.distance);

        for (target, pos) in &targets {
            if target == burst.source || !in_cone(burst.origin, flat, reach, props.cone_deg, pos.0)
            {
                continue;
            }
            // Nor does it go round corners.
            let to = Vec3::new(pos.0.x - burst.origin.x, 0.0, pos.0.z - burst.origin.z);
            let distance = to.length();
            if let Ok(to_dir) = Dir3::new(to)
                && spatial
                    .cast_ray(burst.origin, to_dir, distance, true, &walls)
                    .is_some_and(|hit| hit.distance < distance - BODY_HALF_WIDTH)
            {
                continue;
            }
            damage_mw.write(
                ApplyDamage::at(target, burst.damage, DamageKind::Fire, pos.0 + Vec3::Y * 0.4)
                    .from(burst.source)
                    .along(flat),
            );
            afflict_mw.write(
                Afflict::burning(target, props.burn_dps, props.burn_secs).from(Some(burst.source)),
            );
        }

        if props.drip_every > 0.0 {
            let due = last_drip
                .get(&burst.weapon)
                .is_none_or(|&last| now - last >= props.drip_every);
            if due {
                last_drip.insert(burst.weapon, now);
                // A little short of the end, so it doesn't land in the wall.
                let at = burst.origin + flat * (reach * 0.85).max(0.5);
                fire_mw.write(SpawnFire {
                    position: at,
                    radius: props.drip_radius,
                    duration: props.drip_secs,
                    dps: props.drip_dps,
                    spill: false,
                });
            }
        }
    }
}

/// The stream of one gun.
#[derive(Component)]
pub struct FlameJet {
    weapon: Entity,
    muzzle: Option<Vec3>,
    dir: Vec3,
    idle: f32,
}

/// Start the stream on a gun that has none, keep it going on one that has.
pub fn show_flame_jets(
    mut bursts: MessageReader<FlameBurst>,
    mut commands: Commands,
    mut jets: Query<&mut FlameJet>,
    fx: Option<Res<FxAssets>>,
) {
    let Some(fx) = fx else {
        bursts.clear();
        return;
    };
    for burst in bursts.read() {
        if let Some(mut jet) = jets.iter_mut().find(|j| j.weapon == burst.weapon) {
            jet.dir = burst.dir;
            jet.idle = 0.0;
            continue;
        }
        commands.spawn((
            Name::new("Flame jet"),
            FlameJet {
                weapon: burst.weapon,
                muzzle: burst.muzzle,
                dir: burst.dir,
                idle: 0.0,
            },
            Particles3d(fx.flame_jet.clone()),
            Transform::from_translation(burst.origin).with_rotation(facing(burst.dir)),
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.18),
                intensity: JET_LUMENS,
                range: 7.0,
                shadow_maps_enabled: false,
                ..default()
            },
            LightAnimation {
                base: JET_LUMENS,
                style: LightStyle::Flicker {
                    seed: (burst.weapon.index_u32() % 89) as f32 * 0.41,
                },
            },
        ));
    }
}

/// Ride the muzzle; go out once the trigger is let go (or the gun is gone).
pub fn tick_flame_jets(
    time: Res<Time>,
    mut commands: Commands,
    mut jets: Query<(Entity, &mut FlameJet, &mut Transform)>,
    weapons: Query<&GlobalTransform>,
) {
    for (entity, mut jet, mut transform) in &mut jets {
        jet.idle += time.delta_secs();
        let weapon = weapons.get(jet.weapon).ok();
        if jet.idle > JET_IDLE || weapon.is_none() {
            commands
                .entity(entity)
                .remove::<(FlameJet, PointLight, LightAnimation)>()
                .insert(StopFx::after(JET_LINGER));
            continue;
        }
        if let (Some(weapon), Some(muzzle)) = (weapon, jet.muzzle) {
            transform.translation = weapon.transform_point(muzzle);
        }
        // The light sits a little out along the stream; the emitter at the muzzle.
        transform.rotation = facing(jet.dir);
    }
}

pub struct FlamethrowerPlugin;

impl Plugin for FlamethrowerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FlameBurst>().add_systems(
            Update,
            (burn_in_the_stream, show_flame_jets, tick_flame_jets)
                .after(shoot_weapons)
                .run_if(in_state(GameState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cone_takes_what_is_in_front_and_close() {
        let o = Vec3::ZERO;
        let x = Vec3::X;
        assert!(in_cone(o, x, 5.0, 12.0, Vec3::new(3.0, 0.5, 0.0)));
        assert!(!in_cone(o, x, 5.0, 12.0, Vec3::new(6.0, 0.0, 0.0)), "too far");
        assert!(!in_cone(o, x, 5.0, 12.0, Vec3::new(-1.0, 0.0, 0.0)), "behind");
        assert!(!in_cone(o, x, 5.0, 12.0, Vec3::new(2.0, 0.0, 2.0)), "off to the side");
        // The edge of a body counts: 4 m out, tan 12 * 4 = 0.85, plus the body.
        assert!(in_cone(o, x, 5.0, 12.0, Vec3::new(4.0, 0.0, 1.1)));
    }

    #[test]
    fn a_stream_pointing_nowhere_burns_nothing() {
        assert!(!in_cone(Vec3::ZERO, Vec3::Y, 5.0, 12.0, Vec3::new(1.0, 0.0, 0.0)));
    }
}
