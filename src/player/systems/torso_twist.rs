//! Torso twist (a.k.a. aim offset): the legs face where you walk, the upper body faces
//! where you aim.
//!
//! This is **forward** kinematics, not IK — we set one rotation per spine bone and
//! everything above it (chest, arms, hand, and the weapon parented to the grip bone by
//! `equip.rs`) follows for free. No solver, no pole vector. The support-hand IK in
//! `docs/inverse-kinematics-hardpoints.md` is a separate, harder problem.
//!
//! Two things make or break it:
//!
//! 1. **Rotate about world up, not the bone's own long axis.** Mixamo-style rigs give
//!    every bone an arbitrary local orientation, so a local-axis twist leans and rolls
//!    differently depending on the pose. We build the rotation in world space and
//!    convert it into the bone's parent space — see `twisted_local`.
//! 2. **Ordering.** Clip-driven bones are overwritten every frame by `animate_targets`
//!    inside `AnimationSystems`. The twist has to run after that and before
//!    `TransformSystems::Propagate`, or it is either stomped or never propagated.
//!
//! The twist is clamped; past the limit the body itself turns to take up the slack, so
//! you can't wind the torso around backwards.

use bevy::prelude::*;

use crate::assets::asset_definition::AimBone;
use crate::control::components::CharacterControl;
use crate::player::components::{AutoAim, Player, PlayerDead};

/// How far the torso may lead the hips, in degrees. Beyond this the body turns.
pub const TWIST_LIMIT_DEGREES: f32 = 60.0;

/// How fast the twist chases the target angle (per second, exponential smoothing).
/// Without this the torso snaps instantly when the aim jumps across the character.
const TWIST_RESPONSIVENESS: f32 = 18.0;

/// Fallback chain for mixamo-named rigs (mesh2motion exports these too), used when a
/// def doesn't list its own `aim_bones`. Weights rise up the spine so the bend
/// accumulates gradually instead of hinging at the waist.
pub fn default_aim_bones() -> Vec<AimBone> {
    vec![
        AimBone { bone: "mixamorigSpine".into(), weight: 0.2 },
        AimBone { bone: "mixamorigSpine1".into(), weight: 0.35 },
        AimBone { bone: "mixamorigSpine2".into(), weight: 0.45 },
    ]
}

/// Resolved twist chain for one character. Inserted by `resolve_twist_bones` once the
/// skeleton has actually spawned; `bones` holds entities under *this* character.
#[derive(Component)]
pub struct TorsoTwist {
    /// (bone entity, normalized share of the total twist).
    pub bones: Vec<(Entity, f32)>,
    /// Current smoothed twist in radians, so the torso eases rather than snaps.
    pub current: f32,
}

/// Names to resolve, carried from the def until the skeleton exists. Mirrors the
/// `PendingEquip` pattern in `equip.rs` — scenes load asynchronously, so we retry.
#[derive(Component)]
pub struct PendingTorsoTwist {
    pub bones: Vec<AimBone>,
    tries: u32,
}

impl PendingTorsoTwist {
    pub fn new(bones: Vec<AimBone>) -> Self {
        let bones = if bones.is_empty() { default_aim_bones() } else { bones };
        Self { bones, tries: 0 }
    }
}

/// Give up looking for the spine after this many frames (same budget as equipping).
const RESOLVE_MAX_TRIES: u32 = 600;

/// Runtime switch so the effect can be A/B'd in place — F7 toggles it.
#[derive(Resource)]
pub struct TorsoTwistEnabled(pub bool);

impl Default for TorsoTwistEnabled {
    fn default() -> Self {
        Self(true)
    }
}

// ── The math ────────────────────────────────────────────────────────────────

/// Apply `yaw` (radians, about world up) on top of `local_anim` — the local rotation the
/// animation just wrote — expressed in the bone's parent space so it can be assigned
/// straight back to `Transform::rotation`.
///
/// Conjugating the world twist by the parent's world rotation turns "rotate about world
/// up" into the equivalent rotation in the parent's frame, which then pre-multiplies the
/// animated pose. Two properties this buys us:
///
/// - **The animation survives.** We modify `local_anim` rather than replacing it, so the
///   clip still drives the spine and we only add the offset.
/// - **Stale parent data is harmless.** When this runs, `GlobalTransform`s are still last
///   frame's, and a parent that is itself twisted carries last frame's offset. Because
///   rotations about the same axis commute, that offset cancels in the conjugation — so
///   the twist neither accumulates frame over frame nor drifts down the chain.
pub fn twisted_local(parent_world: Quat, local_anim: Quat, yaw: f32) -> Quat {
    let twist_in_parent_space = parent_world.inverse() * Quat::from_rotation_y(yaw) * parent_world;
    (twist_in_parent_space * local_anim).normalize()
}

/// The horizontal-plane yaw that turns `forward` onto `aim`: the angle `t` for which
/// `Quat::from_rotation_y(t) * forward` points along `aim`. Both are flattened first;
/// `None` if either has no heading once flattened.
///
/// Sign follows the right-hand rule about +Y, so facing -Z and aiming +X is *negative*
/// (a right turn). Callers feed it straight into `from_rotation_y` or an angular
/// velocity about Y, so the convention only has to agree with itself.
pub fn yaw_between(forward: Vec3, aim: Vec3) -> Option<f32> {
    let f = Vec3::new(forward.x, 0.0, forward.z);
    let a = Vec3::new(aim.x, 0.0, aim.z);
    if f.length_squared() < 1e-6 || a.length_squared() < 1e-6 {
        return None;
    }
    let (f, a) = (f.normalize(), a.normalize());
    // cross(f, a).y is sin of the angle, dot is cos — atan2 of the pair is the signed angle.
    Some((f.z * a.x - f.x * a.z).atan2(f.dot(a)))
}

/// Split a total twist across the chain by weight, normalizing whatever weights the
/// def supplied so they always sum to the full angle.
pub fn normalized_weights(bones: &[AimBone]) -> Vec<f32> {
    let total: f32 = bones.iter().map(|b| b.weight.max(0.0)).sum();
    if total <= f32::EPSILON {
        // Degenerate config (all zero / negative): spread it evenly rather than
        // silently doing nothing.
        return vec![1.0 / bones.len().max(1) as f32; bones.len()];
    }
    bones.iter().map(|b| b.weight.max(0.0) / total).collect()
}

// ── Systems ─────────────────────────────────────────────────────────────────

/// Breadth-first search for a named descendant, so bones of *this* character are found
/// even when several players share a skeleton's bone names.
fn find_descendant_named(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<Entity> {
    let mut queue = vec![root];
    while let Some(entity) = queue.pop() {
        if names.get(entity).is_ok_and(|n| n.as_str() == name) {
            return Some(entity);
        }
        if let Ok(kids) = children.get(entity) {
            queue.extend(kids.iter());
        }
    }
    None
}

/// Turn the def's bone names into entities once the skeleton has spawned.
pub fn resolve_twist_bones(
    mut commands: Commands,
    mut pending: Query<(Entity, &mut PendingTorsoTwist), Without<TorsoTwist>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (character, mut pending) in pending.iter_mut() {
        let resolved: Vec<Option<Entity>> = pending
            .bones
            .iter()
            .map(|b| find_descendant_named(character, &b.bone, &children, &names))
            .collect();

        // All-or-nothing: a half-resolved chain would twist unevenly.
        if resolved.iter().any(|e| e.is_none()) {
            pending.tries += 1;
            if pending.tries >= RESOLVE_MAX_TRIES {
                let missing: Vec<&str> = pending
                    .bones
                    .iter()
                    .zip(&resolved)
                    .filter(|(_, e)| e.is_none())
                    .map(|(b, _)| b.bone.as_str())
                    .collect();
                warn!("torso twist disabled: bones {missing:?} never appeared on this rig");
                commands.entity(character).remove::<PendingTorsoTwist>();
            }
            continue;
        }

        let weights = normalized_weights(&pending.bones);
        let bones = resolved
            .into_iter()
            .map(|e| e.expect("checked above"))
            .zip(weights)
            .collect();

        commands
            .entity(character)
            .insert(TorsoTwist { bones, current: 0.0 })
            .remove::<PendingTorsoTwist>();
    }
}

/// Rotate the spine so the upper body leads the hips toward `AutoAim`.
///
/// Must run after `AnimationSystems` (or the clip overwrites us) and before
/// `TransformSystems::Propagate` (or nothing downstream sees it).
#[allow(clippy::type_complexity)]
pub fn apply_torso_twist(
    time: Res<Time>,
    enabled: Res<TorsoTwistEnabled>,
    mut characters: Query<(&Transform, &AutoAim, &mut TorsoTwist), (With<Player>, Without<PlayerDead>)>,
    global_transforms: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    mut bone_transforms: Query<&mut Transform, Without<Player>>,
) {
    let dt = time.delta_secs();

    for (body, aim, mut twist) in characters.iter_mut() {
        // Where the hips point vs. where we want to shoot. The body is steered toward
        // the walk direction, so this difference is exactly the offset the torso covers.
        let forward = body.rotation * Vec3::NEG_Z;
        let target = if enabled.0 {
            let limit = TWIST_LIMIT_DEGREES.to_radians();
            yaw_between(forward, aim.0).unwrap_or(0.0).clamp(-limit, limit)
        } else {
            // Ease back to neutral when switched off rather than popping.
            0.0
        };

        // Exponential smoothing, frame-rate independent.
        twist.current += (target - twist.current) * (1.0 - (-TWIST_RESPONSIVENESS * dt).exp());

        for &(bone, share) in &twist.bones {
            let Ok(parent) = parents.get(bone) else { continue };
            let Ok(parent_world) = global_transforms.get(parent.parent()) else { continue };
            let Ok(mut local) = bone_transforms.get_mut(bone) else { continue };

            // `local.rotation` is this frame's animated pose (we run right after
            // `animate_targets`); `parent_world` is last frame's, which is fine — see
            // `twisted_local`.
            local.rotation = twisted_local(
                parent_world.rotation(),
                local.rotation,
                twist.current * share,
            );
        }
    }
}

/// F7 toggles the effect so it can be compared against the old whole-body turn.
pub fn toggle_torso_twist(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<TorsoTwistEnabled>) {
    if keys.just_pressed(KeyCode::F7) {
        enabled.0 = !enabled.0;
        info!("torso twist {}", if enabled.0 { "on" } else { "off" });
    }
}

/// Steer the body toward the direction of travel. The torso then covers the rest of the
/// way to the aim, which is the whole point of the split: legs walk where you're going,
/// gun points where you're looking.
///
/// When the aim is further off than the torso can twist, the body absorbs the excess so
/// the character can't wind up facing backwards. Standing still, the body holds its
/// facing and only the torso moves.
#[allow(clippy::type_complexity)]
pub fn face_movement_direction(
    mut players: Query<
        (&Transform, &mut avian3d::prelude::AngularVelocity, &AutoAim, &CharacterControl),
        (With<Player>, Without<PlayerDead>),
    >,
) {
    let limit = TWIST_LIMIT_DEGREES.to_radians();

    for (transform, mut angular, aim, control) in players.iter_mut() {
        let forward = transform.rotation * Vec3::NEG_Z;
        let walking = control.walk_direction.length_squared() > 0.01;

        // Prefer the direction of travel; when standing still only correct if the aim
        // has run past what the torso can cover.
        let goal = if walking {
            control.walk_direction
        } else {
            match yaw_between(forward, aim.0) {
                Some(off) if off.abs() > limit => aim.0,
                _ => {
                    angular.0.y = 0.0;
                    continue;
                }
            }
        };

        let Some(error) = yaw_between(forward, goal) else {
            angular.0.y = 0.0;
            continue;
        };
        let max = control.max_turn_speed.max(1.0);
        angular.0.y = (error * 8.0).clamp(-max, max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// Compare rotations by what they do to the basis vectors. `Quat::angle_between`
    /// uses an approximate `acos`, which amplifies float noise badly near zero — it
    /// reports ~1e-3 rad for quaternions whose dot product is 0.9999999.
    fn assert_same_rotation(a: Quat, b: Quat, msg: &str) {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let (ra, rb) = (a * axis, b * axis);
            assert!((ra - rb).length() < 1e-5, "{msg}: {axis:?} -> {ra:?} vs {rb:?}");
        }
    }

    #[test]
    fn twisting_by_zero_leaves_the_animated_pose_alone() {
        let parent = Quat::from_rotation_y(0.7);
        let local = Quat::from_rotation_x(0.3);
        assert_same_rotation(twisted_local(parent, local, 0.0), local, "no twist, no change");
    }

    #[test]
    fn the_twist_lands_in_world_space_whatever_the_bone_rest_orientation() {
        // A bone whose local axes are nothing like the world's — the mixamo case that
        // makes a naive local-axis twist lean and roll.
        let parent = Quat::from_euler(EulerRot::XYZ, 0.4, -1.1, 2.0);
        let local = Quat::from_euler(EulerRot::XYZ, -0.9, 0.2, 1.3);

        // Feeding the result back through the parent must reproduce the bone's world
        // orientation with exactly a 90 degree world-Y rotation applied on the world side.
        let out_world = parent * twisted_local(parent, local, FRAC_PI_2);
        assert_same_rotation(
            out_world,
            Quat::from_rotation_y(FRAC_PI_2) * (parent * local),
            "twist must be about world up, not the bone's own axis",
        );
    }

    #[test]
    fn a_twist_of_the_torso_actually_swings_the_bone_forward_vector() {
        // The gameplay-visible property: whatever the rig, twisting by the angle that
        // takes the body's forward onto the aim makes the bone's forward land on the aim.
        let parent = Quat::from_euler(EulerRot::XYZ, 0.2, 1.4, -0.6);
        let local = Quat::from_euler(EulerRot::XYZ, 0.5, -0.3, 0.9);

        let (forward, aim) = (Vec3::NEG_Z, Vec3::X);
        let yaw = yaw_between(forward, aim).expect("well-defined");

        let before = parent * twisted_local(parent, local, 0.0) * Vec3::NEG_Z;
        let after = parent * twisted_local(parent, local, yaw) * Vec3::NEG_Z;
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize();
        // The bone's heading rotates by the same yaw the body would have turned.
        let expected = Quat::from_rotation_y(yaw) * flat(before);
        assert!((flat(after) - flat(expected)).length() < 1e-5, "torso heading must follow the aim");
    }

    #[test]
    fn holding_a_steady_twist_does_not_accumulate_over_frames() {
        // The bug this guards: deriving the twist from the bone's *world* rotation reuses
        // last frame's already-twisted pose, so a constant aim winds the torso further
        // every frame. Here we simulate frames -- the animation rewrites `local` each
        // time, and the parent carries the previous frame's twist -- and the result must
        // stay put.
        let hips = Quat::from_euler(EulerRot::XYZ, 0.1, 0.8, -0.2);
        let local_anim = Quat::from_euler(EulerRot::XYZ, 0.3, -0.4, 0.15);
        let yaw = 0.5;

        let first = twisted_local(hips, local_anim, yaw);
        let mut settled = first;
        for _ in 0..10 {
            settled = twisted_local(hips, local_anim, yaw);
        }
        assert_same_rotation(settled, first, "a steady aim must hold a steady twist");

        // And a twisted parent must not change the answer, since Y rotations commute.
        let via_twisted_parent = twisted_local(Quat::from_rotation_y(0.9) * hips, local_anim, yaw);
        assert_same_rotation(
            via_twisted_parent,
            first,
            "a parent carrying last frame's twist must not shift this bone",
        );
    }

    #[test]
    fn yaw_between_turns_forward_onto_aim() {
        // Round-trip the definition: rotating `forward` by the result gives `aim`.
        for (forward, aim) in [
            (Vec3::NEG_Z, Vec3::X),
            (Vec3::NEG_Z, Vec3::NEG_X),
            (Vec3::X, Vec3::Z),
            (Vec3::new(0.6, 0.0, -0.8), Vec3::new(-0.28, 0.0, -0.96)),
        ] {
            let yaw = yaw_between(forward, aim).expect("well-defined");
            let turned = Quat::from_rotation_y(yaw) * forward.normalize();
            assert!(
                (turned - aim.normalize()).length() < 1e-5,
                "rotating {forward:?} by {} deg should give {aim:?}, got {turned:?}",
                yaw.to_degrees()
            );
        }
    }

    #[test]
    fn yaw_between_is_signed_by_the_right_hand_rule_about_up() {
        // Facing up the screen (-Z) and aiming right (+X) is a right turn: negative.
        let yaw = yaw_between(Vec3::NEG_Z, Vec3::X).expect("well-defined");
        assert!((yaw + FRAC_PI_2).abs() < 1e-5, "expected -90 deg, got {}", yaw.to_degrees());

        let yaw = yaw_between(Vec3::NEG_Z, Vec3::NEG_X).expect("well-defined");
        assert!((yaw - FRAC_PI_2).abs() < 1e-5, "expected +90 deg, got {}", yaw.to_degrees());
    }

    #[test]
    fn yaw_between_ignores_height_and_rejects_degenerate_input() {
        let yaw = yaw_between(Vec3::new(0.0, 5.0, -1.0), Vec3::new(1.0, -3.0, 0.0))
            .expect("flattens to well-defined vectors");
        assert!((yaw + FRAC_PI_2).abs() < 1e-5, "vertical components must not matter");

        assert!(yaw_between(Vec3::Y, Vec3::X).is_none(), "straight up has no heading");
        assert!(yaw_between(Vec3::NEG_Z, Vec3::ZERO).is_none(), "no aim, no angle");
    }

    #[test]
    fn weights_are_normalized_to_sum_to_one() {
        let bones = vec![
            AimBone { bone: "a".into(), weight: 2.0 },
            AimBone { bone: "b".into(), weight: 3.0 },
            AimBone { bone: "c".into(), weight: 5.0 },
        ];
        let w = normalized_weights(&bones);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6, "shares must total the whole twist");
        assert!((w[2] - 0.5).abs() < 1e-6, "proportions are preserved");
    }

    #[test]
    fn degenerate_weights_fall_back_to_an_even_split() {
        let bones = vec![
            AimBone { bone: "a".into(), weight: 0.0 },
            AimBone { bone: "b".into(), weight: 0.0 },
        ];
        let w = normalized_weights(&bones);
        assert_eq!(w, vec![0.5, 0.5], "a zeroed config still twists rather than doing nothing");
    }

    // ── ECS wiring ──────────────────────────────────────────────────────────
    //
    // The pure functions above cover the math; these cover the parts that only break
    // in a real world: finding bones under the right character, and actually writing a
    // rotation into the skeleton.

    /// A character with a `Hips -> Spine -> Spine1 -> Spine2` chain, like the amy rig.
    fn spawn_rig(app: &mut App, aim: Vec3) -> (Entity, Vec<Entity>) {
        let character = app
            .world_mut()
            .spawn((Player, AutoAim(aim), Transform::default(), CharacterControl::new(1.0, 1.0, 60.0)))
            .id();

        let mut parent = character;
        let mut bones = Vec::new();
        for name in ["mixamorigHips", "mixamorigSpine", "mixamorigSpine1", "mixamorigSpine2"] {
            let bone = app
                .world_mut()
                .spawn((Name::new(name), Transform::default(), GlobalTransform::default()))
                .id();
            app.world_mut().entity_mut(parent).add_child(bone);
            parent = bone;
            bones.push(bone);
        }
        (character, bones)
    }

    #[test]
    fn the_spine_chain_resolves_to_entities_under_this_character() {
        let mut app = App::new();
        app.add_systems(Update, resolve_twist_bones);
        let (character, bones) = spawn_rig(&mut app, Vec3::NEG_Z);
        app.world_mut()
            .entity_mut(character)
            .insert(PendingTorsoTwist::new(Vec::new()));

        app.update();

        let twist = app.world().entity(character).get::<TorsoTwist>().expect("chain resolved");
        // bones[0] is Hips, which is not part of the twist chain.
        let resolved: Vec<Entity> = twist.bones.iter().map(|(e, _)| *e).collect();
        assert_eq!(resolved, bones[1..].to_vec(), "Spine/Spine1/Spine2, in order");
        assert!(
            app.world().entity(character).get::<PendingTorsoTwist>().is_none(),
            "the pending marker is cleared once resolved"
        );
    }

    #[test]
    fn a_second_character_gets_its_own_bones_not_the_first_ones() {
        // Bone names repeat across players, so the search must stay under one character.
        let mut app = App::new();
        app.add_systems(Update, resolve_twist_bones);
        let (a, a_bones) = spawn_rig(&mut app, Vec3::NEG_Z);
        let (b, b_bones) = spawn_rig(&mut app, Vec3::NEG_Z);
        for c in [a, b] {
            app.world_mut().entity_mut(c).insert(PendingTorsoTwist::new(Vec::new()));
        }

        app.update();

        let got = |c: Entity, app: &App| -> Vec<Entity> {
            app.world().entity(c).get::<TorsoTwist>().unwrap().bones.iter().map(|(e, _)| *e).collect()
        };
        assert_eq!(got(a, &app), a_bones[1..].to_vec());
        assert_eq!(got(b, &app), b_bones[1..].to_vec());
    }

    #[test]
    fn an_unknown_rig_gives_up_and_says_so_instead_of_retrying_forever() {
        let mut app = App::new();
        app.add_systems(Update, resolve_twist_bones);
        let (character, _) = spawn_rig(&mut app, Vec3::NEG_Z);
        app.world_mut().entity_mut(character).insert(PendingTorsoTwist::new(vec![AimBone {
            bone: "no_such_bone".into(),
            weight: 1.0,
        }]));

        for _ in 0..RESOLVE_MAX_TRIES {
            app.update();
        }

        let e = app.world().entity(character);
        assert!(e.get::<TorsoTwist>().is_none(), "nothing to twist");
        assert!(e.get::<PendingTorsoTwist>().is_none(), "stopped retrying");
    }

    /// Step the app with a fixed 16 ms tick. A headless `App` otherwise advances the
    /// clock by real elapsed microseconds, which is far too little for the smoothing to
    /// converge and makes the test frame-rate dependent.
    fn tick(app: &mut App, frames: u32) {
        for _ in 0..frames {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }
    }

    #[test]
    fn the_spine_turns_toward_the_aim_and_settles_at_the_clamp() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<TorsoTwistEnabled>();
        app.add_systems(Update, (resolve_twist_bones, apply_torso_twist).chain());

        // Body faces -Z, aim is 90 degrees to the right of that -- past the 60 degree
        // clamp, so the twist should saturate there rather than following all the way.
        let (character, bones) = spawn_rig(&mut app, Vec3::X);
        app.world_mut()
            .entity_mut(character)
            .insert(PendingTorsoTwist::new(Vec::new()));

        // Two seconds of smoothing is ample for an 18/s response.
        tick(&mut app, 125);

        let total = app.world().entity(character).get::<TorsoTwist>().unwrap().current;
        assert!(
            (total.abs().to_degrees() - TWIST_LIMIT_DEGREES).abs() < 1.0,
            "twist should sit at the clamp, got {} deg",
            total.to_degrees()
        );
        // Aiming +X from a -Z facing is a right turn, i.e. negative (see yaw_between).
        assert!(total < 0.0, "twist must go the same way as the aim");

        // And the rotation actually reached the bones, split by weight.
        let yaw_of = |e: Entity, app: &App| {
            let r = app.world().entity(e).get::<Transform>().unwrap().rotation;
            (r * Vec3::NEG_Z).x.asin()
        };
        let spine = yaw_of(bones[1], &app).abs();
        let spine2 = yaw_of(bones[3], &app).abs();
        assert!(spine > 1e-3, "the first spine bone moved");
        assert!(spine2 > spine, "the upper spine takes the larger share");
    }

    #[test]
    fn a_disabled_twist_returns_the_spine_to_neutral() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(TorsoTwistEnabled(false));
        app.add_systems(Update, (resolve_twist_bones, apply_torso_twist).chain());

        let (character, bones) = spawn_rig(&mut app, Vec3::X);
        app.world_mut()
            .entity_mut(character)
            .insert(PendingTorsoTwist::new(Vec::new()));
        tick(&mut app, 125);

        let rot = app.world().entity(bones[3]).get::<Transform>().unwrap().rotation;
        assert!((rot * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-3, "spine stays neutral when off");
    }

    #[test]
    fn the_default_chain_spreads_the_twist_over_three_spine_bones() {
        let bones = default_aim_bones();
        assert_eq!(bones.len(), 3);
        let w = normalized_weights(&bones);
        assert!(w[0] < w[1] && w[1] < w[2], "the bend should accumulate up the spine");
    }
}
