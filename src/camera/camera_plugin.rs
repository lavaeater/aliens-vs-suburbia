use avian3d::prelude::{ PhysicsSystems};
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::schedule::SystemCondition;
use bevy::prelude::{in_state, resource_changed, IntoScheduleConfigs, OnEnter};
use crate::camera::systems::{apply_camera_settings, camera_follow, spawn_camera};
use crate::game_state::GameState;
use crate::settings::resources::GameSettings;

pub struct StatefulCameraPlugin;

impl Plugin for StatefulCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            (spawn_camera, apply_camera_settings).chain(),
        )
        .add_systems(
            OnEnter(GameState::ModelShowcase),
            (spawn_camera, apply_camera_settings).chain(),
        )
        .add_systems(
            PostUpdate,
            camera_follow
                .after(PhysicsSystems::Writeback)
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            Update,
            apply_camera_settings
                .run_if(resource_changed::<GameSettings>)
                .run_if(in_state(GameState::InGame).or_else(in_state(GameState::ModelShowcase))),
        );
    }
}
