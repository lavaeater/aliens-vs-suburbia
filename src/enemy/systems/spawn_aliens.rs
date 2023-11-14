use bevy::asset::AssetServer;
use bevy::prelude::{Commands, Name, Res, Transform};
use bevy::scene::SceneBundle;
use bevy::utils::default;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use big_brain::actions::Steps;
use big_brain::pickers::FirstToScore;
use big_brain::thinker::Thinker;
use crate::ai::components::approach_and_attack_player_components::{ApproachPlayerAction, ApproachAndAttackPlayerData, ApproachAndAttackPlayerScore, AttackPlayerAction};
use crate::ai::components::avoid_wall_components::{AvoidWallsAction, AvoidWallScore, AvoidWallsData};
use crate::ai::components::move_forward_components::{MoveForwardAction, MoveForwardScore};
use crate::enemy::components::general::{Alien, AlienSightShape};
use crate::general::components::{Attack, Health, HittableTarget, Layer};
use crate::player::components::general::{Controller, DynamicMovement};

pub fn spawn_aliens(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let avoid_walls = Steps::build()
        .label("Avoid Walls")
        // ...AvoidWalls...
        .step(AvoidWallsAction {});

    // Build the thinker
    let thinker = Thinker::build()
        .label("Spider Thinker")
        // We don't do anything unless we're thirsty enough.
        .picker(FirstToScore { threshold: 0.3 })
        .when(AvoidWallScore, avoid_walls)
        .when(ApproachAndAttackPlayerScore,
              Steps::build()
                  .label("Approach and Attack Player")
                  // ...ApproachPlayer...
                  .step(ApproachPlayerAction {})
                  // ...AttackPlayer...
                  .step(AttackPlayerAction {}))
        .when(MoveForwardScore,
              Steps::build()
                  .label("Move Forward")
                  .step(MoveForwardAction {}));

    commands.spawn(
        (
            Name::from("Spider"),
            // // We rotat the cone since it is defined as a cone pointing up the y axis. Rotating it -90 degrees around the x axis makes it point forward properly. Maybe.
            HittableTarget {},
            DynamicMovement {},
            Controller {
                turn_speed: 4.0,
                ..default()
            },
            SceneBundle {
                scene: asset_server.load("player.glb#Scene0"),
                transform: Transform::from_xyz(1.0* 2.0, 0.0, 2.0 * 2.0),
                ..Default::default()
            },
            Friction::from(0.0),
            AngularDamping(1.0),
            LinearDamping(0.9),
            RigidBody::Dynamic,
            //AsyncCollider(ComputedCollider::ConvexHull),
            Collider::cuboid(0.5, 0.5, 0.45),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            CollisionLayers::new([Layer::Alien], [Layer::Ball, Layer::Wall, Layer::Floor, Layer::Alien, Layer::Player, Layer::AlienSpawnPoint, Layer::AlienGoal]),
        )).insert((
        Alien {},
        AvoidWallsData::new(1.5),
        ApproachAndAttackPlayerData::default(),
        AlienSightShape::default(),
        Attack::default(),
        Health::default(),
        thinker
    ));
}