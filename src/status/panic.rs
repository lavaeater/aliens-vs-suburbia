//! Panic: a creature that has lost its head runs about at random, zig-zagging, turning
//! away from walls, until it calms down. It overrides whatever brain the creature has
//! (the goal walkers, hunters, fleeing neighbours, townsfolk errands): it runs in
//! `FixedUpdate`, after all of them have set their velocities and before physics steps,
//! and the brains that turn their body themselves skip a panicking one.

use bevy::prelude::*;

use crate::animation::animation_plugin::AnimationKey;
use crate::general::resources::map_resources::MapGraph;
use crate::infestation::hunters::{CreatureAnim, facing, tile_of};
use crate::physics::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};

/// Running speed in a panic, metres per second.
const SPEED: f32 = 3.2;
/// A new direction at least this often.
const RETARGET: (f32, f32) = (0.5, 1.2);
/// How far ahead (metres) the way has to be open for a direction to be taken.
const LOOK_AHEAD: f32 = 1.5;
/// Moving less than this per second while running counts as stuck.
const STUCK_SPEED: f32 = 0.4;
const TURN_RATE: f32 = 12.0;

#[derive(Component, Debug)]
pub struct Panicking {
    left: Timer,
    heading: Vec2,
    retarget: Timer,
    rng: u32,
    last: Option<Vec2>,
}

impl Panicking {
    pub fn new(secs: f32, seed: u32) -> Self {
        Self {
            left: Timer::from_seconds(secs, TimerMode::Once),
            heading: Vec2::ZERO,
            retarget: Timer::from_seconds(0.0, TimerMode::Once),
            rng: seed | 1,
            last: None,
        }
    }

    /// Keep panicking for at least `secs` more.
    pub fn last_at_least(&mut self, secs: f32) {
        if secs > self.left.remaining_secs() {
            self.left = Timer::from_seconds(secs, TimerMode::Once);
        }
    }

    /// In `[0, 1)`.
    fn next(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// A new running direction from `here`, given the old one: a sharp swerve off it rather
/// than a turn back, and only where the way `LOOK_AHEAD` on is `open`. `rolls` are random
/// numbers in `[0, 1)`; with nowhere open, straight back the way it came.
pub fn pick_heading(
    here: Vec2,
    old: Vec2,
    rolls: &mut dyn FnMut() -> f32,
    open: &dyn Fn(Vec2) -> bool,
) -> Vec2 {
    let base = if old == Vec2::ZERO {
        rolls() * std::f32::consts::TAU
    } else {
        old.to_angle()
    };
    for _ in 0..8 {
        // 40 to 130 degrees off the old heading, either side.
        let swerve = (40.0 + rolls() * 90.0).to_radians();
        let side = if rolls() < 0.5 { -1.0 } else { 1.0 };
        let dir = Vec2::from_angle(base + swerve * side);
        if open(here + dir * LOOK_AHEAD) {
            return dir;
        }
    }
    if old == Vec2::ZERO {
        Vec2::from_angle(base)
    } else {
        -old.normalize_or(Vec2::X)
    }
}

/// Run about.
#[allow(clippy::type_complexity)]
pub fn run_in_panic(
    time: Res<Time>,
    mut commands: Commands,
    map_graph: Option<Res<MapGraph>>,
    mut panicking: Query<(
        Entity,
        &mut Panicking,
        &Position,
        &mut LinearVelocity,
        &mut AngularVelocity,
        &mut Rotation,
        Option<&mut CreatureAnim>,
    )>,
) {
    let dt = time.delta_secs();
    let open = |at: Vec2| {
        map_graph.as_ref().is_none_or(|graph| {
            at.x >= 0.0
                && at.y >= 0.0
                && graph
                    .path_finding_grid
                    .has_vertex(tile_of(Vec3::new(at.x, 0.0, at.y)))
        })
    };
    for (entity, mut panic, position, mut velocity, mut spin, mut rotation, anim) in &mut panicking
    {
        panic.left.tick(time.delta());
        if panic.left.is_finished() {
            velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
            commands.entity(entity).remove::<Panicking>();
            continue;
        }
        let here = position.0.xz();
        let stuck = panic
            .last
            .is_some_and(|last| dt > 0.0 && last.distance(here) / dt < STUCK_SPEED);
        panic.last = Some(here);
        panic.retarget.tick(time.delta());
        if panic.retarget.is_finished() || stuck || panic.heading == Vec2::ZERO {
            let old = panic.heading;
            let mut rolls = || panic.next();
            let heading = pick_heading(here, old, &mut rolls, &open);
            let wait = RETARGET.0 + panic.next() * (RETARGET.1 - RETARGET.0);
            panic.heading = heading;
            panic.retarget = Timer::from_seconds(wait, TimerMode::Once);
            // Not stuck on the very next step just because the new way starts slow.
            panic.last = None;
        }
        let dir = panic.heading;
        velocity.0 = Vec3::new(dir.x * SPEED, velocity.0.y, dir.y * SPEED);
        spin.0 = Vec3::ZERO;
        if let Some(goal) = facing(dir) {
            rotation.0 = rotation.0.slerp(goal, (dt * TURN_RATE).min(1.0));
        }
        if let Some(mut anim) = anim {
            anim.set_if_neq(CreatureAnim(AnimationKey::Run));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rolls_from(values: &[f32]) -> impl FnMut() -> f32 + '_ {
        let mut i = 0;
        move || {
            let v = values[i % values.len()];
            i += 1;
            v
        }
    }

    #[test]
    fn a_panic_swerves_off_its_old_heading() {
        let old = Vec2::X;
        let mut rolls = rolls_from(&[0.5, 0.2, 0.9, 0.7]);
        let new = pick_heading(Vec2::ZERO, old, &mut rolls, &|_| true);
        let off = old.angle_to(new).abs().to_degrees();
        assert!((40.0..=130.0).contains(&off), "swerved {off} degrees");
    }

    #[test]
    fn a_panic_does_not_run_into_a_wall() {
        // Wall everywhere with y > 0.5: only the southern swerves are open.
        let open = |p: Vec2| p.y < 0.5;
        let mut rolls = rolls_from(&[0.3, 0.1, 0.6, 0.8, 0.45, 0.2]);
        for _ in 0..20 {
            let dir = pick_heading(Vec2::ZERO, Vec2::X, &mut rolls, &open);
            assert!(open(dir * LOOK_AHEAD), "ran north into the wall: {dir}");
        }
    }

    #[test]
    fn boxed_in_it_turns_back() {
        let dir = pick_heading(Vec2::ZERO, Vec2::X, &mut || 0.5, &|_| false);
        assert!(dir.distance(-Vec2::X) < 1e-5);
    }
}
