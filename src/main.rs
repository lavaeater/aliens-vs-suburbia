use bevy::app::{App, PluginGroup};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::Msaa;
use bevy_xpbd_3d::plugins::{PhysicsPlugins};
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use camera::components::CameraOffset;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use control::components::CharacterControl;
use crate::game_state::game_state_plugin::GamePlugin;

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
mod animation;


fn main() {
    App::new()
        .register_type::<CameraOffset>()
        .register_type::<CurrentTile>()
        .register_type::<CharacterControl>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .insert_resource(Msaa::Sample4)
        .add_plugins(
            DefaultPlugins.set(
                LogPlugin {
                    filter: "wgpu_core=warn,wgpu_hal=warn".into(),
                    level: log::Level::INFO,
                }))
        .add_plugins(PhysicsPlugins::default())
        // .add_plugins(PhysicsDebugPlugin::default())
        // .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(GamePlugin)
        .run();
}
