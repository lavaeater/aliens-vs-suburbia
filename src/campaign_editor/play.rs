//! Play the selected level from the editor (F5): save, start a match on the level's file
//! (`StartLevel`, as `--play --level <path>` does), and come back to the editor with the
//! same level selected when the match ends (won, lost, or quit from the pause menu).

use bevy::prelude::*;

use super::doc::CampaignDoc;
use super::panel::FrameOn;
use crate::campaign::Intermission;
use crate::game_state::GameState;
use crate::general::systems::map_systems::StartLevel;
use crate::player_setup::state::{InputDevice, PlayerRoster};

/// A match started from the campaign editor, and on which level.
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayingFromEditor {
    pub level: usize,
    /// How it ended, once it has.
    pub result: Option<String>,
}

/// The character a match from the editor is played with when nobody joined yet.
const EDITOR_PLAYER: &str = "assets/defs/male-hazmat.ron";

/// Save and play the selected level.
pub fn play_selected(
    commands: &mut Commands,
    doc: &mut CampaignDoc,
    roster: &mut PlayerRoster,
    next: &mut NextState<GameState>,
) {
    let Some((index, level)) = doc.selected.zip(doc.selected_level()) else {
        doc.status = "Select a level to play".into();
        return;
    };
    let path = level.path.clone();
    if let Err(e) = doc.save() {
        doc.status = format!("Not played, the save failed: {e}");
        return;
    }
    if roster.def_paths.is_empty() {
        roster.def_paths = vec![EDITOR_PLAYER.to_string()];
        roster.devices = vec![InputDevice::Keyboard];
        roster.def_overrides = Vec::new();
    }
    commands.insert_resource(StartLevel(path));
    commands.insert_resource(PlayingFromEditor {
        level: index,
        result: None,
    });
    next.set(GameState::InGame);
}

/// F5 in the editor.
pub fn play_key(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<super::panel::TextFocus>,
    mut doc: ResMut<CampaignDoc>,
    mut roster: ResMut<PlayerRoster>,
    mut next: ResMut<NextState<GameState>>,
) {
    if focus.0.is_none() && keys.just_pressed(KeyCode::F5) {
        play_selected(&mut commands, &mut doc, &mut roster, &mut next);
    }
}

/// The match ended (a card) or was left (the menu): back to the editor.
pub fn back_to_editor(
    mut commands: Commands,
    playing: Option<ResMut<PlayingFromEditor>>,
    intermission: Option<Res<Intermission>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(mut playing) = playing else {
        return;
    };
    playing.result = Some(match (state.get(), intermission.as_deref()) {
        (GameState::Intermission, Some(Intermission::Retry)) => "Back from play: lost".into(),
        (GameState::Intermission, _) => "Back from play: won".into(),
        _ => "Back from play".into(),
    });
    commands.remove_resource::<StartLevel>();
    next.set(GameState::CampaignEditor);
}

/// After `open_campaign`: select and frame the level that was played.
pub fn reselect_played(
    mut commands: Commands,
    playing: Option<Res<PlayingFromEditor>>,
    mut doc: ResMut<CampaignDoc>,
    mut frame: ResMut<FrameOn>,
) {
    let Some(playing) = playing else {
        return;
    };
    doc.select(Some(playing.level));
    frame.0 = doc.region_of(playing.level);
    if let Some(result) = &playing.result {
        doc.status.clone_from(result);
    }
    commands.remove_resource::<PlayingFromEditor>();
}
