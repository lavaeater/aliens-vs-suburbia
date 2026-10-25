//! Wires the facts engine into the live game (turbofacts steps 7-8): activates the base
//! stories on entering [`GameState::InGame`] plus the loaded map's own (`MapFile::stories`),
//! mirrors ECS world state into facts each frame (the analog of Kotlin's `FactSystem`),
//! records story items picked up (`ItemFound.<id>`), and logs story effects so the signal
//! path is observable. Story crawls hang off the effects (`crate::crawl`).

use bevy::app::{App, Plugin};
use bevy::prelude::{
    Has, IntoScheduleConfigs, MessageReader, OnEnter, Query, Res, ResMut, Update, With, in_state,
};

use crate::assets::asset_definition::ItemKind;
use crate::general::events::map_events::LoadMap;
use crate::items::ItemPickedUp;

use crate::alien::components::general::Alien;
use crate::alien::wave_manager::WaveManager;
use crate::game_state::GameState;
use crate::game_state::score_keeper::{LevelTracker, Score};
use crate::general::components::Health;
use crate::general::systems::coin_system::TeamWallet;
use crate::player::components::{Player, PlayerDead};
use crate::player::systems::death_revive::RespawnQueue;

use turbofacts::facts_resource::Facts;
use turbofacts::keys;
use turbofacts::messages::StoryEffect;
use turbofacts::persistence::load_stories;
use turbofacts::stories;
use turbofacts::story::StoryStore;
use turbofacts::systems::check_stories;

/// Registers story activation, the derived-fact systems, and effect logging.
pub struct FactsGameIntegrationPlugin;

impl Plugin for FactsGameIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), activate_stories)
            .add_systems(
                Update,
                (
                    // Before the first check, so a map story sees the level start like
                    // the base ones do.
                    load_map_stories.before(check_stories),
                    derive_world_facts,
                    story_item_facts,
                    log_story_effects,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// Loads the built-in stories into the store and activates it (seeding their init facts).
/// Starts from a fresh set each match, and fresh facts: the previous level's stories,
/// items found and objectives must not linger into the next one (or a retry).
fn activate_stories(mut store: ResMut<StoryStore>, mut facts: ResMut<Facts>) {
    *facts = Facts::default();
    store.stories.clear();
    store.add_all(stories::base_stories());
    store.activate(&mut facts);
}

/// Adds the loaded map's own stories (`MapFile::stories`) to the base set.
fn load_map_stories(
    mut loads: MessageReader<LoadMap>,
    mut store: ResMut<StoryStore>,
    mut facts: ResMut<Facts>,
) {
    for LoadMap { map } in loads.read() {
        if map.stories.is_empty() {
            continue;
        }
        match load_stories(&map.stories) {
            Ok(level_stories) if level_stories.is_empty() => {
                bevy::log::warn!("map stories '{}' are missing or empty", map.stories);
            }
            Ok(level_stories) => {
                bevy::log::info!("{} stories from {}", level_stories.len(), map.stories);
                // Initialize only the new ones: `activate` would also re-arm a base story
                // that already fired (Level Start) if the map loaded a frame late.
                for mut story in level_stories {
                    story.initialize(&mut facts);
                    store.add(story);
                }
                store.needs_checking = true;
            }
            Err(e) => bevy::log::error!("map stories '{}': {e}", map.stories),
        }
    }
}

/// Story items (`ItemKind::Key`) become `ItemFound.<id>` facts for stories to react to.
fn story_item_facts(mut picked: MessageReader<ItemPickedUp>, mut facts: ResMut<Facts>) {
    for pickup in picked.read() {
        if let ItemKind::Key { id } = &pickup.kind {
            facts.set_bool(&keys::item_found(id), true);
        }
    }
}

/// Mirrors ECS world state into facts each frame and derives the win/lose **condition**
/// bools that the level-flow stories read. The Rust analog of Kotlin's `FactSystem`: raw
/// resource/query state in, the small set of facts stories evaluate out. Everything uses
/// `set_if_changed`/`set_bool_if_changed` so unchanging values never dirty the store (and so
/// never re-trigger a story check). The stories own the verdict — see `stories.rs`.
#[allow(clippy::too_many_arguments)]
fn derive_world_facts(
    mut facts: ResMut<Facts>,
    players: Query<(&Health, Has<PlayerDead>), With<Player>>,
    respawns: Option<Res<RespawnQueue>>,
    scores: Query<&Score, With<Player>>,
    aliens: Query<(), With<Alien>>,
    level_tracker: Res<LevelTracker>,
    wallet: Res<TeamWallet>,
    wave_manager: Option<Res<WaveManager>>,
) {
    // Player liveness. Downed is not dead: a downed player can be revived, or bleeds out
    // into a respawn while lives last, so the loss is the team wipe (`team_wiped`), not
    // "nobody standing" - which was every time a solo player went down.
    let living = players
        .iter()
        .filter(|(h, downed)| h.health > 0 && !downed)
        .count() as i64;
    set_if_changed(&mut facts, keys::LIVING_PLAYER_COUNT, living);
    let wiped = respawns.is_some_and(|queue| queue.team_wiped(!players.is_empty()));
    set_bool_if_changed(&mut facts, keys::ALL_PLAYERS_DEAD, wiped);

    // Aliens: live count + kill total + the "all spawned aliens cleared" win condition.
    set_if_changed(&mut facts, keys::ENEMY_COUNT, aliens.iter().count() as i64);
    set_if_changed(
        &mut facts,
        keys::ENEMY_KILL_COUNT,
        i64::from(level_tracker.aliens_killed),
    );
    set_if_changed(
        &mut facts,
        keys::ALIENS_TO_SPAWN,
        i64::from(level_tracker.aliens_to_spawn),
    );
    let all_waves_done = wave_manager.as_ref().is_none_or(|wm| !wm.waves_remaining());
    let all_killed = level_tracker.aliens_to_spawn > 0
        && level_tracker.aliens_killed >= level_tracker.aliens_to_spawn;
    set_bool_if_changed(
        &mut facts,
        keys::ALL_ALIENS_DEAD,
        all_waves_done && all_killed,
    );

    // Aliens escaping the goal -> the escape-loss condition.
    set_if_changed(
        &mut facts,
        keys::ALIENS_ESCAPED,
        i64::from(level_tracker.aliens_reached_goal),
    );
    set_if_changed(
        &mut facts,
        keys::ALIENS_ESCAPED_CUTOFF,
        i64::from(level_tracker.aliens_win_cut_off),
    );
    set_bool_if_changed(
        &mut facts,
        keys::TOO_MANY_ALIENS_ESCAPED,
        level_tracker.aliens_reached_goal >= level_tracker.aliens_win_cut_off,
    );

    // Score aggregates (summed across players) for HUD/telemetry/future stories.
    let shots_fired: u32 = scores.iter().map(|s| s.shots_fired).sum();
    let shots_hit: u32 = scores.iter().map(|s| s.shots_hit).sum();
    set_if_changed(&mut facts, keys::SHOTS_FIRED, i64::from(shots_fired));
    set_if_changed(&mut facts, keys::SHOTS_HIT, i64::from(shots_hit));

    set_if_changed(&mut facts, keys::COINS, i64::from(wallet.coins));

    // The level's own objectives: done when every `Objective.*` bool is true.
    let objectives_done = objectives_done(&facts);
    set_bool_if_changed(&mut facts, keys::OBJECTIVES_DONE, objectives_done);

    // Wave progression.
    if let Some(wm) = wave_manager {
        set_if_changed(&mut facts, keys::CURRENT_WAVE, wm.current_wave as i64);
        set_if_changed(&mut facts, keys::WAVE_COUNT, wm.waves.len() as i64);
        set_bool_if_changed(&mut facts, keys::ALL_WAVES_DONE, !wm.waves_remaining());
    }
}

/// Every `Objective.*` bool fact is true; vacuously so on a level without objectives.
fn objectives_done(facts: &Facts) -> bool {
    facts
        .query(&format!("{}*", keys::OBJECTIVE_PREFIX))
        .filter_map(|(_, v)| v.as_bool())
        .all(|done| done)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objectives_are_done_when_every_one_is_true_or_there_are_none() {
        let mut facts = Facts::default();
        assert!(objectives_done(&facts));
        facts.set_bool("Objective.gnome_1", true);
        facts.set_bool("Objective.gnome_2", false);
        assert!(!objectives_done(&facts));
        facts.set_bool("Objective.gnome_2", true);
        assert!(objectives_done(&facts));
    }
}
