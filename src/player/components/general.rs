use bevy::prelude::Component;
use bevy::reflect::Reflect;
use bevy::utils::HashSet;
use bevy_xpbd_3d::math::Vector3;

#[derive(Component)]
pub struct Player {}

#[derive(Component)]
pub struct FollowCamera {
    pub offset: Vector3
}

#[derive(Component, Reflect)]
pub struct KeyboardController {}

#[derive(Hash, PartialEq, Eq, Clone, Reflect)]
pub enum Triggers {
    Throw,
    Jump
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
    StrafeLeft,
    StrafeRight
}

pub trait Opposite {
    fn opposite(&self) -> Self;
}

impl Opposite for ControlDirection {
    fn opposite(&self) -> Self {
        match self {
            ControlDirection::Forward => ControlDirection::Backward,
            ControlDirection::Backward => ControlDirection::Forward,
            ControlDirection::StrafeLeft => ControlDirection::StrafeRight,
            ControlDirection::StrafeRight => ControlDirection::StrafeLeft,
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

#[derive(Component, Reflect)]
pub struct Controller {
    pub triggers: HashSet<Triggers>,
    pub rotations: HashSet<ControlRotation>,
    pub directions: HashSet<ControlDirection>,
    pub has_thrown:bool,
    pub speed: f32,
    pub turn_speed: f32,
    pub rate_of_fire: f32,
    pub fire_cool_down: f32
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            triggers: HashSet::default(),
            rotations: HashSet::default(),
            directions: HashSet::default(),
            has_thrown: false,
            speed: 2.0,
            turn_speed: 2.0,
            rate_of_fire: 5.0,
            fire_cool_down: 0.0
        }
    }
}

#[derive(Component)]
pub struct DynamicMovement {}


#[derive(Component)]
pub struct KinematicMovement {}
