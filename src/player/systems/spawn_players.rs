use bevy::asset::{Assets, AssetServer, LoadState};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::{Commands, Mesh, Res, Transform};
use bevy::scene::{SceneBundle};
use bevy_xpbd_3d::prelude::Collider;

pub fn spawn_players(
    mut commands: Commands,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_mesh_assets: Res<Assets<GltfMesh>>,
    mesh_assets: Res<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let gltf = asset_server.load("player.glb#Scene0");


    if let Some(actual_gltf) = gltf_assets.get(&gltf) {
        let mesh = gltf_mesh_assets.get(actual_gltf.meshes.first().unwrap()).unwrap();
        let mesh_mesh = mesh_assets.get(&mesh.primitives.first().unwrap().mesh).unwrap().clone();
        let scene = actual_gltf.scenes.first().unwrap();

        let collider = Collider::convex_decomposition_from_bevy_mesh(&mesh_mesh).unwrap();

        // to position our 3d model, simply use the Transform
        // in the SceneBundle
        commands.spawn((
            SceneBundle {
                scene: scene.clone(),
                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                ..Default::default()
            },
            collider));
    }
}