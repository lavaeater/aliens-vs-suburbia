use crate::game_state::GameState;
use bevy::app::{App, Plugin};
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::prelude::{Commands, Entity, OnExit, Query, Window, Without};

pub struct ClearGameEntitiesPlugin;

impl Plugin for ClearGameEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(GameState::InGame), clear_game_entities);
    }
}

pub fn clear_game_entities(mut commands: Commands, query: Query<Entity, Without<Window>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
