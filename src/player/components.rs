use bevy::math::Vec3;
use bevy::prelude::{Component, Entity};
use bevy::reflect::Reflect;

#[derive(Component)]
pub struct Player;

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

#[derive(Component)]
pub struct AutoAim(pub Vec3);

/// Marks a scene entity whose outline InheritOutline components have been applied.
#[derive(Component)]
pub struct OutlineDone;

/// Marks a player entity whose non-Character_ weapon nodes have been hidden.
#[derive(Component)]
pub struct WeaponsHidden;
