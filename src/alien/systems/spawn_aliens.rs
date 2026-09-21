
use crate::alien::components::general::{Alien, AlienCounter};
use crate::general::damage::Faction;
use crate::loot::LootDrop;
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::map_components::{AlienSpawnPoint, CoolDown};
use crate::general::events::map_events::SpawnAlien;
use crate::ui::spawn_ui::AddHealthBar;

use bevy::math::{Quat, Vec3};
use bevy::prelude::{
    AssetServer, Commands, Entity, MessageReader, MessageWriter, Name, Query, Res, ResMut, Time, Transform,
};
use avian3d::prelude::Collider;
use crate::alien::enemy_defs::{EnemyDef, EnemyDefCache, LoadedEnemyDef, RangedAttack};
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::control::components::CharacterControl;
use crate::general::components::{Health, TouchDamage};
use crate::general::damage::DamageResistances;
use crate::general::explosion::ExplodesOnDeath;
use crate::player::systems::spawn_players::FixSceneTransform;
use bevy::world_serialization::WorldAssetRoot;
use std::f32::consts::PI;
use avian3d::prelude::Position;
use crate::alien::wave_manager::WaveManager;

pub fn alien_spawner_system(
    time_res: Res<Time>,
    mut spawn_alien_mw: MessageWriter<SpawnAlien>,
    mut alien_spawn_point_query: Query<(&Position, &mut AlienSpawnPoint)>,
    wave_manager: Option<Res<crate::alien::wave_manager::WaveManager>>,
) {
    // Don't spawn between waves or before the first wave starts.
    let mut enemy_def = None;
    if let Some(wm) = wave_manager {
        if !wm.spawning {
            return;
        }
        let wave = wm.waves.get(wm.current_wave);
        let wave_limit = wave.map_or(0, |w| w.alien_count);
        if wm.spawned_this_wave >= wave_limit {
            return;
        }
        enemy_def = wave.and_then(super::super::wave_manager::WaveDef::enemy_def);
    }

    for (position, mut alien_spawn_point) in alien_spawn_point_query.iter_mut() {
        if alien_spawn_point.cool_down(time_res.delta_secs()) {
            spawn_alien_mw.write(SpawnAlien {
                enemy_def: enemy_def.clone(),
                position: position.0,
            });
        }
    }
}

/// Build an alien from an Enemy def. The `Alien` required-components bundle supplies the
/// AI, physics and collision layers; the def overrides model, scale, health, speed,
/// touch damage, loot, resistances, attack and death behaviour.
fn spawn_from_def(commands: &mut Commands, path: &str, loaded: &LoadedEnemyDef, position: Vec3) -> Entity {
    let props = &loaded.props;
    let mut ec = commands.spawn((
        Alien,
        Faction::Alien,
        EnemyDef(path.to_string()),
        Name::new(format!("Enemy {}", std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy()).unwrap_or_default())),
        Transform::from_translation(position),
        WorldAssetRoot(loaded.scene.clone()),
        // Same convention as players: unit root, the model scaled by its def.
        FixSceneTransform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::splat(loaded.def.scale)),
        Collider::cuboid(0.5, 0.5, 0.45),
        CharacterControl::new(props.speed, 3.0, 1.0),
        Health::full(props.health as i32),
        TouchDamage { dps: props.touch_dps },
        CurrentAnimationKey::new(path.to_string(), AnimationKey::Walk),
    ));
    if let Some(table) = &props.loot_table {
        ec.insert(LootDrop(table.clone()));
    }
    if !props.resistances.is_empty() {
        ec.insert(DamageResistances(props.resistances.clone()));
    }
    if let Some(blast) = &props.explodes_on_death {
        ec.insert(ExplodesOnDeath(blast.clone()));
    }
    if let Some(ranged) = RangedAttack::from_attack(&props.attack) {
        ec.insert(ranged);
    }
    ec.id()
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_aliens(
    mut alien_counter: ResMut<AlienCounter>,
    mut spawn_alien_mr: MessageReader<SpawnAlien>,
    mut commands: Commands,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    game_assets: Res<GameAssets>,
    mut game_tracking_mw: MessageWriter<GameTrackingEvent>,
    mut wave_manager: Option<ResMut<WaveManager>>,
    mut def_cache: ResMut<EnemyDefCache>,
    asset_server: Res<AssetServer>,
) {
    if alien_counter.count >= alien_counter.max_count {
        return;
    }
    for spawn_alien in spawn_alien_mr.read() {
        alien_counter.count = alien_counter.count.saturating_add(1);

        // Def-driven enemy: model, stats and behaviour from the wave's def.
        if let Some(path) = &spawn_alien.enemy_def
            && let Some(loaded) = def_cache.get_or_load(path, &asset_server)
        {
            let id = spawn_from_def(&mut commands, path, loaded, spawn_alien.position);
            add_health_bar_mw.write(AddHealthBar { entity: id, name: "ALIEN" });
            game_tracking_mw.write(GameTrackingEvent::AlienSpawned);
            if let Some(ref mut wm) = wave_manager {
                wm.spawned_this_wave = wm.spawned_this_wave.saturating_add(1);
            }
            continue;
        }

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
            Alien,
            Faction::Alien,
            LootDrop("alien".to_string()),
            alien_transform,
            WorldAssetRoot(game_assets.alien_scene.clone()),
            // WindWakerShaderBuilder::default().build(),
            // PixelShaderBuilder::default()
            //     .pixel_density(1.0)   // lower = blockier
            //     .color_levels(2.0)    // lower = fewer colors
            //     .build(),

        )).id();

        add_health_bar_mw.write(AddHealthBar {
            entity: id,
            name: "ALIEN",
        });

        game_tracking_mw.write(GameTrackingEvent::AlienSpawned);

        if let Some(ref mut wm) = wave_manager {
            wm.spawned_this_wave = wm.spawned_this_wave.saturating_add(1);
        }
    }
}

