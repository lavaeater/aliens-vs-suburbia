use bevy::app::{App, Plugin, PostUpdate, Startup};
use bevy::prelude::{in_state, IntoSystemConfigs, OnEnter};
use bevy_xpbd_3d::PhysicsSet;
use bevy::transform::TransformSystem;
use crate::camera::systems::{camera_follow, spawn_camera};
use crate::game_state::GameState;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                spawn_camera,
            ),
        )
            .add_systems(
                PostUpdate,
                camera_follow
                    .after(PhysicsSet::Sync)
                    .before(TransformSystem::TransformPropagate),
            );
    }
}


pub struct StatefulCameraPlugin;

impl Plugin for StatefulCameraPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                OnEnter(GameState::InGame),
                (
                    spawn_camera,
                ),
            )
            .add_systems(
                PostUpdate,
                camera_follow
                    .after(PhysicsSet::Sync)
                    .before(TransformSystem::TransformPropagate)
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
