use crate::animation::animation_plugin::AnimationKey;
use crate::general::components::map_components::CoolDown;
use bevy::math::Vec3;
use bevy::prelude::*;
use bevy_inspector_egui::InspectorOptions;
use std::collections::HashSet;

#[derive(Component, Reflect)]
pub struct InputKeyboard;

/// Tells the keyboard and mouse players' input to stand down, because something else on
/// screen wants the keys or the pointer — a tool screen's panel, a text field.
///
/// Absent in a normal match, so the game never pays for it. Only the keyboard/mouse
/// scheme is gated: a gamepad cannot click a panel or type into a field, so it never
/// conflicts and keeps driving the character.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GameInputBlock {
    /// Ignore key *presses*. Releases still land, so a key held when the block starts
    /// does not stay stuck down after it ends.
    pub keyboard: bool,
    /// Leave `AutoAim` where it is instead of following the cursor.
    pub mouse_aim: bool,
}

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Copy)]
pub enum ControlCommand {
    Throw,
    Jump,
    Build,
    /// Held: revive a downed teammate (E / Circle).
    Interact,
    /// One-shot: lob the current throwable (G / L1). Consumed by `throw_special`.
    ThrowSpecial,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Reflect, Default)]
#[reflect(Default)]
#[type_path = "avs"]
pub enum ControlRotation {
    #[default]
    Left,
    Right,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Reflect, Default)]
#[reflect(Default)]
#[type_path = "avs"]
pub enum ControlDirection {
    #[default]
    Forward,
    Backward,
    Left,
    Right,
}

pub trait Opposite {
    fn opposite(&self) -> Self;
}

impl Opposite for ControlDirection {
    fn opposite(&self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

impl Opposite for ControlRotation {
    fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Component, Default, Reflect, InspectorOptions)]
#[type_path = "avs"]
pub struct CharacterControl {
    pub triggers: HashSet<ControlCommand>,
    pub rotations: HashSet<ControlRotation>,
    pub directions: HashSet<ControlDirection>,
    pub walk_direction: Vec3,
    pub torque: Vec3,
    pub has_thrown: bool,
    pub speed: f32,
    pub max_speed: f32,
    pub turn_speed: f32,
    pub max_turn_speed: f32,
    pub rate_of_fire_per_minute: f32,
    pub fire_cool_down: f32,
}

impl CharacterControl {
    pub fn new(speed: f32, turn_speed: f32, rate_of_fire_per_minute: f32) -> Self {
        Self {
            triggers: HashSet::default(),
            rotations: HashSet::default(),
            directions: HashSet::default(),
            walk_direction: Vec3::ZERO,
            torque: Vec3::ZERO,
            has_thrown: false,
            speed,
            max_speed: speed,
            turn_speed,
            max_turn_speed: turn_speed,
            rate_of_fire_per_minute,
            fire_cool_down: 0.0,
        }
    }
}

impl CoolDown for CharacterControl {
    fn cool_down(&mut self, delta: f32) -> bool {
        self.fire_cool_down -= delta;
        if self.fire_cool_down <= 0.0 {
            self.fire_cool_down = 60.0 / self.rate_of_fire_per_minute;
            return true;
        }
        false
    }
}

#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct DynamicMovement;

#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct KinematicMovement;

/// Which way the legs walk relative to where the body faces. The feet always travel along
/// the walk direction; the stride decides how the body is turned on top of that so the
/// torso twist can reach the aim (`torso_twist::choose_stride`).
#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Reflect, Default)]
#[reflect(Default)]
#[type_path = "avs"]
pub enum Stride {
    #[default]
    Forward,
    /// Walking away from the aim, backpedalling.
    Back,
    /// Stepping toward the body's left, aim across the direction of travel.
    Left,
    Right,
}

impl Stride {
    pub const ALL: [Self; 4] = [Self::Forward, Self::Back, Self::Left, Self::Right];

    /// The clip key for walking this way, aiming or not.
    pub const fn key(self, aiming: bool) -> AnimationKey {
        match (self, aiming) {
            (Self::Forward, false) => AnimationKey::Walk,
            (Self::Forward, true) => AnimationKey::WalkShoot,
            (Self::Back, false) => AnimationKey::WalkBack,
            (Self::Back, true) => AnimationKey::WalkBackShoot,
            (Self::Left, false) => AnimationKey::StrafeLeft,
            (Self::Left, true) => AnimationKey::StrafeLeftShoot,
            (Self::Right, false) => AnimationKey::StrafeRight,
            (Self::Right, true) => AnimationKey::StrafeRightShoot,
        }
    }
}

#[derive(Component, Reflect)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct CharacterState {
    pub state: Vec<AnimationKey>,
    /// How a `Walk` on the stack is played: forwards, backwards or sideways.
    pub stride: Stride,
}

impl CharacterState {
    /// Resolves the current intent stack to the concrete animation clip that
    /// should play.  Handles composite states (e.g. Walk + IdleShoot →
    /// WalkShoot) so callers only need to push/pop simple intent keys.
    ///
    /// `IdleShoot` is the "gun raised" intent, held while a gun is fired
    /// (`player::systems::shoot::aim_animation`); `Throwing` is a one-shot throw.
    pub fn resolve(&self) -> AnimationKey {
        let has = |k: AnimationKey| self.state.contains(&k);
        if has(AnimationKey::Death) {
            return AnimationKey::Death;
        }
        if has(AnimationKey::Building) {
            return AnimationKey::Building;
        }
        if has(AnimationKey::Reload) {
            return AnimationKey::Reload;
        }
        if has(AnimationKey::Duck) {
            return AnimationKey::Duck;
        }
        if has(AnimationKey::Throwing) {
            return AnimationKey::Throwing;
        }
        let aiming = has(AnimationKey::IdleShoot);
        match (has(AnimationKey::Walk), aiming) {
            (true, _) => self.stride.key(aiming),
            (false, true) => AnimationKey::IdleShoot,
            (false, false) => AnimationKey::Idle,
        }
    }

    /// Change the stride. Returns the resolved clip key if the visible animation changed
    /// (only while walking), like `enter_state`.
    pub fn set_stride(&mut self, stride: Stride) -> Option<AnimationKey> {
        let prev = self.resolve();
        self.stride = stride;
        let next = self.resolve();
        if next == prev { None } else { Some(next) }
    }

    /// Push an intent state.  Returns the resolved clip key if the visible
    /// animation changed, `None` if it stayed the same.
    pub fn enter_state(&mut self, state: AnimationKey) -> Option<AnimationKey> {
        let prev = self.resolve();
        if !self.state.contains(&state) {
            self.state.push(state);
        }
        let next = self.resolve();
        if next == prev { None } else { Some(next) }
    }

    /// Remove an intent state.  Returns the resolved clip key if the visible
    /// animation changed, `None` if it stayed the same.
    pub fn leave_state(&mut self, state: AnimationKey) -> Option<AnimationKey> {
        let prev = self.resolve();
        self.state.retain(|s| s != &state);
        if self.state.is_empty() {
            self.state.push(AnimationKey::Idle);
        }
        let next = self.resolve();
        if next == prev { None } else { Some(next) }
    }
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            state: vec![AnimationKey::Idle],
            stride: Stride::Forward,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CharacterState, Stride};
    use crate::animation::animation_plugin::AnimationKey;

    #[test]
    fn the_stride_picks_the_walk_clip_and_only_matters_while_walking() {
        let mut s = CharacterState::default();
        assert_eq!(
            s.set_stride(Stride::Back),
            None,
            "standing: nothing to change"
        );
        assert_eq!(
            s.enter_state(AnimationKey::Walk),
            Some(AnimationKey::WalkBack)
        );
        assert_eq!(
            s.enter_state(AnimationKey::IdleShoot),
            Some(AnimationKey::WalkBackShoot)
        );
        assert_eq!(
            s.set_stride(Stride::Left),
            Some(AnimationKey::StrafeLeftShoot)
        );
        assert_eq!(
            s.leave_state(AnimationKey::Walk),
            Some(AnimationKey::IdleShoot)
        );
    }

    #[test]
    fn default_state_is_idle() {
        assert_eq!(CharacterState::default().resolve(), AnimationKey::Idle);
    }

    #[test]
    fn walk_plus_throwing_resolves_to_throwing() {
        let mut s = CharacterState::default();
        s.enter_state(AnimationKey::Walk);
        s.enter_state(AnimationKey::Throwing);
        assert_eq!(s.resolve(), AnimationKey::Throwing);
    }

    #[test]
    fn firing_a_gun_aims_and_walking_while_firing_walks_and_aims() {
        let mut s = CharacterState::default();
        assert_eq!(
            s.enter_state(AnimationKey::IdleShoot),
            Some(AnimationKey::IdleShoot)
        );
        assert_eq!(
            s.enter_state(AnimationKey::Walk),
            Some(AnimationKey::WalkShoot)
        );
        assert_eq!(
            s.leave_state(AnimationKey::IdleShoot),
            Some(AnimationKey::Walk)
        );
    }

    #[test]
    fn a_throw_plays_over_aiming() {
        let mut s = CharacterState::default();
        s.enter_state(AnimationKey::IdleShoot);
        assert_eq!(
            s.enter_state(AnimationKey::Throwing),
            Some(AnimationKey::Throwing)
        );
        assert_eq!(
            s.leave_state(AnimationKey::Throwing),
            Some(AnimationKey::IdleShoot)
        );
    }

    #[test]
    fn death_takes_priority_over_movement() {
        let mut s = CharacterState::default();
        s.enter_state(AnimationKey::Walk);
        s.enter_state(AnimationKey::Death);
        assert_eq!(s.resolve(), AnimationKey::Death);
    }

    #[test]
    fn leaving_the_last_state_returns_to_idle() {
        let mut s = CharacterState::default();
        s.enter_state(AnimationKey::Walk);
        s.leave_state(AnimationKey::Walk);
        assert_eq!(s.resolve(), AnimationKey::Idle);
    }
}
