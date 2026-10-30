//! Neighbours: the people in the yards. Any character def placed on a map is one (the
//! map loader writes a [`SpawnNeighbour`] for it); the neighbourhood generator places the
//! people defs in `assets/defs/people`. A neighbour minds their own business (their def's
//! `Idle`: watering the garden, on the phone) until a monster comes near, then runs - away
//! from it, and towards a player if one is about - and calls for help when one is on them.
//!
//! They are on the players' side (`Faction::Player`, the player collision layer), so the
//! players' bullets and blasts pass them by and the monsters' claws do not.

use avian3d::prelude::{
    AngularDamping, AngularVelocity, CollisionLayers, Friction, LinearDamping, LinearVelocity,
    LockedAxes, Position, RigidBody, Rotation,
};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

use super::hunters::{CreatureAnim, creature_body, facing};
use crate::alien::components::general::Alien;
use crate::alien::enemy_defs::EnemyDefCache;
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::general::components::map_components::CurrentTile;
use crate::general::components::{CollisionLayer, Health};
use crate::general::damage::Faction;
use crate::model_settings::resources::ModelSettings;
use crate::player::components::{Player, PlayerDead};
use crate::ui::spawn_ui::AddHealthBar;

/// A neighbour's health: a few seconds in a monster's grip.
const NEIGHBOUR_HEALTH: i32 = 60;
/// A monster this close (metres) sends a neighbour running.
const FLEE_FROM: f32 = 5.0;
/// This close, they call for help instead of just running.
const CORNERED: f32 = 1.0;
/// A player this close is somewhere to run to.
const RUN_TO: f32 = 15.0;
/// Running speed, metres per second.
const RUN_SPEED: f32 = 2.2;
/// Seconds they keep running once no monster is near.
const CALM_DOWN: f32 = 2.5;

/// A neighbour, and how long until they stop running.
#[derive(Component, Debug, Default)]
pub struct Neighbour {
    pub scared_for: f32,
}

/// Put a neighbour on the map: `def` is a character def's content path, `position` its
/// spot on the floor, `yaw` the way they face.
#[derive(Message, Clone, Debug)]
pub struct SpawnNeighbour {
    pub def: String,
    pub position: Vec3,
    pub yaw: f32,
}

pub fn spawn_neighbours(
    mut requests: MessageReader<SpawnNeighbour>,
    mut commands: Commands,
    mut cache: ResMut<EnemyDefCache>,
    asset_server: Res<AssetServer>,
    settings: Res<ModelSettings>,
    mut health_bars: MessageWriter<AddHealthBar>,
) {
    for request in requests.read() {
        let Some(loaded) = cache.get_or_load_creature(&request.def, &asset_server) else {
            warn!("neighbour: cannot read def '{}'", request.def);
            continue;
        };
        let name = std::path::Path::new(&request.def)
            .file_stem()
            .map_or_default(|s| s.to_string_lossy().into_owned());
        // Sized like the players (F2), not by the def's own scale.
        let (collider, mut fix) = creature_body(settings.scale);
        fix.translation.y = settings.translation_y;
        let id = commands
            .spawn((
                Name::new(format!("Neighbour {name}")),
                Neighbour::default(),
                Faction::Player,
                Health::full(NEIGHBOUR_HEALTH),
                CurrentTile::default(),
                Transform::from_translation(request.position)
                    .with_rotation(Quat::from_rotation_y(request.yaw)),
                WorldAssetRoot(loaded.scene.clone()),
                fix,
                collider,
                CurrentAnimationKey::new(request.def.clone(), AnimationKey::Idle),
                CreatureAnim(AnimationKey::Idle),
            ))
            .insert((
                RigidBody::Dynamic,
                LockedAxes::new().lock_rotation_x().lock_rotation_z(),
                Friction::new(0.0),
                LinearDamping(5.0),
                AngularDamping(1.0),
                CollisionLayers::new(
                    [CollisionLayer::Player],
                    [
                        CollisionLayer::ImpassableAll,
                        CollisionLayer::ImpassablePlayer,
                        CollisionLayer::Floor,
                        CollisionLayer::Alien,
                        CollisionLayer::Player,
                        CollisionLayer::Ball,
                    ],
                ),
            ))
            .id();
        health_bars.write(AddHealthBar {
            entity: id,
            name: "NEIGHBOUR",
        });
    }
}

/// Which way a neighbour at `at` runs: away from the monster at `monster`, bent towards
/// the player at `player` if there is one. `None` when they are on top of each other.
pub fn run_direction(at: Vec2, monster: Vec2, player: Option<Vec2>) -> Option<Vec2> {
    let away = (at - monster).try_normalize()?;
    let toward = player
        .and_then(|p| (p - at).try_normalize())
        .unwrap_or(Vec2::ZERO);
    (away + toward * 0.6).try_normalize().or(Some(away))
}

/// Run from the monsters.
#[allow(clippy::type_complexity)]
pub fn flee(
    time: Res<Time>,
    mut neighbours: Query<
        (
            &mut Neighbour,
            &Health,
            &Position,
            &mut LinearVelocity,
            &mut AngularVelocity,
            &mut Rotation,
            &mut CreatureAnim,
        ),
        Without<Alien>,
    >,
    monsters: Query<&Position, (With<Alien>, Without<Neighbour>)>,
    players: Query<&Position, (With<Player>, Without<PlayerDead>, Without<Neighbour>)>,
) {
    let dt = time.delta_secs();
    for (mut neighbour, health, position, mut velocity, mut spin, mut rotation, mut anim) in
        &mut neighbours
    {
        spin.0 = Vec3::ZERO;
        if health.health <= 0 {
            continue;
        }
        let here = position.0.xz();
        let nearest = |points: &mut dyn Iterator<Item = Vec2>| {
            points.min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)))
        };
        let monster = nearest(&mut monsters.iter().map(|p| p.0.xz()));
        let monster_distance = monster.map_or(f32::INFINITY, |m| m.distance(here));
        if monster_distance < FLEE_FROM {
            neighbour.scared_for = CALM_DOWN;
        } else {
            neighbour.scared_for = (neighbour.scared_for - dt).max(0.0);
        }
        let running = neighbour.scared_for > 0.0;
        let player =
            nearest(&mut players.iter().map(|p| p.0.xz())).filter(|p| p.distance(here) < RUN_TO);
        let dir = monster
            .filter(|_| running)
            .and_then(|m| run_direction(here, m, player));
        let Some(dir) = dir else {
            velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
            anim.set_if_neq(CreatureAnim(AnimationKey::Idle));
            continue;
        };
        velocity.0 = Vec3::new(dir.x * RUN_SPEED, velocity.0.y, dir.y * RUN_SPEED);
        if let Some(goal) = facing(dir) {
            rotation.0 = rotation.0.slerp(goal, (dt * 8.0).min(1.0));
        }
        anim.set_if_neq(CreatureAnim(if monster_distance < CORNERED {
            AnimationKey::Wave
        } else {
            AnimationKey::Run
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_run_away_and_towards_help() {
        let away = run_direction(Vec2::ZERO, Vec2::new(1.0, 0.0), None).expect("a way");
        assert!(away.distance(Vec2::new(-1.0, 0.0)) < 1e-4);
        // A player off to the side bends the run that way, still away from the monster.
        let bent = run_direction(Vec2::ZERO, Vec2::new(1.0, 0.0), Some(Vec2::new(0.0, 10.0)))
            .expect("a way");
        assert!(bent.x < 0.0 && bent.y > 0.0, "{bent}");
        assert!(run_direction(Vec2::ONE, Vec2::ONE, None).is_none());
    }
}
