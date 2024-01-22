use bevy::app::{App, Startup};
use bevy::DefaultPlugins;
use bevy::prelude::Msaa;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use space_editor::prelude::{PrefabPlugin, simple_editor_setup};
use space_editor::SpaceEditorPlugin;
use type_register_plugin::TypeRegisterPlugin;
use crate::game_state::game_state_plugin::GamePlugin;
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
mod type_register_plugin;

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
