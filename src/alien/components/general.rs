use bevy::math::{EulerRot, Quat};
use bevy::prelude::{Component, Resource};
use bevy_xpbd_3d::prelude::Collider;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Alien;

#[derive(Component, Clone, Debug)]
pub struct AlienSightShape {
    pub shape: Collider,
    pub rotation: Quat,
    pub range: f32,
}

impl Default for AlienSightShape {
    fn default() -> Self {
        AlienSightShape {
            shape: Collider::cone(5.0, 4.0),
            rotation: Quat::from_euler(EulerRot::YXZ, 0.0, -90.0, 0.0),
            range: 5.0,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct AlienCounter {
    pub count: u32,
    pub max_count: u32,
}

impl AlienCounter {
    pub fn new(max_count: u32) -> Self {
        Self {
            count: 0,
            max_count,
        }
    }
}
