use bevy::math::Vec3;
use bevy::prelude::{Component, Reflect};
use std::collections::HashSet;
use bevy_inspector_egui::InspectorOptions;
use crate::animation::animation_plugin::AnimationKey;
use crate::general::components::map_components::CoolDown;

#[derive(Component, Reflect)]
pub struct InputKeyboard;

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Copy)]
pub enum ControlCommand {
    Throw,
    Jump,
    Build
}


#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Reflect)]
pub enum ControlRotation {
    Left,
    Right
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Reflect)]
pub enum ControlDirection {
    Forward,
    Backward,
    Left,
    Right
}

pub trait Opposite {
    fn opposite(&self) -> Self;
}

impl Opposite for ControlDirection {
    fn opposite(&self) -> Self {
        match self {
            ControlDirection::Forward => ControlDirection::Backward,
            ControlDirection::Backward => ControlDirection::Forward,
            ControlDirection::Left => ControlDirection::Right,
            ControlDirection::Right => ControlDirection::Left,
        }
    }
}

impl Opposite for ControlRotation {
    fn opposite(&self) -> Self {
        match self {
            ControlRotation::Left => ControlRotation::Right,
            ControlRotation::Right => ControlRotation::Left,
        }
    }
}

#[derive(Component, Reflect, InspectorOptions)]
pub struct CharacterControl {
    pub triggers: HashSet<ControlCommand>,
    pub rotations: HashSet<ControlRotation>,
    pub directions: HashSet<ControlDirection>,
    pub walk_direction: Vec3,
    pub torque: Vec3,
    pub has_thrown:bool,
    pub speed: f32,
    pub max_speed: f32,
    pub turn_speed: f32,
    pub max_turn_speed: f32,
    pub rate_of_fire_per_minute: f32,
    pub fire_cool_down: f32,
}

impl CharacterControl {
    pub fn new(speed: f32, turn_speed: f32, rate_of_fire_per_minute: f32, ) -> Self {
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


#[derive(Component)]
pub struct DynamicMovement;


#[derive(Component)]
pub struct KinematicMovement;

#[derive(Component)]
pub struct CharacterState {
    pub state: Vec<AnimationKey>,
}

impl CharacterState {
    /// Resolves the current intent stack to the concrete animation clip that
    /// should play.  Handles composite states (e.g. Walk + Throwing →
    /// WalkShoot) so callers only need to push/pop simple intent keys.
    pub fn resolve(&self) -> AnimationKey {
        let has = |k: AnimationKey| self.state.contains(&k);
        if has(AnimationKey::Death)    { return AnimationKey::Death; }
        if has(AnimationKey::Building) { return AnimationKey::Building; }
        if has(AnimationKey::Duck)     { return AnimationKey::Duck; }
        match (has(AnimationKey::Walk), has(AnimationKey::Throwing)) {
            (true,  true)  => AnimationKey::Throwing,
            (true,  false) => AnimationKey::Walk,
            (false, true)  => AnimationKey::Throwing,
            (false, false) => AnimationKey::Idle,
        }
    }

    /// Push an intent state.  Returns the resolved clip key if the visible
    /// animation changed, `None` if it stayed the same.
    pub fn enter_state(&mut self, state: AnimationKey) -> Option<AnimationKey> {
        let prev = self.resolve();
        if !self.state.contains(&state) {
            self.state.push(state);
        }
        let next = self.resolve();
        if next != prev { Some(next) } else { None }
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
        if next != prev { Some(next) } else { None }
    }
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            state: vec![AnimationKey::Idle],
        }
    }
}
