//! Aim-driven weapon placement: the gun leads, the hands follow.
//!
//! The older path (`equip::keep_weapons_snapped`) runs the causality *hand -> gun*: the
//! weapon is parented to the grip bone and rides whatever the animation does with the
//! hand. That is right for a pistol and wrong for a rifle, where what matters is that the
//! barrel points where you are aiming — the hands are a consequence, not a cause.
//!
//! Here the chain is **aim -> gun -> hands**:
//!
//! 1. the weapon's own axis (`stock` -> `muzzle`, or `grip` -> `muzzle` when the weapon has
//!    no stock) is pointed along [`AutoAim`];
//! 2. the weapon is slid so its anchor hardpoint sits on the character's matching one —
//!    a shoulder when both sides carry a `stock`, the trigger hand otherwise;
//! 3. the arms are solved to reach the resulting `grip` and `foregrip` (a later stage).
//!
//! The payoff is that this needs nothing per character-weapon pair: any weapon whose
//! frames are authored can be held by any character whose frames are authored.
//!
//! ## Why the weapon hangs off the character root
//!
//! It must not be parented to a bone the arm IK is about to rotate, or the weapon rides
//! the correction meant to reach it and the two chase each other. The character root is
//! the nearest ancestor no solver touches.
//!
//! ## Ordering, and why stale globals are fine
//!
//! Same slot as `torso_twist`: `PostUpdate`, after `AnimationSystems` (or the clip
//! overwrites the pose) and before `TransformSystems::Propagate` (or nothing reaches
//! `GlobalTransform`). At that point every `GlobalTransform` is still last frame's. That
//! is harmless *because the answer is expressed in the parent's frame*: the anchor bone's
//! world position and the root's inverse are read from the same stale snapshot, so the
//! local transform they produce is consistent, and propagation then applies this frame's
//! root. Only the aim direction is one frame old.

use bevy::prelude::*;

use crate::assets::asset_definition::Hardpoint;
use crate::player::components::{AutoAim, Player};
use crate::player::systems::shoot::Weapon;

/// How fast recoil decays, in fractions per second. Matches `keep_weapons_snapped`.
const RECOIL_DECAY: f32 = 14.0;

/// A weapon placed from the character's aim rather than from a hand bone.
///
/// Carries everything the placement needs so the system never touches the filesystem or
/// re-derives hardpoints per frame.
#[derive(Component)]
pub struct AimedWeapon {
    /// The character holding it.
    pub owner: Entity,
    /// Bone carrying the character-side anchor hardpoint.
    pub anchor_bone: Entity,
    /// The character-side anchor hardpoint, local to `anchor_bone`.
    pub anchor_offset: Vec3,
    /// The weapon-local point pinned to that anchor.
    pub weapon_anchor: Vec3,
    /// Weapon-local aim axis, from the back of the weapon to the muzzle.
    pub weapon_axis: Vec3,
    /// The character's model root, whose world scale says how big the character renders.
    pub model_root: Entity,
    /// The weapon def's scale, calibrated at world scale 1.
    pub def_scale: f32,
}

/// Which hardpoint role pins the weapon to the character, and what the weapon pivots on.
///
/// Preference order is `stock`, then `grip`, then `foregrip`: a rifle anchored at the
/// shoulder is what the whole idea is for, but a weapon with no authored stock still has
/// to work, and pivoting about the trigger hand is the next most honest thing.
pub const ANCHOR_ROLES: [&str; 3] = ["stock", "grip", "foregrip"];

/// Pick the role both sides carry, in [`ANCHOR_ROLES`] order.
pub fn anchor_role<'a>(
    character: impl Fn(&str) -> bool,
    weapon: impl Fn(&str) -> bool,
    roles: &'a [&'a str],
) -> Option<&'a str> {
    roles.iter().copied().find(|role| character(role) && weapon(role))
}

/// The weapon-local axis that gets pointed along the aim: from the anchor to the muzzle.
///
/// Anchor-to-muzzle rather than origin-to-muzzle because the anchor is the point the
/// weapon pivots about; a rifle pinned at the shoulder must swing about the shoulder.
pub fn weapon_axis(anchor: Vec3, muzzle: Vec3) -> Vec3 {
    let axis = muzzle - anchor;
    // A weapon whose muzzle sits on its anchor has no axis to speak of. Point it forward
    // rather than producing a NaN rotation that would blank the model.
    if axis.length_squared() < 1e-8 { Vec3::NEG_Z } else { axis.normalize() }
}

/// Rotation that points `axis_local` along `aim`, rolled so `up_local` stays as upright as
/// it can.
///
/// Two steps, because the aim direction only pins two of the three degrees of freedom:
/// `from_rotation_arc` gives the minimal rotation that lands the axis on the aim, and then
/// a roll about the aim itself brings the weapon's up as close to world up as possible.
/// Without the roll the gun would be correctly pointed but arbitrarily banked — the
/// classic "aligned by one vector" mistake called out in the design doc.
pub fn aimed_rotation(axis_local: Vec3, up_local: Vec3, aim: Vec3) -> Quat {
    let Ok(axis) = Dir3::new(axis_local) else { return Quat::IDENTITY };
    let Ok(aim) = Dir3::new(aim) else { return Quat::IDENTITY };

    let align = Quat::from_rotation_arc(*axis, *aim);
    let current_up = align * up_local;

    // Roll is measured in the plane perpendicular to the aim; components along the aim
    // cannot be corrected by rolling about it.
    let flatten = |v: Vec3| v - *aim * v.dot(*aim);
    let have = flatten(current_up);
    let want = flatten(Vec3::Y);
    // Aiming straight up or down leaves no upright to prefer, and a weapon whose up runs
    // along its own barrel has nothing to roll. Both would give a zero-length direction.
    if have.length_squared() < 1e-6 || want.length_squared() < 1e-6 {
        return align;
    }

    let have = have.normalize();
    let want = want.normalize();
    let angle = have.dot(want).clamp(-1.0, 1.0).acos() * have.cross(want).dot(*aim).signum();
    Quat::from_axis_angle(*aim, angle) * align
}

/// Where the weapon origin goes so its `anchor_local` point lands on `anchor_world`.
pub fn aimed_translation(
    rotation: Quat,
    anchor_local: Vec3,
    world_scale: f32,
    anchor_world: Vec3,
) -> Vec3 {
    anchor_world - rotation * (anchor_local * world_scale)
}

/// The full world placement for an aimed weapon.
pub fn aimed_transform(
    weapon_anchor: Vec3,
    weapon_axis: Vec3,
    world_scale: f32,
    anchor_world: Vec3,
    aim: Vec3,
) -> Transform {
    let rotation = aimed_rotation(weapon_axis, Vec3::Y, aim);
    Transform {
        translation: aimed_translation(rotation, weapon_anchor, world_scale, anchor_world),
        rotation,
        scale: Vec3::splat(world_scale),
    }
}

/// The weapon-local translation of a hardpoint, as a plain point.
pub fn hardpoint_point(hardpoint: &Hardpoint) -> Vec3 {
    Vec3::from(hardpoint.translation)
}

/// Place every aimed weapon for this frame.
pub fn aim_weapons(
    time: Res<Time>,
    aimed: Query<(Entity, &AimedWeapon)>,
    aims: Query<&AutoAim, With<Player>>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
    mut recoil_q: Query<&mut Weapon>,
) {
    let dt = time.delta_secs();

    for (weapon, aimed) in aimed.iter() {
        let Ok(aim) = aims.get(aimed.owner) else { continue };
        let Ok(bone) = globals.get(aimed.anchor_bone) else { continue };
        let Ok(owner) = globals.get(aimed.owner) else { continue };

        // The character's model root carries how big the character is drawn; the weapon
        // tracks that rather than whatever scale the rig baked into its bones.
        let root_scale = globals.get(aimed.model_root).map(|gt| gt.scale().x).unwrap_or(1.0);
        let world_scale = aimed.def_scale * root_scale;

        let anchor_world = bone.transform_point(aimed.anchor_offset);
        let desired = aimed_transform(
            aimed.weapon_anchor,
            aimed.weapon_axis,
            world_scale,
            anchor_world,
            aim.0,
        );

        // Expressed in the owner's frame, since that is what the weapon is parented to.
        let local = Transform::from_matrix(
            owner.to_matrix().inverse() * desired.to_matrix(),
        );

        let Ok(mut transform) = transforms.get_mut(weapon) else { continue };
        *transform = local;

        if let Ok(mut gun) = recoil_q.get_mut(weapon) {
            if gun.recoil > 0.0001 {
                transform.rotation *= Quat::from_rotation_x(gun.recoil);
                gun.recoil = (gun.recoil - gun.recoil * RECOIL_DECAY * dt).max(0.0);
            } else {
                gun.recoil = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    /// The whole point: the barrel ends up pointing where you are aiming.
    #[test]
    fn the_weapon_axis_ends_up_along_the_aim() {
        let axis = Vec3::new(1.0, 0.1, 0.0).normalize();
        for aim in [Vec3::X, Vec3::NEG_Z, Vec3::new(0.4, -0.2, 0.9)] {
            let rotation = aimed_rotation(axis, Vec3::Y, aim);
            assert!(
                close(rotation * axis, aim.normalize()),
                "aim {aim:?} gave {:?}",
                rotation * axis
            );
        }
    }

    /// Aim alone leaves the weapon free to bank around the barrel. Rolling upright is
    /// what stops a rifle being held sideways.
    #[test]
    fn a_horizontally_aimed_weapon_is_not_banked() {
        let rotation = aimed_rotation(Vec3::X, Vec3::Y, Vec3::NEG_Z);
        let up = rotation * Vec3::Y;
        assert!(up.y > 0.999, "up ended at {up:?}");
    }

    /// ...and the roll is a genuine improvement over the bare alignment, not a no-op.
    #[test]
    fn rolling_upright_beats_the_bare_alignment() {
        // An axis rolled about itself: `from_rotation_arc` alone carries that bank over.
        let axis = Vec3::X;
        let up_local = Vec3::new(0.0, 0.6, 0.8).normalize();
        let aim = Vec3::NEG_Z;
        let bare = Quat::from_rotation_arc(axis, aim) * up_local;
        let rolled = aimed_rotation(axis, up_local, aim) * up_local;
        assert!(rolled.y > bare.y, "rolled {rolled:?} was no better than {bare:?}");
    }

    /// Aiming straight up has no upright to prefer. It must still produce a usable
    /// rotation rather than a NaN one, which would make the weapon vanish.
    #[test]
    fn aiming_straight_up_still_points_the_barrel() {
        let rotation = aimed_rotation(Vec3::X, Vec3::Y, Vec3::Y);
        assert!(rotation.is_finite());
        assert!(close(rotation * Vec3::X, Vec3::Y));
    }

    #[test]
    fn a_degenerate_axis_or_aim_leaves_the_weapon_alone() {
        assert_eq!(aimed_rotation(Vec3::ZERO, Vec3::Y, Vec3::X), Quat::IDENTITY);
        assert_eq!(aimed_rotation(Vec3::X, Vec3::Y, Vec3::ZERO), Quat::IDENTITY);
    }

    /// The anchor is the contract with the character: whatever the rotation, that point
    /// sits exactly where the character's hardpoint is.
    #[test]
    fn the_anchor_point_lands_on_the_characters_hardpoint() {
        let anchor_world = Vec3::new(2.0, 1.5, -3.0);
        let placed = aimed_transform(
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::X,
            0.25,
            anchor_world,
            Vec3::new(1.0, 0.0, 1.0),
        );
        assert!(close(placed.transform_point(Vec3::new(0.0, 0.2, 0.0)), anchor_world));
    }

    /// A rifle pinned at the shoulder must swing about the shoulder, so the axis is
    /// measured from the anchor rather than from the model origin.
    #[test]
    fn the_axis_runs_from_the_anchor_to_the_muzzle() {
        let axis = weapon_axis(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0));
        assert!(close(axis, Vec3::X));
    }

    #[test]
    fn a_muzzle_sitting_on_the_anchor_does_not_produce_a_nan_axis() {
        let axis = weapon_axis(Vec3::ONE, Vec3::ONE);
        assert!(axis.is_finite() && axis.length() > 0.5);
    }

    /// Shoulder-anchored is the goal; trigger-hand is the fallback that keeps weapons
    /// with no authored stock working.
    #[test]
    fn the_shoulder_wins_when_both_sides_have_a_stock() {
        let both = |role: &str| matches!(role, "stock" | "grip" | "foregrip");
        assert_eq!(anchor_role(both, both, &ANCHOR_ROLES), Some("stock"));
    }

    #[test]
    fn a_weapon_without_a_stock_falls_back_to_the_grip() {
        let character = |role: &str| matches!(role, "stock" | "grip" | "foregrip");
        // The Assault Rifle as authored today: grip, foregrip, muzzle, no stock.
        let weapon = |role: &str| matches!(role, "grip" | "foregrip");
        assert_eq!(anchor_role(character, weapon, &ANCHOR_ROLES), Some("grip"));
    }

    #[test]
    fn nothing_in_common_means_no_aimed_placement() {
        let character = |role: &str| role == "stock";
        let weapon = |role: &str| role == "grip";
        assert_eq!(anchor_role(character, weapon, &ANCHOR_ROLES), None);
    }
}
