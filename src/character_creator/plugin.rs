use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs, OnEnter, OnExit};
use crate::character_creator::composer::recompose_character_system;
use crate::character_creator::config::{CharacterConfig, ComposedSpriteSheet};
use crate::character_creator::ui::{
    spawn_character_creator_ui, sync_labels, sync_preview_image, CreatorSelections,
};
use crate::game_state::GameState;
use crate::ui::spawn_ui::{cleanup_state, StateMarker};
use crate::ui::ui_plugin::spawn_ui_camera;

pub struct CharacterCreatorPlugin;

impl Plugin for CharacterCreatorPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CharacterConfig>()
            .init_resource::<ComposedSpriteSheet>()
            .init_resource::<CreatorSelections>()
            // Recompose whenever CharacterConfig changes (works in any state).
            .add_systems(Update, recompose_character_system)
            // Character creator screen.
            .add_systems(
                OnEnter(GameState::CharacterCreator),
                (spawn_ui_camera, spawn_character_creator_ui),
            )
            .add_systems(
                Update,
                (sync_labels, sync_preview_image)
                    .run_if(in_state(GameState::CharacterCreator)),
            )
            .add_systems(OnExit(GameState::CharacterCreator), cleanup_state);
    }
}
