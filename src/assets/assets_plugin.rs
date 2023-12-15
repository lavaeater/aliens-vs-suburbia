use bevy::app::{App, Plugin};
use bevy::asset::Handle;
use bevy::prelude::Resource;
use bevy::scene::Scene;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {

    }
}

#[derive(Resource)]
pub struct GameAssets {
    pub girl_scene: Handle<Scene>,
    pub ball_scene: Handle<Scene>,
    pub alien_scene: Handle<Scene>,
}