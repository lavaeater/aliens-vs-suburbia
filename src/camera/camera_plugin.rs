use bevy::app::{App, Plugin, PostUpdate, Startup};
use bevy::prelude::IntoSystemConfigs;
use bevy_xpbd_3d::PhysicsSet;
use bevy::transform::TransformSystem;
use crate::camera::systems::{camera_follow, spawn_camera};

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
                .after(PhysicsSet::Sync)
                .before(TransformSystem::TransformPropagate),
        );
    }
}
