use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs, OnEnter};
use avian3d::prelude::PhysicsSystems;
use bevy::transform::TransformSystems;
use crate::camera::systems::{apply_camera_settings, camera_follow, init_wall_materials, spawn_camera, wall_occlusion_system};
use crate::game_state::GameState;

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
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    init_wall_materials,
                    wall_occlusion_system,
                    apply_camera_settings,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}
