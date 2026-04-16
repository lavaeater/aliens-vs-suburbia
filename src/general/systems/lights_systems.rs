use bevy::math::Quat;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLight};
use bevy::prelude::{Commands, EulerRot, Name, Transform, default};

pub fn spawn_lights(
    mut commands: Commands,
) {
    commands.spawn((
        Name::from("Directional Light"),
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform {
            rotation: Quat::from_euler(EulerRot::XYZ, -0.5, 0.2, 0.4),
            ..default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 10.0,
            ..default()
        }.build(),
    ));
}
