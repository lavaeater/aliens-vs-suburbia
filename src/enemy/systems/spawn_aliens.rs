use std::collections::HashMap;
use bevy::asset::AssetServer;
use bevy::math::{EulerRot, Quat};
use bevy::prelude::{Commands, Name, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use bonsai_bt::{Action, BT, Invert, Select, Sequence, Wait, While};
use crate::enemy::components::bonsai_ai_components::{BonsaiTree, BonsaiTreeStatus, AlienBrain};
use crate::enemy::components::bonsai_ai_components::AlienBehavior::{ApproachPlayer, CanISeePlayer, Loiter};
use crate::enemy::components::general::{Alien, AlienSightShape};
use crate::general::components::{HittableTarget, Layer};
use crate::player::components::general::{Controller, ControlRotation, DynamicMovement};

pub fn spawn_aliens(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // Create BT
    // let loiter = Action(Loiter);
    let can_i_see_player = Action(CanISeePlayer);
    let approach_player = Action(ApproachPlayer);
    // let loiter_unless_see = While(Box::new(Invert(Box::new(can_i_see_player.clone()))), vec![loiter]);
    let approach_if_see = While(Box::new(can_i_see_player.clone()),vec![approach_player]);
    // let loiter_until_see = Select(vec![approach_if_see, loiter_unless_see]);
    let blackboard: HashMap<String, serde_json::Value> = HashMap::new();
    let bt = BT::new(approach_if_see, blackboard);

    commands.spawn(
        (
            Name::from("Spider"),
            // // We rotat the cone since it is defined as a cone pointing up the y axis. Rotating it -90 degrees around the x axis makes it point forward properly. Maybe.
            HittableTarget {},
            DynamicMovement {},
            Controller::default(),
            SceneBundle {
                scene: asset_server.load("player.glb#Scene0"),
                transform: Transform::from_xyz(8.0 * 2.0, 0.0, 4.0 * 2.0),
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
        AlienSightShape::default(),
        BonsaiTreeStatus {
            current_action_status: bonsai_bt::Status::Running,
        },
        BonsaiTree {
            tree: bt,
        },
        AlienBrain::default()
    ));
}