//! Infestation levels (`MapFile::infestation`): no goal to defend. Spawners open up at
//! random spots around the map, one every `every` seconds, and each sends out waves of
//! the map's `waves` enemies before slowing to a trickle. The monsters hunt the
//! neighbours standing in their yards (and the players), the neighbours run, and the
//! players have to kill the monsters and destroy the spawners.
//!
//! - [`spawners`]: when and where a spawner opens, and what it sends out.
//! - [`hunters`]: the monsters' body, pathing and animation.
//! - [`neighbours`]: the people in the yards (any character def placed on a map).
//!
//! The verdict is the stories' (`facts`): this module keeps the facts in [`keys`] up to
//! date and the `ObjectiveText` line, and holds `LevelTracker::aliens_to_spawn` at kills +
//! living monsters + what is still to come, so the base `Aliens Cleared` story wins the
//! level once the last spawner is gone and its monsters with it. Losing every neighbour
//! is the level story's to call (`NeighboursLost`).

pub mod hunters;
pub mod neighbours;
pub mod spawners;

use bevy::prelude::*;

use crate::alien::components::general::{Alien, AlienCounter};
use crate::alien::wave_manager::WaveManager;
use crate::facts::facts_resource::Facts;
use crate::game_state::GameState;
use crate::game_state::score_keeper::LevelTracker;
use crate::general::components::map_components::{InfestationRules, WaveDef};
use crate::general::events::map_events::LoadMap;
use crate::map::MapFeatures;

pub use hunters::{make_builtin_hunter, make_hunter};
pub use neighbours::{Neighbour, SpawnNeighbour};
pub use spawners::Spawner;

/// The facts an infestation keeps for the stories.
pub mod keys {
    /// Spawners the level will open in all.
    pub const SPAWNERS_TOTAL: &str = "SpawnersTotal";
    /// Spawners destroyed so far.
    pub const SPAWNERS_DESTROYED: &str = "SpawnersDestroyed";
    /// Spawners still to open or still standing.
    pub const SPAWNERS_LEFT: &str = "SpawnersLeft";
    /// Neighbours alive.
    pub const NEIGHBOURS_ALIVE: &str = "NeighboursAlive";
    /// Neighbours the level started with.
    pub const NEIGHBOURS_TOTAL: &str = "NeighboursTotal";
    /// Every neighbour is dead (false on a level that has none).
    pub const NEIGHBOURS_LOST: &str = "NeighboursLost";
    /// Every one of them, for `crate::validate`.
    pub const ALL: &[&str] = &[
        SPAWNERS_TOTAL,
        SPAWNERS_DESTROYED,
        SPAWNERS_LEFT,
        NEIGHBOURS_ALIVE,
        NEIGHBOURS_TOTAL,
        NEIGHBOURS_LOST,
    ];
}

/// The running infestation. `rules` is `None` on any other level, and everything here
/// sits still.
#[derive(Resource, Debug, Default)]
pub struct Infestation {
    pub rules: Option<InfestationRules>,
    /// What spawners send out: the map's waves.
    pub waves: Vec<WaveDef>,
    /// Map tiles a spawner may open on (padded grid, as the loader builds it): lawn and
    /// street, away from the walls.
    pub open_ground: Vec<(usize, usize)>,
    /// Seconds until the next spawner opens.
    pub next_in: f32,
    pub opened: u32,
    pub destroyed: u32,
    /// The most neighbours seen alive at once: the level's count, once they have spawned.
    pub neighbours_total: u32,
    pub rng: u64,
}

impl Infestation {
    /// Spawners that have not opened yet.
    pub fn unopened(&self) -> u32 {
        self.rules
            .as_ref()
            .map_or(0, |r| r.spawners.saturating_sub(self.opened))
    }

    /// A random number in `lo..hi`.
    pub fn random(&mut self, lo: f32, hi: f32) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        let unit = (x >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo).max(0.0) * unit
    }
}

/// Seconds before the first spawner opens: time to look around.
const FIRST_SPAWNER_AFTER: f32 = 10.0;
/// Live monsters at once, at most.
const MAX_MONSTERS: i32 = 80;

pub struct InfestationPlugin;

impl Plugin for InfestationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Infestation>()
            .add_message::<SpawnNeighbour>()
            .add_observer(spawners::spawner_destroyed)
            .add_systems(
                Update,
                (
                    start_infestation.after(crate::general::systems::map_systems::map_loader),
                    neighbours::spawn_neighbours,
                    spawners::open_spawners,
                    spawners::run_spawners,
                    hunters::choose_prey,
                    hunters::hunt,
                    neighbours::flee,
                    hunters::animate_creatures,
                    infestation_facts,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), end_infestation);
    }
}

/// A map loading starts (or, without rules, ends) the infestation. Runs after the map
/// loader, so it can take the waves back off the `WaveManager`: here they belong to the
/// spawners, and a wave manager with waves would never call them done.
fn start_infestation(
    mut loads: MessageReader<LoadMap>,
    mut infestation: ResMut<Infestation>,
    mut waves: ResMut<WaveManager>,
    mut counter: ResMut<AlienCounter>,
) {
    for LoadMap { map } in loads.read() {
        let Some(rules) = map.infestation.clone() else {
            *infestation = Infestation::default();
            continue;
        };
        info!(
            "infestation: {} spawners every {}-{} s, {} waves each",
            rules.spawners, rules.every.0, rules.every.1, rules.waves
        );
        *infestation = Infestation {
            rules: Some(rules),
            waves: map.waves.clone(),
            open_ground: spawners::open_ground(&map.tiles),
            next_in: FIRST_SPAWNER_AFTER,
            rng: map.seed ^ 0x001A_F357_A710_u64 | 1,
            ..Infestation::default()
        };
        *waves = WaveManager {
            waves: Vec::new(),
            ..WaveManager::default()
        };
        counter.max_count = MAX_MONSTERS;
    }
}

fn end_infestation(mut infestation: ResMut<Infestation>) {
    *infestation = Infestation::default();
}

/// Whether every neighbour is gone: only once there were some.
pub const fn neighbours_lost(total: u32, alive: u32) -> bool {
    total > 0 && alive == 0
}

/// The facts, the HUD line and the tracker's total (see the module docs).
#[allow(clippy::too_many_arguments)]
fn infestation_facts(
    mut infestation: ResMut<Infestation>,
    mut facts: ResMut<Facts>,
    mut tracker: ResMut<LevelTracker>,
    spawners: Query<&Spawner>,
    monsters: Query<(), With<Alien>>,
    neighbours: Query<&crate::general::components::Health, With<Neighbour>>,
) {
    let Some(rules) = infestation.rules.clone() else {
        return;
    };
    let alive = neighbours.iter().filter(|h| h.health > 0).count() as u32;
    infestation.neighbours_total = infestation.neighbours_total.max(alive);
    let total = infestation.neighbours_total;
    let left = infestation.unopened() + spawners.iter().count() as u32;

    let ints = [
        (keys::SPAWNERS_TOTAL, rules.spawners),
        (keys::SPAWNERS_DESTROYED, infestation.destroyed),
        (keys::SPAWNERS_LEFT, left),
        (keys::NEIGHBOURS_ALIVE, alive),
        (keys::NEIGHBOURS_TOTAL, total),
    ];
    for (key, value) in ints {
        if facts.try_int(key) != Some(i64::from(value)) {
            facts.set_int(key, i64::from(value));
        }
    }
    let lost = neighbours_lost(total, alive);
    if facts.try_bool(keys::NEIGHBOURS_LOST) != Some(lost) {
        facts.set_bool(keys::NEIGHBOURS_LOST, lost);
    }
    let objective = if left == 0 {
        "Spawners destroyed! Clear out the rest of them".to_string()
    } else {
        format!(
            "Destroy the spawners ({}/{})   Neighbours left: {alive}/{total}",
            infestation.destroyed, rules.spawners
        )
    };
    if facts.try_text(crate::facts::keys::OBJECTIVE_TEXT) != Some(objective.as_str()) {
        facts.set_text(crate::facts::keys::OBJECTIVE_TEXT, objective);
    }

    // Still to come: what the standing spawners have left (one for the trickle that
    // never ends while one stands), plus one while any have yet to open.
    let pending: u32 = spawners
        .iter()
        .map(Spawner::pending)
        .sum::<u32>()
        .saturating_add(u32::from(infestation.unopened() > 0));
    let to_spawn = tracker
        .aliens_killed
        .saturating_add(i32::try_from(monsters.iter().count()).unwrap_or(i32::MAX))
        .saturating_add(i32::try_from(pending).unwrap_or(i32::MAX));
    tracker.aliens_to_spawn = to_spawn;
}

/// Lawn or street, which nothing blocks.
fn is_ground(tile: u64) -> bool {
    let flags = crate::map::BitFlags::<MapFeatures>::from_bits_truncate(tile);
    (flags.contains(MapFeatures::Grass) || flags.contains(MapFeatures::Rock))
        && !flags.contains(MapFeatures::ImpassableForEnemies)
        && !flags.contains(MapFeatures::ImpassableForPlayers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_neighbours_are_lost_only_once_there_were_some() {
        assert!(!neighbours_lost(0, 0), "a level without neighbours");
        assert!(!neighbours_lost(5, 1));
        assert!(neighbours_lost(5, 0));
    }

    #[test]
    fn random_numbers_stay_in_range() {
        let mut infestation = Infestation {
            rng: 42,
            ..Infestation::default()
        };
        for _ in 0..1000 {
            let x = infestation.random(20.0, 40.0);
            assert!((20.0..40.0).contains(&x), "{x}");
        }
    }
}
