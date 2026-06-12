//! Wires the facts engine into the live game (turbofacts steps 7-8): activates the base
//! stories on entering [`GameState::InGame`], mirrors ECS world state into facts each frame
//! (the analog of Kotlin's `FactSystem`), and logs story effects so the signal path is
//! observable. The facts here are groundwork — they run alongside the existing
//! `LevelTracker` flow without replacing it.

use bevy::prelude::{
    in_state, IntoScheduleConfigs, MessageReader, OnEnter, Query, Res, ResMut, Update, With,
};
use bevy::app::{App, Plugin};

use crate::alien::components::general::Alien;
use crate::alien::wave_manager::WaveManager;
use crate::game_state::score_keeper::{LevelTracker, Score};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::general::systems::coin_system::TeamWallet;
use crate::player::components::Player;

use turbofacts::facts_resource::Facts;
use turbofacts::keys;
use turbofacts::messages::StoryEffect;
use turbofacts::stories;
use turbofacts::story::StoryStore;

/// Registers story activation, the derived-fact systems, and effect logging.
pub struct FactsGameIntegrationPlugin;

impl Plugin for FactsGameIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), activate_stories)
            .add_systems(
                Update,
                (derive_world_facts, log_story_effects).run_if(in_state(GameState::InGame)),
            );
    }
}

/// Loads the built-in stories into the store and activates it (seeding their init facts).
fn activate_stories(mut store: ResMut<StoryStore>, mut facts: ResMut<Facts>) {
    if store.stories.is_empty() {
        store.add_all(stories::base_stories());
    }
    store.activate(&mut facts);
}

/// Mirrors ECS world state into facts each frame and derives the win/lose **condition**
/// bools that the level-flow stories read. The Rust analog of Kotlin's `FactSystem`: raw
/// resource/query state in, the small set of facts stories evaluate out. Everything uses
/// `set_if_changed`/`set_bool_if_changed` so unchanging values never dirty the store (and so
/// never re-trigger a story check). The stories own the verdict — see `stories.rs`.
fn derive_world_facts(
    mut facts: ResMut<Facts>,
    players: Query<&Health, With<Player>>,
    scores: Query<&Score, With<Player>>,
    aliens: Query<(), With<Alien>>,
    level_tracker: Res<LevelTracker>,
    wallet: Res<TeamWallet>,
    wave_manager: Option<Res<WaveManager>>,
) {
    // Player liveness.
    let player_count = players.iter().count();
    let living = players.iter().filter(|h| h.health > 0).count() as i64;
    set_if_changed(&mut facts, keys::LIVING_PLAYER_COUNT, living);
    // Only a loss once at least one player has spawned (the old inline non-empty guard).
    set_bool_if_changed(
        &mut facts,
        keys::ALL_PLAYERS_DEAD,
        player_count > 0 && living == 0,
    );

    // Aliens: live count + kill total + the "all spawned aliens cleared" win condition.
    set_if_changed(&mut facts, keys::ENEMY_COUNT, aliens.iter().count() as i64);
    set_if_changed(
        &mut facts,
        keys::ENEMY_KILL_COUNT,
        level_tracker.aliens_killed as i64,
    );
    set_if_changed(
        &mut facts,
        keys::ALIENS_TO_SPAWN,
        level_tracker.aliens_to_spawn as i64,
    );
    let all_waves_done = wave_manager.as_ref().is_none_or(|wm| !wm.waves_remaining());
    let all_killed = level_tracker.aliens_to_spawn > 0
        && level_tracker.aliens_killed >= level_tracker.aliens_to_spawn;
    set_bool_if_changed(&mut facts, keys::ALL_ALIENS_DEAD, all_waves_done && all_killed);

    // Aliens escaping the goal -> the escape-loss condition.
    set_if_changed(
        &mut facts,
        keys::ALIENS_ESCAPED,
        level_tracker.aliens_reached_goal as i64,
    );
    set_if_changed(
        &mut facts,
        keys::ALIENS_ESCAPED_CUTOFF,
        level_tracker.aliens_win_cut_off as i64,
    );
    set_bool_if_changed(
        &mut facts,
        keys::TOO_MANY_ALIENS_ESCAPED,
        level_tracker.aliens_reached_goal >= level_tracker.aliens_win_cut_off,
    );

    // Score aggregates (summed across players) for HUD/telemetry/future stories.
    let shots_fired: u32 = scores.iter().map(|s| s.shots_fired).sum();
    let shots_hit: u32 = scores.iter().map(|s| s.shots_hit).sum();
    set_if_changed(&mut facts, keys::SHOTS_FIRED, shots_fired as i64);
    set_if_changed(&mut facts, keys::SHOTS_HIT, shots_hit as i64);

    set_if_changed(&mut facts, keys::COINS, wallet.coins as i64);

    // Wave progression.
    if let Some(wm) = wave_manager {
        set_if_changed(&mut facts, keys::CURRENT_WAVE, wm.current_wave as i64);
        set_if_changed(&mut facts, keys::WAVE_COUNT, wm.waves.len() as i64);
        set_bool_if_changed(&mut facts, keys::ALL_WAVES_DONE, !wm.waves_remaining());
    }
}

/// Writes an int fact only when it actually changed, so we don't dirty the store (and
/// re-trigger story checks) every single frame for unchanging values.
fn set_if_changed(facts: &mut Facts, key: &str, value: i64) {
    if facts.try_int(key) != Some(value) {
        facts.set_int(key, value);
    }
}

/// Bool equivalent of [`set_if_changed`].
fn set_bool_if_changed(facts: &mut Facts, key: &str, value: bool) {
    if facts.try_bool(key) != Some(value) {
        facts.set_bool(key, value);
    }
}

/// Logs story effects so the signal path is observable until gameplay handlers are added.
fn log_story_effects(mut reader: MessageReader<StoryEffect>) {
    for effect in reader.read() {
        bevy::log::info!("story effect: {}", effect.effect);
    }
}
