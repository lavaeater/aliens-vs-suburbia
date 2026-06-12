use bevy::math::{EulerRot, Quat};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::ai::components::move_towards_goal_components::MoveTowardsGoalData;
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::control::components::{CharacterControl, KinematicMovement};
use crate::general::components::{Attack, CollisionLayer, Health, HittableTarget, TouchDamage};
use crate::player::systems::spawn_players::FixSceneTransform;
use avian3d::prelude::{
    AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes,
    RigidBody,
};
use crate::CurrentTile;

fn locked_axes()-> LockedAxes {
    LockedAxes::new().lock_rotation_x().lock_rotation_z()
}

#[derive(Component, Default, Reflect, Clone, Copy, Debug, PartialEq)]
#[type_path = "avs"]
#[require(
    Name::from("Alien"),
    HittableTarget,
    KinematicMovement,
    FixSceneTransform::new(
        Vec3::new(0.0, -0.35, 0.0),
        Quat::from_euler(EulerRot::YXZ, 180.0f32.to_radians(), 0.0, 0.0),
        Vec3::new(0.5, 0.5, 0.5),
    ),
    CharacterControl::new(1.0, 3.0, 1.0),
    Friction::new(0.0),
    AngularDamping(1.0),
    LinearDamping(0.9),
    RigidBody::Dynamic,
    Collider::capsule(1.0, 1.0),
    LockedAxes = locked_axes(),
    CollisionLayers::new(
       [CollisionLayer::Alien],
       [
           CollisionLayer::Ball,
           CollisionLayer::ImpassableAll,
           CollisionLayer::ImpassableAlien,
           CollisionLayer::Floor,
           CollisionLayer::Alien,
           CollisionLayer::Player,
           CollisionLayer::AlienGoal,
           CollisionLayer::Sensor,
           CollisionLayer::PlayerAimSensor,
       ]),
    CurrentTile::default(),
    CurrentAnimationKey::new("aliens".into(), AnimationKey::Walk),
    AvoidWallsData::new(0.125, 0.125, 0.125, 5.0),
    MoveTowardsGoalData { path: None },
    AlienSightShape::default(),
    Attack::default(),
    Health::default(),
    TouchDamage { dps: 10.0 }
)]
pub struct Alien;

#[derive(Component, Clone, Debug)]
#[allow(dead_code)]
pub struct AlienSightShape {
    pub shape: Collider,
    pub rotation: Quat,
    pub range: f32,
}

impl Default for AlienSightShape {
    fn default() -> Self {
        AlienSightShape {
            shape: Collider::cone(5.0, 4.0),
            rotation: Quat::from_euler(EulerRot::YXZ, 0.0, -90.0, 0.0),
            range: 5.0,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct AlienCounter {
    pub count: u32,
    pub max_count: u32,
}

impl AlienCounter {
    pub fn new(max_count: u32) -> Self {
        Self {
            count: 0,
            max_count,
        }
    }
}
