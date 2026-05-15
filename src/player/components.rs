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

/// Marks a player entity whose weapon nodes have been hidden.
#[derive(Component)]
pub struct WeaponsHidden;

/// Weapon mesh-node names present in the toon-shooter character models.
/// Nodes matching any of these names are hidden on spawn and can be revealed
/// individually by a pickup system later.
pub const WEAPON_NODES: &[&str] = &[
    "AK",
    "GrenadeLauncher",
    "Knife",
    "Knife_1",
    "Knife_2",
    "Pistol",
    "Revolver",
    "RevolverSmall",
    "RocketLauncher",
    "ShortCannon",
    "Shotgun",
    "Shovel",
    "SMG",
    "Sniper",
    "Sniper_2",
];
