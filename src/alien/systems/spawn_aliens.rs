use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Commands, MessageReader, MessageWriter, Name, Query, Res, ResMut, Time, Transform};
use bevy::scene::SceneRoot;
use avian3d::prelude::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, Position, RigidBody};
use std::f32::consts::PI;
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::ai::components::move_towards_goal_components::MoveTowardsGoalData;
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
    mut spawn_alien_mw: MessageWriter<SpawnAlien>,
    mut alien_spawn_point_query: Query<(&Position, &mut AlienSpawnPoint)>,
) {
    for (position, mut alien_spawn_point) in alien_spawn_point_query.iter_mut() {
        if alien_spawn_point.cool_down(time_res.delta_secs()) {
            spawn_alien_mw.write(SpawnAlien {
                position: position.0,
            });
        }
    }
}


pub fn spawn_aliens(
    mut alien_counter: ResMut<AlienCounter>,
    mut spawn_alien_mr: MessageReader<SpawnAlien>,
    mut commands: Commands,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    game_assets: Res<GameAssets>,
    mut game_tracking_mw: MessageWriter<GameTrackingEvent>,
) {
    if alien_counter.count >= alien_counter.max_count {
        return;
    }
    for spawn_alien in spawn_alien_mr.read() {
        alien_counter.count += 1;

        let alien_transform = Transform::from_xyz(spawn_alien.position.x, spawn_alien.position.y, spawn_alien.position.z)
            .with_scale(Vec3::new(0.25, 0.25, 0.25))
            .with_rotation(Quat::from_rotation_y(PI * 2.0));
      
        let id = commands.spawn((
            (
                Name::from("Spider"),
                HittableTarget {},
                DynamicMovement {},
                FixSceneTransform::new(
                    Vec3::new(0.0, -0.35, 0.0),
                    Quat::from_euler(EulerRot::YXZ, 180.0f32.to_radians(), 0.0, 0.0),
                    Vec3::new(0.5, 0.5, 0.5),
                ),
                CharacterControl::new(1.0, 3.0, 1.0),
                SceneRoot(game_assets.alien_scene.clone()),
                alien_transform,
                Friction::new(0.0),
                AngularDamping(1.0),
                LinearDamping(0.9),
                RigidBody::Dynamic,
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
            ),
            (
                CurrentTile::default(),
                CurrentAnimationKey::new("aliens".into(), AnimationKey::Walking),
                Alien {},
                AvoidWallsData::new(0.125, 0.125, 0.125, 5.0),
                ApproachAndAttackPlayerData::default(),
                MoveTowardsGoalData { path: None },
                AlienSightShape::default(),
                Attack::default(),
                Health::default(),
                WindWakerShaderBuilder::default().build(),
            )
        )).id();
      
        add_health_bar_mw.write(AddHealthBar {
            entity: id,
            name: "ALIEN",
        });

        game_tracking_mw.write(GameTrackingEvent::AlienSpawned);
    }
}
