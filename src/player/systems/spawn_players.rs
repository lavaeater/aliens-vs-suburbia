use bevy::asset::{Assets, AssetServer, Handle};
use bevy::gltf::Gltf;
use bevy::prelude::{Commands, Mesh, Res, Transform};
use bevy::scene::{Scene, SceneBundle};
use bevy_xpbd_3d::prelude::Collider;
use crate::general::systems::load_models::Handles;

pub fn spawn_players(
    mut commands: Commands,
    scene_handles: Res<Handles<Scene>>,
    mesh_handles: Res<Handles<Mesh>>,
    meshes: Res<Assets<Mesh>>
) {
    // let scene = scenes.get(&scene_handles.handles["player"]).unwrap();
    let mesh = meshes.get(&mesh_handles.handles.get("player").unwrap()).unwrap();

    let collider = Collider::convex_decomposition_from_bevy_mesh(&mesh).unwrap();

    // to position our 3d model, simply use the Transform
    // in the SceneBundle
    commands.spawn((SceneBundle {
        scene: scene_handles.handles["player"].clone(),
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
        ..Default::default()
    },
        collider));
}