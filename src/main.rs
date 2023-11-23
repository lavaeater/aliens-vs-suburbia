use belly::build::BellyPlugin;
use bevy::app::{App, Plugin, PluginGroup, PreUpdate, Startup, Update};
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
use crate::camera::components::camera::CameraOffset;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use crate::general::systems::collision_handling_system::collision_handling_system;
use crate::general::systems::health_monitor_system::health_monitor_system;
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::throwing_system::throwing;
use crate::player::components::general::Controller;
use crate::towers::systems::{alien_in_range_scorer_system, shoot_alien_system};
use crate::ui::spawn_ui::{add_health_bar, AddHealthBar, fellow_system, spawn_ui};
use bevy::prelude::IntoSystemConfigs;
use alien_plugin::AlienPlugin;
use build_mode_plugin::BuildModePlugin;
use camera_plugin::CameraPlugin;
use map_plugin::MapPlugin;
use crate::control_plugin::ControlPlugin;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod enemy;
pub(crate) mod ai;
pub(crate) mod towers;
pub(crate) mod ui;
mod build_mode_plugin;
mod map_plugin;
mod alien_plugin;
mod control_plugin;
mod camera_plugin;

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
        .add_plugins(BigBrainPlugin::new(PreUpdate))
        .add_plugins(BellyPlugin)
        .add_plugins(BuildModePlugin)
        .add_plugins(MapPlugin)
        .add_plugins(AlienPlugin)
        .add_plugins(ControlPlugin)
        .add_plugins(CameraPlugin)
        .add_systems(
            Startup,
            (
                spawn_lights,
                spawn_ui,
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
                alien_in_range_scorer_system,
            ),
        )
        .run();
}
