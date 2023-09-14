use bevy::asset::{Asset, AssetServer, Handle};
use bevy::prelude::{Mesh, Res, ResMut, Resource};
use bevy::scene::Scene;
use bevy::utils::HashMap;

#[derive(Resource)]
pub struct Handles<T> where T: Asset {
    pub handles: HashMap<String, Handle<T>>,
}

pub fn load_models(
    asset_server: Res<AssetServer>,
    mut scene_handles: ResMut<Handles<Scene>>,
    mut mesh_handles: ResMut<Handles<Mesh>>,
) {
    let scene: Handle<Scene> = asset_server.load("player.glb#Scene0");
    let mesh: Handle<Mesh> = asset_server.load("player.glb#Mesh0");

    scene_handles.handles.insert("player".to_string(), scene);
    mesh_handles.handles.insert("player".to_string(), mesh);
}