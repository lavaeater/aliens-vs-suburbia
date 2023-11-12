use bevy::prelude::Component;
use bevy::utils::HashSet;
use bevy_xpbd_3d::math::Vector3;

#[derive(Component)]
pub struct Player {}

#[derive(Component, )]
pub struct FollowCamera {
    pub offset: Vector3
}

#[derive(Component)]
pub struct KeyboardController {}

#[derive(Hash, PartialEq, Eq)]
pub enum Triggers {
    Throw,
    Jump
}

#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub enum ControlRotation {
    Left,
    Right
}

#[derive(Hash, PartialEq, Eq, Copy, Clone)]
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

#[derive(Component, Default)]
pub struct Controller {
    pub triggers: HashSet<Triggers>,
    pub rotations: HashSet<ControlRotation>,
    pub directions: HashSet<ControlDirection>,
    pub has_thrown:bool,
}

#[derive(Component)]
pub struct DynamicMovement {}


#[derive(Component)]
pub struct KinematicMovement {}
