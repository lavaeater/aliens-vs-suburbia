use bevy::math::Vec3;
use bevy::prelude::*;
use bevy::reflect::Reflect;

#[derive(Component, Default, Reflect, Clone)]
#[reflect(Component)]
pub struct Player {
    pub key: String
}

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct IsBuilding;

#[derive(Hash, PartialEq, Eq, Clone, Component)]
pub struct BuildingIndicator(pub Entity, pub i32);

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct IsBuildIndicator;

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct IsObstacle;

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct ShootingTower;

#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct AutoAim(pub Vec3);

impl Default for AutoAim {
    fn default() -> Self {
        Self(Vec3::Z)
    }
}