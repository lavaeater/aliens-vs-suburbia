use bevy::app::{App, FixedUpdate, Startup, Update};
use bevy::DefaultPlugins;
use bevy::prelude::{Mesh, Msaa};
use bevy::scene::Scene;
use bevy::time::{Fixed, Time};
use bevy::utils::HashMap;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::plugins::{PhysicsPlugins};
use crate::player::systems::spawn_players::spawn_players;
use camera::systems::spawn_camera::spawn_camera;
use crate::camera::components::camera::CameraOffset;
use crate::camera::systems::camera_follow::camera_follow;
use crate::enemy::systems::bonsai_ai_systems::{approach_player_system, can_i_see_player_system, loiter_system, update_behavior_tree};
use crate::enemy::systems::spawn_aliens::spawn_aliens;
use crate::general::systems::dynamic_movement::dynamic_movement;
use crate::general::systems::kill_the_balls::kill_the_balls;
use crate::general::systems::kinematic_movement::kinematic_movement;
use crate::general::systems::lights::spawn_lights;
use crate::general::systems::load_models::Handles;
use crate::general::systems::map::spawn_map;
use crate::general::systems::throwing::throwing;
use crate::player::systems::keyboard_control::keyboard_control;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod enemy;

pub const METERS_PER_PIXEL: f64 = 16.0;

fn main() {
    App::new()
        .register_type::<CameraOffset>()
        .insert_resource(Msaa::Sample4)
        .insert_resource(Handles::<Mesh> {
            handles: HashMap::new()
        })
        .insert_resource(Handles::<Scene> {
            handles: HashMap::new()
        })
        .insert_resource(Time::<Fixed>::from_seconds(0.1))
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        // .add_plugins(PhysicsDebugPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_systems(
            Startup,
            (
                spawn_map,
                spawn_players,
                spawn_aliens,
                spawn_camera,
                spawn_lights,
            ))
        .add_systems(
            Update,
            (
                camera_follow,
                keyboard_control,
                kinematic_movement,
                dynamic_movement,
                throwing,
                kill_the_balls,
            ))
        .add_systems(
            FixedUpdate,
            (
                // alien_sight,
                loiter_system,
                can_i_see_player_system,
                approach_player_system,
                update_behavior_tree,
            ))
        .run();
}
