//! The gait: where each foot should be, in world space, at any point in a walk cycle.
//!
//! Pure — no ECS, no queries, no `Transform`s. It answers one question ("where do the two
//! feet go?") and knows nothing about how they get there; `arm_ik::solve_elbow` does that
//! part, unchanged, since hip/knee/ankle is the same two-bone problem as
//! shoulder/elbow/wrist. Same reason `hardpoint.rs` is pure: the interesting behaviour is
//! testable without spawning a world.
//!
//! # The planted foot does not move
//!
//! The one idea the whole module is built on. A foot carrying weight is nailed to a
//! *world* position and stays there while the hips travel away from it; only the swinging
//! foot moves, arcing to where the body is about to be. Animating both feet in character
//! space is what produces skating, and it is structural rather than a tuning problem —
//! the character translates out from under any local-space pose, however good the curve.
//!
//! # Distance, not time
//!
//! [`GaitState::update`] advances the cycle by how far the character travelled, not by how
//! long the frame took. Stride length and speed then cannot disagree: sprint and the steps
//! lengthen, get shoved and the feet keep up, stop and the cycle stops with the feet down.
//! Driving it from a clock instead means tuning a playback rate against movement speed and
//! watching the two drift apart.
//!
//! The single exception is documented on [`SETTLE_RATE`].

use bevy::math::Vec3;

/// Which leg. Also the index into the per-foot arrays on [`GaitState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foot {
    Left,
    Right,
}

impl Foot {
    pub const BOTH: [Foot; 2] = [Foot::Left, Foot::Right];

    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Foot::Left => 0,
            Foot::Right => 1,
        }
    }

    /// Which way this foot sits off the centreline, along the character's right.
    #[must_use]
    pub fn lateral_sign(self) -> f32 {
        match self {
            Foot::Left => -1.0,
            Foot::Right => 1.0,
        }
    }

    /// Feet are half a cycle apart: one plants as the other lifts.
    #[must_use]
    pub fn phase_offset(self) -> f32 {
        match self {
            Foot::Left => 0.5,
            Foot::Right => 0.0,
        }
    }
}

/// The shape of the walk. One cycle is *both* steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaitParams {
    /// Metres of travel per full cycle. Longer strides mean fewer, bigger steps at the
    /// same speed.
    pub stride_length: f32,
    /// Distance between the feet, across the direction of travel.
    pub stance_width: f32,
    /// How high the swinging foot lifts at mid-step.
    pub step_height: f32,
    /// Fraction of the cycle each foot spends on the ground.
    ///
    /// This one number is the difference between a walk and a run. Above 0.5 the two
    /// stance phases overlap and there is always a foot down (double support, a walk);
    /// below 0.5 they leave a gap where neither foot is planted (a flight phase, a run).
    /// Exactly 0.5 is the boundary, and looks like a march.
    pub duty_factor: f32,
}

impl Default for GaitParams {
    /// A human walk at roughly human scale.
    fn default() -> Self {
        Self {
            stride_length: 1.6,
            stance_width: 0.3,
            step_height: 0.15,
            duty_factor: 0.6,
        }
    }
}

impl GaitParams {
    /// The same walk on a body `factor` times a human's size.
    ///
    /// Gait is a matter of proportion, not of metres: everyone takes a stride of roughly
    /// twice their leg length, and a character a quarter of human size that keeps the
    /// human's 1.6 m stride is reaching several body-lengths ahead of itself with every
    /// step. The lengths scale; the duty factor does not, being a fraction of the cycle
    /// rather than a distance — a mouse and a horse both walk at about 0.6.
    #[must_use]
    pub fn scaled(&self, factor: f32) -> Self {
        Self {
            stride_length: self.stride_length * factor,
            stance_width: self.stance_width * factor,
            step_height: self.step_height * factor,
            duty_factor: self.duty_factor,
        }
    }

    /// Shrink the stride until the feet can actually get to where it puts them.
    ///
    /// A leg of length `L` whose hip rides `h` above the ground can only put its foot
    /// `sqrt(L² - h²)` away from directly underneath itself. Ask for more and the foot never
    /// arrives: the IK straightens the leg and stops short, which reads as walking on
    /// tiptoe, or on nothing at all.
    ///
    /// What buys a person their long stride is that their hips *drop* as the legs scissor
    /// apart — the body bobs several centimetres every step. The pelvis here belongs to the
    /// animation, which holds it at a steady height, so that budget does not exist and the
    /// stride has to live within whatever slack the rig's own pose leaves. A rig posed with
    /// nearly straight legs takes small quick steps, which is exactly what a person does
    /// when they refuse to bend their knees.
    ///
    /// The forward lead and the sideways stance share the reach, hence the circle: at
    /// touchdown the foot is `stance_width / 2` off to the side *and* `duty_factor / 2`
    /// strides ahead.
    #[must_use]
    pub fn fit_to_reach(&self, leg_length: f32, hip_height: f32) -> Self {
        let reach = horizontal_reach(leg_length, hip_height);
        let stance_width = self.stance_width.min(reach);
        let half = stance_width * 0.5;
        let lead = (reach * reach - half * half).max(0.0).sqrt();
        let duty = self.duty_factor.clamp(0.01, 0.99);
        Self {
            stride_length: self.stride_length.min(lead / (duty * 0.5)),
            stance_width,
            step_height: self.step_height,
            duty_factor: self.duty_factor,
        }
    }
}

/// How much of a leg's horizontal reach the gait may spend.
///
/// The rest is left as a bend in the knee. A leg solved dead straight looks like a stilt and
/// has nowhere to go if the ground turns out to be slightly lower than expected.
const REACH_MARGIN: f32 = 0.85;

/// How far from under its own hip a foot can be put down, and still reach the floor.
///
/// Pythagoras on the leg: the hip is `hip_height` up, the foot is on the ground, and the
/// straight line between them cannot exceed `leg_length`.
#[must_use]
pub fn horizontal_reach(leg_length: f32, hip_height: f32) -> f32 {
    (leg_length * leg_length - hip_height * hip_height).max(0.0).sqrt() * REACH_MARGIN
}

/// Cycles per second used to finish a step that was interrupted by stopping.
///
/// The one place a clock is allowed in, and only because the alternative is worse: stop
/// mid-stride and a purely distance-driven cycle freezes with a foot hanging in the air
/// forever, since no further distance is ever travelled. So a foot already in the air
/// keeps swinging until it lands, and *then* everything stops. A foot on the ground never
/// lifts for this — standing still can never start a step.
pub const SETTLE_RATE: f32 = 1.0;

/// Where a foot is in its own cycle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FootPhase {
    /// On the ground, not moving. Carries how far through the stance it is, which the
    /// caller can use for weight shifting.
    Planted { progress: f32 },
    /// In the air, `progress` running 0 at liftoff to 1 at touchdown.
    Swinging { progress: f32 },
}

impl FootPhase {
    #[must_use]
    pub fn is_planted(self) -> bool {
        matches!(self, FootPhase::Planted { .. })
    }
}

/// Where `foot` is within the cycle, given the whole body's `cycle` in `[0, 1)`.
#[must_use]
pub fn foot_phase(cycle: f32, foot: Foot, duty_factor: f32) -> FootPhase {
    let duty = duty_factor.clamp(0.01, 0.99);
    let local = (cycle + foot.phase_offset()).rem_euclid(1.0);
    if local < duty {
        FootPhase::Planted { progress: local / duty }
    } else {
        FootPhase::Swinging { progress: (local - duty) / (1.0 - duty) }
    }
}

/// The swinging foot's position, arcing from `from` up and over to `to`.
///
/// Horizontal travel is eased rather than linear so the foot leaves and meets the ground
/// with no horizontal speed — a linear ramp lands the foot moving sideways, which reads as
/// a skid at exactly the moment the foot is supposed to grip. The lift is a sine hump for
/// the same reason: zero at both ends, so touchdown is not a collision.
#[must_use]
pub fn swing_position(from: Vec3, to: Vec3, progress: f32, step_height: f32) -> Vec3 {
    let t = progress.clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    let lift = (t * std::f32::consts::PI).sin() * step_height;
    from.lerp(to, eased) + Vec3::Y * lift
}

/// Where a swinging foot should aim to land.
///
/// Two parts. The body keeps moving while the foot is in the air, so the target leads it
/// by the distance still to be covered before touchdown. And the foot should land *ahead*
/// of the hip by half of the distance the body will cover during the stance that follows,
/// so that the hip passes over the planted foot and leaves it behind symmetrically — land
/// on top of the hip and the leg has nothing left to push against by mid-stance.
///
/// `swing_remaining` is in cycle fractions, so both parts scale with `stride_length` and
/// the whole thing stays distance-driven.
#[must_use]
pub fn plant_target(
    hip_ground: Vec3,
    forward: Vec3,
    right: Vec3,
    foot: Foot,
    params: &GaitParams,
    swing_remaining: f32,
) -> Vec3 {
    let lead = (swing_remaining + params.duty_factor * 0.5) * params.stride_length;
    hip_ground
        + forward * lead
        + right * (foot.lateral_sign() * params.stance_width * 0.5)
}

/// The nearest point to `plant` that the leg can actually stand on, horizontally.
fn pull_into_reach(plant: Vec3, ctx: &GaitContext) -> Vec3 {
    let from_hip = Vec3::new(plant.x - ctx.hip_ground.x, 0.0, plant.z - ctx.hip_ground.z);
    let distance = from_hip.length();
    if distance <= ctx.reach || distance < 1e-6 {
        return plant;
    }
    ctx.hip_ground.with_y(plant.y) + from_hip * (ctx.reach / distance)
}

/// Everything the gait needs to know about the body this frame.
#[derive(Clone, Copy, Debug)]
pub struct GaitContext {
    /// The hips, projected down onto the ground plane — the point the feet are placed
    /// around.
    pub hip_ground: Vec3,
    /// Unit, the direction of travel. Facing is the sensible fallback when stopped.
    pub forward: Vec3,
    /// Unit, the character's right.
    pub right: Vec3,
    /// Ground height under the character. A constant on a flat map; a raycast later.
    pub ground_y: f32,
    /// How far from under the hips a foot can be planted and still touch the ground — see
    /// [`horizontal_reach`].
    pub reach: f32,
    /// Metres travelled since the last update. Drives the whole cycle.
    pub distance: f32,
    /// Seconds since the last update. Used only to settle an interrupted step.
    pub dt: f32,
}

/// The walk cycle's running state: the phase, and where each foot is nailed.
#[derive(Clone, Debug)]
pub struct GaitState {
    cycle: f32,
    /// Where each foot is currently planted, or last was.
    plant: [Vec3; 2],
    /// Where each foot lifted from, held for the duration of its swing.
    swing_from: [Vec3; 2],
    swinging: [bool; 2],
}

impl GaitState {
    /// Start with both feet planted either side of `hip_ground`.
    #[must_use]
    pub fn standing(hip_ground: Vec3, right: Vec3, params: &GaitParams) -> Self {
        let plant = Foot::BOTH.map(|foot| {
            hip_ground + right * (foot.lateral_sign() * params.stance_width * 0.5)
        });
        Self { cycle: 0.0, plant, swing_from: plant, swinging: [false; 2] }
    }

    /// Advance the cycle and return where both feet go this frame, indexed by
    /// [`Foot::index`].
    ///
    /// Advancing and querying are one call because they cannot be separated safely: the
    /// plant points are latched exactly at the liftoff and touchdown the advance detects,
    /// and a caller that advanced twice before asking, or asked without advancing, would
    /// get feet nailed to the wrong places.
    pub fn update(&mut self, ctx: &GaitContext, params: &GaitParams) -> [Vec3; 2] {
        self.advance_cycle(ctx, params);

        let mut out = [Vec3::ZERO; 2];
        for foot in Foot::BOTH {
            let i = foot.index();
            match foot_phase(self.cycle, foot, params.duty_factor) {
                FootPhase::Planted { .. } => {
                    if self.swinging[i] {
                        // Touchdown: nail the foot where it actually landed, not where the
                        // target said it would. They differ whenever velocity changed
                        // mid-swing, and believing the prediction over the foot's own
                        // position is a visible jump.
                        self.plant[i] = self.swing_end(ctx, params, foot, 1.0).with_y(ctx.ground_y);
                        self.swinging[i] = false;
                    }
                    // A plant that the leg cannot reach is not a plant. Standing still
                    // never produces a touchdown, so nothing else would ever revisit a
                    // footprint left somewhere impossible -- by a resolve that happened
                    // before the model was sized, or by the character being moved out from
                    // under its own feet. A real foot in that position shuffles.
                    self.plant[i] = pull_into_reach(self.plant[i], ctx);
                    // A plant holds its *ground* position; the ground's height is read fresh
                    // every frame. Only the horizontal part is what "planted" means, and
                    // holding a latched height instead leaves the feet hanging wherever the
                    // character last touched down -- a character that spawns in the air and
                    // falls keeps its feet up at the spawn point, standing on nothing.
                    out[i] = self.plant[i].with_y(ctx.ground_y);
                }
                FootPhase::Swinging { progress } => {
                    if !self.swinging[i] {
                        // Liftoff: remember where it left, so the arc starts from the real
                        // footprint rather than from wherever the hips are now.
                        self.swing_from[i] = self.plant[i];
                        self.swinging[i] = true;
                    }
                    out[i] = self.swing_end(ctx, params, foot, progress);
                }
            }
        }
        out
    }

    /// The swinging foot's position at `progress`, against a plant target recomputed from
    /// this frame's velocity.
    ///
    /// Recomputed rather than latched at liftoff so that turning or accelerating mid-step
    /// is followed rather than ignored. The easing absorbs it: the target's influence is
    /// weighted by `eased`, so a late change moves the foot least when it is closest to
    /// landing.
    fn swing_end(
        &self,
        ctx: &GaitContext,
        params: &GaitParams,
        foot: Foot,
        progress: f32,
    ) -> Vec3 {
        let i = foot.index();
        let target = plant_target(
            ctx.hip_ground.with_y(ctx.ground_y),
            ctx.forward,
            ctx.right,
            foot,
            params,
            (1.0 - progress) * (1.0 - params.duty_factor.clamp(0.01, 0.99)),
        );
        swing_position(self.swing_from[i], target, progress, params.step_height)
    }

    fn advance_cycle(&mut self, ctx: &GaitContext, params: &GaitParams) {
        let stride = params.stride_length.max(1e-3);
        let by_distance = ctx.distance.abs() / stride;

        // A foot already in the air must land even if the character stopped dead; see
        // `SETTLE_RATE`. Never lifts a planted foot, so standing still stays still.
        let settling = Foot::BOTH
            .iter()
            .any(|&foot| !foot_phase(self.cycle, foot, params.duty_factor).is_planted());
        let by_settle = if settling { ctx.dt * SETTLE_RATE } else { 0.0 };

        self.cycle = (self.cycle + by_distance.max(by_settle)).rem_euclid(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(hip: Vec3, distance: f32) -> GaitContext {
        GaitContext {
            hip_ground: hip,
            forward: Vec3::Z,
            right: Vec3::X,
            ground_y: 0.0,
            // Generous: these tests are about the cycle, not about what a leg can reach.
            reach: 100.0,
            distance,
            dt: 1.0 / 60.0,
        }
    }

    #[test]
    fn a_stride_never_exceeds_what_the_legs_can_reach() {
        // Human proportions: a 0.85 m leg with the hip 0.8 m up has 0.28 m of horizontal
        // reach, and the default 1.6 m stride plants the foot 0.48 m ahead -- too far.
        let fitted = GaitParams::default().fit_to_reach(0.85, 0.8);
        let lead = fitted.stride_length * fitted.duty_factor * 0.5;
        let lateral = fitted.stance_width * 0.5;
        let needed = (lead * lead + lateral * lateral).sqrt();
        assert!(
            needed <= horizontal_reach(0.85, 0.8) + 1e-5,
            "the foot is planted {needed} out, past a reach of {}",
            horizontal_reach(0.85, 0.8)
        );
    }

    #[test]
    fn a_gait_that_already_fits_is_left_alone() {
        // Only a cap. A rig with room to spare keeps the stride it was given.
        let params = GaitParams { stride_length: 0.4, ..Default::default() };
        let fitted = params.fit_to_reach(0.85, 0.3);
        assert_eq!(fitted, params);
    }

    #[test]
    fn a_foot_left_somewhere_impossible_shuffles_back_under_the_hips() {
        // The shape of the shipped bug: the character is dropped onto the map and its feet
        // are still planted where it spawned. Standing still, no foot ever swings, so
        // nothing but this rule ever moves them again.
        let params = GaitParams::default();
        let hip = Vec3::new(0.0, 0.0, 0.0);
        let mut gait = GaitState::standing(hip, Vec3::X, &params);

        let mut context = ctx(hip, 0.0);
        context.reach = 0.3;
        let feet = gait.update(&context, &params);
        for foot in Foot::BOTH {
            let position = feet[foot.index()];
            let distance = Vec3::new(position.x, 0.0, position.z).length();
            assert!(
                distance <= 0.3 + 1e-5,
                "{foot:?} foot is {distance} from the hips, past a reach of 0.3"
            );
        }
    }

    #[test]
    fn a_planted_foot_follows_the_ground_down() {
        // Its horizontal position is nailed; its height is not. The character falls, and the
        // feet have to come with it rather than stay hanging where it spawned.
        let params = GaitParams::default();
        let mut gait = GaitState::standing(Vec3::ZERO, Vec3::X, &params);
        let mut context = ctx(Vec3::new(0.0, -1.0, 0.0), 0.0);
        context.ground_y = -1.0;
        let feet = gait.update(&context, &params);
        for foot in Foot::BOTH {
            assert!(
                (feet[foot.index()].y - (-1.0)).abs() < 1e-5,
                "{foot:?} foot stayed at {} instead of following the ground to -1.0",
                feet[foot.index()].y
            );
        }
    }

    #[test]
    fn a_walk_always_has_a_foot_down_and_a_run_does_not() {
        // The duty factor is the whole difference between the two gaits.
        let walk = GaitParams { duty_factor: 0.6, ..Default::default() };
        let run = GaitParams { duty_factor: 0.4, ..Default::default() };

        let planted = |params: &GaitParams, cycle: f32| {
            Foot::BOTH
                .iter()
                .filter(|&&f| foot_phase(cycle, f, params.duty_factor).is_planted())
                .count()
        };

        let mut walk_min = 2;
        let mut run_min = 2;
        for step in 0u8..200 {
            let cycle = f32::from(step) / 200.0;
            walk_min = walk_min.min(planted(&walk, cycle));
            run_min = run_min.min(planted(&run, cycle));
        }
        assert_eq!(walk_min, 1, "a walk never lifts both feet");
        assert_eq!(run_min, 0, "a run has a flight phase");
    }

    #[test]
    fn the_feet_are_half_a_cycle_apart() {
        let duty = 0.6;
        for step in 0u8..100 {
            let cycle = f32::from(step) / 100.0;
            let left = foot_phase(cycle, Foot::Left, duty);
            let right = foot_phase(cycle + 0.5, Foot::Right, duty);
            assert_eq!(left, right);
        }
    }

    #[test]
    fn a_planted_foot_does_not_move_while_the_body_walks_over_it() {
        // The invariant the module exists for: no skating.
        let params = GaitParams::default();
        let mut gait = GaitState::standing(Vec3::ZERO, Vec3::X, &params);

        let mut hip = Vec3::ZERO;
        let mut planted_positions: Vec<Vec3> = Vec::new();
        for _ in 0..30 {
            hip += Vec3::Z * 0.02;
            let feet = gait.update(&ctx(hip, 0.02), &params);
            // The right foot starts its cycle planted.
            if foot_phase(gait.cycle, Foot::Right, params.duty_factor).is_planted() {
                planted_positions.push(feet[Foot::Right.index()]);
            }
        }

        assert!(planted_positions.len() > 10, "expected a long stance to sample");
        let first = planted_positions[0];
        for p in &planted_positions {
            assert!(
                p.distance(first) < 1e-5,
                "planted foot slid from {first:?} to {p:?} while the hips advanced 0.6m",
            );
        }
    }

    #[test]
    fn a_swinging_foot_leaves_and_meets_the_ground_without_sliding() {
        // Zero horizontal speed at both ends: the foot must not be moving sideways at the
        // moment it takes the character's weight.
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(0.0, 0.0, 1.0);

        let start = swing_position(from, to, 0.0, 0.2);
        let just_after = swing_position(from, to, 0.01, 0.2);
        let just_before = swing_position(from, to, 0.99, 0.2);
        let end = swing_position(from, to, 1.0, 0.2);

        assert!(start.abs_diff_eq(from, 1e-6));
        assert!(end.abs_diff_eq(to, 1e-6));
        // Over 1% of the swing, an eased curve covers far less than the 1% a linear one
        // would.
        assert!(just_after.z < 0.001, "liftoff was not eased: {}", just_after.z);
        assert!(just_before.z > 0.999, "touchdown was not eased: {}", just_before.z);
    }

    #[test]
    fn the_step_lifts_highest_in_the_middle_and_is_flat_at_the_ends() {
        let from = Vec3::ZERO;
        let to = Vec3::Z;
        assert!((swing_position(from, to, 0.5, 0.2).y - 0.2).abs() < 1e-6);
        assert!(swing_position(from, to, 0.0, 0.2).y.abs() < 1e-6);
        assert!(swing_position(from, to, 1.0, 0.2).y.abs() < 1e-6);
    }

    #[test]
    fn the_cycle_follows_distance_and_ignores_frame_rate() {
        // Two paths covering the same ground must land on the same phase, whether that is
        // one long frame or ten short ones.
        let params = GaitParams::default();
        let mut coarse = GaitState::standing(Vec3::ZERO, Vec3::X, &params);
        let mut fine = GaitState::standing(Vec3::ZERO, Vec3::X, &params);

        coarse.update(&ctx(Vec3::ZERO, 0.5), &params);
        let mut hip = Vec3::ZERO;
        for _ in 0..10 {
            hip += Vec3::Z * 0.05;
            fine.update(&ctx(hip, 0.05), &params);
        }
        assert!((coarse.cycle - fine.cycle).abs() < 1e-5);
    }

    #[test]
    fn one_stride_length_of_travel_is_exactly_one_cycle() {
        let params = GaitParams::default();
        let mut gait = GaitState::standing(Vec3::ZERO, Vec3::X, &params);
        let mut hip = Vec3::ZERO;
        let step = params.stride_length / 100.0;

        let mut touchdowns = 0;
        let mut was_planted = true;
        for _ in 0..100 {
            hip += Vec3::Z * step;
            gait.update(&ctx(hip, step), &params);
            let planted = foot_phase(gait.cycle, Foot::Right, params.duty_factor).is_planted();
            if planted && !was_planted {
                touchdowns += 1;
            }
            was_planted = planted;
        }
        assert_eq!(touchdowns, 1, "each foot should plant once per stride length");
    }

    #[test]
    fn stopping_finishes_the_step_in_the_air_and_then_holds_still() {
        // A distance-driven cycle would freeze a foot mid-air forever; the settle rate is
        // the narrow exception that lets it land.
        let params = GaitParams::default();
        let mut gait = GaitState::standing(Vec3::ZERO, Vec3::X, &params);

        // Walk until the left foot is airborne.
        let mut hip = Vec3::ZERO;
        while foot_phase(gait.cycle, Foot::Left, params.duty_factor).is_planted() {
            hip += Vec3::Z * 0.02;
            gait.update(&ctx(hip, 0.02), &params);
        }

        // Stop dead. The foot must still come down.
        for _ in 0..600 {
            gait.update(&ctx(hip, 0.0), &params);
        }
        let phase = foot_phase(gait.cycle, Foot::Left, params.duty_factor);
        assert!(phase.is_planted(), "the airborne foot never landed: {phase:?}");

        // And once down, standing still starts no new step.
        let settled = gait.cycle;
        for _ in 0..600 {
            gait.update(&ctx(hip, 0.0), &params);
        }
        assert!(
            (gait.cycle - settled).abs() < 1e-6,
            "standing still advanced the cycle from {settled} to {}",
            gait.cycle,
        );
    }

    #[test]
    fn the_plant_target_leads_the_hip_and_sits_on_the_correct_side() {
        let params = GaitParams::default();
        let left = plant_target(Vec3::ZERO, Vec3::Z, Vec3::X, Foot::Left, &params, 0.0);
        let right = plant_target(Vec3::ZERO, Vec3::Z, Vec3::X, Foot::Right, &params, 0.0);

        assert!(left.z > 0.0, "the foot should land ahead of the hip");
        assert!((left.z - right.z).abs() < 1e-6, "both feet lead by the same distance");
        assert!(left.x < 0.0 && right.x > 0.0, "left foot left, right foot right");
        assert!((right.x - left.x - params.stance_width).abs() < 1e-6);

        // Still to swing means still further to lead.
        let further = plant_target(Vec3::ZERO, Vec3::Z, Vec3::X, Foot::Left, &params, 0.4);
        assert!(further.z > left.z);
    }

    #[test]
    fn a_foot_lands_where_it_actually_was_not_where_the_prediction_said() {
        // Turn hard mid-swing, then check the nailed position matches the foot's last
        // airborne position rather than jumping to a freshly predicted target.
        let params = GaitParams::default();
        let mut gait = GaitState::standing(Vec3::ZERO, Vec3::X, &params);
        let mut hip = Vec3::ZERO;

        // The pair must be sampled either side of the same touchdown -- comparing against
        // any later airborne sample is comparing across a different step entirely.
        let i = Foot::Left.index();
        let mut airborne_before_touchdown = None;
        let mut previous = None;
        let mut touchdown = None;
        for tick in 0..200 {
            hip += Vec3::Z * 0.02;
            // Swerve: forward swings to +X partway through, so the prediction the foot
            // took off towards is not the one it lands into.
            let mut c = ctx(hip, 0.02);
            if tick > 40 {
                c.forward = Vec3::X;
            }
            let feet = gait.update(&c, &params);
            let planted = foot_phase(gait.cycle, Foot::Left, params.duty_factor).is_planted();
            if planted {
                if tick > 40 && let Some(prev) = previous {
                    airborne_before_touchdown = Some(prev);
                    touchdown = Some(feet[i]);
                    break;
                }
                previous = None;
            } else {
                previous = Some(feet[i]);
            }
        }

        let landed = touchdown.expect("the left foot never landed after the swerve");
        let airborne = airborne_before_touchdown.expect("no airborne sample before touchdown");
        // Flat on the ground, and horizontally within a frame or two of where the foot
        // already was. Not zero: by touchdown the foot is glued to a target that travels
        // with the body, so it covers one frame of body travel (0.02m) like everything
        // else. What this rules out is the failure that matters -- landing metres away, at
        // a plant point predicted before the swerve.
        assert!(landed.y.abs() < 1e-6);
        let slack = 0.02 * 2.0;
        assert!(
            (landed.x - airborne.x).abs() < slack && (landed.z - airborne.z).abs() < slack,
            "foot jumped on landing: {airborne:?} -> {landed:?}",
        );
    }
}
