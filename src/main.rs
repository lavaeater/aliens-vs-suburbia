use belly::build::BellyPlugin;
use bevy::app::{App, PluginGroup, PreUpdate, Startup, Update};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::Msaa;
use bevy::time::{Fixed, Time};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use big_brain::BigBrainPlugin;
use crate::player::systems::spawn_players::spawn_players;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use camera::components::CameraOffset;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use crate::general::systems::collision_handling_system::collision_handling_system;
use crate::general::systems::health_monitor_system::health_monitor_system;
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::throwing_system::throwing;
use control::components::Controller;
use crate::towers::systems::{tower_has_alien_in_range_scorer_system, shoot_alien_system};
use crate::ui::spawn_ui::{add_health_bar, AddHealthBar, fellow_system, spawn_ui};
use alien::alien_plugin::AlienPlugin;
use building::build_mode_plugin::BuildModePlugin;
use camera::camera_plugin::CameraPlugin;
use ai::ai_plugin::AiPlugin;
use control::control_plugin::ControlPlugin;
use crate::game_state::game_state_plugin::GamePlugin;
use crate::map::map_plugins::MapPlugin;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod alien;
pub(crate) mod ai;
pub(crate) mod towers;
pub(crate) mod ui;
mod control;
mod building;
mod map;
pub(crate) mod game_state;


fn main() {
    App::new()
        .add_event::<AddHealthBar>()
        .register_type::<CameraOffset>()
        .register_type::<CurrentTile>()
        .register_type::<Controller>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .insert_resource(Msaa::Sample4)
        .insert_resource(Time::<Fixed>::from_seconds(0.05))
        .insert_resource(Msaa::Sample4)
        .add_plugins(
            DefaultPlugins.set(
                LogPlugin {
                    filter: "wgpu_core=warn,wgpu_hal=warn".into(),
                    level: log::Level::INFO,
                }))
        .add_plugins(PhysicsPlugins::default())
        // .add_plugins(PhysicsDebugPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(GamePlugin)
        .add_plugins(ControlPlugin)
        .add_plugins(CameraPlugin)
        .add_systems(
            Startup,
            (
                spawn_lights,
            ))
        .add_systems(
            Update,
            (
                spawn_players,
                throwing,
                collision_handling_system,
            ))
        .add_systems(
            Update,
            (
                shoot_alien_system,
                health_monitor_system,
                add_health_bar,
                fellow_system,
            ),
        )
        .add_systems(
            PreUpdate,
            (
                tower_has_alien_in_range_scorer_system,
            ),
        )
        .run();
}
