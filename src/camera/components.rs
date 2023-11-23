use bevy::math::Vec3;
use bevy::prelude::Component;
use bevy::reflect::Reflect;
use bevy_xpbd_3d::math::Vector3;

#[derive(Component)]
pub struct GameCamera {}

#[derive(Component, Reflect)]
pub struct CameraOffset(pub Vec3);

#[derive(Component)]
pub struct FollowCamera {
    pub offset: Vector3
}
