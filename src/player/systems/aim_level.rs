//! The height a player aims at.
//!
//! `AutoAim` is a compass heading only. This gives it a height: the barrel aims at a point
//! [`AIM_DISTANCE`] ahead that is as high above the ground there as the muzzle is above the
//! ground under the player, so the gun follows the slope instead of pointing into a hill or
//! over a valley. When the sight line meets a monster first, the barrel aims at the monster's
//! centre instead (`docs/playtest.md`, *Aim Level*).
//!
//! Only the look follows it: the gun's pitch (`weapon_aim`), the muzzle flash, where tracers
//! and blood land, and what launchers fire along. What a bullet hits is still the level ray
//! of `shoot_weapons`, which the tall colliders on hills (`ground_follow`) are built for.

use crate::physics::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::general::components::CollisionLayer;
use crate::general::damage::Faction;
use crate::map::ground::GroundHeight;
use crate::player::components::{AutoAim, Player, PlayerDead};
use crate::player::systems::equip::EquippedWeapon;
use crate::player::systems::shoot::{Weapon, muzzle_origin};

/// How far ahead, in metres, the barrel aims over the ground when no monster is in the way.
pub const AIM_DISTANCE: f32 = 6.0;

/// How far the sight line looks for a monster without a gun's own range to go by.
const SIGHT_RANGE: f32 = 20.0;

/// The point a player's barrel aims at this frame, `None` until it has been worked out.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AimLevel {
    pub target: Option<Vec3>,
    /// The monster the sight line meets, whose centre `target` is.
    pub monster: Option<Entity>,
}

impl AimLevel {
    /// The way to aim from `origin`: at the target, or `fallback` without one (or with the
    /// target on top of `origin`).
    pub fn dir_from(&self, origin: Vec3, fallback: Vec3) -> Vec3 {
        self.target
            .and_then(|target| (target - origin).try_normalize())
            .unwrap_or(fallback)
    }
}

/// Where to aim from `muzzle` along the level heading `heading`: a monster's centre when
/// the sight line meets one, else [`AIM_DISTANCE`] ahead at the muzzle's height above the
/// ground.
pub fn aim_target(
    ground: &GroundHeight,
    muzzle: Vec3,
    heading: Vec3,
    monster_centre: Option<Vec3>,
) -> Vec3 {
    monster_centre.unwrap_or_else(|| {
        let flat = heading.with_y(0.0).normalize_or(Vec3::X);
        ground.carried(muzzle, muzzle + flat * AIM_DISTANCE)
    })
}

/// Work out every living player's [`AimLevel`]. Before `shoot_weapons`; the gun's own
/// placement (`aim_weapons`, `PostUpdate`) reads it later in the frame. The muzzle is where
/// the gun was last frame, which only lags by its own small swing.
#[allow(clippy::type_complexity)]
pub fn level_aims(
    spatial: SpatialQuery,
    ground: Res<GroundHeight>,
    mut players: Query<
        (&AutoAim, &Position, Option<&EquippedWeapon>, &mut AimLevel),
        (With<Player>, Without<PlayerDead>),
    >,
    weapons: Query<(&Weapon, &GlobalTransform)>,
    targets: Query<(&Faction, &GlobalTransform)>,
) {
    // The same things a bullet stops at: monsters, and the walls that hide them.
    let filter =
        SpatialQueryFilter::from_mask([CollisionLayer::Alien, CollisionLayer::ImpassableAll]);

    for (aim, position, equipped, mut level) in &mut players {
        let heading = aim.0.with_y(0.0).normalize_or(Vec3::X);
        let gun = equipped.and_then(|equipped| weapons.get(equipped.0).ok());
        let muzzle = gun.map_or_else(
            || muzzle_origin(None, None, position.0, heading),
            |(weapon, transform)| {
                muzzle_origin(weapon.muzzle.as_ref(), Some(transform), position.0, heading)
            },
        );
        let range = gun.map_or(SIGHT_RANGE, |(weapon, _)| weapon.range);

        let monster = Dir3::new(heading)
            .ok()
            .and_then(|dir| spatial.cast_ray(muzzle, dir, range, true, &filter))
            .and_then(|hit| match targets.get(hit.entity) {
                Ok((Faction::Alien, transform)) => Some((hit.entity, transform.translation())),
                _ => None,
            });

        let new = AimLevel {
            target: Some(aim_target(
                &ground,
                muzzle,
                heading,
                monster.map(|(_, centre)| centre),
            )),
            monster: monster.map(|(entity, _)| entity),
        };
        // Only on change, so `Changed<AimLevel>` stays meaningful.
        level.set_if_neq(new);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn on_flat_ground_the_aim_is_level_at_barrel_height() {
        let ground = GroundHeight::default();
        let muzzle = Vec3::new(3.0, 0.8, 2.0);
        let target = aim_target(&ground, muzzle, Vec3::Z, None);
        assert!(close(target, muzzle + Vec3::Z * AIM_DISTANCE), "{target:?}");
    }

    /// Uphill the aim rises with the ground, keeping the muzzle's height above it.
    #[test]
    fn on_a_slope_the_aim_follows_the_ground() {
        use crate::general::components::map_components::MapFile;
        // Rises a metre for every elevation step east.
        let slope = GroundHeight::new(
            &MapFile {
                elevation: vec![vec![0, 100, 200, 300, 400, 500, 600]; 7],
                ..MapFile::default()
            },
            1.0,
        );
        let muzzle = Vec3::new(3.0, 1.0, 3.0);
        let target = aim_target(&slope, muzzle, Vec3::X, None);
        let ahead = muzzle.x + AIM_DISTANCE;
        let rise = slope.height(ahead, muzzle.z) - slope.height(muzzle.x, muzzle.z);
        assert!(rise > 0.5, "the test slope should rise, rose {rise}");
        assert!((target.y - (muzzle.y + rise)).abs() < 1e-4, "{target:?}");
    }

    #[test]
    fn a_heading_with_height_is_levelled_first() {
        let ground = GroundHeight::default();
        let muzzle = Vec3::new(0.0, 0.8, 0.0);
        let target = aim_target(&ground, muzzle, Vec3::new(1.0, 0.5, 0.0), None);
        assert!(close(target, muzzle + Vec3::X * AIM_DISTANCE), "{target:?}");
    }

    #[test]
    fn a_monster_in_the_sight_line_is_aimed_at_its_centre() {
        let ground = GroundHeight::default();
        let centre = Vec3::new(4.0, 0.3, 0.0);
        let target = aim_target(&ground, Vec3::Y * 0.8, Vec3::X, Some(centre));
        assert!(close(target, centre), "{target:?}");
    }

    #[test]
    fn the_direction_falls_back_without_a_target() {
        let none = AimLevel::default();
        assert_eq!(none.dir_from(Vec3::ZERO, Vec3::Z), Vec3::Z);
        let on_top = AimLevel {
            target: Some(Vec3::ONE),
            monster: None,
        };
        assert_eq!(on_top.dir_from(Vec3::ONE, Vec3::Z), Vec3::Z);
    }

    #[test]
    fn the_direction_points_down_at_a_low_monster() {
        let level = AimLevel {
            target: Some(Vec3::new(4.0, 0.0, 0.0)),
            monster: None,
        };
        let dir = level.dir_from(Vec3::new(0.0, 1.0, 0.0), Vec3::X);
        assert!(dir.y < 0.0 && dir.x > 0.0, "{dir:?}");
    }
}
