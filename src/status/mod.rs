//! Status effects: what a hit does *after* it lands.
//!
//! Damage and status are two layers on purpose:
//!
//! - **Damage** (`general::damage`) is one number now: an [`ApplyDamage`] of some
//!   [`DamageKind`], scaled by the target's `DamageResistances`. Every kind goes through
//!   the same pipe; a new kind needs no new system.
//! - **Status** (this module) is what lingers: an [`Afflict`] puts a [`StatusKind`] on a
//!   target for a while. The status ticks on its own (burning deals Fire damage through
//!   `ApplyDamage` every quarter second), shows on the body, and makes the creature *react*.
//!
//! The two meet in three places, so content can tune them without code:
//!
//! - each status has a damage kind ([`StatusKind::damage_kind`]); the target's
//!   resistance to that kind scales the affliction's strength and length, and a
//!   resistance of 0 makes it immune (a fire-proof golem never catches fire);
//! - the same `DamageRules` decide whether an affliction may land (a player's
//!   flamethrower does not set the neighbours alight unless friendly fire is on);
//! - [`Reactions`] (from `EnemyProps::reactions`) say what a creature does about a status:
//!   [`Reaction::Panic`] (run about at random, the default for burning) or
//!   [`Reaction::Ignore`] (keep coming). Players never react: they have a player.
//!
//! Adding a status: a variant here, its component and tick in its own file (like
//! [`burning`]), a match arm in [`apply_afflictions`].

pub mod burning;
pub mod panic;

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game_state::GameState;
use crate::general::components::{Health, Indestructible};
use crate::general::damage::{DamageResistances, DamageRules, Faction};
use crate::gore::components::DamageKind;
use crate::player::components::Player;
use burning::Burning;
use panic::Panicking;

#[allow(unused_imports)] // named in the docs above
use crate::general::damage::ApplyDamage;

/// Something that lingers on a creature after a hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum StatusKind {
    /// On fire: Fire damage over time, flames and a light on the body, panic.
    Burning,
}

impl StatusKind {
    /// The damage kind whose resistance guards against this status.
    pub const fn damage_kind(self) -> DamageKind {
        match self {
            Self::Burning => DamageKind::Fire,
        }
    }

    /// What a creature does about it unless its def says otherwise.
    pub const fn default_reaction(self) -> Reaction {
        match self {
            Self::Burning => Reaction::Panic,
        }
    }
}

/// What a creature does while it has a status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum Reaction {
    /// Drop everything and run about at random until it is over.
    #[default]
    Panic,
    /// Carry on as if nothing happened (it still hurts).
    Ignore,
}

/// A creature's own reactions, overriding [`StatusKind::default_reaction`].
#[derive(Component, Clone, Debug, Default)]
pub struct Reactions(pub HashMap<StatusKind, Reaction>);

impl Reactions {
    pub fn to(&self, kind: StatusKind) -> Reaction {
        self.0
            .get(&kind)
            .copied()
            .unwrap_or_else(|| kind.default_reaction())
    }
}

/// Put a status on `target`: `strength` is its damage per second, `secs` how long it lasts
/// (before resistances). A target that has it already keeps the stronger and the longer.
#[derive(Message, Clone, Copy, Debug)]
pub struct Afflict {
    pub target: Entity,
    pub kind: StatusKind,
    pub strength: f32,
    pub secs: f32,
    /// Who did it: faction rules apply, and kills by the status are credited to them.
    pub source: Option<Entity>,
}

impl Afflict {
    pub const fn burning(target: Entity, dps: f32, secs: f32) -> Self {
        Self {
            target,
            kind: StatusKind::Burning,
            strength: dps,
            secs,
            source: None,
        }
    }

    pub const fn from(mut self, source: Option<Entity>) -> Self {
        self.source = source;
        self
    }
}

/// An affliction's strength and length on a target with resistance `multiplier` to its
/// damage kind. `None`: immune. Resistance shortens it as well as weakening it, so a
/// half-resistant creature burns half as hot for half as long.
pub fn resisted(strength: f32, secs: f32, multiplier: f32) -> Option<(f32, f32)> {
    if multiplier <= 0.0 || strength <= 0.0 || secs <= 0.0 {
        return None;
    }
    Some((strength * multiplier, secs * multiplier.min(1.0)))
}

/// How long a creature keeps running after the fire is out.
const PANIC_AFTERWARDS: f32 = 0.8;

/// Land every [`Afflict`]: rules, resistances, the status itself and the reaction.
#[allow(clippy::type_complexity)]
pub fn apply_afflictions(
    mut msgs: MessageReader<Afflict>,
    mut commands: Commands,
    rules: Res<DamageRules>,
    mut targets: Query<(
        &Health,
        Option<&Faction>,
        Option<&DamageResistances>,
        Option<&Reactions>,
        Option<&mut Burning>,
        Option<&mut Panicking>,
        Has<Indestructible>,
        Has<Player>,
    )>,
    factions: Query<&Faction>,
) {
    for msg in msgs.read() {
        let Ok((health, faction, resistances, reactions, burning, panicking, indestructible, player)) =
            targets.get_mut(msg.target)
        else {
            continue;
        };
        if indestructible || health.health <= 0 {
            continue;
        }
        let source_faction = msg.source.and_then(|s| factions.get(s).ok()).copied();
        if !rules.allows(source_faction, faction.copied()) {
            continue;
        }
        let multiplier = resistances.map_or(1.0, |r| r.multiplier(msg.kind.damage_kind()));
        let Some((strength, secs)) = resisted(msg.strength, msg.secs, multiplier) else {
            continue;
        };

        match msg.kind {
            StatusKind::Burning => match burning {
                Some(mut burning) => burning.refresh(strength, secs, msg.source),
                None => {
                    commands
                        .entity(msg.target)
                        .insert(Burning::new(strength, secs, msg.source));
                }
            },
        }

        let reaction = reactions.map_or_else(|| msg.kind.default_reaction(), |r| r.to(msg.kind));
        if player || reaction != Reaction::Panic {
            continue;
        }
        let panic_for = secs + PANIC_AFTERWARDS;
        match panicking {
            Some(mut panicking) => panicking.last_at_least(panic_for),
            None => {
                commands
                    .entity(msg.target)
                    .insert(Panicking::new(panic_for, msg.target.to_bits() as u32));
            }
        }
    }
}

pub struct StatusPlugin;

impl Plugin for StatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Afflict>()
            .add_systems(
                Update,
                (
                    apply_afflictions,
                    burning::tick_burning,
                    burning::spread_burning,
                    burning::show_burning,
                    burning::follow_burning,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            // After every frame-rate walker has had its say, before physics steps.
            .add_systems(
                FixedUpdate,
                panic::run_in_panic.run_if(in_state(GameState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::components::general::Alien;

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<Afflict>();
        app.init_resource::<DamageRules>();
        app.add_systems(Update, apply_afflictions);
        app
    }

    fn creature(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((Alien, Faction::Alien, Health::full(100)))
            .id()
    }

    #[test]
    fn resistance_weakens_and_shortens_and_zero_is_immune() {
        assert_eq!(resisted(10.0, 4.0, 1.0), Some((10.0, 4.0)));
        assert_eq!(resisted(10.0, 4.0, 0.5), Some((5.0, 2.0)));
        // Weak to fire burns hotter but not longer.
        assert_eq!(resisted(10.0, 4.0, 2.0), Some((20.0, 4.0)));
        assert_eq!(resisted(10.0, 4.0, 0.0), None);
    }

    #[test]
    fn a_monster_set_alight_burns_and_panics() {
        let mut app = app();
        let alien = creature(&mut app);
        app.world_mut().write_message(Afflict::burning(alien, 8.0, 3.0));
        app.update();
        assert!(app.world().get::<Burning>(alien).is_some());
        assert!(app.world().get::<Panicking>(alien).is_some());
    }

    #[test]
    fn a_fireproof_monster_does_not_catch() {
        let mut app = app();
        let golem = creature(&mut app);
        let mut res = DamageResistances::default();
        res.0.insert(DamageKind::Fire, 0.0);
        app.world_mut().entity_mut(golem).insert(res);
        app.world_mut().write_message(Afflict::burning(golem, 8.0, 3.0));
        app.update();
        assert!(app.world().get::<Burning>(golem).is_none());
    }

    #[test]
    fn a_monster_that_ignores_fire_burns_without_panicking() {
        let mut app = app();
        let demon = creature(&mut app);
        app.world_mut().entity_mut(demon).insert(Reactions(HashMap::from([(
            StatusKind::Burning,
            Reaction::Ignore,
        )])));
        app.world_mut().write_message(Afflict::burning(demon, 8.0, 3.0));
        app.update();
        assert!(app.world().get::<Burning>(demon).is_some());
        assert!(app.world().get::<Panicking>(demon).is_none());
    }

    #[test]
    fn players_burn_but_keep_their_heads() {
        let mut app = app();
        let player = app
            .world_mut()
            .spawn((Player, Faction::Player, Health::full(100)))
            .id();
        app.world_mut().write_message(Afflict::burning(player, 8.0, 3.0));
        app.update();
        assert!(app.world().get::<Burning>(player).is_some());
        assert!(app.world().get::<Panicking>(player).is_none());
    }

    #[test]
    fn a_players_flames_spare_the_other_players_side() {
        let mut app = app();
        let shooter = app.world_mut().spawn(Faction::Player).id();
        let neighbour = app
            .world_mut()
            .spawn((Faction::Player, Health::full(100)))
            .id();
        app.world_mut()
            .write_message(Afflict::burning(neighbour, 8.0, 3.0).from(Some(shooter)));
        app.update();
        assert!(app.world().get::<Burning>(neighbour).is_none());
    }
}
