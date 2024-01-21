pub(crate) mod xpbd_plugin;
mod player_prefab;

use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmosphereCamera;
use bevy_mod_outline::{OutlineBundle, OutlineVolume};
use bevy_xpbd_3d::components::{Collider};
use crate::game_state::GameState;
use space_editor::prelude::{PrefabBundle};
use space_editor::space_editor_ui::ext::bevy_panorbit_camera;use crate::assets::assets_plugin::GameAssets;
use crate::player::bundle::PlayerBundle;
use crate::player::systems::spawn_players::FixSceneTransform;

pub struct PlaygroundPlugin;

impl Plugin for PlaygroundPlugin {
    fn build(&self, app: &mut App) {
        app

           .add_systems(
                OnEnter(GameState::Playground),
                (
                    load_level,
                ))
        ;
    }
}

fn load_level(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
) {

    // Render the mesh with the custom texture using a PbrBundle, add the marker.
    commands.spawn(
        PrefabBundle::new("levels/solar_punk_village.scn.ron"))
    ;

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(1.8, 1.8, 1.8).looking_at(Vec3::ZERO, Vec3::Y);


    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .insert((
            bevy_panorbit_camera::PanOrbitCamera::default(),
            AtmosphereCamera::default(),
        ))
    ;

    

    commands.spawn((
        FixSceneTransform::new(
            Vec3::new(0.0, -0.37, 0.0),
            Quat::from_euler(
                EulerRot::YXZ,
                180.0f32.to_radians(), 0.0, 0.0),
            Vec3::new(0.5, 0.5, 0.5),
        ),
        SceneBundle {
            scene: game_assets.girl_scene.clone(),
            transform: Transform::from_xyz(4.0, 2.0, 4.0),
            ..Default::default()
        },
        PlayerBundle::new(
            "player",
            "Player One",
        ),
        OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                width: 1.0,
                colour: Color::BLACK,
            },
            ..default()
        }
    )).with_children(|children|
        { // Spawn the child colliders positioned relative to the rigid body
            children.spawn(
                (
                    Collider::capsule(0.4, 0.2),
                    Transform::from_xyz(0.0, 0.0, 0.0)));
        });

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
    commands
        .spawn(
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