use bevy::app::{App, Startup};
use bevy::DefaultPlugins;
use bevy::prelude::Msaa;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use crate::player::systems::spawn_players::spawn_players;
use crate::general::systems::camera::spawn_camera;
use crate::general::systems::lights::spawn_lights;

pub(crate) mod player;
pub(crate) mod general;

fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(WorldInspectorPlugin::new())

        .add_systems(Startup,
                     (
                         spawn_players,
                         spawn_camera,
                         spawn_lights
                     ))
        .run();
}
