use crate::game_state::game_state_plugin::GamePlugin;
use bevy::app::{App};
use bevy::prelude::Msaa;
use bevy::DefaultPlugins;
use bevy_atmosphere::plugin::AtmospherePlugin;
use bevy_xpbd_3d::plugins::PhysicsPlugins;

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
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(AtmospherePlugin)
        .add_plugins(GamePlugin)
        .run();
}
