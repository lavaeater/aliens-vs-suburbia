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
use crate::player::systems::gait::{horizontal_reach, Foot, GaitContext, GaitParams, GaitState};

/// The gait shape, as a tunable resource.
///
/// A wrapper rather than deriving `Resource` on `GaitParams` itself, which would drag
/// `bevy_ecs` into a module whose whole point is that it has no engine in it.
#[derive(Resource, Debug, Clone, Copy, Default, Deref, DerefMut)]
pub struct GaitSettings(pub GaitParams);

/// Knees bend forward, where elbows bend back. The character's forward is `-Z`, which is
/// the whole of the difference between solving a leg and solving an arm.
pub const KNEE_POLE: Vec3 = Vec3::NEG_Z;

/// Hip-to-ankle on the human the default [`GaitParams`] describe.
///
/// The gait defaults are stated in metres because that is the only way to state them
/// concretely, but what they really describe is a set of proportions. Dividing a rig's own
/// leg length by this recovers those proportions for a character of any size — which
/// matters here, where the player is a 0.25-scale model whose legs are 20 cm long and a
/// literal 1.6 m stride would plant its feet five leg-lengths in front of it.
const REFERENCE_LEG_LENGTH: f32 = 0.85;

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
    /// Set on the first frame the legs are driven, not at resolve.
    ///
    /// Where the feet stand depends on the ground and on the character's size, and neither
    /// is known reliably at resolve — the model may not have been scaled yet. Starting the
    /// stance with the wrong numbers puts the first plants out of the legs' reach, and a
    /// character standing still never takes the step that would replace them.
    pub gait: Option<GaitState>,
    /// Last frame's world position, so the cycle can be advanced by distance travelled.
    last_position: Vec3,
    /// The character's own child that the skeleton hangs from — the model's origin, which
    /// on a standing rig is the floor it stands on.
    ///
    /// The ground is read from this entity live, every frame, rather than measured once.
    /// `fix_scene_transform` writes the model's real scale and offset in `Update`, and
    /// nothing orders it against [`resolve_legs`] — an unchained `add_systems` tuple is not
    /// a sequence — so any measurement taken at resolve is a coin flip on whether the model
    /// had been sized yet. Reading it live also means the playground's scale slider moves
    /// the feet with the model instead of leaving them behind.
    model_root: Entity,
    /// Ankle height above the model's origin, in the model's own units.
    ///
    /// Measured within the model, so the model root's own scale and offset — the parts that
    /// may not be set yet — cancel out. Multiplied by the live scale when used.
    foot_lift: f32,
    /// Hip-to-ankle in the model's own units, for scaling the gait. See
    /// [`GaitParams::scaled`].
    leg_length: f32,
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

/// Where `bone` sits relative to `root`, composed from local transforms alone.
///
/// Deliberately not `GlobalTransform`. Everything in this file runs before
/// `TransformSystems::Propagate`, so a global is a frame stale — and on the one frame that
/// matters here, the frame the skeleton first becomes findable, it is worse than stale:
/// `fix_scene_transform` has just written the model root's real scale and offset into its
/// *local* transform, and the global still holds the unscaled identity it spawned with.
/// Measuring the rig against that gives a ground offset from a character four times the
/// size of the one on screen, with the feet up around its head.
///
/// Composing the locals also drops out the character's own world transform, which is what
/// we want: the answer is an offset within the character, not a place in the world.
fn offset_within(bone: Entity, root: Entity, parents: &Query<&ChildOf>, transforms: &Query<&Transform>) -> Option<Transform> {
    let mut chain = Vec::new();
    let mut current = bone;
    while current != root {
        chain.push(current);
        current = parents.get(current).ok()?.parent();
    }
    let mut out = Transform::IDENTITY;
    for entity in chain.iter().rev() {
        out = out * *transforms.get(*entity).ok()?;
    }
    Some(out)
}

/// The ancestor of `bone` that is a direct child of `character` — the scene's own root.
fn child_of_character(bone: Entity, character: Entity, parents: &Query<&ChildOf>) -> Option<Entity> {
    let mut current = bone;
    loop {
        let parent = parents.get(current).ok()?.parent();
        if parent == character {
            return Some(current);
        }
        current = parent;
    }
}

/// Turn a spawned skeleton into two leg chains, once it exists.
#[allow(clippy::type_complexity)]
pub fn resolve_legs(
    mut commands: Commands,
    mut pending: Query<(Entity, &mut PendingLegs), (With<Player>, Without<Legs>)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    transforms: Query<&Transform>,
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

        // Measure the rig inside the model, where the model root's own transform cancels
        // out: how far above the model's origin the ankles rest, and how long a leg is.
        let Some(model_root) = child_of_character(left.foot, character, &parents) else {
            pending.tries += 1;
            continue;
        };
        let (Some(hip), Some(knee), Some(ankle)) = (
            offset_within(left.upper, model_root, &parents, &transforms),
            offset_within(left.lower, model_root, &parents, &transforms),
            offset_within(left.foot, model_root, &parents, &transforms),
        ) else {
            pending.tries += 1;
            continue;
        };

        let foot_lift = ankle.translation.y;
        let leg_length = hip.translation.distance(knee.translation)
            + knee.translation.distance(ankle.translation);

        // The ankles have to be below the hips. If they are not, the rig is not posed yet;
        // retry rather than plant the feet somewhere the legs have to reach up to.
        if leg_length < 1e-4 || foot_lift >= hip.translation.y {
            pending.tries += 1;
            if pending.tries >= RESOLVE_MAX_TRIES {
                warn!("procedural legs disabled: this rig's ankles never settled below its hips");
                commands.entity(character).remove::<PendingLegs>();
            }
            continue;
        }

        let Ok(body) = transforms.get(character) else { continue };

        commands
            .entity(character)
            .insert(Legs {
                chains: [left, right],
                gait: None,
                last_position: body.translation,
                model_root,
                foot_lift,
                leg_length,
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
        if legs.chains.iter().any(|c| globals.get(c.foot).is_err())
            || globals.get(legs.model_root).is_err()
        {
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

        // The ground under the character, read from the model itself: the model's origin is
        // the floor it was authored standing on, and the ankles rest a little above that.
        // Live rather than latched, so the feet follow the model when it is rescaled and,
        // more to the point, when it is dropped onto the map at spawn.
        let Ok(model) = globals.get(legs.model_root) else { continue };
        let model_scale = model.scale().y;
        let ground_y = model.translation().y + legs.foot_lift * model_scale;
        let gait_scale = legs.leg_length * model_scale / REFERENCE_LEG_LENGTH;

        // How much stride the legs can actually pay for, from where the hips are riding this
        // frame. Live, because the animation moves the pelvis: a crouch shortens the steps
        // by itself, without a special case.
        let leg_world = legs.leg_length * model_scale;
        let hip_height = globals
            .get(legs.chains[0].upper)
            .map_or(leg_world, |hip| hip.translation().y - ground_y);

        let ctx = GaitContext {
            hip_ground: position.with_y(ground_y),
            forward,
            right,
            ground_y,
            reach: horizontal_reach(leg_world, hip_height),
            distance: if flat.length() > MOVING_EPSILON { flat.length() } else { 0.0 },
            dt,
        };

        // Read from the resource every frame rather than a snapshot taken at resolve, so
        // tuning the gait live moves the feet immediately -- scaled to this rig, so the
        // slider still reads in human metres whatever size the character is.
        let scaled = params.0.scaled(gait_scale).fit_to_reach(leg_world, hip_height);
        let gait = legs
            .gait
            .get_or_insert_with(|| GaitState::standing(ctx.hip_ground, ctx.right, &scaled));
        let targets = gait.update(&ctx, &scaled);
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

/// The solve driven through a real ECS world.
///
/// The unit tests above cover naming, and `gait`'s cover the footfalls, but neither can
/// catch a leg pointing at the sky: that only shows up once the targets, the chain and
/// transform propagation are in the same world together. A window would show it too, but
/// this runs in a second and stays as a regression test.
#[cfg(test)]
mod world_tests {
    use super::*;
    use bevy::app::App;
    use bevy::transform::TransformPlugin;
    use bevy::MinimalPlugins;

    /// The game's own player, to scale: a character whose origin sits in the middle of its
    /// collider, wearing a model root that `fix_scene_transform` shrinks to a quarter size,
    /// pushes down and spins to face the other way (`player-settings.ron`). Bones hang down
    /// the `-Y` axis, as a rig's do.
    struct Rig {
        app: App,
        character: Entity,
        model: Entity,
        hip: Entity,
        feet: [Entity; 2],
    }

    const BODY_Y: f32 = 0.5;
    const MODEL_SCALE: f32 = 0.25;
    const MODEL_DROP: f32 = -0.25;
    // Heights within the model, taken from swat-2's own skeleton (`packs/mesh2motion`).
    // The proportions matter: the rig stands with its knees slightly bent, so hip-to-ankle
    // is about 9% shorter than the leg, and that slack is what lets a foot reach out to the
    // side of the hip and still touch the floor.
    const PELVIS_IN_MODEL: f32 = 0.660;
    const HIP_IN_MODEL: f32 = 0.676;
    const KNEE_IN_MODEL: f32 = 0.385;
    const ANKLE_IN_MODEL: f32 = 0.073;
    /// Where the ankles rest relative to the character's origin.
    const ANKLE_REST: f32 = MODEL_DROP + MODEL_SCALE * ANKLE_IN_MODEL;

    /// Stand in for `fix_scene_transform`: it writes the model root's real scale and offset
    /// into the *local* transform, in `Update`, ahead of `resolve_legs` — which is what left
    /// the `GlobalTransform` a frame behind and a size wrong on the one frame the legs
    /// resolve.
    #[derive(Component)]
    struct NeedsFixing;

    fn fix_model_root(
        mut commands: Commands,
        mut roots: Query<(Entity, &mut Transform), With<NeedsFixing>>,
    ) {
        for (entity, mut transform) in roots.iter_mut() {
            transform.translation.y = MODEL_DROP;
            transform.scale = Vec3::splat(MODEL_SCALE);
            transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
            commands.entity(entity).remove::<NeedsFixing>();
        }
    }

    fn rig() -> Rig {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.init_resource::<GaitSettings>()
            .init_resource::<LegIkEnabled>();
        app.add_systems(Update, (fix_model_root, resolve_legs).chain());
        app.add_systems(
            PostUpdate,
            apply_leg_ik.before(bevy::transform::TransformSystems::Propagate),
        );

        let world = app.world_mut();
        let character = world
            .spawn((
                Player,
                PendingLegs::default(),
                Transform::from_xyz(0.0, BODY_Y, 0.0),
                Visibility::default(),
            ))
            .id();
        // Spawned unfixed, exactly as the scene arrives: full size, no offset.
        let model = world
            .spawn((Name::new("ModelRoot"), Transform::IDENTITY, NeedsFixing))
            .insert(ChildOf(character))
            .id();
        let pelvis = world
            .spawn((Name::new("pelvis"), Transform::from_xyz(0.0, PELVIS_IN_MODEL, 0.0)))
            .insert(ChildOf(model))
            .id();

        let mut feet = [Entity::PLACEHOLDER; 2];
        for foot in Foot::BOTH {
            let side = if foot == Foot::Left { "l" } else { "r" };
            let x = foot.lateral_sign() * 0.07;
            let thigh = world
                .spawn((
                    Name::new(format!("thigh_{side}")),
                    Transform::from_xyz(x, HIP_IN_MODEL - PELVIS_IN_MODEL, 0.0),
                ))
                .insert(ChildOf(pelvis))
                .id();
            // Knees forward, so the leg has slack. A rig posed with dead-straight legs is
            // already at full extension and can never quite reach the ground once the
            // stance puts the foot off to the side of the hip.
            let shin = world
                .spawn((
                    Name::new(format!("calf_{side}")),
                    Transform::from_xyz(0.0, KNEE_IN_MODEL - HIP_IN_MODEL, 0.135),
                ))
                .insert(ChildOf(thigh))
                .id();
            feet[foot.index()] = world
                .spawn((
                    Name::new(format!("foot_{side}")),
                    Transform::from_xyz(0.0, ANKLE_IN_MODEL - KNEE_IN_MODEL, -0.135),
                ))
                .insert(ChildOf(shin))
                .id();
        }

        // One tick, deliberately: the legs must resolve on the very frame the model root is
        // fixed, which is the frame whose globals still say otherwise.
        app.update();
        Rig { app, character, model, hip: pelvis, feet }
    }

    impl Rig {
        fn world_y(&self, entity: Entity) -> f32 {
            self.app
                .world()
                .entity(entity)
                .get::<GlobalTransform>()
                .expect("propagated")
                .translation()
                .y
        }

        /// Walk forward `steps` frames of `per_step` metres each.
        fn walk(&mut self, steps: u32, per_step: f32) {
            for _ in 0..steps {
                let mut transform = self
                    .app
                    .world_mut()
                    .get_mut::<Transform>(self.character)
                    .expect("character");
                transform.translation.z -= per_step;
                self.app.update();
            }
        }
    }

    #[test]
    fn the_rig_is_measured_at_the_size_it_is_worn() {
        let rig = rig();
        let legs = rig
            .app
            .world()
            .entity(rig.character)
            .get::<Legs>()
            .expect("legs resolved from the rig's bone names");
        // Measured inside the model, so both numbers are in the model's own units and are
        // the same whether or not the model root has been sized yet.
        assert!(
            (legs.foot_lift - ANKLE_IN_MODEL).abs() < 1e-4,
            "foot lift was {}, expected {ANKLE_IN_MODEL}",
            legs.foot_lift
        );
        assert!(
            (legs.leg_length - 0.663).abs() < 5e-3,
            "leg length was {}, so the stride would be sized for the wrong character",
            legs.leg_length
        );
    }

    #[test]
    fn the_feet_follow_a_character_that_spawns_in_the_air_and_falls() {
        // The bug as it actually shipped. The player is dropped onto the map and settles a
        // metre lower; standing still, no foot ever swings, so nothing ever produced the
        // touchdown that re-grounded the plants. The feet stayed nailed at the spawn height
        // and the legs pointed up at them.
        let mut rig = rig();
        for _ in 0..40 {
            let mut transform = rig
                .app
                .world_mut()
                .get_mut::<Transform>(rig.character)
                .expect("character");
            transform.translation.y -= 0.03;
            rig.app.update();
        }
        // Let it land. Reading the ground from a `GlobalTransform` costs a frame of lag, so
        // mid-fall the feet trail the body by a frame's worth of drop -- unavoidable in a
        // slot that runs before propagation, and invisible next to a step.
        for _ in 0..5 {
            rig.app.update();
        }

        let hip_y = rig.world_y(rig.hip);
        let ground = rig.world_y(rig.model) + ANKLE_IN_MODEL * MODEL_SCALE;
        for foot in Foot::BOTH {
            let y = rig.world_y(rig.feet[foot.index()]);
            assert!(y < hip_y, "{foot:?} foot is above the hip ({y} vs {hip_y}) after the fall");
            assert!(
                (y - ground).abs() < 0.02,
                "{foot:?} foot is at y {y}, left behind above the ground at {ground}"
            );
        }
    }

    #[test]
    fn the_feet_stay_under_the_character_while_it_walks() {
        let mut rig = rig();
        let step_height = {
            let legs = rig.app.world().entity(rig.character).get::<Legs>().expect("legs");
            let gait_scale = legs.leg_length * MODEL_SCALE / REFERENCE_LEG_LENGTH;
            GaitParams::default().scaled(gait_scale).step_height
        };
        rig.walk(120, 0.005);

        let hip_y = rig.world_y(rig.hip);
        let ground = BODY_Y + ANKLE_REST;
        for foot in Foot::BOTH {
            let y = rig.world_y(rig.feet[foot.index()]);
            assert!(
                y < hip_y,
                "{foot:?} foot is above the hip ({y} vs {hip_y}) -- the legs are pointing up"
            );
            assert!(
                y > ground - 0.02 && y < ground + step_height + 0.02,
                "{foot:?} foot is at y {y}, not within a step of the ground at {ground}"
            );
        }
    }

    #[test]
    fn a_step_is_taken_at_the_characters_own_scale() {
        // The whole point of scaling the gait: a quarter-size character must not reach a
        // metre and a half ahead of itself. Both feet stay within a body's width of the
        // hips, in every direction.
        let mut rig = rig();
        let mut worst: f32 = 0.0;
        for _ in 0..120 {
            rig.walk(1, 0.005);
            let hip = rig
                .app
                .world()
                .entity(rig.hip)
                .get::<GlobalTransform>()
                .expect("propagated")
                .translation();
            for foot in Foot::BOTH {
                let position = rig
                    .app
                    .world()
                    .entity(rig.feet[foot.index()])
                    .get::<GlobalTransform>()
                    .expect("propagated")
                    .translation();
                worst = worst.max(Vec3::new(position.x - hip.x, 0.0, position.z - hip.z).length());
            }
        }
        // Leg length is 0.2, so half a leg ahead is a stride, and anything approaching a
        // metre means the human-scale defaults went in unscaled.
        assert!(worst < 0.2, "a foot reached {worst} m from the hips of a 0.2 m leg");
    }
}
