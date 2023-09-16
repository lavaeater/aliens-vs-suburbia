use bevy::prelude::Component;
use bevy_xpbd_3d::math::Vector3;

#[derive(Component)]
pub struct Player {}

#[derive(Component, )]
pub struct FollowCamera {
    pub offset: Vector3
}

