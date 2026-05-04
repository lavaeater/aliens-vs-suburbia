use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::{AssetServer, Assets, Handle};
use bevy::gltf::Gltf;
use bevy::prelude::{Local, Res, ResMut, Resource};
use bevy::scene::Scene;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<GameAssets>()
            .add_systems(Startup, load_assets)
            .add_systems(Update, log_animation_names)
        ;
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

fn load_assets(
    asset_server: Res<AssetServer>,
    mut game_assets: ResMut<GameAssets>,
) {
    *game_assets = GameAssets {
        player_scene: asset_server.load("models/Adventurer.glb#Scene0"),
        ball_scene: asset_server.load("ball_fab.glb#Scene0"),
        alien_scene: asset_server.load("quaternius/alien.glb#Scene0"),
        alien_construct: asset_server.load("player.glb#Scene0"),
        player_gltf: asset_server.load("models/Adventurer.glb"),
        alien_gltf: asset_server.load("quaternius/alien.glb"),
    }
}

fn log_animation_names(
    game_assets: Res<GameAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    mut logged: Local<bool>,
) {
    if *logged {
        return;
    }
    let Some(player_gltf) = gltf_assets.get(&game_assets.player_gltf) else { return };
    let Some(alien_gltf) = gltf_assets.get(&game_assets.alien_gltf) else { return };

    println!("=== Adventurer.glb animations ===");
    for (i, _) in player_gltf.animations.iter().enumerate() {
        let name = player_gltf.named_animations.iter()
            .find(|(_, h)| player_gltf.animations[i] == **h)
            .map(|(n, _)| n.as_ref())
            .unwrap_or("<unnamed>");
        println!("  [{i}] {name}");
    }

    println!("=== alien.glb animations ===");
    for (i, _) in alien_gltf.animations.iter().enumerate() {
        let name = alien_gltf.named_animations.iter()
            .find(|(_, h)| alien_gltf.animations[i] == **h)
            .map(|(n, _)| n.as_ref())
            .unwrap_or("<unnamed>");
        println!("  [{i}] {name}");
    }

    *logged = true;
}