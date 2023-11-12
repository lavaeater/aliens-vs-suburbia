use std::collections::HashMap;
use bevy::asset::AssetServer;
use bevy::math::{EulerRot, Quat};
use bevy::prelude::{Commands, Name, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use bonsai_bt::{Action, BT};
use crate::enemy::components::bonsai_ai_components::{BonsaiTree, BonsaiTreeStatus, LoiterData};
use crate::enemy::components::bonsai_ai_components::AlienBehavior::Loiter;
use crate::enemy::components::general::{Alien, AlienSightShape};
use crate::general::components::{HittableTarget, Layer};
use crate::player::components::general::{Controller, ControlRotation, DynamicMovement};

pub fn spawn_aliens(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // Create BT
    let loiter = Action(Loiter);
    let blackboard: HashMap<String, serde_json::Value> = HashMap::new();
    let mut bt = BT::new(loiter, blackboard);

    commands.spawn(
        (
            Name::from("Spider"),
            // // We rotat the cone since it is defined as a cone pointing up the y axis. Rotating it -90 degrees around the x axis makes it point forward properly. Maybe.
            HittableTarget {},
            DynamicMovement {},
            Controller::default(),
            SceneBundle {
                scene: asset_server.load("player.glb#Scene0"),
                transform: Transform::from_xyz(2.0, 0.0, 2.0),
                ..Default::default()
            },
            Friction::from(0.0),
            AngularDamping(1.0),
            LinearDamping(0.9),
            RigidBody::Dynamic,
            //AsyncCollider(ComputedCollider::ConvexHull),
            Collider::cuboid(0.5, 0.5, 0.45),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            CollisionLayers::new([Layer::Alien], [Layer::Ball, Layer::Wall, Layer::Floor, Layer::Alien, Layer::Player]),
        )).insert((
        Alien {},
        AlienSightShape(Collider::cone(5.0, 0.5), Quat::from_euler(EulerRot::YXZ, 0.0, -90.0, 0.0)),
        BonsaiTreeStatus {
            current_action_status: bonsai_bt::Status::Running,
        },
        BonsaiTree {
            tree: bt,
        },
        LoiterData {
            last_rotation_direction: ControlRotation::Left,
            turns: 0,
            max_turns: 100,
        }
    ));
}