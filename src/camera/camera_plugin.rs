use avian3d::prelude::{ PhysicsSystems};
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::schedule::SystemCondition;
use bevy::prelude::{in_state, resource_changed, IntoScheduleConfigs, OnEnter, ResMut};
use crate::camera::components::{CameraFocus, CameraShake};
use crate::camera::systems::{apply_camera_settings, camera_follow, spawn_camera};
use crate::game_state::GameState;
use crate::settings::resources::GameSettings;

pub struct StatefulCameraPlugin;

impl Plugin for StatefulCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraFocus>()
        .init_resource::<CameraShake>()
        .add_systems(
            OnEnter(GameState::InGame),
            (reset_focus, spawn_camera, apply_camera_settings).chain(),
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

/// A new level starts with an unprimed focus so the camera snaps to the players instead
/// of lerping over from wherever the last one ended.
fn reset_focus(mut focus: ResMut<CameraFocus>) {
    *focus = CameraFocus::default();
}
