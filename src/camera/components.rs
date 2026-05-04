use bevy::math::Vec3;
use bevy::prelude::Component;
use bevy::reflect::Reflect;

#[derive(Component)]
pub struct GameCamera {}

#[derive(Component, Reflect)]
pub struct CameraOffset(pub Vec3);

#[derive(Component)]
pub struct FollowCamera {
    pub offset: Vec3
}

#[derive(Component)]
pub struct PixelCanvas;
