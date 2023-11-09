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

#[derive(Hash, PartialEq, Eq)]
pub enum Rotations {
    Left,
    Right
}

#[derive(Hash, PartialEq, Eq)]
pub enum Directions {
    Forward,
    Backward,
    Left,
    Right
}

#[derive(Component, Default)]
pub struct Controller {
    pub triggers: HashSet<Triggers>,
    pub rotations: HashSet<Rotations>,
    pub directions: HashSet<Directions>,
    pub has_thrown:bool,
}

#[derive(Component)]
pub struct DynamicMovement {}


#[derive(Component)]
pub struct KinematicMovement {}
