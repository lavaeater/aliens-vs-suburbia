//! Hunters: the monsters a spawner sends out. Each picks the nearest prey - a neighbour,
//! or a player (neighbours first: a neighbour counts as nearer than it is) - walks there
//! along the map's pathfinding grid, and goes straight for it once it is close. Touch
//! damage does the hurting (`touch_damage_system`); a hunter on top of its prey plays its
//! attack.
//!
//! A hunter drives its own velocity and facing, so the goal-walking behaviours an `Alien`
//! comes with are taken off it ([`make_hunter`]). Its body is a player's: a 0.5 tall
//! capsule with the model's feet at its bottom and the model turned to face -Z, the way
//! the shared Quaternius rig needs.

use avian3d::prelude::{AngularVelocity, Collider, LinearVelocity, Position, Rotation};
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use pathfinding::directed::astar::astar;

use super::Neighbour;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::ai::components::move_towards_goal_components::MoveTowardsGoalData;
use crate::alien::components::general::Alien;
use crate::alien::enemy_defs::LoadedEnemyDef;
use crate::animation::animation_plugin::{
    AnimationKey, AnimationStore, CurrentAnimationKey, get_child_with_component_recursive,
};
use crate::control::components::KinematicMovement;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use crate::general::resources::map_resources::MapGraph;
use crate::player::components::{Player, PlayerDead};
use crate::player::systems::spawn_players::FixSceneTransform;

/// Seconds between a hunter's rethinks (prey and path), give or take a quarter.
const RETHINK: f32 = 0.8;
/// Closer than this (metres) a hunter stops following the path and goes straight in.
const STRAIGHT_IN: f32 = 2.5;
/// Closer than this it attacks.
const STRIKE: f32 = 0.8;
/// A neighbour counts as this much nearer than they are: they are what the monsters came
/// for.
const NEIGHBOUR_BIAS: f32 = 0.7;
/// How fast a creature turns to face where it goes (per second, as a blend factor).
const TURN_RATE: f32 = 8.0;

/// A monster on the hunt.
#[derive(Component, Debug, Default)]
pub struct Hunter {
    pub prey: Option<Entity>,
    /// Tiles to walk, next first (padded grid, as `CurrentTile`).
    pub path: Vec<(usize, usize)>,
    /// Seconds to the next rethink.
    pub rethink: f32,
    /// Metres per second.
    pub speed: f32,
}

/// The animation a creature (a hunter or a neighbour) wants; [`animate_creatures`]
/// plays it when it changes.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatureAnim(pub AnimationKey);

/// A player's body for a creature: the capsule, and the model's offset and turn within it.
pub fn creature_body(scale: f32) -> (Collider, FixSceneTransform) {
    (
        Collider::capsule(0.23, 0.04),
        FixSceneTransform::new(
            Vec3::new(0.0, -0.25, 0.0),
            Quat::from_rotation_y(std::f32::consts::PI),
            Vec3::splat(scale),
        ),
    )
}

/// Turn a freshly spawned def enemy into a hunter.
pub fn make_hunter(entity: &mut EntityCommands, loaded: &LoadedEnemyDef) {
    entity
        .remove::<(KinematicMovement, AvoidWallsData, MoveTowardsGoalData)>()
        .insert((
            creature_body(loaded.def.scale),
            Hunter {
                speed: loaded.props.speed,
                ..Hunter::default()
            },
            CreatureAnim(AnimationKey::Walk),
        ));
}

/// The built-in alien as a hunter: its own body, the hunting brain.
pub fn make_builtin_hunter(entity: &mut EntityCommands) {
    entity
        .remove::<(KinematicMovement, AvoidWallsData, MoveTowardsGoalData)>()
        .insert((
            Hunter {
                speed: 1.5,
                ..Hunter::default()
            },
            CreatureAnim(AnimationKey::Walk),
        ));
}

/// The padded-grid tile a world position is on (as `update_current_tile_system` works it
/// out, for one-metre tiles).
pub fn tile_of(at: Vec3) -> (usize, usize) {
    let snap = |v: f32| (v + 0.5).max(0.0) as usize;
    (snap(at.x), snap(at.z))
}

/// Of `candidates` (position, is a neighbour), the index of the one to go for.
pub fn pick_prey(from: Vec2, candidates: &[(Vec2, bool)]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .map(|(i, &(at, neighbour))| {
            let d = from.distance(at) * if neighbour { NEIGHBOUR_BIAS } else { 1.0 };
            (i, d)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Every so often each hunter picks its prey again and paths to it.
#[allow(clippy::type_complexity)]
pub fn choose_prey(
    time: Res<Time>,
    map_graph: Res<MapGraph>,
    mut hunters: Query<(Entity, &mut Hunter, &Position, &CurrentTile)>,
    neighbours: Query<(Entity, &Position, &Health), With<Neighbour>>,
    players: Query<(Entity, &Position), (With<Player>, Without<PlayerDead>)>,
) {
    let dt = time.delta_secs();
    let mut prey: Vec<(Entity, Vec3, bool)> = neighbours
        .iter()
        .filter(|(_, _, health)| health.health > 0)
        .map(|(e, p, _)| (e, p.0, true))
        .collect();
    prey.extend(players.iter().map(|(e, p)| (e, p.0, false)));
    let spots: Vec<(Vec2, bool)> = prey.iter().map(|&(_, at, n)| (at.xz(), n)).collect();

    for (entity, mut hunter, position, tile) in &mut hunters {
        hunter.rethink -= dt;
        let prey_gone = hunter
            .prey
            .is_some_and(|p| !prey.iter().any(|&(e, ..)| e == p));
        if hunter.rethink > 0.0 && !prey_gone {
            continue;
        }
        // Spread the rethinks out so a wave does not path on the same frame.
        let jitter = (entity.to_bits() % 7) as f32 / 7.0 - 0.5;
        hunter.rethink = RETHINK * (1.0 + jitter * 0.5);

        let Some(&(target, at, _)) = pick_prey(position.0.xz(), &spots).and_then(|i| prey.get(i))
        else {
            hunter.prey = None;
            hunter.path.clear();
            continue;
        };
        hunter.prey = Some(target);
        let goal = tile_of(at);
        let grid = &map_graph.path_finding_grid;
        hunter.path = astar(
            &tile.tile,
            |t| grid.neighbours(*t).into_iter().map(|n| (n, 1)),
            |t| grid.distance(*t, goal),
            |t| *t == goal,
        )
        .map(|(path, _)| path.into_iter().skip(1).collect())
        .unwrap_or_default();
    }
}

/// The yaw that faces a body's -Z along `dir` (on the floor plane).
pub fn facing(dir: Vec2) -> Option<Quat> {
    let dir = dir.try_normalize()?;
    Some(Quat::from_rotation_y((-dir.x).atan2(-dir.y)))
}

/// Walk each hunter along its path, then straight at its prey.
#[allow(clippy::type_complexity)]
pub fn hunt(
    time: Res<Time>,
    mut hunters: Query<
        (
            &mut Hunter,
            &Position,
            &mut LinearVelocity,
            &mut AngularVelocity,
            &mut Rotation,
            &mut CreatureAnim,
        ),
        With<Alien>,
    >,
    targets: Query<&Position, Without<Hunter>>,
) {
    let dt = time.delta_secs();
    for (mut hunter, position, mut velocity, mut spin, mut rotation, mut anim) in &mut hunters {
        spin.0 = Vec3::ZERO;
        let Some(prey_at) = hunter.prey.and_then(|p| targets.get(p).ok()).map(|p| p.0) else {
            velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
            anim.set_if_neq(CreatureAnim(AnimationKey::Idle));
            continue;
        };
        let here = position.0.xz();
        let distance = here.distance(prey_at.xz());
        // Drop the waypoints reached.
        while hunter
            .path
            .first()
            .is_some_and(|&(x, y)| here.distance(Vec2::new(x as f32, y as f32)) < 0.6)
        {
            hunter.path.remove(0);
        }
        let toward = match hunter.path.first() {
            Some(&(x, y)) if distance > STRAIGHT_IN => Vec2::new(x as f32, y as f32) - here,
            _ => prey_at.xz() - here,
        };
        let dir = toward.normalize_or_zero();
        let speed = if distance < STRIKE * 0.5 {
            // Leaning in: keeps the bodies touching without climbing over the prey.
            hunter.speed * 0.3
        } else {
            hunter.speed
        };
        velocity.0 = Vec3::new(dir.x * speed, velocity.0.y, dir.y * speed);
        if let Some(goal) = facing(dir) {
            rotation.0 = rotation.0.slerp(goal, (dt * TURN_RATE).min(1.0));
        }
        anim.set_if_neq(CreatureAnim(if distance < STRIKE {
            AnimationKey::Punch
        } else {
            AnimationKey::Walk
        }));
    }
}

/// Play each creature's wanted animation when it changes.
pub fn animate_creatures(
    store: Option<Res<AnimationStore>>,
    mut creatures: Query<(Entity, &CreatureAnim, &mut CurrentAnimationKey)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(store) = store else { return };
    for (entity, wanted, mut current) in &mut creatures {
        if current.key == wanted.0 {
            continue;
        }
        // Not loaded yet: try again next frame (the first key starts with the model).
        let Some(player_entity) = get_child_with_component_recursive(entity, &children, &players)
        else {
            continue;
        };
        let Ok(mut player) = players.get_mut(player_entity) else {
            continue;
        };
        store.stop(&current.group, current.key, &mut player);
        store.play(&current.group, wanted.0, &mut player, 0.0);
        current.key = wanted.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunters_go_for_the_neighbours_first() {
        let from = Vec2::ZERO;
        // A player at 4 m, a neighbour at 5: the neighbour counts as 3.5.
        let prey = [(Vec2::new(4.0, 0.0), false), (Vec2::new(0.0, 5.0), true)];
        assert_eq!(pick_prey(from, &prey), Some(1));
        // A player much closer still wins.
        let prey = [(Vec2::new(1.0, 0.0), false), (Vec2::new(0.0, 5.0), true)];
        assert_eq!(pick_prey(from, &prey), Some(0));
        assert_eq!(pick_prey(from, &[]), None);
    }

    #[test]
    fn facing_turns_minus_z_along_the_direction() {
        for dir in [Vec2::X, Vec2::Y, Vec2::new(-1.0, 1.0), Vec2::new(0.0, -1.0)] {
            let q = facing(dir).expect("a direction");
            let forward = (q * Vec3::NEG_Z).xz();
            assert!(
                forward.distance(dir.normalize()) < 1e-4,
                "{dir} -> {forward}"
            );
        }
        assert!(facing(Vec2::ZERO).is_none());
    }

    #[test]
    fn a_position_is_on_the_tile_round_it() {
        assert_eq!(tile_of(Vec3::new(3.4, 0.0, 7.6)), (3, 8));
        assert_eq!(tile_of(Vec3::new(-2.0, 0.0, 0.0)), (0, 0));
    }
}
