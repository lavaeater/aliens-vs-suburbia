use bevy::app::{App, Plugin, Startup, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs};
use crate::game_state::GameState;
use crate::sprite_billboard::material::SpriteBillboardMaterial;
use crate::sprite_billboard::systems::{billboard_system, setup_billboard_mesh};

pub struct SpriteBillboardPlugin;

impl Plugin for SpriteBillboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::pbr::MaterialPlugin::<SpriteBillboardMaterial>::default())
            .add_systems(Startup, setup_billboard_mesh)
            .add_systems(
                Update,
                billboard_system.run_if(in_state(GameState::InGame)),
            );
    }
}
