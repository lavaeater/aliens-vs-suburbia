use bevy::math::{EulerRot, Quat};
use bevy::prelude::Component;
use bevy_xpbd_3d::components::Collider;


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
            shape: Collider::cone(10.0, 10.0),
            rotation: Quat::from_euler(EulerRot::YXZ, 0.0, -90.0, 0.0),
            range: 5.0,
        }
    }
}