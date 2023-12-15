use bevy::app::{App, Plugin, Startup};
use bevy::asset::{AssetServer, Handle};
use bevy::prelude::{Res, ResMut, Resource};
use bevy::scene::Scene;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<GameAssets>()
            .add_systems(Startup, load_assets)
        ;
    }
}

#[derive(Resource, Default)]
pub struct GameAssets {
    pub girl_scene: Handle<Scene>,
    pub ball_scene: Handle<Scene>,
    pub alien_scene: Handle<Scene>,
    pub alien_construct: Handle<Scene>,
}

fn load_assets(
    asset_server: Res<AssetServer>,
    mut game_assets: ResMut<GameAssets>,
) {
    *game_assets = GameAssets {
        girl_scene: asset_server.load("girl/girl.glb#Scene0"),
        ball_scene: asset_server.load("ball_fab.glb#Scene0"),
        alien_scene: asset_server.load("quaternius/alien.glb#Scene0"),
        alien_construct: asset_server.load("player.glb#Scene0"),
    }
}