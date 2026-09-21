//! The one place damage is applied.
//!
//! Everything that hurts something — bullets, thrown balls, melee, fire, towers, abilities,
//! aliens chewing on walls — writes an [`ApplyDamage`] message. [`apply_damage`] is the only
//! system that mutates [`Health`] downwards. It owns the rules that used to be scattered
//! across every hit site: who may hurt whom ([`DamageRules`] + [`Faction`]), per-kind
//! multipliers ([`DamageResistances`]), [`Indestructible`], and the bookkeeping that has to
//! happen exactly once per lethal blow (the gore [`DamageDealt`] message, score events and
//! the alien counter).
//!
//! Healing is *not* damage and does not go through here; it stays a direct `Health` edit.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::alien::components::general::{Alien, AlienCounter};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::{Health, Indestructible};
use crate::gore::components::{DamageDealt, DamageKind};

/// Who an entity fights for. Consulted by [`DamageRules`] to decide whether a hit lands.
/// Entities without a faction can be hurt by anyone (props, crates).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
#[type_path = "avs"]
pub enum Faction {
    Player,
    Alien,
    /// Walls, towers, anything built or placed.
    Structure,
}

/// The who-can-hurt-whom matrix. Lives in a resource so a playtest can flip it live.
#[derive(Resource, Clone, Copy, Debug, Reflect)]
#[reflect(Resource)]
#[type_path = "avs"]
pub struct DamageRules {
    /// Can a player's bullets/fire/explosions hurt another player?
    pub friendly_fire: bool,
    /// Can aliens damage walls and towers? (`DestroyTheMap` relies on this being true.)
    pub aliens_hurt_structures: bool,
    /// Can a player's own shots damage walls and towers?
    pub players_hurt_structures: bool,
}

impl Default for DamageRules {
    fn default() -> Self {
        Self { friendly_fire: false, aliens_hurt_structures: true, players_hurt_structures: false }
    }
}

impl DamageRules {
    /// Whether a hit from `source` may land on `target`. Faction-less sources (fire fields,
    /// map hazards) and faction-less targets (props) are always allowed.
    pub const fn allows(&self, source: Option<Faction>, target: Option<Faction>) -> bool {
        match (source, target) {
            (None, _) | (_, None) => true,
            (Some(Faction::Player), Some(Faction::Player)) => self.friendly_fire,
            (Some(Faction::Player), Some(Faction::Structure)) => self.players_hurt_structures,
            (Some(Faction::Alien), Some(Faction::Alien))
            | (Some(Faction::Structure), Some(Faction::Structure)) => false,
            (Some(Faction::Alien), Some(Faction::Structure)) => self.aliens_hurt_structures,
            _ => true,
        }
    }
}

/// Per-kind damage multipliers on a target. Missing kinds are 1.0; 0.0 means immune.
/// This is where "walls shrug off bullets but explosives wreck them" lives.
#[derive(Component, Clone, Debug, Default)]
pub struct DamageResistances(pub HashMap<DamageKind, f32>);

impl DamageResistances {
    pub fn multiplier(&self, kind: DamageKind) -> f32 {
        self.0.get(&kind).copied().unwrap_or(1.0)
    }
}

/// A request to hurt `target`. Written by anything that deals damage; consumed only by
/// [`apply_damage`].
#[derive(Message, Clone, Copy, Debug)]
pub struct ApplyDamage {
    pub target: Entity,
    /// Raw damage before resistances. Non-positive amounts are ignored.
    pub amount: i32,
    pub kind: DamageKind,
    /// World position of the hit, forwarded to the gore layer.
    pub position: Vec3,
    /// Unit direction the gore should spray, pointing away from the source.
    pub normal: Vec3,
    /// Who dealt it. `None` for hazards with no owner (burning ground, map traps).
    /// Kills are credited to this entity in the score.
    pub source: Option<Entity>,
}

impl ApplyDamage {
    /// A hit at `position` with no meaningful direction (spray straight up).
    pub const fn at(target: Entity, amount: i32, kind: DamageKind, position: Vec3) -> Self {
        Self { target, amount, kind, position, normal: Vec3::Y, source: None }
    }

    pub const fn from(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    pub fn along(mut self, normal: Vec3) -> Self {
        self.normal = normal.normalize_or(Vec3::Y);
        self
    }
}

/// Applies every queued [`ApplyDamage`] and announces the result.
///
/// Runs before `health_monitor_system` so a lethal hit is despawned the same frame.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn apply_damage(
    mut requests: MessageReader<ApplyDamage>,
    rules: Res<DamageRules>,
    mut targets: Query<(
        &mut Health,
        Option<&Faction>,
        Option<&DamageResistances>,
        Has<Alien>,
        Has<Indestructible>,
    )>,
    factions: Query<&Faction>,
    mut alien_counter: Option<ResMut<AlienCounter>>,
    mut damage_mw: MessageWriter<DamageDealt>,
    mut game_mw: MessageWriter<GameTrackingEvent>,
) {
    for req in requests.read() {
        let Ok((mut health, target_faction, resistances, is_alien, indestructible)) =
            targets.get_mut(req.target)
        else {
            continue;
        };
        if indestructible {
            continue;
        }
        let source_faction = req.source.and_then(|s| factions.get(s).ok()).copied();
        if !rules.allows(source_faction, target_faction.copied()) {
            continue;
        }
        let amount = scaled(req.amount, resistances.map_or(1.0, |r| r.multiplier(req.kind)));
        if amount <= 0 {
            continue;
        }

        // Already dead things (a downed player, an alien mid-despawn) take the hit for the
        // gore but never count as a second kill.
        let was_alive = health.health > 0;
        let lethal = health.apply(amount);

        damage_mw.write(DamageDealt {
            target: req.target,
            position: req.position,
            normal: req.normal,
            amount,
            kind: req.kind,
            lethal,
        });
        if let Some(source) = req.source {
            game_mw.write(GameTrackingEvent::ShotHit(source));
        }
        if lethal && was_alive && is_alien {
            game_mw.write(GameTrackingEvent::AlienKilled(req.source.unwrap_or(req.target)));
            if let Some(counter) = alien_counter.as_mut() {
                counter.count = counter.count.saturating_sub(1);
            }
        }
    }
}

/// Multiply and round to the nearest whole point, so a 0.5x resistance on 1 damage still
/// rounds to something rather than silently vanishing.
fn scaled(amount: i32, multiplier: f32) -> i32 {
    (amount as f32 * multiplier).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::Health;

    #[derive(Resource, Default)]
    struct Caught {
        dealt: Vec<DamageDealt>,
        events: Vec<GameTrackingEvent>,
    }

    fn catch(
        mut dealt: MessageReader<DamageDealt>,
        mut events: MessageReader<GameTrackingEvent>,
        mut caught: ResMut<Caught>,
    ) {
        caught.dealt.extend(dealt.read().copied());
        caught.events.extend(events.read().cloned());
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<ApplyDamage>();
        app.add_message::<DamageDealt>();
        app.add_message::<GameTrackingEvent>();
        app.init_resource::<DamageRules>();
        app.init_resource::<Caught>();
        let mut counter = AlienCounter::new(10);
        counter.count = 1;
        app.insert_resource(counter);
        app.add_systems(Update, (apply_damage, catch).chain());
        app
    }

    fn hit(app: &mut App, req: ApplyDamage) {
        app.world_mut().write_message(req);
        app.update();
    }

    #[test]
    fn a_plain_hit_reduces_health_and_is_announced() {
        let mut app = test_app();
        let t = app.world_mut().spawn(Health { health: 50, max_health: 50 }).id();
        hit(&mut app, ApplyDamage::at(t, 20, DamageKind::Ballistic, Vec3::ZERO));
        assert_eq!(app.world().get::<Health>(t).unwrap().health, 30);
        let caught = app.world().resource::<Caught>();
        assert_eq!(caught.dealt.len(), 1);
        assert_eq!(caught.dealt[0].amount, 20);
        assert!(!caught.dealt[0].lethal);
    }

    #[test]
    fn indestructible_targets_ignore_everything() {
        let mut app = test_app();
        let t = app.world_mut().spawn((Health { health: 50, max_health: 50 }, Indestructible)).id();
        hit(&mut app, ApplyDamage::at(t, 999, DamageKind::Explosive, Vec3::ZERO));
        assert_eq!(app.world().get::<Health>(t).unwrap().health, 50);
        assert!(app.world().resource::<Caught>().dealt.is_empty());
    }

    #[test]
    fn friendly_fire_is_off_by_default() {
        let mut app = test_app();
        let shooter = app.world_mut().spawn(Faction::Player).id();
        let t = app.world_mut().spawn((Health { health: 50, max_health: 50 }, Faction::Player)).id();
        hit(&mut app, ApplyDamage::at(t, 20, DamageKind::Ballistic, Vec3::ZERO).from(shooter));
        assert_eq!(app.world().get::<Health>(t).unwrap().health, 50);
    }

    #[test]
    fn friendly_fire_can_be_switched_on() {
        let mut app = test_app();
        app.insert_resource(DamageRules { friendly_fire: true, ..Default::default() });
        let shooter = app.world_mut().spawn(Faction::Player).id();
        let t = app.world_mut().spawn((Health { health: 50, max_health: 50 }, Faction::Player)).id();
        hit(&mut app, ApplyDamage::at(t, 20, DamageKind::Ballistic, Vec3::ZERO).from(shooter));
        assert_eq!(app.world().get::<Health>(t).unwrap().health, 30);
    }

    #[test]
    fn aliens_hurt_structures_but_players_do_not_by_default() {
        let mut app = test_app();
        let alien = app.world_mut().spawn(Faction::Alien).id();
        let player = app.world_mut().spawn(Faction::Player).id();
        let wall = app.world_mut().spawn((Health { health: 50, max_health: 50 }, Faction::Structure)).id();
        hit(&mut app, ApplyDamage::at(wall, 10, DamageKind::Blunt, Vec3::ZERO).from(player));
        assert_eq!(app.world().get::<Health>(wall).unwrap().health, 50);
        hit(&mut app, ApplyDamage::at(wall, 10, DamageKind::Blunt, Vec3::ZERO).from(alien));
        assert_eq!(app.world().get::<Health>(wall).unwrap().health, 40);
    }

    #[test]
    fn ownerless_damage_hits_anyone() {
        let mut app = test_app();
        let t = app.world_mut().spawn((Health { health: 50, max_health: 50 }, Faction::Player)).id();
        hit(&mut app, ApplyDamage::at(t, 5, DamageKind::Fire, Vec3::ZERO));
        assert_eq!(app.world().get::<Health>(t).unwrap().health, 45);
    }

    #[test]
    fn resistances_scale_the_amount() {
        let mut app = test_app();
        let mut res = DamageResistances::default();
        res.0.insert(DamageKind::Ballistic, 0.0);
        res.0.insert(DamageKind::Explosive, 2.0);
        let wall = app.world_mut().spawn((Health { health: 100, max_health: 100 }, res)).id();
        hit(&mut app, ApplyDamage::at(wall, 30, DamageKind::Ballistic, Vec3::ZERO));
        assert_eq!(app.world().get::<Health>(wall).unwrap().health, 100, "immune to bullets");
        hit(&mut app, ApplyDamage::at(wall, 30, DamageKind::Explosive, Vec3::ZERO));
        assert_eq!(app.world().get::<Health>(wall).unwrap().health, 40, "double from explosives");
    }

    #[test]
    fn a_lethal_hit_on_an_alien_counts_once_and_credits_the_source() {
        let mut app = test_app();
        let shooter = app.world_mut().spawn(Faction::Player).id();
        let alien = app.world_mut().spawn((Health { health: 10, max_health: 10 }, Alien, Faction::Alien)).id();
        hit(&mut app, ApplyDamage::at(alien, 10, DamageKind::Ballistic, Vec3::ZERO).from(shooter));
        // A second hit on the corpse (e.g. same frame as the despawn) is not a second kill.
        hit(&mut app, ApplyDamage::at(alien, 10, DamageKind::Ballistic, Vec3::ZERO).from(shooter));

        let caught = app.world().resource::<Caught>();
        let kills: Vec<_> = caught
            .events
            .iter()
            .filter(|e| matches!(e, GameTrackingEvent::AlienKilled(_)))
            .collect();
        assert_eq!(kills.len(), 1);
        assert!(matches!(kills[0], GameTrackingEvent::AlienKilled(e) if *e == shooter));
        assert_eq!(app.world().resource::<AlienCounter>().count, 0);
        assert!(caught.dealt[0].lethal);
    }

    #[test]
    fn zero_or_negative_damage_is_ignored() {
        let mut app = test_app();
        let t = app.world_mut().spawn(Health { health: 50, max_health: 50 }).id();
        hit(&mut app, ApplyDamage::at(t, 0, DamageKind::Ballistic, Vec3::ZERO));
        hit(&mut app, ApplyDamage::at(t, -5, DamageKind::Ballistic, Vec3::ZERO));
        assert_eq!(app.world().get::<Health>(t).unwrap().health, 50);
        assert!(app.world().resource::<Caught>().dealt.is_empty());
    }
}
