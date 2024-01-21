use bevy::math::Vec3;
use bevy::prelude::*;
use bevy_inspector_egui::InspectorOptions;
use crate::animation::animation_plugin::AnimationKey;
use crate::general::components::map_components::CoolDown;

#[derive(Component, Default, Reflect, Clone)]
#[reflect(Component)]
pub struct InputKeyboard;

#[derive(Reflect, Clone, PartialEq, Eq, Hash, Debug, Default, Copy)]
pub struct ControllerFlag(pub u8);

impl ControllerFlag {
    pub const NOTHING: Self = ControllerFlag(0);
    pub const THROW: Self = ControllerFlag(1);
    pub const JUMP: Self = ControllerFlag(2);
    pub const BUILD: Self = ControllerFlag(4);
    pub const LEFT: Self = ControllerFlag(8);
    pub const RIGHT: Self = ControllerFlag(16);
    pub const FORWARD: Self = ControllerFlag(32);
    pub const BACKWARD: Self = ControllerFlag(64);

    pub fn set(&mut self, flag: ControllerFlag) {
        self.0 |= flag.0;
    }

    pub fn unset(&mut self, flag: ControllerFlag) -> bool {
        if self.has(flag) {
            self.0 &= !flag.0;
            return true;
        }
        return false;
    }

    pub fn has(&self, flag: ControllerFlag) -> bool {
        self.0 & flag.0 == flag.0
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

pub trait Opposite {
    fn opposite(&self) -> Self;
}

impl Opposite for ControllerFlag {
    fn opposite(&self) -> Self {
        match self {
            &ControllerFlag::THROW => { ControllerFlag::NOTHING }
            &ControllerFlag::JUMP => { ControllerFlag::NOTHING }
            &ControllerFlag::BUILD => { ControllerFlag::NOTHING }
            &ControllerFlag::LEFT => { ControllerFlag::RIGHT }
            &ControllerFlag::RIGHT => { ControllerFlag::LEFT }
            &ControllerFlag::FORWARD => { ControllerFlag::BACKWARD }
            &ControllerFlag::BACKWARD => { ControllerFlag::FORWARD }
            _ => { ControllerFlag::NOTHING }
        }
    }
}

#[derive(Component, Reflect, Clone, InspectorOptions)]
#[reflect(Component)]
pub struct CharacterControl {
    pub rotations: ControllerFlag,
    pub triggers: ControllerFlag,
    pub directions: ControllerFlag,
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

impl Default for CharacterControl {
    fn default() -> Self {
        Self {
            rotations: ControllerFlag::NOTHING,
            triggers: ControllerFlag::NOTHING,
            directions: ControllerFlag::NOTHING,
            walk_direction: Vec3::ZERO,
            torque: Vec3::ZERO,
            has_thrown: false,
            speed: 3.0,
            max_speed: 3.0,
            turn_speed: 3.0,
            max_turn_speed: 3.0,
            rate_of_fire_per_minute: 60.0,
            fire_cool_down: 0.0,
        }
    }
}

impl CharacterControl {
    pub fn new(speed: f32, turn_speed: f32, rate_of_fire_per_minute: f32) -> Self {
        Self {
            speed,
            max_speed: speed,
            turn_speed,
            max_turn_speed: turn_speed,
            rate_of_fire_per_minute,
            ..default()
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

#[derive(Component, Default, Reflect, Clone)]
#[reflect(Component)]
pub struct DynamicMovement;


#[derive(Component)]
pub struct KinematicMovement;

#[derive(Component, Reflect, Clone)]
#[reflect(Component)]
pub struct CharacterState {
    pub state: Vec<AnimationKey>,
}

impl CharacterState {
    pub fn enter_state(&mut self, state: AnimationKey) -> bool {
        if let Some(latest_state) = self.state.last() {
            if latest_state != &state {
                self.state.push(state);
                return true;
            }
        }
        false
    }

    pub fn leave_state(&mut self, state: AnimationKey) -> (bool, AnimationKey) {
        if self.state.len() > 1 {
            if let Some(latest_state) = self.state.last() {
                if latest_state == &state {
                    self.state.pop();
                    return (true, *self.state.last().unwrap());
                }
            }
        }
        (false, state)
    }
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            state: vec![AnimationKey::Idle]
        }
    }
}
