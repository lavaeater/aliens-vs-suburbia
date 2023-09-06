use bevy::asset::AssetServer;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::SceneBundle;

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    let scene = asset_server.load("player.glb#Scene0");

    // to position our 3d model, simply use the Transform
    // in the SceneBundle
    commands.spawn(SceneBundle {
        scene,
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
        ..Default::default()
    });
}