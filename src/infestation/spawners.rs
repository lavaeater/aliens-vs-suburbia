//! Spawners: where one opens, the waves it sends out, and its end.
//!
//! Every `every` seconds (a fresh pick from the range each time) a spawner opens on a
//! random patch of open lawn or street well away from the players. It sends out
//! `waves` waves drawn from the map's `waves`, a pause between each, then one monster
//! every so often until it is destroyed. It is shot like an alien (the `Alien` collision
//! layer and faction), and blows up when it goes.

use crate::physics::prelude::{Collider, CollisionLayers, Position, RigidBody};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use bevy_wind_waker_shader::WindWakerShaderBuilder;

use super::Infestation;
use crate::assets::assets_plugin::GameAssets;
use crate::building::systems::ToWorldCoordinates;
use crate::fx::{FxKind, SpawnFx};
use crate::game_state::score_keeper::{LevelState, LevelTracker};
use crate::general::components::map_components::{AlienSpawnPoint, WaveDef};
use crate::general::components::{CollisionLayer, Health};
use crate::general::damage::Faction;
use crate::general::events::map_events::SpawnAlien;
use crate::general::systems::lights_systems::{LightAnimation, LightStyle};
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::Player;
use crate::ui::spawn_ui::AddHealthBar;

/// A spawner's health: a few magazines' worth.
const SPAWNER_HEALTH: i32 = 400;
/// No spawner opens closer than this to a player (metres).
const CLEAR_OF_PLAYERS: f32 = 14.0;
/// Nor closer than this to another spawner.
const CLEAR_OF_SPAWNERS: f32 = 8.0;
/// Seconds between a spawner's waves.
const BETWEEN_WAVES: (f32, f32) = (8.0, 16.0);
/// Seconds between monsters once its waves are out.
const TRICKLE: (f32, f32) = (12.0, 22.0);

/// One wave a spawner sends out.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnerWave {
    pub enemy_def: Option<String>,
    pub count: u32,
    /// Seconds between two monsters of the wave.
    pub interval: f32,
}

impl From<&WaveDef> for SpawnerWave {
    fn from(wave: &WaveDef) -> Self {
        Self {
            enemy_def: (!wave.enemy_def.is_empty()).then(|| wave.enemy_def.clone()),
            count: wave.count,
            interval: if wave.spawn_rate_per_minute > 0.0 {
                60.0 / wave.spawn_rate_per_minute
            } else {
                3.0
            },
        }
    }
}

/// A monster a spawner sends out: its enemy def, `None` for the built-in alien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monster(pub Option<String>);

/// An open spawner and where it is in its waves.
#[derive(Component, Debug, Clone)]
pub struct Spawner {
    pub waves: Vec<SpawnerWave>,
    /// The wave being sent out (`waves.len()` = trickling).
    pub wave: usize,
    /// Monsters of that wave still to come.
    pub left_in_wave: u32,
    /// Seconds to the next monster.
    pub cooldown: f32,
}

impl Spawner {
    pub fn new(waves: Vec<SpawnerWave>) -> Self {
        let left_in_wave = waves.first().map_or(0, |w| w.count);
        Self {
            waves,
            wave: 0,
            left_in_wave,
            cooldown: 1.5,
        }
    }

    /// Monsters still to come: the rest of its waves, plus one for the trickle.
    pub fn pending(&self) -> u32 {
        let later: u32 = self
            .waves
            .iter()
            .skip(self.wave.saturating_add(1))
            .map(|w| w.count)
            .sum();
        self.left_in_wave.saturating_add(later).saturating_add(1)
    }

    /// Advance `dt` seconds; the monster that comes out, if one does. `pause` picks a
    /// pause (seconds) from a range when one is due.
    pub fn tick(&mut self, dt: f32, mut pause: impl FnMut((f32, f32)) -> f32) -> Option<Monster> {
        self.cooldown -= dt;
        if self.cooldown > 0.0 {
            return None;
        }
        let trickling = self.wave >= self.waves.len();
        let Some(wave) = self.waves.get(self.wave).or_else(|| self.waves.last()) else {
            self.cooldown = pause(TRICKLE);
            return None;
        };
        let monster = Monster(wave.enemy_def.clone());
        if trickling {
            self.cooldown = pause(TRICKLE);
            return Some(monster);
        }
        let interval = wave.interval;
        self.left_in_wave = self.left_in_wave.saturating_sub(1);
        if self.left_in_wave == 0 {
            self.wave = self.wave.saturating_add(1);
            self.left_in_wave = self.waves.get(self.wave).map_or(0, |w| w.count);
            self.cooldown = pause(if self.wave < self.waves.len() {
                BETWEEN_WAVES
            } else {
                TRICKLE
            });
        } else {
            self.cooldown = interval;
        }
        Some(monster)
    }
}

/// Lawn and street tiles with lawn or street all round them, in the padded grid's
/// coordinates (the map loader pads the map by a tile of void).
pub fn open_ground(tiles: &[Vec<u64>]) -> Vec<(usize, usize)> {
    let at = |x: usize, y: usize| tiles.get(y).and_then(|row| row.get(x)).copied();
    let mut out = Vec::new();
    for (y, row) in tiles.iter().enumerate().skip(1) {
        for x in 1..row.len() {
            let all_round = (y.saturating_sub(1)..=y.saturating_add(1)).all(|ny| {
                (x.saturating_sub(1)..=x.saturating_add(1))
                    .all(|nx| at(nx, ny).is_some_and(super::is_ground))
            });
            if all_round {
                out.push((x.saturating_add(1), y.saturating_add(1)));
            }
        }
    }
    out
}

/// Where the next spawner opens: a random tile from `ground` clear of `players` and
/// `spawners` (world positions; tile `(x, y)` is at world `(x, y)` on the floor plane).
pub fn pick_tile(
    ground: &[(usize, usize)],
    players: &[Vec2],
    spawners: &[Vec2],
    roll: f32,
) -> Option<(usize, usize)> {
    let clear = |tile: &&(usize, usize)| {
        let at = Vec2::new(tile.0 as f32, tile.1 as f32);
        players.iter().all(|p| p.distance(at) >= CLEAR_OF_PLAYERS)
            && spawners.iter().all(|s| s.distance(at) >= CLEAR_OF_SPAWNERS)
    };
    let candidates: Vec<&(usize, usize)> = ground.iter().filter(clear).collect();
    let pool = if candidates.is_empty() {
        // Nowhere far enough: anywhere will do rather than no spawner at all.
        ground.iter().collect()
    } else {
        candidates
    };
    let index =
        ((roll.clamp(0.0, 0.999) * pool.len() as f32) as usize).min(pool.len().saturating_sub(1));
    pool.get(index).map(|&&tile| tile)
}

/// Open the next spawner when its time comes.
#[allow(clippy::too_many_arguments)]
pub fn open_spawners(
    time: Res<Time>,
    tracker: Res<LevelTracker>,
    mut infestation: ResMut<Infestation>,
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    tile_defs: Res<TileDefinitions>,
    players: Query<&Position, With<Player>>,
    spawners: Query<&Position, With<Spawner>>,
    mut health_bars: MessageWriter<AddHealthBar>,
) {
    let Some(rules) = infestation.rules.clone() else {
        return;
    };
    if !matches!(tracker.level_state, LevelState::InProgress) || infestation.unopened() == 0 {
        return;
    }
    infestation.next_in -= time.delta_secs();
    if infestation.next_in > 0.0 {
        return;
    }
    infestation.next_in = infestation.random(rules.every.0, rules.every.1);

    let players: Vec<Vec2> = players.iter().map(|p| p.0.xz()).collect();
    let others: Vec<Vec2> = spawners.iter().map(|p| p.0.xz()).collect();
    let roll = infestation.random(0.0, 1.0);
    let Some(tile) = pick_tile(&infestation.open_ground, &players, &others, roll) else {
        warn!("infestation: no open ground for a spawner");
        return;
    };
    let mut waves = Vec::new();
    for _ in 0..rules.waves.max(1) {
        let count = infestation.waves.len();
        let pick = (infestation.random(0.0, count as f32) as usize).min(count.saturating_sub(1));
        if let Some(wave) = infestation.waves.get(pick) {
            waves.push(SpawnerWave::from(wave));
        }
    }
    infestation.opened = infestation.opened.saturating_add(1);
    let at = tile.to_world_coords(&tile_defs) + Vec3::new(0.0, -tile_defs.wall_height, 0.0);
    info!(
        "infestation: spawner {} of {} opens at {tile:?}",
        infestation.opened, rules.spawners
    );
    let id = commands
        .spawn((
            Name::from(format!("Spawner {}:{}", tile.0, tile.1)),
            Spawner::new(waves),
            // The spawn point's hum and look come with it (`music`), nothing else: the
            // wave manager's spawning is idle on an infestation level.
            AlienSpawnPoint::new(0.0),
            Faction::Alien,
            Health::full(SPAWNER_HEALTH),
            WorldAssetRoot(game_assets.alien_construct.clone()),
            WindWakerShaderBuilder::default().build(),
            RigidBody::Static,
            Collider::cuboid(0.9, 0.9, 0.9),
            Position::from(at),
            Transform::from_translation(at).with_scale(Vec3::splat(1.6)),
            // On the alien layer so bullets stop on it; players and aliens bump into it.
            CollisionLayers::new(
                [CollisionLayer::Alien],
                [
                    CollisionLayer::Player,
                    CollisionLayer::Alien,
                    CollisionLayer::Ball,
                ],
            ),
        ))
        .with_child((
            PointLight {
                color: Color::srgb(1.0, 0.15, 0.3),
                intensity: 60_000.0,
                range: 6.0,
                ..default()
            },
            LightAnimation {
                base: 60_000.0,
                style: LightStyle::Pulse {
                    speed: 0.8,
                    depth: 0.6,
                },
            },
            Transform::from_xyz(0.0, 1.2, 0.0),
        ))
        .id();
    health_bars.write(AddHealthBar {
        entity: id,
        name: "SPAWNER",
    });
}

/// Every open spawner sends out what is due.
pub fn run_spawners(
    time: Res<Time>,
    tracker: Res<LevelTracker>,
    mut infestation: ResMut<Infestation>,
    mut spawners: Query<(&mut Spawner, &Position)>,
    mut spawn: MessageWriter<SpawnAlien>,
) {
    if infestation.rules.is_none() || !matches!(tracker.level_state, LevelState::InProgress) {
        return;
    }
    let dt = time.delta_secs();
    for (mut spawner, position) in &mut spawners {
        let out = spawner.tick(dt, |(lo, hi)| infestation.random(lo, hi));
        if let Some(Monster(enemy_def)) = out {
            // Out of the side of it, not on top of it.
            let angle = infestation.random(0.0, std::f32::consts::TAU);
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.4;
            spawn.write(SpawnAlien {
                enemy_def,
                position: position.0 + offset + Vec3::Y * 0.6,
                hunts: true,
            });
        }
    }
}

/// A spawner going (destroyed, or the level ending): count it and blow it up.
pub fn spawner_destroyed(
    removed: On<Remove, Spawner>,
    positions: Query<(&Position, &Health)>,
    mut infestation: ResMut<Infestation>,
    mut fx: MessageWriter<SpawnFx>,
) {
    let Ok((position, health)) = positions.get(removed.entity) else {
        return;
    };
    // Only a kill counts; leaving the level despawns the healthy ones too.
    if health.health > 0 || infestation.rules.is_none() {
        return;
    }
    infestation.destroyed = infestation.destroyed.saturating_add(1);
    info!("infestation: spawner destroyed ({})", infestation.destroyed);
    fx.write(SpawnFx::at(FxKind::Explosion, position.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wave(count: u32) -> SpawnerWave {
        SpawnerWave {
            enemy_def: Some("defs/Monster.ron".into()),
            count,
            interval: 1.0,
        }
    }

    #[test]
    fn a_spawner_sends_its_waves_then_trickles() {
        let mut spawner = Spawner::new(vec![wave(2), wave(1)]);
        assert_eq!(spawner.pending(), 2 + 1 + 1);
        let mut out = 0;
        // Pauses are a flat 5 s here; 60 seconds at a tenth of a second.
        for _ in 0..600 {
            if spawner.tick(0.1, |_| 5.0).is_some() {
                out += 1;
            }
        }
        assert_eq!(spawner.wave, 2, "both waves out");
        assert!(out > 3, "and then some: {out}");
        assert_eq!(spawner.pending(), 1, "only the trickle is left");
    }

    #[test]
    fn a_spawner_without_waves_never_sends_anything() {
        let mut spawner = Spawner::new(Vec::new());
        for _ in 0..100 {
            assert!(spawner.tick(1.0, |_| 1.0).is_none());
        }
    }

    #[test]
    fn spawners_open_away_from_the_players() {
        let ground: Vec<(usize, usize)> = (0..50).map(|x| (x, 5)).collect();
        let players = [Vec2::new(0.0, 5.0)];
        for roll in [0.0, 0.3, 0.7, 0.99] {
            let (x, _) = pick_tile(&ground, &players, &[], roll).expect("a tile");
            assert!(x >= 14, "{x}");
        }
        assert!(pick_tile(&[], &players, &[], 0.5).is_none());
    }

    #[test]
    fn open_ground_is_lawn_or_street_all_round() {
        use crate::map::MapFeatures;
        let lawn = MapFeatures::Grass as u64;
        let wall = MapFeatures::Floor as u64 | MapFeatures::ImpassableForPlayers as u64;
        let mut tiles = vec![vec![lawn; 5]; 5];
        tiles[0] = vec![wall; 5];
        let ground = open_ground(&tiles);
        // Row 1 touches the wall; rows 2 and 3 (padded: 3 and 4) are clear in columns 1-3.
        assert!(ground.contains(&(2, 3)));
        assert!(!ground.iter().any(|&(_, y)| y == 2), "{ground:?}");
        assert!(!ground.iter().any(|&(x, _)| x == 1 || x == 5), "{ground:?}");
    }
}
