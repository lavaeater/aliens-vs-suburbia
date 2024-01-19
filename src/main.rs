use bevy::app::{App, PluginGroup};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::Msaa;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_mod_outline::OutlineBundle;
use bevy_xpbd_3d::components::{CollisionLayers, LockedAxes};
use bevy_xpbd_3d::plugins::{PhysicsDebugPlugin, PhysicsPlugins};
use space_editor::prelude::{EditorRegistryExt, simple_editor_setup};
use space_editor::space_prefab::ext::Startup;
use space_editor::SpaceEditorPlugin;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use camera::camera_components::CameraOffset;
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::CurrentTile;
use control::components::CharacterControl;
use crate::animation::animation_plugin::CurrentAnimationKey;
use crate::control::components::{CharacterState, DynamicMovement, InputKeyboard};
use crate::game_state::game_state_plugin::GamePlugin;
use crate::game_state::score_keeper::Score;
use crate::player::components::{AutoAim, Player};

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
mod constants;
mod assets;
mod inspection;
mod generate_mesh;
mod playground;

fn main() {
    #[cfg(feature = "editor")]
    App::default()
        .add_plugins(DefaultPlugins)
        .add_plugins(SpaceEditorPlugin)
        .editor_registry::<Player>()
        .editor_registry::<InputKeyboard>()
        .editor_registry::<CharacterControl>()
        .editor_registry::<DynamicMovement>()
        .editor_registry::<LockedAxes>()
        .editor_registry::<CollisionLayers>()
        .editor_registry::<Health>()
        .editor_registry::<CurrentTile>()
        .editor_registry::<CurrentAnimationKey>()
        .editor_registry::<CharacterState>()
        .editor_registry::<Score>()
        .editor_registry::<AutoAim>()
        .add_systems(Startup, simple_editor_setup)
        .run();

    #[cfg(feature = "default")]
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
        .add_plugins(AtmospherePlugin)
        .add_plugins(PhysicsDebugPlugin::default())
        // .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(GamePlugin)
        .run();
}
