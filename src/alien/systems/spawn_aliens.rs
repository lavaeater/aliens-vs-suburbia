use bevy::animation::AnimationClip;
use bevy::asset::{AssetServer, Handle};
use bevy::math::Vec3;
use bevy::prelude::{Added, AnimationPlayer, Commands, EventReader, EventWriter, Name, Query, Res, ResMut, Resource, Time, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use bevy_xpbd_3d::math::PI;
use bevy_xpbd_3d::prelude::Position;
use big_brain::actions::Steps;
use big_brain::pickers::Highest;
use big_brain::thinker::Thinker;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::{AvoidWallsAction, AvoidWallScore, AvoidWallsData};
use crate::ai::components::destroy_the_map_components::{DestroyTheMapAction, DestroyTheMapScore};
use crate::ai::components::move_towards_goal_components::{MoveTowardsGoalAction, MoveTowardsGoalData, MoveTowardsGoalScore};
use crate::alien::components::general::{Alien, AlienCounter, AlienSightShape};
use crate::control::components::{Controller, DynamicMovement};
use crate::general::components::{Attack, CollisionLayer, Health, HittableTarget};
use crate::general::components::map_components::{AlienSpawnPoint, CoolDown, CurrentTile};
use crate::general::events::map_events::SpawnAlien;
use crate::ui::spawn_ui::AddHealthBar;

pub fn alien_spawner_system(
    time_res: Res<Time>,
    mut spawn_alien_event_writer: EventWriter<SpawnAlien>,
    mut alien_spawn_point_query: Query<(&Position, &mut AlienSpawnPoint)>,
) {
    for (position, mut alien_spawn_point) in alien_spawn_point_query.iter_mut() {
        if alien_spawn_point.cool_down(time_res.delta_seconds()) {
            spawn_alien_event_writer.send(SpawnAlien {
                position: position.0,
            });
        }
    }
}

#[derive(Resource)]
pub struct AlienAnimations(Vec<Handle<AnimationClip>>);

pub fn load_alien_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(AlienAnimations(vec![
        asset_server.load("quaternius/alien_rotated.glb#Animation0"),
        asset_server.load("quaternius/alien_rotated.glb#Animation1"),
        asset_server.load("quaternius/alien_rotated.glb#Animation2"),
        asset_server.load("quaternius/alien_rotated.glb#Animation3"),
        asset_server.load("quaternius/alien_rotated.glb#Animation4"),
        asset_server.load("quaternius/alien_rotated.glb#Animation5"),
        asset_server.load("quaternius/alien_rotated.glb#Animation6"),
        asset_server.load("quaternius/alien_rotated.glb#Animation7"),
        asset_server.load("quaternius/alien_rotated.glb#Animation8"),
        asset_server.load("quaternius/alien_rotated.glb#Animation9"),
        asset_server.load("quaternius/alien_rotated.glb#Animation10"),
        asset_server.load("quaternius/alien_rotated.glb#Animation11"),
        asset_server.load("quaternius/alien_rotated.glb#Animation12"),
        asset_server.load("quaternius/alien_rotated.glb#Animation13"),
    ]));
}

pub fn spawn_aliens(
    mut alien_counter: ResMut<AlienCounter>,
    mut spawn_alien_event_reader: EventReader<SpawnAlien>,
    mut commands: Commands,
    mut add_health_bar_ew: EventWriter<AddHealthBar>,
    asset_server: Res<AssetServer>,
) {
    if alien_counter.count >= alien_counter.max_count {
        return;
    }
    for spawn_alien in spawn_alien_event_reader.read() {
        alien_counter.count += 1;
        let avoid_walls = Steps::build()
            .label("Avoid Walls")
            .step(AvoidWallsAction {});

        // Build the thinker
        let thinker = Thinker::build()
            .label("Spider Thinker")
            .picker(Highest {})
            .when(AvoidWallScore, avoid_walls)
            .when(MoveTowardsGoalScore,
                  Steps::build()
                      .label("Move Towards Goal")
                      .step(MoveTowardsGoalAction {}))
            .when(DestroyTheMapScore,
                  Steps::build()
                      .label("Destroy the Map")
                      .step(DestroyTheMapAction {}));

        let alien_transform = Transform::from_xyz(spawn_alien.position.x, spawn_alien.position.y, spawn_alien.position.z)
            .with_scale(Vec3::new(0.25, 0.25, 0.25))
            .with_rotation(bevy::math::Quat::from_rotation_y(PI * 2.0));
        let id = commands.spawn(
            (
                Name::from("Spider"),
                // // We rotat the cone since it is defined as a cone pointing up the y axis. Rotating it -90 degrees around the x axis makes it point forward properly. Maybe.
                HittableTarget {},
                DynamicMovement {},
                Controller::new(1.0, 3.0, 1.0),
                SceneBundle {
                    scene: asset_server.load("quaternius/alien_rotated.glb#Scene0"),
                    transform: alien_transform,
                    ..Default::default()
                },
                Friction::from(0.0),
                AngularDamping(1.0),
                LinearDamping(0.9),
                RigidBody::Dynamic,
                //AsyncCollider(ComputedCollider::ConvexHull),
                Collider::capsule(0.25, 0.25),
                LockedAxes::new().lock_rotation_x().lock_rotation_z(),
                CollisionLayers::new(
                    [CollisionLayer::Alien],
                    [
                        CollisionLayer::Ball,
                        CollisionLayer::Impassable,
                        CollisionLayer::Floor,
                        CollisionLayer::Alien,
                        CollisionLayer::Player,
                        CollisionLayer::AlienGoal,
                        CollisionLayer::Sensor
                    ]),
            )).insert((
            CurrentTile::default(),
            Alien {},
            AvoidWallsData::new(0.125, 0.125, 0.125, 5.0),
            ApproachAndAttackPlayerData::default(),
            MoveTowardsGoalData { path: None },
            AlienSightShape::default(),
            Attack::default(),
            Health::default(),
            thinker
        )).id();

        add_health_bar_ew.send(AddHealthBar {
            entity: id,
            name: "ALIEN",
        });
    }
}

pub fn animate_aliens(
    animations: Res<AlienAnimations>,
    mut players: Query<&mut AnimationPlayer, Added<AnimationPlayer>>,
) {
    for mut player in players.iter_mut() {
        player.play(animations.0[13].clone_weak()).repeat();
    }
}