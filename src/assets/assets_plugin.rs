use bevy::app::{App, Plugin, Startup};
use bevy::asset::{AssetServer, Handle};
use bevy::gltf::{Gltf, GltfAssetLabel};
use bevy::prelude::{Res, ResMut, Resource};
use bevy::scene::Scene;
use crate::model_settings::resources::{CharacterFolder, ModelSettings};

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>()
            .add_systems(Startup, load_assets);
    }
}

#[derive(Resource, Default)]
pub struct GameAssets {
    pub player_scene: Handle<Scene>,
    pub ball_scene: Handle<Scene>,
    pub alien_scene: Handle<Scene>,
    pub alien_construct: Handle<Scene>,
    pub player_gltf: Handle<Gltf>,
    pub alien_gltf: Handle<Gltf>,
}

pub fn load_player_assets(
    asset_server: &AssetServer,
    game_assets: &mut GameAssets,
    model_settings: &ModelSettings,
    char_folder: &CharacterFolder,
) {
    let path = model_settings.current_model_path(char_folder)
        .unwrap_or_else(|| "packs/toon-shooter/characters/Character Soldier.glb".to_string());
    game_assets.player_scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
    game_assets.player_gltf = asset_server.load(path);
}

fn load_assets(
    asset_server: Res<AssetServer>,
    mut game_assets: ResMut<GameAssets>,
    model_settings: Res<ModelSettings>,
    char_folder: Res<CharacterFolder>,
) {
    load_player_assets(&asset_server, &mut game_assets, &model_settings, &char_folder);
    game_assets.ball_scene = asset_server.load("ball_fab.glb#Scene0");
    game_assets.alien_scene = asset_server.load("quaternius/alien.glb#Scene0");
    game_assets.alien_construct = asset_server.load("player.glb#Scene0");
    game_assets.alien_gltf = asset_server.load("quaternius/alien.glb");
}
