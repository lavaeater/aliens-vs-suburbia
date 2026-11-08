//! `--smoke`: drive a match through what physics touches, with nobody at the controls.
//!
//! On entering a match it builds one of every tower next to the first player; then every
//! [`EVERY`] seconds it kills the aliens nearest the players (dismemberment, gibs, loot), sets
//! off an explosion among them (shove, wall shielding, fire every other time) and throws a
//! grenade or molotov at the nearest. Players are healed so the run doesn't end in a wipe.
//! With `--perf` it is a load test for the gore; on its own a crash test for a backend.

use bevy::prelude::*;

use crate::alien::components::general::Alien;
use crate::assets::asset_definition::AmmoKind;
use crate::control::components::{CharacterControl, ControlCommand};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::general::components::map_components::MapModelDefinitions;
use crate::general::damage::ApplyDamage;
use crate::general::explosion::{Explode, ExplosionProps};
use crate::gore::components::DamageKind;
use crate::physics::prelude::Position;
use crate::player::ammo::AmmoPouch;
use crate::player::components::{AutoAim, Player};
use crate::towers::events::BuildTower;

/// Seconds between rounds.
const EVERY: f32 = 2.0;
/// Aliens killed outright each round.
const KILLS: usize = 3;
/// How far from the player the towers go up, and apart.
const TOWER_SPACING: f32 = 2.0;

pub struct SmokePlugin;

impl Plugin for SmokePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Smoke>()
            .add_systems(OnEnter(GameState::InGame), |mut smoke: ResMut<Smoke>| {
                *smoke = Smoke::default();
            })
            .add_systems(
                Update,
                (build_towers, havoc).run_if(in_state(GameState::InGame)),
            );
    }
}

#[derive(Resource, Default)]
struct Smoke {
    towers_built: bool,
    next: f32,
    rounds: u32,
    kills: u32,
}

fn build_towers(
    mut smoke: ResMut<Smoke>,
    players: Query<&Position, With<Player>>,
    defs: Res<MapModelDefinitions>,
    mut build: MessageWriter<BuildTower>,
) {
    if smoke.towers_built {
        return;
    }
    let Some(at) = players.iter().next() else {
        return;
    };
    for option in 0..defs.build_indicators.len() {
        let offset = Vec3::new(TOWER_SPACING * (option as f32 + 1.0), 0.0, TOWER_SPACING);
        build.write(BuildTower {
            position: at.0 + offset,
            option,
        });
    }
    smoke.towers_built = true;
    info!("smoke: built {} towers", defs.build_indicators.len());
}

#[allow(clippy::type_complexity)]
fn havoc(
    time: Res<Time>,
    mut smoke: ResMut<Smoke>,
    mut players: Query<
        (
            &Position,
            &mut Health,
            &mut AutoAim,
            &mut CharacterControl,
            &mut AmmoPouch,
        ),
        With<Player>,
    >,
    aliens: Query<(Entity, &Position), With<Alien>>,
    mut damage: MessageWriter<ApplyDamage>,
    mut explode: MessageWriter<Explode>,
) {
    if time.elapsed_secs() < smoke.next {
        return;
    }
    smoke.next = time.elapsed_secs() + EVERY;
    smoke.rounds += 1;
    let fire = smoke.rounds % 2 == 0;
    let mut killed: usize = 0;
    for (at, mut health, mut aim, mut control, mut pouch) in &mut players {
        let max = health.max_health;
        health.heal(max);
        let mut near: Vec<_> = aliens
            .iter()
            .map(|(alien, pos)| (alien, pos.0, pos.0.distance_squared(at.0)))
            .collect();
        near.sort_by(|a, b| a.2.total_cmp(&b.2));
        let Some(&(_, nearest, _)) = near.first() else {
            continue;
        };
        for &(alien, pos, _) in near.iter().take(KILLS) {
            let away = (pos - at.0).with_y(0.0).normalize_or(Vec3::X);
            damage.write(
                ApplyDamage::at(alien, 10_000, DamageKind::Ballistic, pos).along(away),
            );
            killed += 1;
        }
        if let Some(&(_, blast, _)) = near.get(KILLS) {
            explode.write(Explode {
                position: blast,
                props: ExplosionProps {
                    fire,
                    ..default()
                },
                source: None,
            });
        }
        aim.0 = (nearest - at.0).with_y(0.0).normalize_or(Vec3::X);
        pouch.add(if fire { AmmoKind::Molotov } else { AmmoKind::Grenade }, 1);
        control.triggers.insert(ControlCommand::ThrowSpecial);
    }
    smoke.kills += killed as u32;
    info!(
        "smoke: round {}, {} kills so far, {} aliens left",
        smoke.rounds,
        smoke.kills,
        aliens.iter().count().saturating_sub(killed)
    );
}
