//! `TransformInterpolation`: physics ticks at 20 Hz, so an entity whose `Transform` changes in
//! the fixed step is eased between its last two ticks every frame (as Avian's
//! `bevy_transform_interpolation`). Works on any entity, body or not.
//!
//! Before a tick the true pose is put back, so fixed systems never see an eased one; after the
//! fixed loop the eased pose is written. Someone else writing the `Transform` outside the fixed
//! step (the camera follows in `Update`) wins: the easing notices and starts again from there.

use bevy::prelude::*;

/// Poses at the start and the end of the last tick, and the last one written by the easing.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TransformEasing {
    start: Option<Pose>,
    end: Option<Pose>,
    written: Option<Pose>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose(Vec3, Quat);

impl Pose {
    fn of(t: &Transform) -> Self {
        Self(t.translation, t.rotation)
    }

    fn apply(self, t: &mut Transform) {
        t.translation = self.0;
        t.rotation = self.1;
    }
}

/// `FixedFirst`: back to the true pose, which becomes the start of the tick.
pub(super) fn restore(mut eased: Query<(&mut Transform, &mut TransformEasing)>) {
    for (mut t, mut e) in &mut eased {
        let ours = e.written == Some(Pose::of(&t));
        if let (true, Some(end)) = (ours, e.end) {
            end.apply(&mut t);
        }
        e.start = Some(Pose::of(&t));
        e.written = None;
    }
}

/// `FixedLast`: where the tick left it.
pub(super) fn capture(mut eased: Query<(&Transform, &mut TransformEasing)>) {
    for (t, mut e) in &mut eased {
        e.end = Some(Pose::of(t));
    }
}

/// After the fixed loop, every frame: between start and end by how far into the next tick the
/// frame is.
pub(super) fn ease(time: Res<Time<Fixed>>, mut eased: Query<(&mut Transform, &mut TransformEasing)>) {
    let alpha = time.overstep_fraction();
    for (mut t, mut e) in &mut eased {
        let (Some(start), Some(end)) = (e.start, e.end) else { continue };
        let now = Pose::of(&t);
        // Moved by someone else since the last write (or the last tick): theirs to keep.
        if e.written.is_some_and(|w| w != now) || (e.written.is_none() && now != end) {
            e.start = Some(now);
            e.end = Some(now);
            e.written = None;
            continue;
        }
        let pose = Pose(start.0.lerp(end.0, alpha), start.1.slerp(end.1, alpha));
        pose.apply(&mut t);
        e.written = Some(pose);
    }
}
