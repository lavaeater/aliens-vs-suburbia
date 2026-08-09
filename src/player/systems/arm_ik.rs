//! Two-bone analytic IK: put the hands on the weapon.
//!
//! The weapon has already been placed from the aim (`weapon_aim`). This is the second half
//! of the promise — the arms are solved so the character's `grip` and `foregrip`
//! hardpoints land on the weapon's, whatever the weapon and whatever the character.
//!
//! ## Analytic, not iterative
//!
//! An arm is two bones and one target, which the law of cosines answers exactly in closed
//! form. No iterations, no convergence, no jitter, and the whole thing is unit-testable
//! without an ECS. A general solver (`bevy_mod_inverse_kinematics`) earns its keep on
//! longer chains; here it would be a worse answer arrived at more slowly.
//!
//! ## Forward kinematics by hand, and why
//!
//! Every `GlobalTransform` is one frame stale when this runs (same slot as `torso_twist`,
//! for the same reasons). Reading the arm's world pose from those would be doubly wrong:
//! stale *and* carrying the correction this system itself wrote last frame, which would
//! compound. So the chain is re-composed from this frame's animated **local** transforms,
//! starting at the shoulder's parent — a bone no solver touches, whose one-frame lag is a
//! rigid offset shared by the weapon (placed from an equally stale bone) and therefore
//! cancels in the relative geometry that matters.
//!
//! ## Reach
//!
//! The target is clamped into `[|upper - lower|, upper + lower]`. Out of reach, the arm
//! points at the target fully extended rather than snapping or producing NaNs — a
//! stretched arm reads as reaching, which is what is actually happening.

use bevy::prelude::*;

/// How far the elbow is pushed toward the pole hint, as a fraction of the arm's length.
const POLE_STRENGTH: f32 = 1.0;

/// One arm to be solved onto a hardpoint on the weapon.
#[derive(Clone)]
pub struct WeaponArm {
    /// Upper arm — the bone that swings from the shoulder.
    pub upper: Entity,
    /// Forearm — its parent is `upper`.
    pub lower: Entity,
    /// Wrist. Rotating this is what orients the grip; the fingers below it ride along.
    pub hand: Entity,
    /// Bone the character-side hardpoint is anchored to. Often a finger rather than the
    /// hand itself, which is why it is tracked separately from `hand`.
    pub effector: Entity,
    /// The character-side hardpoint frame, local to `effector`.
    pub effector_frame: Transform,
    /// The weapon-side hardpoint frame, local to the weapon.
    pub weapon_frame: Transform,
    /// Which way the elbow bends, in the character's local space.
    pub pole: Vec3,
}

/// The arms of a character holding an aim-driven weapon.
#[derive(Component, Clone)]
pub struct WeaponArms {
    pub weapon: Entity,
    pub arms: Vec<WeaponArm>,
}

/// Where the two-bone chain sits in a list of bones walking *up* from the effector.
///
/// Rigs differ in how many bones sit between the hardpoint and the arm — a grip anchored
/// to a thumb tip is four bones below the shoulder, one anchored to the hand is two — so
/// the chain is found rather than assumed: the first ancestor that looks like a hand fixes
/// the wrist, and the two bones above it are the arm.
///
/// `ancestors` is the effector's ancestors, nearest first. Returns `(upper, lower)` as
/// indices into it.
pub fn arm_chain(ancestors: &[&str]) -> Option<(usize, usize)> {
    let hand = ancestors
        .iter()
        .position(|name| name.to_ascii_lowercase().contains("hand"))
        // A hardpoint anchored directly to the hand has no "hand" among its *ancestors*,
        // so fall back to treating the effector itself as the wrist.
        .unwrap_or(0);
    let lower = hand + 1;
    let upper = hand + 2;
    (upper < ancestors.len()).then_some((upper, lower))
}

/// Where the elbow goes so that a two-bone arm reaches `target`.
///
/// The law of cosines gives the angle at the shoulder; the pole direction then decides
/// which way around the arm's axis the elbow swings, since every rotation of the elbow
/// about the shoulder-to-target line reaches the target equally well.
pub fn solve_elbow(
    shoulder: Vec3,
    target: Vec3,
    upper_len: f32,
    lower_len: f32,
    pole: Vec3,
) -> Vec3 {
    let to_target = target - shoulder;
    let Ok(axis) = Dir3::new(to_target) else {
        // Target sitting on the shoulder: no direction to work with. Put the elbow where
        // the pole says and let the forearm do what it can.
        return shoulder + pole.normalize_or(Vec3::NEG_Y) * upper_len;
    };

    // Clamped so an unreachable target extends the arm instead of asking the triangle to
    // close: acos of an out-of-range cosine is NaN, and a NaN bone rotation blanks the
    // whole character.
    let min = (upper_len - lower_len).abs() + 1e-4;
    let max = upper_len + lower_len - 1e-4;
    let distance = to_target.length().clamp(min.min(max), max.max(min));

    let cos_shoulder =
        ((upper_len * upper_len + distance * distance - lower_len * lower_len)
            / (2.0 * upper_len * distance))
            .clamp(-1.0, 1.0);
    let angle = cos_shoulder.acos();

    // The bend plane: the component of the pole hint perpendicular to the arm's axis.
    let flattened = pole - *axis * pole.dot(*axis);
    let bend = flattened.try_normalize().unwrap_or_else(|| {
        // Pole parallel to the arm says nothing about the plane. Any perpendicular will
        // do, and `any_orthonormal_vector` always yields one.
        axis.any_orthonormal_vector()
    });

    shoulder + (*axis * angle.cos() + bend * angle.sin() * POLE_STRENGTH) * upper_len
}

/// Rotation that swings `from` onto `to`, applied on top of a bone's current world
/// rotation.
///
/// This is "aim the bone at the point" rather than "set the bone to a known orientation",
/// which matters because rigs bake arbitrary bone orientations: we never need to know what
/// a bone's rest pose meant, only where it currently points and where it should.
pub fn aim_bone(current_world: Quat, from: Vec3, to: Vec3) -> Quat {
    let (Ok(from), Ok(to)) = (Dir3::new(from), Dir3::new(to)) else {
        return current_world;
    };
    Quat::from_rotation_arc(*from, *to) * current_world
}

/// The local rotation to store on a bone whose parent sits at `parent_world`.
pub fn local_from_world(parent_world: Quat, world: Quat) -> Quat {
    (parent_world.inverse() * world).normalize()
}

/// Bend the elbows down and back, the neutral rifle stance. Expressed in the character's
/// own space so it turns with the body rather than pointing a fixed way in the world.
pub const DEFAULT_POLE: Vec3 = Vec3::new(0.0, -1.0, -1.0);

/// The bones from `stop` (exclusive) down to `effector` (inclusive), in order.
///
/// `None` when `effector` is not below `stop` — a hardpoint anchored outside the arm, in
/// which case there is nothing sensible to solve.
fn path_down(effector: Entity, stop: Entity, parents: &Query<&ChildOf>) -> Option<Vec<Entity>> {
    let mut path = vec![effector];
    let mut current = effector;
    // Rigs are shallow; the bound only stops a malformed hierarchy from hanging the frame.
    for _ in 0..32 {
        let parent = parents.get(current).ok()?.parent();
        if parent == stop {
            path.reverse();
            return Some(path);
        }
        path.push(parent);
        current = parent;
    }
    None
}

/// Solve both arms onto the weapon.
pub fn solve_weapon_arms(
    hand_align: Res<HandAlignEnabled>,
    characters: Query<(&WeaponArms, &GlobalTransform)>,
    parents: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
) {
    for (arms, owner) in characters.iter() {
        // The weapon's own `GlobalTransform` is still last frame's — `aim_weapons` wrote
        // its `Transform` moments ago and nothing has propagated yet. Compose it here
        // instead, against the same stale owner the bones hang off, so weapon and arms
        // are solved in one consistent frame.
        let Ok(weapon_local) = transforms.get(arms.weapon).copied() else { continue };
        let weapon_world = owner.mul_transform(weapon_local);

        for arm in &arms.arms {
            solve_arm(
                arm,
                &weapon_world,
                owner,
                hand_align.0,
                &parents,
                &globals,
                &mut transforms,
            );
        }
    }
}

/// Whether hands are rotated to match the weapon's grip frame. `F8` toggles it, the same
/// A/B affordance `F7` gives the torso twist.
#[derive(Resource)]
pub struct HandAlignEnabled(pub bool);

impl Default for HandAlignEnabled {
    fn default() -> Self {
        Self(true)
    }
}

pub fn toggle_hand_align(
    keys: Res<ButtonInput<KeyCode>>,
    mut enabled: ResMut<HandAlignEnabled>,
) {
    if keys.just_pressed(KeyCode::F8) {
        enabled.0 = !enabled.0;
        info!("hand alignment {}", if enabled.0 { "on" } else { "off" });
    }
}

fn solve_arm(
    arm: &WeaponArm,
    weapon_world: &GlobalTransform,
    owner: &GlobalTransform,
    align_hand: bool,
    parents: &Query<&ChildOf>,
    globals: &Query<&GlobalTransform>,
    transforms: &mut Query<&mut Transform>,
) -> Option<()> {
    let base = parents.get(arm.upper).ok()?.parent();
    let base_world = *globals.get(base).ok()?;

    // Forward kinematics from this frame's animated locals — see the module docs for why
    // the bones' own globals are not used.
    let upper_world = base_world.mul_transform(*transforms.get(arm.upper).ok()?);
    let lower_world = upper_world.mul_transform(*transforms.get(arm.lower).ok()?);

    let mut hand_world = lower_world;
    for bone in path_down(arm.hand, arm.lower, parents)? {
        hand_world = hand_world.mul_transform(*transforms.get(bone).ok()?);
    }
    // The hardpoint usually hangs a few finger joints below the wrist.
    let mut hardpoint_world = hand_world;
    if arm.effector != arm.hand {
        for bone in path_down(arm.effector, arm.hand, parents)? {
            hardpoint_world = hardpoint_world.mul_transform(*transforms.get(bone).ok()?);
        }
    }
    let hardpoint_world = hardpoint_world.mul_transform(arm.effector_frame);
    let target = weapon_world.mul_transform(arm.weapon_frame);

    // The hardpoint is rigidly bolted to the wrist, so the wrist's pose determines it.
    // Working out where the *wrist* has to be, rather than solving for the hardpoint and
    // rotating afterwards, is what lets orientation and position both come out exact:
    // rotating the hand last would drag the hardpoint off the target it had just reached.
    let hand_rotation = hand_world.rotation();
    let offset_in_hand = hand_rotation.inverse()
        * (hardpoint_world.translation() - hand_world.translation());
    let hardpoint_in_hand = hand_rotation.inverse() * hardpoint_world.rotation();

    let wanted_hand_rotation = if align_hand {
        target.rotation() * hardpoint_in_hand.inverse()
    } else {
        hand_rotation
    };
    let wanted_hand =
        target.translation() - wanted_hand_rotation * offset_in_hand;

    let shoulder = upper_world.translation();
    let elbow = lower_world.translation();
    let hand = hand_world.translation();

    let upper_len = (elbow - shoulder).length();
    let lower_len = (hand - elbow).length();
    if upper_len < 1e-5 || lower_len < 1e-5 {
        return None;
    }

    let pole = owner.rotation() * arm.pole;
    let wanted_elbow = solve_elbow(shoulder, wanted_hand, upper_len, lower_len, pole);

    // Upper arm: swing the shoulder so the elbow lands where the solve wants it.
    let upper_rotation =
        aim_bone(upper_world.rotation(), elbow - shoulder, wanted_elbow - shoulder);
    let swing = upper_rotation * upper_world.rotation().inverse();

    // Everything below the shoulder is rigid under that swing, so the forearm's new
    // orientation and the wrist's new position follow without re-composing the chain.
    let lower_rotation_after_swing = swing * lower_world.rotation();
    let hand_after_swing = wanted_elbow + swing * (hand - elbow);

    // Forearm: swing the elbow so the wrist lands where it needs to be.
    //
    // With the wrist about to be set to a known orientation, aiming it is exact — the
    // hardpoint's offset from the wrist is then exactly what the target assumed. Without
    // that (F8 off), the wrist keeps whatever orientation the swing gave it and the
    // assumption breaks, so aim the *hardpoint* at the target instead and accept whatever
    // roll the animation had.
    let lower_rotation = if align_hand {
        aim_bone(
            lower_rotation_after_swing,
            hand_after_swing - wanted_elbow,
            wanted_hand - wanted_elbow,
        )
    } else {
        let hardpoint_after_swing =
            wanted_elbow + swing * (hardpoint_world.translation() - elbow);
        aim_bone(
            lower_rotation_after_swing,
            hardpoint_after_swing - wanted_elbow,
            target.translation() - wanted_elbow,
        )
    };

    transforms.get_mut(arm.upper).ok()?.rotation =
        local_from_world(base_world.rotation(), upper_rotation);
    transforms.get_mut(arm.lower).ok()?.rotation =
        local_from_world(upper_rotation, lower_rotation);

    if align_hand {
        // The wrist's parent is the forearm, which has just moved; its new world rotation
        // is what the hand's local has to be expressed against.
        let hand_parent = parents.get(arm.hand).ok()?.parent();
        let parent_rotation = if hand_parent == arm.lower {
            lower_rotation
        } else {
            // A rig with extra bones between forearm and wrist: re-compose down to it.
            let mut world = GlobalTransform::from(Transform::from_rotation(lower_rotation));
            for bone in path_down(hand_parent, arm.lower, parents)? {
                world = world.mul_transform(*transforms.get(bone).ok()?);
            }
            world.rotation()
        };
        transforms.get_mut(arm.hand).ok()?.rotation =
            local_from_world(parent_rotation, wanted_hand_rotation);
    }
    Some(())
}

/// How far the head may be turned from its animated pose to look down the sights.
///
/// A clamp, because the alignment is absolute: a weapon pointed behind the character would
/// otherwise wring the neck right round. Past the limit the head turns as far as it can
/// and the aim wins the rest.
pub const MAX_HEAD_TURN_DEGREES: f32 = 55.0;

/// Lines a character's `sight` frame up with the weapon's, by turning one bone.
///
/// Position is left alone deliberately. The head cannot *move* to the sight without
/// dragging the spine with it, and the useful half of aiming down sights is the head
/// pointing where the gun points — which is pure rotation.
#[derive(Component, Clone)]
pub struct SightAlign {
    pub weapon: Entity,
    /// Bone to turn — the character's `sight` anchor, usually the head.
    pub bone: Entity,
    /// The character-side `sight` frame, local to `bone`.
    pub bone_frame: Transform,
    /// The weapon-side `sight` frame, local to the weapon.
    pub weapon_frame: Transform,
}

/// Rotate `from` toward `to`, by at most `max_radians`.
pub fn limited_turn(from: Quat, to: Quat, max_radians: f32) -> Quat {
    let angle = from.angle_between(to);
    if angle <= max_radians || angle < 1e-6 {
        return to;
    }
    from.slerp(to, max_radians / angle).normalize()
}

/// Turn each character's sight bone to line up with the weapon's sight frame.
pub fn align_sights(
    characters: Query<(&SightAlign, &GlobalTransform)>,
    parents: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
) {
    for (sight, owner) in characters.iter() {
        let Ok(weapon_local) = transforms.get(sight.weapon).copied() else { continue };
        let weapon_world = owner.mul_transform(weapon_local);
        let target = weapon_world.mul_transform(sight.weapon_frame).rotation();

        let Ok(parent) = parents.get(sight.bone).map(|p| p.parent()) else { continue };
        let Ok(parent_world) = globals.get(parent).copied() else { continue };
        let Ok(bone_local) = transforms.get(sight.bone).copied() else { continue };

        // Same stale-globals reasoning as the arms: compose this frame's animated local
        // against the parent's world rather than reading the bone's own global, which
        // still carries the turn written last frame.
        let animated = parent_world.mul_transform(bone_local).rotation();
        let wanted = target * sight.bone_frame.rotation.inverse();
        let limited = limited_turn(animated, wanted, MAX_HEAD_TURN_DEGREES.to_radians());

        if let Ok(mut transform) = transforms.get_mut(sight.bone) {
            transform.rotation = local_from_world(parent_world.rotation(), limited);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reachable_target_gives_a_bent_arm_that_reaches_it() {
        let shoulder = Vec3::ZERO;
        let target = Vec3::new(1.2, 0.0, 0.0);
        let elbow = solve_elbow(shoulder, target, 1.0, 1.0, Vec3::NEG_Y);

        assert!((elbow - shoulder).length() - 1.0 < 1e-3, "upper arm keeps its length");
        assert!(
            ((target - elbow).length() - 1.0).abs() < 1e-3,
            "forearm reaches the target: {}",
            (target - elbow).length()
        );
        assert!(elbow.y < -0.1, "the elbow bent toward the pole, got {elbow:?}");
    }

    /// Which side the elbow falls on is the only thing the pole decides, and getting it
    /// wrong is the difference between an arm and a broken doll.
    #[test]
    fn the_pole_decides_which_way_the_elbow_bends() {
        let down = solve_elbow(Vec3::ZERO, Vec3::new(1.2, 0.0, 0.0), 1.0, 1.0, Vec3::NEG_Y);
        let up = solve_elbow(Vec3::ZERO, Vec3::new(1.2, 0.0, 0.0), 1.0, 1.0, Vec3::Y);
        assert!(down.y < 0.0 && up.y > 0.0, "down {down:?} up {up:?}");
    }

    /// Out of reach must extend the arm, not produce NaN — `acos` of a cosine outside
    /// [-1, 1] would, and a NaN rotation makes the whole character vanish.
    #[test]
    fn an_unreachable_target_extends_the_arm_instead_of_breaking() {
        let target = Vec3::new(9.0, 0.0, 0.0);
        let elbow = solve_elbow(Vec3::ZERO, target, 1.0, 1.0, Vec3::NEG_Y);
        assert!(elbow.is_finite(), "got {elbow:?}");
        // Straight, not folded: the elbow sits on the line to the target. The clamp leaves
        // a hair of bend on purpose, so this asks for alignment rather than equality.
        assert!(
            elbow.normalize().dot(target.normalize()) > 0.999,
            "arm did not extend toward the target: {elbow:?}"
        );
    }

    /// A target closer than the arm can fold is the same hazard from the other side.
    #[test]
    fn a_target_inside_the_folded_arm_does_not_break_either() {
        let elbow = solve_elbow(Vec3::ZERO, Vec3::new(0.01, 0.0, 0.0), 1.0, 0.5, Vec3::NEG_Y);
        assert!(elbow.is_finite(), "got {elbow:?}");
    }

    #[test]
    fn a_target_on_the_shoulder_is_survivable() {
        let elbow = solve_elbow(Vec3::ZERO, Vec3::ZERO, 1.0, 1.0, Vec3::NEG_Y);
        assert!(elbow.is_finite());
    }

    /// A pole pointing along the arm says nothing about the bend plane; any perpendicular
    /// is a valid answer, but NaN is not.
    #[test]
    fn a_pole_parallel_to_the_arm_still_gives_a_finite_elbow() {
        let elbow = solve_elbow(Vec3::ZERO, Vec3::new(1.2, 0.0, 0.0), 1.0, 1.0, Vec3::X);
        assert!(elbow.is_finite(), "got {elbow:?}");
        assert!((elbow.length() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn aiming_a_bone_turns_its_current_direction_onto_the_wanted_one() {
        let current = Quat::from_rotation_z(0.3);
        let aimed = aim_bone(current, Vec3::X, Vec3::Y);
        assert!((aimed * (current.inverse() * Vec3::X) - Vec3::Y).length() < 1e-4);
    }

    #[test]
    fn aiming_with_a_degenerate_direction_leaves_the_bone_alone() {
        let current = Quat::from_rotation_z(0.3);
        assert_eq!(aim_bone(current, Vec3::ZERO, Vec3::Y), current);
        assert_eq!(aim_bone(current, Vec3::X, Vec3::ZERO), current);
    }

    /// The grip on swat-2 is anchored to `thumb_02_r`, four bones below the shoulder.
    #[test]
    fn the_arm_is_found_above_the_wrist_however_deep_the_hardpoint_sits() {
        let ancestors = ["thumb_01_r", "hand_r", "lowerarm_r", "upperarm_r", "clavicle_r"];
        assert_eq!(arm_chain(&ancestors), Some((3, 2)));
    }

    /// Mixamo names differ but carry the same word.
    #[test]
    fn a_mixamo_rig_resolves_the_same_way() {
        let ancestors = ["mixamorigRightHand", "mixamorigRightForeArm", "mixamorigRightArm"];
        assert_eq!(arm_chain(&ancestors), Some((2, 1)));
    }

    /// A hardpoint on the hand itself has no "hand" ancestor to find.
    #[test]
    fn a_hardpoint_on_the_wrist_uses_the_two_bones_above_it() {
        let ancestors = ["lowerarm_r", "upperarm_r", "clavicle_r"];
        assert_eq!(arm_chain(&ancestors), Some((2, 1)));
    }

    /// The whole point of the clamp: an absolute alignment would wring the neck right
    /// round when the weapon points behind the character.
    #[test]
    fn a_turn_past_the_limit_stops_at_the_limit() {
        let from = Quat::IDENTITY;
        let to = Quat::from_rotation_y(std::f32::consts::PI);
        let limited = limited_turn(from, to, 55f32.to_radians());
        assert!(
            (limited.angle_between(from).to_degrees() - 55.0).abs() < 1e-2,
            "turned {} degrees",
            limited.angle_between(from).to_degrees()
        );
    }

    #[test]
    fn a_turn_within_the_limit_is_taken_in_full() {
        let from = Quat::IDENTITY;
        let to = Quat::from_rotation_y(0.3);
        assert!(limited_turn(from, to, 55f32.to_radians()).angle_between(to) < 1e-5);
    }

    #[test]
    fn turning_to_where_you_already_are_is_not_a_division_by_zero() {
        let same = Quat::from_rotation_y(0.4);
        assert!(limited_turn(same, same, 0.0).is_finite());
    }

    /// A hardpoint anchored somewhere with no arm above it must be refused, not solved
    /// against whatever bone happens to be there.
    #[test]
    fn a_chain_that_runs_out_of_bones_is_refused() {
        assert_eq!(arm_chain(&["hand_r"]), None);
        assert_eq!(arm_chain(&[]), None);
    }
}
