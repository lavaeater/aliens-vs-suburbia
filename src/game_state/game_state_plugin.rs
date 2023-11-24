use bevy::app::{App, Plugin};
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::prelude::{Commands, Entity, OnExit, Query};
use crate::ai::ai_plugin::StatefulAiPlugin;
use crate::alien::alien_plugin::StatefulAlienPlugin;
use crate::building::build_mode_plugin::StatefulBuildModePlugin;
use crate::game_state::GameState;
use crate::map::map_plugins::StatefulMapPlugin;
use crate::ui::spawn_ui::GotoState;
use crate::ui::ui_plugin::UiPlugin;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_state::<GameState>()
            .add_event::<GotoState>()
            .add_plugins((
                StatefulMapPlugin,
                UiPlugin,
                StatefulAiPlugin,
                StatefulBuildModePlugin,
                StatefulAlienPlugin,
                ClearGameEntitiesPlugin
            ))

        ;
    }
}

pub struct ClearGameEntitiesPlugin;

impl Plugin for ClearGameEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnExit(GameState::InGame), clear_game_entities);
    }
}

pub fn clear_game_entities(
    mut commands: Commands,
    query: Query<Entity>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}