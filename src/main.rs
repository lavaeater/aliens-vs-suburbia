use bevy::app::{App, Startup, Update};
use bevy::DefaultPlugins;
use bevy::prelude::{Mesh, Msaa};
use bevy::scene::Scene;
use bevy::utils::HashMap;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_third_person_camera::{ThirdPersonCamera, ThirdPersonCameraPlugin};
use bevy_xpbd_3d::components::Collider;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use crate::player::systems::spawn_players::spawn_players;
use camera::systems::spawn_camera::spawn_camera;
use crate::general::systems::dynamic_movement::dynamic_movement;
use crate::general::systems::kinematic_movement::kinematic_movement;
use crate::general::systems::lights::spawn_lights;
use crate::general::systems::load_models::Handles;
use crate::general::systems::map::spawn_map;
use crate::player::systems::keyboard_control::keyboard_control;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;

pub const METERS_PER_PIXEL: f64 = 16.0;


fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(Handles::<Mesh> {
            handles: HashMap::new()
        })
        .insert_resource(Handles::<Scene> {
            handles: HashMap::new()
        })
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(ThirdPersonCameraPlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(WorldInspectorPlugin::new())

        .add_systems(
            Startup,
            (
                spawn_map,
                spawn_players,
                spawn_camera,
                spawn_lights,
            ))
        .add_systems(
            Update,
            (
                keyboard_control,
                kinematic_movement,
                dynamic_movement,
            ))
        .run();
}
