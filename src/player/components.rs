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

/// Marks a player who is downed (health ≤ 0) and waiting for a revive.
/// While this component is present the player cannot move or act.
/// Removed when a teammate completes a revive.
#[derive(Component)]
pub struct PlayerDead {
    /// Accumulated revive progress from 0.0 (none) to 1.0 (complete).
    pub revive_progress: f32,
    /// Entity of the WorldFollower revive-progress bar, spawned on death.
    pub revive_bar: Option<Entity>,
}

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
  "Pistol_1",
  "Pistol_2",
    "Revolver",
    "Revolver_1",
    "Revolver_2",
    "Revolver_Small",
    "RocketLauncher",
    "ShortCannon",
    "Shotgun",
    "Shovel",
    "SMG",
    "Sniper",
    "Sniper_2",
];
