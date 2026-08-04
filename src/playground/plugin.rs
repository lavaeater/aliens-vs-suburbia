//! Wires the playground into the app.
//!
//! Everything here is gated on [`in_playground`], and the four pieces of normal-match
//! behaviour the playground replaces are gated on `in_normal_game` at their own
//! definition sites (map loading, the HUD, wave spawning, the win/lose transition).

use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;

use crate::alien::wave_manager::WaveManager;
use crate::game_state::GameState;
use crate::general::components::map_components::MapFile;
use crate::general::events::map_events::LoadMap;
use crate::playground::dummies::{respawn_dummies, spawn_dummy_posts};
use crate::playground::state::in_playground;
use crate::playground::ui::{
    clear_playground_viewport, end_playground_session, spawn_playground_ui,
    sync_playground_viewport,
};

const PLAYGROUND_MAP: &str = "assets/maps/playground.ron";

pub struct PlaygroundPlugin;

impl Plugin for PlaygroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                load_playground_map,
                silence_waves,
                spawn_playground_ui,
                spawn_dummy_posts,
            )
                .run_if(in_playground),
        )
        .add_systems(
            OnExit(GameState::InGame),
            (clear_playground_viewport, end_playground_session).run_if(in_playground),
        )
        .add_systems(
            Update,
            (sync_playground_viewport, respawn_dummies)
                .run_if(in_state(GameState::InGame))
                .run_if(in_playground),
        );
    }
}

/// Load the sandbox arena instead of `level_01`.
fn load_playground_map(mut load_map_mw: MessageWriter<LoadMap>) {
    let text = match std::fs::read_to_string(PLAYGROUND_MAP) {
        Ok(text) => text,
        Err(err) => {
            error!("playground: cannot read {PLAYGROUND_MAP}: {err}");
            return;
        }
    };
    match ron::from_str::<MapFile>(&text) {
        Ok(map) => { load_map_mw.write(LoadMap { map }); }
        Err(err) => error!("playground: cannot parse {PLAYGROUND_MAP}: {err}"),
    }
}

/// `WaveManager::default()` ships a full set of hardcoded waves, and `map_loader` only
/// overrides them when the map file declares some — so an empty `waves: []` is not enough
/// to stop aliens pouring in. Clear them explicitly.
fn silence_waves(mut waves: ResMut<WaveManager>) {
    waves.waves.clear();
    waves.current_wave = 0;
    waves.spawning = false;
}
