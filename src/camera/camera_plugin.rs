use bevy::app::{App, Plugin, PostUpdate, Startup};
use bevy::prelude::{in_state, IntoScheduleConfigs, OnEnter};
use avian3d::prelude::PhysicsSystems;
use bevy::transform::TransformSystems;
use crate::camera::systems::{camera_follow, spawn_camera};
use crate::game_state::GameState;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                spawn_camera,
            )
        )
            .add_systems(
            PostUpdate,
            camera_follow
                .after(PhysicsSystems::Writeback)
                .before(TransformSystems::Propagate),
        );
    }
}


pub struct StatefulCameraPlugin;

impl Plugin for StatefulCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                spawn_camera,
            )
        )
            .add_systems(
                PostUpdate,
                camera_follow
                    .after(PhysicsSystems::Writeback)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
