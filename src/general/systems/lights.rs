use bevy::math::{Quat, Vec3};
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLight, DirectionalLightBundle};
use bevy::prelude::{Commands, Transform};
use bevy::utils::default;
use std::f32::consts::PI;

pub fn spawn_lights(
    mut commands: Commands,
) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 1000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
// The default cascade config is designed to handle large scenes.
// As this example has a much smaller world, we can tighten the shadow
// bounds for better visual quality.
        cascade_shadow_config: CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 10.0,
            ..default()
        }
            .into(),
        ..default()
    });
}