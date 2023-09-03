use bevy::asset::AssetServer;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::SceneBundle;

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    // note that we have to include the `Scene0` label
    let my_gltf = asset_server.load("my.glb#Scene0");

    // to position our 3d model, simply use the Transform
    // in the SceneBundle
    commands.spawn(SceneBundle {
        scene: my_gltf,
        transform: Transform::from_xyz(2.0, 0.0, -5.0),
        ..Default::default()
    });
}