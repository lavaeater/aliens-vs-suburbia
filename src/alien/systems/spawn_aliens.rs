use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{ Commands, EventReader, EventWriter, Name, Query, Res, ResMut, Time, Transform};
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
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::assets::assets_plugin::GameAssets;
use crate::control::components::{CharacterControl, DynamicMovement};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::{Attack, CollisionLayer, Health, HittableTarget};
use crate::general::components::map_components::{AlienSpawnPoint, CoolDown, CurrentTile};
use crate::general::events::map_events::SpawnAlien;
use crate::player::systems::spawn_players::FixSceneTransform;
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


pub fn spawn_aliens(
    mut alien_counter: ResMut<AlienCounter>,
    mut spawn_alien_event_reader: EventReader<SpawnAlien>,
    mut commands: Commands,
    mut add_health_bar_ew: EventWriter<AddHealthBar>,
    game_assets: Res<GameAssets>,
    mut game_tracking_event_ew: EventWriter<GameTrackingEvent>
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
            .with_rotation(Quat::from_rotation_y(PI * 2.0));
        let id = commands.spawn(
            (
                Name::from("Spider"),
                // // We rotat the cone since it is defined as a cone pointing up the y axis. Rotating it -90 degrees around the x axis makes it point forward properly. Maybe.
                HittableTarget {},
                DynamicMovement {},
                FixSceneTransform::new(
                    Vec3::new(0.0, -0.35, 0.0),
                    Quat::from_euler(
                        EulerRot::YXZ,
                        180.0f32.to_radians(), 0.0, 0.0),
                    Vec3::new(0.5, 0.5, 0.5),
                ),
                CharacterControl::new(1.0, 3.0, 1.0),
                SceneBundle {
                    scene: game_assets.alien_scene.clone(),
                    transform: alien_transform,
                    ..Default::default()
                },
                Friction::from(0.0),
                AngularDamping(1.0),
                LinearDamping(0.9),
                RigidBody::Dynamic,
                //AsyncCollider(ComputedCollider::ConvexHull),
                Collider::capsule(1.0, 1.0),
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
                        CollisionLayer::Sensor,
                        CollisionLayer::PlayerAimSensor,
                    ]),
            )).insert((
            CurrentTile::default(),
            CurrentAnimationKey::new("aliens".into(), AnimationKey::Walking),
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

        game_tracking_event_ew.send(GameTrackingEvent::AlienSpawned);
    }
}