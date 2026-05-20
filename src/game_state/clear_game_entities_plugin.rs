use bevy::app::{App, Plugin};
use bevy::prelude::{Commands, Entity, OnExit, Query, Window, Without};
use avian3d::prelude::{Collider, RigidBody};
use crate::game_state::GameState;

pub struct ClearGameEntitiesPlugin;

impl Plugin for ClearGameEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnExit(GameState::InGame), clear_game_entities)
            .add_systems(OnExit(GameState::ModelShowcase), clear_game_entities);
    }
}

pub fn clear_game_entities(
    mut commands: Commands,
    query: Query<Entity, Without<Window>>,
    physics_query: Query<Entity, (Without<Window>, bevy::prelude::With<RigidBody>)>,
) {
    // Strip colliders from physics bodies first so avian3d can clear contacts
    // before the entity is despawned — otherwise a debug_assert fires inside avian3d.
    for entity in physics_query.iter() {
        commands.entity(entity).remove::<(Collider, RigidBody)>();
    }
    for entity in query.iter() {
        commands.entity(entity).try_despawn();
    }
}
