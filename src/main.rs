use bevy::app::{App, Plugin, PluginGroup};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::{Msaa, OnEnter};
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_mod_outline::OutlineBundle;
use bevy_video_glitch::VideoGlitchSettings;
use bevy_xpbd_3d::components::{CollisionLayers, LockedAxes};
use bevy_xpbd_3d::plugins::{PhysicsPlugins};
use bevy_xpbd_3d::prelude::Position;
use space_editor::prelude::{EditorRegistryExt, PrefabPlugin, simple_editor_setup};
use space_editor::space_prefab::ext::Startup;
use space_editor::SpaceEditorPlugin;
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::CurrentTile;
use control::components::CharacterControl;
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::camera::camera_components::CameraOffset;
use crate::control::components::{CharacterState, ControllerFlag, DynamicMovement, InputKeyboard};
use crate::game_state::game_state_plugin::GamePlugin;
use crate::game_state::GameState;
use crate::game_state::score_keeper::Score;
use crate::player::components::{AutoAim, Player};
use crate::playground::xpbd_plugin::CustomXpbdPlugin;

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
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(SpaceEditorPlugin)
        .add_plugins(TypeRegisterPlugin)
        .add_systems(Startup, simple_editor_setup)
        .run();


    #[cfg(feature = "default")]
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(
            (PrefabPlugin,
             CustomXpbdPlugin, )
        )
        .add_plugins(TypeRegisterPlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(AtmospherePlugin)
        .add_plugins(GamePlugin)
        .run();
}

pub struct TypeRegisterPlugin;

impl Plugin for TypeRegisterPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<CollisionLayer>()
            .register_type::<CameraOffset>()
            .register_type::<ControllerFlag>()
            .editor_registry::<Player>()
            .editor_registry::<InputKeyboard>()
            .editor_registry::<CharacterControl>()
            .editor_registry::<DynamicMovement>()
            .editor_registry::<LockedAxes>()
            .editor_registry::<CollisionLayers>()
            .editor_registry::<Health>()
            .editor_registry::<CurrentTile>()
            .register_type::<(usize, usize)>()
            .register_type::<AnimationKey>()
            .register_type::<Vec<AnimationKey>>()
            .editor_registry::<CurrentAnimationKey>()
            .editor_registry::<CharacterState>()
            .editor_registry::<Score>()
            .editor_registry::<AutoAim>()
            .editor_registry::<Position>();
    }
}
