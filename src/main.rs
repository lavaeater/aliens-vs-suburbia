use bevy::app::{App, Startup};
use bevy::DefaultPlugins;
use bevy::prelude::Msaa;
use bevy_atmosphere::plugin::AtmospherePlugin;
use avian3d::PhysicsPlugins;
use type_register_plugin::TypeRegisterPlugin;
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
mod constants;
mod assets;
mod inspection;
mod generate_mesh;
mod playground;
mod type_register_plugin;

fn main() {

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TypeRegisterPlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(AtmospherePlugin)
        .add_plugins(GamePlugin)
        .run();
}
    