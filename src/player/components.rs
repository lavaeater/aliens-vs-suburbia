use bevy::math::Vec3;
use bevy::prelude::{Component, Entity};
use bevy::reflect::Reflect;

#[derive(Component, Default, Reflect, Clone)]
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

#[derive(Component, Default, Reflect, Clone)]
pub struct AutoAim(pub Vec3);
