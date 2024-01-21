use bevy::math::{Quat};
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLight, DirectionalLightBundle};
use bevy::prelude::{Commands, EulerRot, Transform};
use bevy::utils::default;
use bevy::core::Name;

pub fn spawn_lights(
    mut commands: Commands,
) {
    commands.spawn((
        Name::from("Directional Light"),
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                illuminance: 5000.0,
                shadows_enabled: true,
                ..default()
            },
            transform: Transform {
                rotation: Quat::from_euler(EulerRot::XYZ, -0.5, 0.2, 0.4),
                ..default()
            },
// The default cascade config is designed to handle large scenes.
// As this example has a much smaller world, we can tighten the shadow
// bounds for better visual quality.
            cascade_shadow_config: CascadeShadowConfigBuilder::default()
                .into(),
            ..default()
        }));
}