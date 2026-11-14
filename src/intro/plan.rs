//! The flight as plain maths (pure, tested): where the copter is, which way it leans,
//! where the camera is and what it looks at, and where each player on a rope is, all as
//! functions of the seconds since the intro began.
//!
//! The copter follows a Catmull-Rom curve through a handful of points: high over the
//! clouds behind the level, down through them, pulling out low over the town, running in
//! towards the players' start and flaring into a hover over it. The team goes down the
//! ropes, the camera pulls back into the game's own view, and the copter climbs away.

use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// How long each part of the intro takes, in seconds. Tune to the music.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntroTiming {
    /// Over the clouds and down through them.
    pub dive: f32,
    /// Low over the town towards the start.
    pub low_pass: f32,
    /// Slowing into the hover over the start.
    pub flare: f32,
    /// The team goes down the ropes.
    pub rappel: f32,
    /// The camera settles into the game's view; play begins at its end.
    pub handoff: f32,
}

impl Default for IntroTiming {
    fn default() -> Self {
        Self {
            dive: 10.0,
            low_pass: 9.0,
            flare: 4.0,
            rappel: 5.0,
            handoff: 3.0,
        }
    }
}

impl IntroTiming {
    /// When the copter hangs over the start and the ropes go down.
    pub fn hover_at(&self) -> f32 {
        self.dive + self.low_pass + self.flare
    }

    /// When the last player is on the ground and the players are handed over.
    pub fn landed_at(&self) -> f32 {
        self.hover_at() + self.rappel
    }

    /// When the camera is the game's own again.
    pub fn end(&self) -> f32 {
        self.landed_at() + self.handoff
    }

    /// The same timing with no negative or zero-length parts (a file can say anything).
    pub fn sane(self) -> Self {
        let at_least = |v: f32, min: f32| if v.is_finite() { v.max(min) } else { min };
        Self {
            dive: at_least(self.dive, 1.0),
            low_pass: at_least(self.low_pass, 1.0),
            flare: at_least(self.flare, 0.5),
            rappel: at_least(self.rappel, 1.0),
            handoff: at_least(self.handoff, 0.5),
        }
    }
}

/// The cloud deck, in metres above the players' start.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudDeck {
    pub base: f32,
    pub top: f32,
}

impl Default for CloudDeck {
    fn default() -> Self {
        Self {
            base: 70.0,
            top: 115.0,
        }
    }
}

/// The game's own camera, for the camera to settle into: the direction from the focus to
/// the camera, and how many metres of the world the orthographic view shows top to bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsoView {
    pub dir: Vec3,
    pub height: f32,
}

/// One frame's camera: where it is, what it looks at and its vertical field of view in
/// degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov_degrees: f32,
}

/// The copter hangs this high over the start while the team goes down.
pub const HOVER_HEIGHT: f32 = 13.0;
/// The low pass flies this high over the ground under it.
const LOW_ALTITUDE: f32 = 32.0;
/// Over the clouds, the dive starts this far above their top.
const OVER_THE_CLOUDS: f32 = 45.0;
/// Cruising speed in metres a second, faster over a level too wide to cross at it.
const CRUISE: f32 = 22.0;
const FASTEST: f32 = 45.0;
/// The field of view the hand-off ends at, just before the switch to orthographic: thin
/// enough that the switch does not show.
pub const HANDOFF_FOV: f32 = 3.0;
/// Seconds between one player starting down the rope and the next.
const STAGGER: f32 = 0.45;
/// Where the ropes hang from, under the copter's middle.
pub const HATCH_DROP: f32 = 1.3;

/// The flight to one level's start.
#[derive(Debug, Clone, PartialEq)]
pub struct FlightPlan {
    pub timing: IntroTiming,
    /// Where the players stand once down (the centre of their spawn points).
    pub landing: Vec3,
    /// Where the copter hovers.
    pub hover: Vec3,
    /// Horizontal unit vector the copter flies along on its run in.
    pub heading: Vec3,
    pub clouds_base: f32,
    pub clouds_top: f32,
    /// The curve's points, with one extra at each end for the tangents.
    points: Vec<Vec3>,
    /// The dive, the low pass and the flare, measured along the curve.
    phases: [Stretch; 3],
}

/// Part of the curve, sampled by length so the copter can move along it evenly: the
/// distance covered at each sample, and the segment and parameter it is at.
#[derive(Debug, Clone, PartialEq, Default)]
struct Stretch {
    samples: Vec<(f32, usize, f32)>,
}

/// Samples per curve segment when measuring it.
const SAMPLES: usize = 48;

impl Stretch {
    fn measure(points: &[Vec3], first: usize, count: usize) -> Self {
        let at = |seg: usize, t: f32| curve_point(points, seg, t);
        let mut samples = vec![(0.0, first, 0.0)];
        let mut length = 0.0;
        let mut last = at(first, 0.0);
        for seg in first..first + count {
            for k in 1..=SAMPLES {
                let t = k as f32 / SAMPLES as f32;
                let here = at(seg, t);
                length += here.distance(last);
                last = here;
                samples.push((length, seg, t));
            }
        }
        Self { samples }
    }

    fn length(&self) -> f32 {
        self.samples.last().map_or(0.0, |s| s.0)
    }

    /// The segment and parameter `f` (0-1) of the way along by distance.
    fn at(&self, f: f32) -> (usize, f32) {
        let want = f.clamp(0.0, 1.0) * self.length();
        let next = self.samples.partition_point(|s| s.0 < want);
        let (Some(&(d1, s1, t1)), Some(&(d0, s0, t0))) =
            (self.samples.get(next), self.samples.get(next.saturating_sub(1)))
        else {
            return self.samples.last().map_or((0, 0.0), |s| (s.1, s.2));
        };
        if d1 <= d0 {
            return (s1, t1);
        }
        // Across a segment boundary the earlier sample is the start of the later segment.
        let t0 = if s0 == s1 { t0 } else { 0.0 };
        let k = (want - d0) / (d1 - d0);
        (s1, t0 + (t1 - t0) * k)
    }
}

/// Segment `seg` of the curve through `points` (segment 0 runs from point 1 to point 2;
/// points 0 and the last are only there for the tangents).
fn curve_point(points: &[Vec3], seg: usize, t: f32) -> Vec3 {
    let p = |i: usize| points.get(i).or(points.last()).copied().unwrap_or(Vec3::ZERO);
    catmull_rom(p(seg), p(seg + 1), p(seg + 2), p(seg + 3), t)
}

/// Uniform Catmull-Rom between `p1` and `p2`.
fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Fast at first, gliding to a stop: the flare.
fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

impl FlightPlan {
    /// The flight to `landing` on a level whose middle is `centre` and that reaches
    /// `radius` metres from it. `ground` gives the ground height anywhere (x, z), so the
    /// low pass clears the hills.
    pub fn new(
        landing: Vec3,
        centre: Vec3,
        radius: f32,
        timing: IntroTiming,
        deck: CloudDeck,
        ground: impl Fn(f32, f32) -> f32,
    ) -> Self {
        let timing = timing.sane();
        // Come in across the town, from the far side of the middle: most of the level
        // goes by under the copter. A start right in the middle comes in diagonally.
        let away = (landing - centre).with_y(0.0);
        let heading = if away.length() > radius.max(1.0) * 0.2 {
            away.normalize()
        } else {
            Vec3::new(1.0, 0.0, 1.0).normalize()
        };
        // Across the level from its far side, at cruising speed or faster.
        let across = (landing - centre).with_y(0.0).length() + radius.max(0.0);
        let speed = (across / timing.low_pass).clamp(CRUISE, FASTEST);
        let run = speed * timing.low_pass;
        // The flare eases to a stop from cruising speed: half the distance at that speed.
        let flare = speed * timing.flare * 0.5;
        let hover = landing + Vec3::Y * HOVER_HEIGHT;
        let clouds_base = landing.y + deck.base.max(HOVER_HEIGHT + 20.0);
        let clouds_top = landing.y + deck.top.max(deck.base + 10.0);
        let clear = |p: Vec3, over: f32| p.with_y(p.y.max(ground(p.x, p.z) + over));

        let approach = clear(hover - heading * flare + Vec3::Y * 3.0, HOVER_HEIGHT);
        let low_start = clear(
            hover - heading * (flare + run) + Vec3::Y * LOW_ALTITUDE,
            LOW_ALTITUDE,
        );
        let low_mid = clear(
            hover - heading * (flare + run * 0.5) + Vec3::Y * (LOW_ALTITUDE * 0.8),
            LOW_ALTITUDE * 0.8,
        );
        // The dive covers what cruising speed does in its time, no steeper than about
        // 50 degrees.
        let top = clouds_top + OVER_THE_CLOUDS;
        let drop = (top - low_start.y).max(0.0);
        let dive_path = speed * timing.dive;
        let dive_run = (dive_path * dive_path - drop * drop).max(0.0).sqrt().max(drop * 0.85);
        let pull_out = clear(
            low_start - heading * dive_run * 0.2 + Vec3::Y * 18.0,
            LOW_ALTITUDE,
        );
        let in_cloud = (low_start - heading * dive_run * 0.55)
            .with_y((clouds_base + clouds_top) * 0.5);
        let start = (low_start - heading * dive_run).with_y(top);

        let points = vec![
            start - heading * 40.0 + Vec3::Y * 4.0,
            start,
            in_cloud,
            pull_out,
            low_start,
            low_mid,
            approach,
            hover,
            hover + heading * 2.0,
        ];
        // start -> in_cloud -> pull_out -> low_start | -> low_mid -> approach | -> hover
        let phases = [
            Stretch::measure(&points, 0, 3),
            Stretch::measure(&points, 3, 2),
            Stretch::measure(&points, 5, 1),
        ];
        Self {
            timing,
            landing,
            hover,
            heading,
            clouds_base,
            clouds_top,
            points,
            phases,
        }
    }

    /// Point `i` of the curve proper (the tangent points are -1 and len).
    fn point(&self, i: usize) -> Vec3 {
        self.points.get(i).copied().unwrap_or(self.hover)
    }

    /// Phase `phase` (dive, low pass, flare), `f` of the way along it by distance.
    fn along(&self, phase: usize, f: f32) -> Vec3 {
        let (seg, t) = self.phases.get(phase).map_or((0, 0.0), |p| p.at(f));
        curve_point(&self.points, seg, t)
    }

    /// The copter's position `t` seconds in.
    pub fn copter_position(&self, t: f32) -> Vec3 {
        let tm = self.timing;
        if t <= 0.0 {
            // Already flying before the intro began (the chase camera looks back a bit).
            const H: f32 = 0.05;
            let ahead = self.along(0, H / tm.dive);
            return self.point(1) + (ahead - self.point(1)) / H * t;
        }
        if t < tm.dive {
            return self.along(0, t / tm.dive);
        }
        if t < tm.dive + tm.low_pass {
            return self.along(1, (t - tm.dive) / tm.low_pass);
        }
        if t < tm.hover_at() {
            return self.along(2, ease_out((t - tm.dive - tm.low_pass) / tm.flare));
        }
        let hovering = t - tm.hover_at();
        let bob = Vec3::Y * (hovering * 1.3).sin() * 0.25;
        let leave = (t - tm.landed_at() - 0.8).max(0.0);
        // Away along the heading, climbing: speeding up as it goes.
        let away = self.heading * (3.0 * leave * leave) + Vec3::Y * (1.5 * leave * leave + 2.0 * leave);
        self.hover + bob + away
    }

    /// The copter's velocity, from either side of `t`.
    pub fn copter_velocity(&self, t: f32) -> Vec3 {
        const H: f32 = 0.05;
        (self.copter_position(t + H) - self.copter_position(t - H)) / (2.0 * H)
    }

    /// Which way the copter points and leans: nose along its travel (the heading once it
    /// hovers), dipped when fast, lifted when slowing, banked into turns. The model's
    /// nose is -Z.
    pub fn copter_rotation(&self, t: f32) -> Quat {
        let v = self.copter_velocity(t).with_y(0.0);
        let speed = v.length();
        let facing = if speed > 4.0 && t < self.timing.hover_at() {
            v / speed
        } else {
            self.heading
        };
        let yaw = (-facing.x).atan2(-facing.z);
        let before = self.copter_velocity(t - 0.25).with_y(0.0);
        let after = self.copter_velocity(t + 0.25).with_y(0.0);
        let accel = (after.length() - before.length()) / 0.5;
        let turn = before.normalize_or_zero().cross(after.normalize_or_zero()).y / 0.5;
        let pitch = (-speed * 0.006 - accel * 0.035).clamp(-0.4, 0.3);
        let bank = (turn * speed * 0.02).clamp(-0.5, 0.5);
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(bank)
    }

    /// The camera behind and above the copter, a little late so it swings into turns.
    fn chase(&self, t: f32, back: f32, up: f32) -> Shot {
        let late = self.copter_position(t - 0.4);
        let dir = self.copter_velocity(t - 0.4).normalize_or(self.heading);
        let flat = dir.with_y(0.0).normalize_or(self.heading);
        let copter = self.copter_position(t);
        Shot {
            eye: late - flat * back + Vec3::Y * up,
            target: copter + dir * 12.0 - Vec3::Y * 3.0,
            fov_degrees: 55.0,
        }
    }

    /// The camera off to the side of the hover, taking in the copter and the ground.
    fn rappel_shot(&self, t: f32) -> Shot {
        let side = Vec3::Y.cross(self.heading).normalize_or(Vec3::X);
        let drift = ((t - self.timing.hover_at()) * 0.06).min(0.6);
        let around = Quat::from_rotation_y(drift) * (side * 15.0 - self.heading * 9.0);
        Shot {
            eye: self.landing + around + Vec3::Y * 6.0,
            target: self.landing + Vec3::Y * (HOVER_HEIGHT * 0.45),
            fov_degrees: 55.0,
        }
    }

    /// The game's view of the start, at the hand-off's thin field of view: the same
    /// direction and the same height of world on screen as the orthographic camera.
    pub fn iso_shot(&self, iso: IsoView, fov_degrees: f32) -> Shot {
        let target = self.landing;
        let distance = iso.height / (2.0 * (fov_degrees.to_radians() * 0.5).tan());
        Shot {
            eye: target + iso.dir.normalize_or(Vec3::new(0.0, 1.0, 1.0).normalize()) * distance,
            target,
            fov_degrees,
        }
    }

    /// The camera `t` seconds in.
    pub fn shot_at(&self, t: f32, iso: IsoView) -> Shot {
        let tm = self.timing;
        let dive_end = tm.dive;
        if t < dive_end + tm.low_pass {
            // Pulled well back over the clouds, closing in as it gets low.
            let k = smoothstep(t / dive_end);
            return self.chase(t, 30.0 - 16.0 * k, 9.0 - 4.0 * k);
        }
        if t < tm.hover_at() {
            let k = smoothstep((t - dive_end - tm.low_pass) / tm.flare);
            return blend(self.chase(t, 14.0, 5.0), self.rappel_shot(t), k);
        }
        if t < tm.landed_at() {
            return self.rappel_shot(t);
        }
        let k = smoothstep((t - tm.landed_at()) / tm.handoff);
        self.dolly(self.rappel_shot(tm.landed_at()), iso, k)
    }

    /// From `from` into the game's view, `k` of the way: the field of view narrows while
    /// the camera backs off so the world on screen grows to what the orthographic camera
    /// shows. At `k` = 1 it is [`Self::iso_shot`] at [`HANDOFF_FOV`].
    fn dolly(&self, from: Shot, iso: IsoView, k: f32) -> Shot {
        let to = self.iso_shot(iso, HANDOFF_FOV);
        let from_dir = (from.eye - from.target).normalize_or(Vec3::Y);
        let to_dir = (to.eye - to.target).normalize_or(Vec3::Y);
        let dir = from_dir.lerp(to_dir, k).normalize_or(to_dir);
        let half = |fov: f32| (fov.to_radians() * 0.5).tan();
        let from_height = 2.0 * from.eye.distance(from.target) * half(from.fov_degrees);
        // The angle narrows geometrically: an even zoom to the eye.
        let fov = from.fov_degrees * (HANDOFF_FOV / from.fov_degrees).powf(k);
        let height = from_height + (iso.height - from_height) * k;
        let target = from.target.lerp(to.target, k);
        Shot {
            eye: target + dir * (height / (2.0 * half(fov))),
            target,
            fov_degrees: fov,
        }
    }

    /// Where rope `i` hangs from at `t`: under the copter, alternating sides.
    pub fn hatch(&self, i: usize, t: f32) -> Vec3 {
        let side = Vec3::Y.cross(self.heading).normalize_or(Vec3::X);
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let out = 0.55 + 0.25 * (i / 2) as f32;
        self.copter_position(t) - Vec3::Y * HATCH_DROP + side * sign * out
    }

    /// Where player `i` of `count`, bound for `ground`, is on the rope at `t`, and
    /// whether they are down. They go one after another, each eased off the hatch and
    /// onto the ground.
    pub fn rappeller_at(&self, i: usize, count: usize, t: f32, ground: Vec3) -> (Vec3, bool) {
        let start = self.timing.hover_at() + i as f32 * STAGGER;
        let last_start = count.saturating_sub(1) as f32 * STAGGER;
        let length = (self.timing.rappel - last_start - 0.3).max(0.6);
        let k = ((t - start) / length).clamp(0.0, 1.0);
        (self.hatch(i, t).lerp(ground, smoothstep(k)), k >= 1.0)
    }
}

fn blend(a: Shot, b: Shot, k: f32) -> Shot {
    Shot {
        eye: a.eye.lerp(b.eye, k),
        target: a.target.lerp(b.target, k),
        fov_degrees: a.fov_degrees + (b.fov_degrees - a.fov_degrees) * k,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> FlightPlan {
        FlightPlan::new(
            Vec3::new(60.0, 0.0, 20.0),
            Vec3::new(40.0, 0.0, 40.0),
            40.0,
            IntroTiming::default(),
            CloudDeck::default(),
            |_, _| 0.0,
        )
    }

    fn iso() -> IsoView {
        IsoView {
            dir: Vec3::new(0.5, 0.7, 0.5).normalize(),
            height: 16.0,
        }
    }

    #[test]
    fn the_flight_starts_over_the_clouds_and_ends_in_the_hover() {
        let p = plan();
        assert!(p.copter_position(0.0).y > p.clouds_top);
        let hover = p.copter_position(p.timing.hover_at());
        assert!(hover.distance(p.hover) < 0.5, "{hover} vs {}", p.hover);
    }

    #[test]
    fn the_dive_goes_through_the_clouds() {
        let p = plan();
        let inside = (0..100)
            .map(|i| p.copter_position(p.timing.dive * i as f32 / 100.0).y)
            .any(|y| y > p.clouds_base && y < p.clouds_top);
        assert!(inside);
        assert!(p.copter_position(p.timing.dive).y < p.clouds_base);
    }

    #[test]
    fn the_path_is_continuous() {
        let p = plan();
        let mut last = p.copter_position(0.0);
        let mut t = 0.0;
        while t < p.timing.end() {
            t += 0.02;
            let here = p.copter_position(t);
            assert!(here.distance(last) < 2.5, "jump at {t}: {last} -> {here}");
            last = here;
        }
    }

    #[test]
    fn the_low_pass_clears_a_hill() {
        let hill = |x: f32, _z: f32| if x < 40.0 { 50.0 } else { 0.0 };
        let p = FlightPlan::new(
            Vec3::new(60.0, 0.0, 20.0),
            Vec3::new(40.0, 0.0, 40.0),
            40.0,
            IntroTiming::default(),
            CloudDeck::default(),
            hill,
        );
        let low_start = p.point(4);
        assert!(low_start.y > hill(low_start.x, low_start.z) + 20.0);
    }

    #[test]
    fn the_copter_slows_into_the_hover() {
        let p = plan();
        let fast = p.copter_velocity(p.timing.dive + 2.0).length();
        let slow = p.copter_velocity(p.timing.hover_at() - 0.1).length();
        assert!(fast > 10.0 && slow < fast * 0.3, "{fast} then {slow}");
    }

    #[test]
    fn the_team_goes_down_in_turn_and_lands() {
        let p = plan();
        let t0 = p.timing.hover_at() + 0.2;
        let ground = p.landing + Vec3::X;
        let (first, _) = p.rappeller_at(0, 3, t0 + 1.0, ground);
        let (third, _) = p.rappeller_at(2, 3, t0 + 1.0, ground);
        assert!(first.y < third.y, "the first is lower: {first} vs {third}");
        for i in 0..3 {
            let (at, down) = p.rappeller_at(i, 3, p.timing.landed_at(), ground);
            assert!(down, "player {i} is down");
            assert!(at.distance(ground) < 1e-3);
        }
    }

    #[test]
    fn the_handoff_ends_in_the_games_view() {
        let p = plan();
        let end = p.shot_at(p.timing.end(), iso());
        let want = p.iso_shot(iso(), HANDOFF_FOV);
        assert!(end.eye.distance(want.eye) < 0.5, "{} vs {}", end.eye, want.eye);
        assert!((end.fov_degrees - HANDOFF_FOV).abs() < 1e-3);
        // Same world height on screen as the orthographic camera.
        let shown = 2.0 * end.eye.distance(end.target) * (end.fov_degrees.to_radians() * 0.5).tan();
        assert!((shown - iso().height).abs() < 0.01);
    }

    #[test]
    fn the_camera_does_not_jump_between_phases() {
        let p = plan();
        let mut last = p.shot_at(0.0, iso());
        let mut t = 0.0;
        // Up to the hand-off; it backs off fast by design, so check its look-at only.
        while t < p.timing.landed_at() {
            t += 0.02;
            let here = p.shot_at(t, iso());
            assert!(here.eye.distance(last.eye) < 2.0, "eye jump at {t}");
            assert!(here.target.distance(last.target) < 2.0, "target jump at {t}");
            last = here;
        }
    }

    #[test]
    fn a_silly_timing_is_made_sane() {
        let t = IntroTiming {
            dive: -1.0,
            low_pass: f32::NAN,
            ..IntroTiming::default()
        }
        .sane();
        assert!(t.dive >= 1.0 && t.low_pass >= 1.0);
    }
}
