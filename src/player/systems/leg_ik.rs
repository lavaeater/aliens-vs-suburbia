//! Procedural legs: drive the feet from [`gait`] and solve the knees onto them.
//!
//! Full-procedural, not corrective. Whatever the walk clip does to the legs is overwritten
//! here — the same slot and the same reason as the torso twist, `PostUpdate` after
//! `AnimationSystems` and before `TransformSystems::Propagate`. The clip still owns the
//! rest of the body, so a rig with no usable walk cycle still walks.
//!
//! Three parts, and only the middle one is new work:
//!
//! 1. [`gait`] decides where the feet go in world space. Pure, tested separately.
//! 2. This module finds the two hip/knee/ankle chains and keeps the character's ground
//!    height honest.
//! 3. `arm_ik::solve_elbow` bends them, unchanged. A leg is a two-bone chain with a pole
//!    hint exactly like an arm; the only difference is which way the joint bends, which is
//!    one constant ([`KNEE_POLE`]).
//!
//! `F9` toggles it, matching `F7` for the twist and `F8` for hand alignment.

use bevy::prelude::*;

use crate::player::components::{Player, PlayerDead};
use crate::player::systems::arm_ik::{aim_bone, local_from_world, solve_elbow};
use crate::player::systems::gait::{Foot, GaitContext, GaitParams, GaitState};

/// The gait shape, as a tunable resource.
///
/// A wrapper rather than deriving `Resource` on `GaitParams` itself, which would drag
/// `bevy_ecs` into a module whose whole point is that it has no engine in it.
#[derive(Resource, Debug, Clone, Copy, Default, Deref, DerefMut)]
pub struct GaitSettings(pub GaitParams);

/// Knees bend forward, where elbows bend back. The character's forward is `-Z`, which is
/// the whole of the difference between solving a leg and solving an arm.
pub const KNEE_POLE: Vec3 = Vec3::NEG_Z;

/// Give up looking for legs after this many frames, as `resolve_twist_bones` does.
const RESOLVE_MAX_TRIES: u32 = 600;

/// Below this much movement in a frame, treat the character as standing: the direction of
/// travel is noise at that scale and would spin the plant targets around.
const MOVING_EPSILON: f32 = 1e-4;

/// One resolved leg.
#[derive(Clone, Copy, Debug)]
pub struct LegChain {
    /// Thigh — the bone the solve rotates at the hip.
    pub upper: Entity,
    /// Shin.
    pub lower: Entity,
    /// The ankle, whose world position is what the gait is aiming at.
    pub foot: Entity,
}

/// A character walking on procedural legs.
#[derive(Component)]
pub struct Legs {
    /// Indexed by [`Foot::index`].
    pub chains: [LegChain; 2],
    pub gait: GaitState,
    /// Last frame's world position, so the cycle can be advanced by distance travelled.
    last_position: Vec3,
    /// Foot height above the character's origin in the rig's rest pose.
    ///
    /// Self-calibrating, and it has to be: model roots sit at different heights above the
    /// floor depending on how a def was authored. Measuring the rig's own ankles once, at
    /// resolve time, gets the plant height right for any model without map knowledge or a
    /// per-def offset to keep in sync.
    ground_offset: f32,
}

/// Bone names still to be resolved, retried until the skeleton spawns. Mirrors
/// `PendingTorsoTwist`.
#[derive(Component, Default)]
pub struct PendingLegs {
    tries: u32,
}

/// Runtime switch, `F9`, so procedural legs can be A/B'd against the clip in place.
#[derive(Resource)]
pub struct LegIkEnabled(pub bool);

impl Default for LegIkEnabled {
    fn default() -> Self {
        Self(true)
    }
}

// ── Finding the legs ────────────────────────────────────────────────────────

/// Whether a bone name looks like the ankle of a leg.
///
/// Matched by shape rather than by a table of known rigs, the same way `arm_chain` finds
/// the elbow: mixamo says `mixamorigLeftFoot`, mesh2motion and UE-style rigs say `foot_l`,
/// and a table would need an entry per pack forever.
///
/// Toes are excluded deliberately — they contain neither "foot" nor "ankle" in either
/// convention, but an IK bone might, so those are excluded by name too.
#[must_use]
pub fn is_foot_bone(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.contains("ik") || lower.contains("toe") || lower.contains("ball") {
        return false;
    }
    lower.contains("foot") || lower.contains("ankle")
}

/// Which side of the body a bone name belongs to, or `None` if it says nothing.
///
/// Handles both conventions: a `left`/`right` word anywhere (mixamo), or an `_l`/`_r`
/// suffix, optionally followed by a numeric index (`foot_l`, `foot_l_01`).
#[must_use]
pub fn bone_side(name: &str) -> Option<Foot> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("left") {
        return Some(Foot::Left);
    }
    if lower.contains("right") {
        return Some(Foot::Right);
    }
    // Suffix form: split on separators and look for a bare "l" or "r" segment.
    let mut segments = lower.split(['_', '.', '-']).rev();
    // Skip a trailing numeric index, so `foot_l_01` still reads as left.
    let last = segments.find(|s| !s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty())?;
    match last {
        "l" => Some(Foot::Left),
        "r" => Some(Foot::Right),
        _ => None,
    }
}

/// Every descendant of `root`, breadth-first.
fn descendants(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut queue = vec![root];
    while let Some(entity) = queue.pop() {
        if let Ok(kids) = children.get(entity) {
            out.extend(kids.iter());
            queue.extend(kids.iter());
        }
    }
    out
}

/// Build a leg from its ankle: the shin and thigh are simply its two ancestors.
///
/// Ancestors rather than names, because the two bones above the ankle are the knee and hip
/// on every biped rig, whatever they are called.
fn chain_from_foot(foot: Entity, parents: &Query<&ChildOf>) -> Option<LegChain> {
    let lower = parents.get(foot).ok()?.parent();
    let upper = parents.get(lower).ok()?.parent();
    // The thigh must have a parent of its own (pelvis) for the solve to have a base.
    parents.get(upper).ok()?;
    Some(LegChain { upper, lower, foot })
}

/// Turn a spawned skeleton into two leg chains, once it exists.
#[allow(clippy::type_complexity)]
pub fn resolve_legs(
    mut commands: Commands,
    mut pending: Query<(Entity, &mut PendingLegs), (With<Player>, Without<Legs>)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    params: Res<GaitSettings>,
) {
    for (character, mut pending) in pending.iter_mut() {
        let mut found: [Option<Entity>; 2] = [None; 2];
        for bone in descendants(character, &children) {
            let Ok(name) = names.get(bone) else { continue };
            if !is_foot_bone(name.as_str()) {
                continue;
            }
            if let Some(side) = bone_side(name.as_str()) {
                // First match wins: rigs sometimes carry a second, deeper foot-ish bone.
                found[side.index()].get_or_insert(bone);
            }
        }

        let (Some(left), Some(right)) = (found[0], found[1]) else {
            pending.tries += 1;
            if pending.tries >= RESOLVE_MAX_TRIES {
                warn!("procedural legs disabled: no left/right foot bones on this rig");
                commands.entity(character).remove::<PendingLegs>();
            }
            continue;
        };

        // All-or-nothing, like the twist chain: one leg solved and one animated would look
        // far worse than neither.
        let (Some(left), Some(right)) =
            (chain_from_foot(left, &parents), chain_from_foot(right, &parents))
        else {
            pending.tries += 1;
            continue;
        };

        let Ok(body) = globals.get(character) else { continue };
        let Ok(foot_world) = globals.get(left.foot) else { continue };
        let ground_offset = foot_world.translation().y - body.translation().y;

        let hip_ground = body.translation().with_y(body.translation().y + ground_offset);
        let right_dir = body.rotation() * Vec3::X;

        commands
            .entity(character)
            .insert(Legs {
                chains: [left, right],
                gait: GaitState::standing(hip_ground, right_dir, &params),
                last_position: body.translation(),
                ground_offset,
            })
            .remove::<PendingLegs>();
    }
}

// ── Driving them ────────────────────────────────────────────────────────────

/// Step the gait and bend both legs onto its feet.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn apply_leg_ik(
    mut commands: Commands,
    time: Res<Time>,
    enabled: Res<LegIkEnabled>,
    params: Res<GaitSettings>,
    mut characters: Query<(Entity, &mut Legs), (With<Player>, Without<PlayerDead>)>,
    globals: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    if !enabled.0 {
        return;
    }
    let dt = time.delta_secs();

    for (character, mut legs) in characters.iter_mut() {
        // The chains hold bone entities, which die with the skeleton. A model swap or a
        // revive respawns it, so drop the stale chains and resolve again rather than
        // solving against entities that no longer exist.
        if legs.chains.iter().any(|c| globals.get(c.foot).is_err()) {
            commands
                .entity(character)
                .remove::<Legs>()
                .insert(PendingLegs::default());
            continue;
        }
        let Ok(body) = globals.get(character) else { continue };
        let position = body.translation();

        // `position` is last frame's world transform -- everything in this slot runs
        // before `Propagate`. Consistently one frame late on both samples, so the delta
        // it yields is still the distance actually travelled.
        let travel = position - legs.last_position;
        let flat = Vec3::new(travel.x, 0.0, travel.z);
        legs.last_position = position;

        // Direction of travel, not facing: the character strafes, and the feet should step
        // where the body is going rather than where the gun is pointing.
        let forward = flat
            .try_normalize()
            .unwrap_or_else(|| (body.rotation() * Vec3::NEG_Z).with_y(0.0).normalize_or(Vec3::NEG_Z));
        let right = Vec3::Y.cross(forward).normalize_or(body.rotation() * Vec3::X);

        let ground_y = position.y + legs.ground_offset;
        let ctx = GaitContext {
            hip_ground: position.with_y(ground_y),
            forward,
            right,
            ground_y,
            distance: if flat.length() > MOVING_EPSILON { flat.length() } else { 0.0 },
            dt,
        };

        // Read from the resource every frame rather than a snapshot taken at resolve, so
        // tuning the gait live moves the feet immediately.
        let targets = legs.gait.update(&ctx, &params);
        for foot in Foot::BOTH {
            let chain = legs.chains[foot.index()];
            solve_leg(&chain, targets[foot.index()], body, &parents, &globals, &mut transforms);
        }
    }
}

/// Bend one leg so its ankle reaches `target`.
///
/// The same two-bone solve as `arm_ik::solve_arm`, minus the hand alignment: a foot's
/// position is what plants it, and its orientation is left to the animation for now.
fn solve_leg(
    chain: &LegChain,
    target: Vec3,
    owner: &GlobalTransform,
    parents: &Query<&ChildOf>,
    globals: &Query<&GlobalTransform>,
    transforms: &mut Query<&mut Transform>,
) -> Option<()> {
    let base = parents.get(chain.upper).ok()?.parent();
    let base_world = *globals.get(base).ok()?;

    // Forward kinematics from this frame's animated locals. The bones' own globals are a
    // frame stale *and* carry last frame's correction, so using them would compound — the
    // same trap documented in `arm_ik`.
    let upper_world = base_world.mul_transform(*transforms.get(chain.upper).ok()?);
    let lower_world = upper_world.mul_transform(*transforms.get(chain.lower).ok()?);
    let foot_world = lower_world.mul_transform(*transforms.get(chain.foot).ok()?);

    let hip = upper_world.translation();
    let knee = lower_world.translation();
    let ankle = foot_world.translation();

    let upper_len = (knee - hip).length();
    let lower_len = (ankle - knee).length();
    if upper_len < 1e-5 || lower_len < 1e-5 {
        return None;
    }

    let pole = owner.rotation() * KNEE_POLE;
    let wanted_knee = solve_elbow(hip, target, upper_len, lower_len, pole);

    // Thigh: swing the hip so the knee lands where the solve wants it.
    let upper_rotation = aim_bone(upper_world.rotation(), knee - hip, wanted_knee - hip);
    let swing = upper_rotation * upper_world.rotation().inverse();

    // Everything below the hip is rigid under that swing, so the shin's new orientation
    // and the ankle's carried position follow without re-composing the chain.
    let lower_rotation_after_swing = swing * lower_world.rotation();
    let ankle_after_swing = wanted_knee + swing * (ankle - knee);

    // Shin: swing the knee so the ankle lands on the target.
    let lower_rotation = aim_bone(
        lower_rotation_after_swing,
        ankle_after_swing - wanted_knee,
        target - wanted_knee,
    );

    transforms.get_mut(chain.upper).ok()?.rotation =
        local_from_world(base_world.rotation(), upper_rotation);
    transforms.get_mut(chain.lower).ok()?.rotation =
        local_from_world(upper_rotation, lower_rotation);
    Some(())
}

/// F9 toggles procedural legs, so the clip's own walk can be compared against them.
pub fn toggle_leg_ik(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<LegIkEnabled>) {
    if keys.just_pressed(KeyCode::F9) {
        enabled.0 = !enabled.0;
        info!("procedural legs {}", if enabled.0 { "on" } else { "off" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foot_bones_are_recognised_across_rig_conventions() {
        assert!(is_foot_bone("mixamorigLeftFoot"));
        assert!(is_foot_bone("foot_r"));
        assert!(is_foot_bone("ankle_l"));
        assert!(is_foot_bone("Ankle.R"));
    }

    #[test]
    fn toes_and_ik_helpers_are_not_feet() {
        // A toe would give the solve a chain one bone too long; an IK helper bone is not
        // part of the deform skeleton at all.
        assert!(!is_foot_bone("mixamorigLeftToeBase"));
        assert!(!is_foot_bone("ball_l"));
        assert!(!is_foot_bone("ik_foot_l"));
        assert!(!is_foot_bone("mixamorigLeftHand"));
        assert!(!is_foot_bone("spine_01"));
    }

    #[test]
    fn sides_are_read_from_words_or_suffixes() {
        assert_eq!(bone_side("mixamorigLeftFoot"), Some(Foot::Left));
        assert_eq!(bone_side("mixamorigRightFoot"), Some(Foot::Right));
        assert_eq!(bone_side("foot_l"), Some(Foot::Left));
        assert_eq!(bone_side("foot_r"), Some(Foot::Right));
        assert_eq!(bone_side("foot.L"), Some(Foot::Left));
        assert_eq!(bone_side("thigh_r_01"), Some(Foot::Right), "a numeric index is skipped");
        assert_eq!(bone_side("pelvis"), None);
    }

    #[test]
    fn a_side_is_not_invented_from_an_unrelated_letter() {
        // "l" has to be its own segment: `spinal` and `armor` must not read as sides.
        assert_eq!(bone_side("spinal"), None);
        assert_eq!(bone_side("armor"), None);
    }
}
