//! What happens when a player's health hits zero.
//!
//! 1. **Downed** — `PlayerDead` goes on; they cannot move or act. A teammate holding
//!    Interact (E / Circle) within [`REVIVE_RANGE`] for [`REVIVE_DURATION`] brings them
//!    back at half health.
//! 2. **Bleed-out** — after `GameSettings::bleed_out_secs` untended, the body is despawned
//!    through the gore path, guns and ammo are dropped as pickups, one life is spent and
//!    a [`Respawning`] entry is queued.
//! 3. **Respawn** — after `respawn_secs` the roster slot is re-spawned via `SpawnPlayer`,
//!    either beside a living teammate the player picked (left/right on their own device
//!    cycles the anchor) or on the nearest walkable tile to where they fell that has no
//!    alien nearby. With no lives left they are out; when everyone is out the level fails.

use avian3d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;
use lava_ui_builder::{progress_bar, WorldFollower};

use crate::alien::components::general::Alien;
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::assets::asset_definition::ItemKind;
use crate::building::systems::ToWorldCoordinates;
use crate::control::bindings::GamepadBindings;
use crate::control::components::{CharacterControl, ControlCommand};
use crate::game_state::score_keeper::{LevelState, LevelTracker};
use crate::general::components::Health;
use crate::general::events::map_events::SpawnPlayer;
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::map_systems::TileDefinitions;
use crate::gore::components::{DamageKind, EntityDied, LastHit};
use crate::items::SpawnItem;
use crate::player::ammo::AmmoPouch;
use crate::player::components::{Lives, Player, PlayerDead, PlayerSlot};
use crate::player::systems::loadout::Weapons;
use crate::player_setup::state::{InputDevice, PlayerRoster};
use crate::settings::resources::GameSettings;

const REVIVE_RANGE: f32 = 1.8;
const REVIVE_DURATION: f32 = 3.0;
/// Respawn tiles closer than this to any alien are skipped.
const RESPAWN_ALIEN_CLEARANCE: f32 = 4.0;

/// A slot waiting to come back, or out of lives.
#[derive(Debug, Clone)]
pub struct Respawning {
    pub slot: usize,
    pub lives_left: u32,
    pub timer: Timer,
    pub death_position: Vec3,
    /// Slot of the living teammate to spawn beside, if the player picked one.
    pub anchor: Option<usize>,
}

/// Slots between bodies: respawn countdowns and the players who are out for good.
#[derive(Resource, Default, Debug)]
pub struct RespawnQueue {
    pub pending: Vec<Respawning>,
    pub out: Vec<usize>,
    /// Set once any player has spawned this level, so an empty map is not a team wipe.
    pub started: bool,
}

impl RespawnQueue {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn pending_for(&self, slot: usize) -> Option<&Respawning> {
        self.pending.iter().find(|r| r.slot == slot)
    }

    pub fn is_out(&self, slot: usize) -> bool {
        self.out.contains(&slot)
    }
}

/// When a player's health reaches 0, mark them as downed instead of letting
/// them be despawned. Zeros their velocity and spawns a revive progress bar.
#[allow(clippy::type_complexity)]
pub fn detect_player_death(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut query: Query<
        (Entity, &Health, &mut LinearVelocity),
        (With<Player>, Without<PlayerDead>, Changed<Health>),
    >,
    mut anim_ew: MessageWriter<AnimationEvent>,
) {
    for (entity, health, mut vel) in query.iter_mut() {
        if !health.is_dead() { continue; }

        vel.0 = Vec3::ZERO;

        let bar = commands.spawn((
            WorldFollower { target: entity, offset: Vec2::new(-30.0, -60.0) },
            progress_bar(0.0, 60.0, 8.0, Color::srgb(0.2, 0.6, 1.0), Color::srgba(0.0, 0.0, 0.0, 0.6)),
            Node { position_type: PositionType::Absolute, ..default() },
        )).id();

        commands.entity(entity).insert(PlayerDead {
            revive_progress: 0.0,
            revive_bar: Some(bar),
            bleed_out: settings.bleed_out_secs,
        });

        anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Death));
    }
}

/// Hold Interact near a downed player to revive them over REVIVE_DURATION seconds.
/// Releasing resets progress.
#[allow(clippy::type_complexity)]
pub fn player_revive_system(
    mut commands: Commands,
    time: Res<Time>,
    living_players: Query<(&Position, &CharacterControl), (With<Player>, Without<PlayerDead>)>,
    mut dead_players: Query<(Entity, &Position, &mut PlayerDead, &mut Health)>,
    mut anim_ew: MessageWriter<AnimationEvent>,
    mut bar_query: Query<&mut lava_ui_builder::ProgressBar>,
) {
    for (dead_entity, dead_pos, mut dead, mut health) in dead_players.iter_mut() {
        let helped = living_players.iter().any(|(p, control)| {
            control.triggers.contains(&ControlCommand::Interact) && p.0.distance(dead_pos.0) <= REVIVE_RANGE
        });

        if helped {
            dead.revive_progress += time.delta_secs() / REVIVE_DURATION;
        } else {
            dead.revive_progress = (dead.revive_progress - time.delta_secs() / REVIVE_DURATION).max(0.0);
        }

        // Sync progress bar.
        if let Some(bar_entity) = dead.revive_bar
            && let Ok(mut bar) = bar_query.get_mut(bar_entity)
        {
            bar.value = dead.revive_progress.clamp(0.0, 1.0);
        }

        if dead.revive_progress >= 1.0 {
            // Revive: restore half health, remove downed state.
            if let Some(bar_entity) = dead.revive_bar {
                commands.entity(bar_entity).despawn();
            }
            health.health = health.max_health / 2;
            commands.entity(dead_entity).remove::<PlayerDead>();
            anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, dead_entity, AnimationKey::Death));
        }
    }
}

/// Count down untended downed players; on bleed-out drop their kit, spend a life and
/// queue the respawn (or mark them out).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn tick_bleed_out(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut queue: ResMut<RespawnQueue>,
    mut downed: Query<
        (Entity, &PlayerSlot, &Position, &mut PlayerDead, Option<&Lives>, Option<&Weapons>, Option<&AmmoPouch>, Option<&LastHit>),
        With<Player>,
    >,
    mut died_mw: MessageWriter<EntityDied>,
    mut spawn_item_mw: MessageWriter<SpawnItem>,
) {
    for (entity, slot, pos, mut dead, lives, loadout, pouch, last_hit) in downed.iter_mut() {
        dead.bleed_out -= time.delta_secs();
        if dead.bleed_out > 0.0 {
            continue;
        }

        if settings.drop_on_death {
            for (i, kind) in dropped_kit(loadout, pouch).into_iter().enumerate() {
                spawn_item_mw.write(SpawnItem { kind, position: pos.0 + scatter(i) });
            }
        }

        let lives_left = lives.map_or(settings.lives_per_player, |l| l.0).saturating_sub(1);
        if lives_left > 0 {
            queue.pending.push(Respawning {
                slot: slot.0,
                lives_left,
                timer: Timer::from_seconds(settings.respawn_secs, TimerMode::Once),
                death_position: pos.0,
                anchor: None,
            });
        } else {
            queue.out.push(slot.0);
        }

        let last = last_hit.copied().unwrap_or(LastHit { normal: Vec3::Y, kind: DamageKind::Blunt });
        died_mw.write(EntityDied { entity, position: pos.0, normal: last.normal, kind: last.kind });
        if let Some(bar) = dead.revive_bar {
            commands.entity(bar).despawn();
        }
        commands.entity(entity).despawn();
    }
}

/// Everything a bled-out player leaves on the ground: each carried gun and every
/// non-empty ammo pool.
pub fn dropped_kit(loadout: Option<&Weapons>, pouch: Option<&AmmoPouch>) -> Vec<ItemKind> {
    let mut kit = Vec::new();
    if let Some(loadout) = loadout {
        kit.extend(loadout.slots.iter().map(|s| ItemKind::WeaponPickup { def: s.def_path.clone() }));
    }
    if let Some(pouch) = pouch {
        let mut pools: Vec<_> = pouch.0.iter().filter(|(_, n)| **n > 0).collect();
        pools.sort_by_key(|(k, _)| format!("{k:?}"));
        kit.extend(pools.into_iter().map(|(kind, rounds)| ItemKind::AmmoPickup { kind: *kind, rounds: *rounds }));
    }
    kit
}

/// Offsets for the n-th dropped item so a pile does not stack in one point.
fn scatter(i: usize) -> Vec3 {
    let angle = i as f32 * 2.4; // golden-angle-ish spread
    let radius = 0.5 + 0.15 * i as f32;
    Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
}

/// Let a waiting player pick which living teammate to spawn beside: left/right on their
/// own device cycles through the living, and back to "where I fell".
#[allow(clippy::type_complexity)]
pub fn choose_respawn_anchor(
    mut queue: ResMut<RespawnQueue>,
    roster: Option<Res<PlayerRoster>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    bindings: Option<Res<GamepadBindings>>,
    living: Query<&PlayerSlot, (With<Player>, Without<PlayerDead>)>,
) {
    let Some(roster) = roster else { return };
    let mut living_slots: Vec<usize> = living.iter().map(|s| s.0).collect();
    living_slots.sort_unstable();
    if living_slots.is_empty() {
        return;
    }
    let pads: Vec<&Gamepad> = gamepads.iter().collect();

    for entry in &mut queue.pending {
        let step = match roster.devices.get(entry.slot) {
            Some(InputDevice::Keyboard) => {
                i32::from(keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD))
                    - i32::from(keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA))
            }
            Some(InputDevice::Gamepad(index)) => match (pads.get(*index), bindings.as_ref()) {
                (Some(pad), Some(b)) => {
                    i32::from(pad.just_pressed(b.next_build_item)) - i32::from(pad.just_pressed(b.prev_build_item))
                }
                _ => 0,
            },
            None => 0,
        };
        if step != 0 {
            entry.anchor = cycle_anchor(entry.anchor, &living_slots, step);
        }
    }
}

/// Step through `None` (own death spot) then each living slot, wrapping both ways.
pub fn cycle_anchor(current: Option<usize>, living: &[usize], step: i32) -> Option<usize> {
    let n = living.len() as i32 + 1; // +1 for "where I fell"
    let idx = match current {
        None => 0,
        Some(slot) => living.iter().position(|s| *s == slot).map_or(0, |p| p as i32 + 1),
    };
    let next = (idx + step).rem_euclid(n);
    if next == 0 { None } else { living.get(next as usize - 1).copied() }
}

/// Fire `SpawnPlayer` for respawns whose timer has run out.
#[allow(clippy::type_complexity)]
pub fn tick_respawns(
    time: Res<Time>,
    mut queue: ResMut<RespawnQueue>,
    map_graph: Option<Res<MapGraph>>,
    tile_defs: Option<Res<TileDefinitions>>,
    living: Query<(&PlayerSlot, &Position), (With<Player>, Without<PlayerDead>)>,
    aliens: Query<&Position, With<Alien>>,
    mut spawn_mw: MessageWriter<SpawnPlayer>,
) {
    let alien_positions: Vec<Vec3> = aliens.iter().map(|p| p.0).collect();
    let mut ready = Vec::new();
    for (i, entry) in queue.pending.iter_mut().enumerate() {
        entry.timer.tick(time.delta());
        if entry.timer.is_finished() {
            ready.push(i);
        }
    }
    for i in ready.into_iter().rev() {
        let entry = queue.pending.remove(i);
        let anchor_pos = entry
            .anchor
            .and_then(|slot| living.iter().find(|(s, _)| s.0 == slot).map(|(_, p)| p.0));
        let position = if let Some(p) = anchor_pos { p + Vec3::new(0.8, 0.0, 0.8) } else {
            let walkable: Vec<Vec3> = match (map_graph.as_ref(), tile_defs.as_ref()) {
                (Some(graph), Some(defs)) => graph
                    .path_finding_grid
                    .iter()
                    .map(|tile| tile.to_world_coords(defs))
                    .collect(),
                _ => Vec::new(),
            };
            respawn_position(entry.death_position, &walkable, &alien_positions)
        };
        spawn_mw.write(SpawnPlayer {
            position: position + Vec3::Y,
            slot: Some(entry.slot),
            lives: Some(entry.lives_left),
        });
    }
}

/// Nearest walkable tile to `death` with no alien within [`RESPAWN_ALIEN_CLEARANCE`];
/// the death spot itself when nothing qualifies.
pub fn respawn_position(death: Vec3, walkable: &[Vec3], aliens: &[Vec3]) -> Vec3 {
    let clear = |p: &Vec3| aliens.iter().all(|a| a.distance(*p) >= RESPAWN_ALIEN_CLEARANCE);
    walkable
        .iter()
        .filter(|p| clear(p))
        .min_by(|a, b| a.distance_squared(death).total_cmp(&b.distance_squared(death)))
        .copied()
        .unwrap_or(death)
}

/// Fail the level once nobody is alive, nobody is coming back, and someone did play.
pub fn check_team_wipe(
    queue: Res<RespawnQueue>,
    players: Query<(), With<Player>>,
    mut tracker: Option<ResMut<LevelTracker>>,
) {
    let Some(tracker) = tracker.as_mut() else { return };
    if !queue.started || !players.is_empty() || !queue.pending.is_empty() {
        return;
    }
    if matches!(tracker.level_state, LevelState::InProgress) {
        tracker.level_state = LevelState::Failed;
    }
}

pub fn reset_respawn_queue(mut queue: ResMut<RespawnQueue>) {
    queue.reset();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::AmmoKind;

    #[test]
    fn anchor_cycles_through_fell_here_and_each_living_player() {
        let living = [0, 2];
        assert_eq!(cycle_anchor(None, &living, 1), Some(0));
        assert_eq!(cycle_anchor(Some(0), &living, 1), Some(2));
        assert_eq!(cycle_anchor(Some(2), &living, 1), None, "wraps back to the death spot");
        assert_eq!(cycle_anchor(None, &living, -1), Some(2), "and backwards");
        assert_eq!(cycle_anchor(Some(9), &living, 1), Some(0), "a dead anchor restarts from the top");
    }

    #[test]
    fn respawn_prefers_the_nearest_tile_clear_of_aliens() {
        let death = Vec3::ZERO;
        let walkable = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(6.0, 0.0, 0.0), Vec3::new(-8.0, 0.0, 0.0)];
        let aliens = [Vec3::new(2.0, 0.0, 0.0)];
        // (1,0,0) is 1 unit from the alien: rejected. (6,0,0) is 4 away: fine and nearer than -8.
        assert_eq!(respawn_position(death, &walkable, &aliens), Vec3::new(6.0, 0.0, 0.0));
        assert_eq!(respawn_position(death, &[], &aliens), death, "nothing walkable: fall back to the spot");
    }

    #[test]
    fn dropped_kit_lists_every_gun_and_non_empty_pool() {
        let loadout = Weapons::new("c.ron", ["a.ron".to_string(), "b.ron".to_string()]);
        let pouch = AmmoPouch::from_loadout(&[(AmmoKind::Pistol, 10), (AmmoKind::Shells, 0)]);
        let kit = dropped_kit(Some(&loadout), Some(&pouch));
        assert_eq!(kit.len(), 3);
        assert!(matches!(&kit[0], ItemKind::WeaponPickup { def } if def == "a.ron"));
        assert!(matches!(&kit[2], ItemKind::AmmoPickup { kind: AmmoKind::Pistol, rounds: 10 }));
    }
}
