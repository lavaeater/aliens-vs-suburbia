use bevy::math::Quat;
use bevy::prelude::Component;
use bevy_xpbd_3d::components::Collider;


#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Alien;

#[derive(Component, Clone, Debug)]
pub struct AlienSightShape(pub Collider, pub Quat);