
use crate::alien::components::general::{Alien, AlienCounter};
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::map_components::{AlienSpawnPoint, CoolDown};
use crate::general::events::map_events::SpawnAlien;
use crate::ui::spawn_ui::AddHealthBar;

use bevy::math::{Quat, Vec3};
use bevy::prelude::{
    Commands, MessageReader, MessageWriter, Query, Res, ResMut, Time, Transform,
};
use bevy::scene::SceneRoot;
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use std::f32::consts::PI;
use avian3d::prelude::Position;

pub fn alien_spawner_system(
    time_res: Res<Time>,
    mut spawn_alien_mw: MessageWriter<SpawnAlien>,
    mut alien_spawn_point_query: Query<(&Position, &mut AlienSpawnPoint)>,
    wave_manager: Option<Res<crate::alien::wave_manager::WaveManager>>,
) {
    // Don't spawn between waves or before the first wave starts.
    if let Some(wm) = wave_manager {
        if !wm.spawning {
            return;
        }
        let wave_limit = wm
            .waves
            .get(wm.current_wave)
            .map(|w| w.alien_count)
            .unwrap_or(0);
        if wm.spawned_this_wave >= wave_limit {
            return;
        }
    }

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
    mut wave_manager: Option<ResMut<crate::alien::wave_manager::WaveManager>>,
) {
    if alien_counter.count >= alien_counter.max_count {
        return;
    }
    for spawn_alien in spawn_alien_mr.read() {
        alien_counter.count += 1;

        let alien_transform = Transform::from_xyz(
            spawn_alien.position.x,
            spawn_alien.position.y,
            spawn_alien.position.z,
        )
        .with_scale(Vec3::new(0.25, 0.25, 0.25))
        .with_rotation(Quat::from_rotation_y(PI * 2.0));

        /*
               CharacterControl::new(1.0, 3.0, 1.0),
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
               CurrentTile::default(),
               CurrentAnimationKey::new("aliens".into(), AnimationKey::Walk),
               AvoidWallsData::new(0.125, 0.125, 0.125, 5.0),
               ApproachAndAttackPlayerData::default(),
               MoveTowardsGoalData { path: None },
               AlienSightShape::default(),
               Attack::default(),
               Health::default(),
               TouchDamage { dps: 10.0 }
        */

        let id = commands.spawn((
            Alien::default(),
            alien_transform,
            SceneRoot(game_assets.alien_scene.clone()),
            WindWakerShaderBuilder::default().build(),
        )).id();

        add_health_bar_mw.write(AddHealthBar {
            entity: id,
            name: "ALIEN",
        });

        game_tracking_mw.write(GameTrackingEvent::AlienSpawned);

        if let Some(ref mut wm) = wave_manager {
            wm.spawned_this_wave += 1;
        }
    }
}

