use bevy::app::{App, Startup};
use bevy::DefaultPlugins;
use bevy::prelude::Msaa;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use crate::player::systems::spawn_players::spawn_players;

pub(crate) mod player;
pub(crate) mod general;

fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_systems(Startup, (
            spawn_players,
))
        .run();
}
