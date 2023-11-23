use bevy::prelude::{Component, Entity};
use bevy::reflect::Reflect;

#[derive(Component)]
pub struct Player {}

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct IsBuilding;

#[derive(Hash, PartialEq, Eq, Clone, Component)]
pub struct BuildingIndicator(pub Entity, pub i32);

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct IsBuildIndicator {}

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct IsObstacle {}

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component)]
pub struct ShootingTower {}
