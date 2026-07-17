//! Hardpoint frame algebra for snapping weapons onto characters.
//!
//! A *hardpoint* is a coordinate frame (position + rotation, no scale) attached to
//! a model — "this part connects here, oriented like this". We represent it as an
//! [`Isometry3d`]. Snapping a weapon into a hand means making the weapon's grip
//! frame coincide with the character's grip frame.
//!
//! All the quaternion-heavy math lives here and is unit-tested, so the rest of the
//! game thinks only in named hardpoints and `Transform`s. See
//! `docs/inverse-kinematics-hardpoints.md`.
//!
//! Staged foundation: these helpers are exercised by unit tests now and wired into
//! the browser authoring UI + runtime snap in the next steps.
#![allow(dead_code)]

use bevy::math::{EulerRot, Isometry3d, Quat, Vec3};
use bevy::prelude::Transform;
use crate::assets::asset_definition::Hardpoint;

/// The frame of a stored [`Hardpoint`] as an [`Isometry3d`].
pub fn hardpoint_frame(h: &Hardpoint) -> Isometry3d {
    frame_from_euler(h.translation, h.rotation_euler_deg)
}

/// Build a hardpoint frame from an authored translation and XYZ Euler angles in
/// degrees (how hardpoints are stored in defs and edited in the asset browser).
pub fn frame_from_euler(translation: [f32; 3], euler_deg: [f32; 3]) -> Isometry3d {
    let [rx, ry, rz] = euler_deg;
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        rx.to_radians(),
        ry.to_radians(),
        rz.to_radians(),
    );
    Isometry3d::new(Vec3::from(translation), rotation)
}

/// The **local** transform to give a weapon parented to the character's hand bone
/// so the weapon's grip frame coincides with the character's grip frame.
///
/// - `grip_offset`: the character's grip hardpoint, local to the hand bone.
/// - `weapon_grip`: the weapon's grip hardpoint, local to the weapon origin.
///
/// Derivation: we want `weapon_local * weapon_grip == grip_offset` (the weapon's
/// grip frame, expressed in hand-bone-local space, lands on the character's grip
/// frame). Solving for `weapon_local` gives `grip_offset * weapon_grip.inverse()`.
pub fn weapon_local(grip_offset: Isometry3d, weapon_grip: Isometry3d) -> Isometry3d {
    grip_offset * weapon_grip.inverse()
}

/// The **world** transform for a weapon that is *not* parented to the hand bone,
/// given the hand bone's current world frame. Equivalent to parenting to the hand
/// and using [`weapon_local`].
pub fn weapon_world(
    hand_world: Isometry3d,
    grip_offset: Isometry3d,
    weapon_grip: Isometry3d,
) -> Isometry3d {
    hand_world * weapon_local(grip_offset, weapon_grip)
}

/// Convert a hardpoint frame to a `Transform` (scale 1) for spawning.
pub fn transform_from_frame(iso: Isometry3d) -> Transform {
    Transform {
        translation: iso.translation.into(),
        rotation: iso.rotation,
        scale: Vec3::ONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Isometry3d, b: Isometry3d) -> bool {
        let t = (Vec3::from(a.translation) - Vec3::from(b.translation)).length() < 1e-4;
        // Quaternions q and -q represent the same rotation, so compare via dot.
        let r = a.rotation.dot(b.rotation).abs() > 1.0 - 1e-4;
        t && r
    }

    #[test]
    fn weapon_grip_lands_on_character_grip() {
        // Whatever the two frames are, the weapon's grip must end up exactly on the
        // character's grip frame once placed with weapon_local.
        let grip_offset = frame_from_euler([0.05, -0.02, 0.10], [90.0, 0.0, 15.0]);
        let weapon_grip = frame_from_euler([0.0, 0.3, 0.0], [0.0, 45.0, 0.0]);

        let placed = weapon_local(grip_offset, weapon_grip) * weapon_grip;
        assert!(approx_eq(placed, grip_offset), "weapon grip did not land on character grip");
    }

    #[test]
    fn identity_weapon_grip_is_just_the_offset() {
        // If the weapon's grip is at its origin (identity), the local transform is
        // exactly the character's grip offset.
        let grip_offset = frame_from_euler([0.1, 0.2, 0.3], [10.0, 20.0, 30.0]);
        let local = weapon_local(grip_offset, Isometry3d::IDENTITY);
        assert!(approx_eq(local, grip_offset));
    }

    #[test]
    fn world_matches_parented_local() {
        // Placing in world space (hand_world * local) must equal transforming the
        // local result by the hand frame.
        let hand_world = frame_from_euler([1.0, 2.0, -0.5], [0.0, 90.0, 0.0]);
        let grip_offset = frame_from_euler([0.02, 0.0, 0.06], [90.0, 0.0, 0.0]);
        let weapon_grip = frame_from_euler([0.0, 0.25, 0.0], [0.0, 0.0, 10.0]);

        let world = weapon_world(hand_world, grip_offset, weapon_grip);
        let expected = hand_world * weapon_local(grip_offset, weapon_grip);
        assert!(approx_eq(world, expected));

        // And the weapon's grip frame ends up on the hand's grip frame in world space.
        let grip_in_world = world * weapon_grip;
        let hand_grip_world = hand_world * grip_offset;
        assert!(approx_eq(grip_in_world, hand_grip_world));
    }
}
