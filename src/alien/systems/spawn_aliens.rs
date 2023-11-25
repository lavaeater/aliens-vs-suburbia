use bevy::animation::AnimationClip;
use bevy::asset::{AssetServer, Handle};

use bevy::hierarchy::Parent;
use bevy::log::info;
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Added, AnimationPlayer, Commands, Component, Entity, EventReader, EventWriter, Name, Query, Reflect, Res, ResMut, Resource, Time, Transform};
use bevy::scene::SceneBundle;
use bevy::utils::{HashMap};
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

#[derive(Resource)]
pub struct AnimationStore<S: Into<String>> {
    pub anims: HashMap<S, HashMap<AnimationKey, Handle<AnimationClip>>>,
}

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug, Reflect)]
pub enum AnimationKey {
    Clapping,
    Death,
    Idle,
    IdleHold,
    Jump,
    Punch,
    Run,
    RunHold,
    RunningJump,
    Sitting,
    Standing,
    Swimming,
    SwordSlash,
    Walking,
}

pub fn load_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let mut store = AnimationStore::<String> {
        anims: HashMap::new()
    };
    store.anims.insert("aliens".into(),
                       HashMap::new());
    let alien_anims = store
        .anims
        .get_mut("aliens")
        .unwrap();
    alien_anims.insert(AnimationKey::Walking, asset_server.load("quaternius/alien.glb#Animation13"));
    alien_anims.insert(AnimationKey::Idle, asset_server.load("quaternius/alien.glb#Animation2"));


    store
        .anims
        .insert("players".into(),
                HashMap::new());
    let player_anims = store
        .anims
        .get_mut("players")
        .unwrap();
    player_anims.insert(AnimationKey::Walking, asset_server.load("quaternius/worker.glb#Animation22"));
    player_anims.insert(AnimationKey::Idle, asset_server.load("quaternius/worker.glb#Animation4"));

    commands.insert_resource(store);
}

#[derive(Component, Debug, Reflect)]
pub struct CurrentAnimationKey {
    pub group: String,
    pub key: AnimationKey,
}

impl CurrentAnimationKey {
    pub fn new(group: String, key: AnimationKey) -> Self {
        CurrentAnimationKey { group, key }
    }
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
                FixSceneTransform::new(
                    Vec3::new(0.0, -0.35, 0.0),
                    Quat::from_euler(
                        EulerRot::YXZ,
                        180.0f32.to_radians(), 0.0, 0.0),
                    Vec3::new(0.5, 0.5, 0.5),
                ),
                Controller::new(1.0, 3.0, 1.0),
                SceneBundle {
                    scene: asset_server.load("quaternius/alien.glb#Scene0"),
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
                        CollisionLayer::Sensor
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
    }
}

pub fn start_some_animations(
    anim_store: Res<AnimationStore<String>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    anim_key_query: Query<&CurrentAnimationKey>,
    parent_query: Query<&Parent>,
) {
    for (entity, mut anim_player) in players.iter_mut() {
        if let Some(super_ent) = get_parent_recursive(entity, &parent_query) {
            if let Ok(anim_key) = anim_key_query.get(super_ent) {
                if let Some(anim) = anim_store.anims.get(&anim_key.group).unwrap().get(&anim_key.key) {
                    anim_player.play(anim.clone_weak()).repeat();
                }
            }
        }
    }
}

pub fn get_parent_recursive(entity: Entity, parent_query: &Query<&Parent>) -> Option<Entity> {
    match parent_query.get(entity) {
        Ok(parent) => {
            get_parent_recursive(parent.get(), parent_query)
        }
        Err(_) => {
            Some(entity)
        }
    }
}