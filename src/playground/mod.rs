use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmosphereCamera;
use crate::game_state::GameState;
use space_editor::prelude::PrefabPlugin;

pub struct PlaygroundPlugin;

impl Plugin for PlaygroundPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(PrefabPlugin)
            .add_systems(OnEnter(GameState::Playground), load_level);
    }
}

fn load_level(
    mut commands: Commands,
    asset_server: ResMut<AssetServer>,
) {

    // Render the mesh with the custom texture using a PbrBundle, add the marker.
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("levels/solarpunk_village.glb#Scene0"),
            ..default()
        }
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(1.8, 1.8, 1.8).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((
        Camera3dBundle {
            transform: camera_and_light_transform,
            ..default()
        },
        AtmosphereCamera::default(),
    ));

    // Light up the scene.
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1000.0,
            range: 100.0,
            ..default()
        },
        transform: camera_and_light_transform,
        ..default()
    });

    // Text to describe the controls.
    commands.spawn(
        TextBundle::from_section(
            "Controls:\nSpace: Change UVs\nX/Y/Z: Rotate\nR: Reset orientation",
            TextStyle {
                font_size: 20.0,
                ..default()
            },
        )
            .with_style(Style {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                ..default()
            }),
    );
}