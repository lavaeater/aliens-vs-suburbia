use crate::game_state::game_state_plugin::GamePlugin;
use bevy::app::{App, Startup};
use bevy::prelude::Msaa;
use bevy::DefaultPlugins;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use space_editor::prelude::{simple_editor_setup, PrefabPlugin, XpbdPlugin};
use space_editor::SpaceEditorPlugin;
use type_register_plugin::TypeRegisterPlugin;

pub(crate) mod ai;
pub(crate) mod alien;
mod animation;
mod assets;
mod building;
pub(crate) mod camera;
mod constants;
mod control;
pub(crate) mod game_state;
pub(crate) mod general;
mod generate_mesh;
mod inspection;
mod map;
pub(crate) mod player;
mod playground;
pub(crate) mod towers;
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
        .add_plugins((PrefabPlugin, XpbdPlugin))
        .add_plugins(TypeRegisterPlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(AtmospherePlugin)
        .add_plugins(GamePlugin)
        .run();
}
