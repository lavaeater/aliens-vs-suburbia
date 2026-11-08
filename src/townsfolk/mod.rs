//! Townsfolk: the people who live on a level (`townsfolk: N` in its `.map`). Each has a
//! home (a front door, [`places`]) and a life run by a behaviour tree (`bevy_behave`):
//! go somewhere near (a neighbour's door, a spot down the street), stay a while, walk
//! home, go in for a bit, and again. When monsters come near they run (the neighbours'
//! own flight, `infestation::neighbours::flee`): every task gives up when its person is
//! scared, and the tree falls back to fleeing.
//!
//! ```text
//! Forever
//! `- Fallback
//!    |- Sequence: IsScared?, Flee (until calm)
//!    `- Sequence: PickErrand, WalkTo(errand), Linger, WalkTo(home), StayIn
//! ```
//!
//! A townsperson is an `infestation::Neighbour` (body, health, animation, flight) with a
//! [`Townsfolk`] home; the tree is a child entity, so it goes with them. Indoors they are
//! hidden ([`Indoors`]): monsters don't hunt them and they don't take fright.

pub mod places;

use bevy::prelude::*;
use bevy_behave::prelude::*;
use pathfinding::directed::astar::astar;

use crate::game_state::GameState;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use crate::general::events::map_events::LoadMap;
use crate::general::resources::map_resources::MapGraph;
use crate::infestation::hunters::{CreatureAnim, facing};
use crate::infestation::{Neighbour, SpawnNeighbour};
use crate::animation::animation_plugin::AnimationKey;
use crate::physics::prelude::{LinearVelocity, Position, Rotation};
use places::{Places, find_places, mix, pick_errand, pick_homes};

/// Walking speed, metres per second.
const WALK_SPEED: f32 = 1.1;
/// How far (tiles) an errand takes someone from where they are.
const ERRAND_REACH: usize = 30;
/// A walk that gets nowhere for this long (seconds) gives up.
const STUCK_AFTER: f32 = 3.0;
/// Seconds a walk may take at most.
const WALK_TIMEOUT: f32 = 120.0;
/// Seconds lingering at an errand, and indoors at home: a base and a random extra.
const LINGER: (f32, f32) = (4.0, 8.0);
const STAY_IN: (f32, f32) = (8.0, 25.0);

pub struct TownsfolkPlugin;

impl Plugin for TownsfolkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BehavePlugin::default())
            .init_resource::<TownPlaces>()
            .add_observer(give_a_life)
            .add_observer(is_scared)
            .add_observer(pick_an_errand)
            .add_observer(stop_walking)
            .add_observer(come_out)
            .add_systems(
                Update,
                (
                    plan_town.after(crate::general::systems::map_systems::map_loader),
                    flee_until_calm,
                    walk,
                    linger,
                    stay_in,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// The current level's places.
#[derive(Resource, Debug, Default)]
pub struct TownPlaces(pub Places);

/// Someone who lives here: their front door (padded-grid tile), where they are off to,
/// and their own dice.
#[derive(Component, Debug, Clone)]
pub struct Townsfolk {
    pub home: (usize, usize),
    pub errand: Option<(usize, usize)>,
    roll: u64,
}

impl Townsfolk {
    pub const fn new(home: (usize, usize), seed: u64) -> Self {
        Self {
            home,
            errand: None,
            roll: seed,
        }
    }

    /// The next roll of their dice.
    fn roll(&mut self) -> u64 {
        self.roll = mix(self.roll, 0x5EED);
        self.roll
    }

    /// A duration between `base` and `base + extra`.
    fn seconds(&mut self, (base, extra): (f32, f32)) -> f32 {
        base + extra * ((self.roll() % 1000) as f32 / 1000.0)
    }
}

/// At home, out of sight.
#[derive(Component, Debug)]
pub struct Indoors;

// --- The tree ---------------------------------------------------------------------------

/// Trigger: succeeds when the person is scared (and not indoors).
#[derive(Clone)]
pub struct IsScared;

/// Trigger: pick where to go next; fails when there is nowhere.
#[derive(Clone)]
pub struct PickErrand;

/// Task: runs (the flight is `neighbours::flee`'s) until the person has calmed down.
#[derive(Component, Clone)]
pub struct Flee;

/// Where a walk goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    Errand,
    Home,
}

/// Task: walk to the errand or home along the pathfinding grid. Fails when scared, stuck
/// or there is no way.
#[derive(Component, Clone)]
pub struct WalkTo {
    pub to: Dest,
    path: Option<Vec<(usize, usize)>>,
    stuck_for: f32,
    last: Vec2,
}

impl WalkTo {
    pub const fn new(to: Dest) -> Self {
        Self {
            to,
            path: None,
            stuck_for: 0.0,
            last: Vec2::ZERO,
        }
    }
}

/// Task: stand about for a while. Fails when scared.
#[derive(Component, Clone, Default)]
pub struct Linger {
    left: Option<f32>,
}

/// Task: go in and stay in for a while.
#[derive(Component, Clone, Default)]
pub struct StayIn {
    left: Option<f32>,
}

/// A townsperson's life.
pub fn life() -> Tree<Behave> {
    behave! {
        Behave::Forever => {
            Behave::Fallback => {
                Behave::Sequence => {
                    Behave::trigger(IsScared),
                    Behave::spawn_named("Flee", Flee),
                },
                Behave::Sequence => {
                    Behave::trigger(PickErrand),
                    Behave::spawn_named(
                        "Walk to the errand",
                        (WalkTo::new(Dest::Errand), BehaveTimeout::from_secs(WALK_TIMEOUT, false))
                    ),
                    Behave::spawn_named("Linger", Linger::default()),
                    Behave::spawn_named(
                        "Walk home",
                        (WalkTo::new(Dest::Home), BehaveTimeout::from_secs(WALK_TIMEOUT, false))
                    ),
                    Behave::spawn_named("Stay in", StayIn::default()),
                },
            }
        }
    }
}

fn give_a_life(add: On<Add<Townsfolk>>, mut commands: Commands) {
    commands.entity(add.event().entity).with_child((
        Name::new("Townsfolk life"),
        BehaveTree::new(life()),
    ));
}

/// Whether a townsperson is scared: their `Neighbour` says so and they are out.
fn scared(neighbour: &Neighbour, indoors: bool) -> bool {
    neighbour.scared_for > 0.0 && !indoors
}

fn is_scared(
    trigger: On<BehaveTrigger<IsScared>>,
    folk: Query<(&Neighbour, Has<Indoors>)>,
    mut commands: Commands,
) {
    let ctx = trigger.event().ctx();
    let yes = folk
        .get(ctx.target_entity())
        .is_ok_and(|(n, indoors)| scared(n, indoors));
    commands.trigger(if yes { ctx.success() } else { ctx.failure() });
}

fn pick_an_errand(
    trigger: On<BehaveTrigger<PickErrand>>,
    places: Res<TownPlaces>,
    mut folk: Query<(&mut Townsfolk, &CurrentTile)>,
    mut commands: Commands,
) {
    let ctx = trigger.event().ctx();
    let picked = folk.get_mut(ctx.target_entity()).ok().and_then(|(mut t, tile)| {
        let roll = t.roll();
        t.errand = pick_errand(&places.0, tile.tile, ERRAND_REACH, roll);
        t.errand
    });
    commands.trigger(if picked.is_some() {
        ctx.success()
    } else {
        ctx.failure()
    });
}

/// The level's places, and its townsfolk, when a map loads.
fn plan_town(
    mut loads: MessageReader<LoadMap>,
    mut places: ResMut<TownPlaces>,
    mut spawn: MessageWriter<SpawnNeighbour>,
) {
    for load in loads.read() {
        places.0 = find_places(&load.map);
        let count = load.map.townsfolk.unwrap_or(0) as usize;
        if count == 0 {
            continue;
        }
        let people = crate::map::neighbourhood::people_defs();
        if people.is_empty() {
            warn!("townsfolk: no people defs in {}", crate::map::neighbourhood::PEOPLE_DIR);
            continue;
        }
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        let homes = pick_homes(&places.0, count, seed);
        info!(
            "townsfolk: {} at home, {} doors and {} street spots",
            homes.len(),
            places.0.doors.len(),
            places.0.streets.len()
        );
        for (i, home) in homes.into_iter().enumerate() {
            let roll = mix(seed, i as u64);
            let Some(def) = people.get((roll as usize) % people.len()) else {
                continue;
            };
            spawn.write(SpawnNeighbour {
                def: def.clone(),
                position: Vec3::new(home.0 as f32, 1.0, home.1 as f32),
                yaw: (roll % 628) as f32 / 100.0,
                home: Some((home, roll)),
            });
        }
    }
}

// --- The tasks --------------------------------------------------------------------------

fn flee_until_calm(
    tasks: Query<&BehaveCtx, With<Flee>>,
    folk: Query<&Neighbour>,
    mut commands: Commands,
) {
    for ctx in &tasks {
        if folk.get(ctx.target_entity()).is_ok_and(|n| n.scared_for <= 0.0) {
            commands.trigger(ctx.success());
        }
    }
}

/// Walk the townsfolk with a `WalkTo` task along their paths.
#[allow(clippy::type_complexity)]
fn walk(
    time: Res<Time>,
    map_graph: Res<MapGraph>,
    mut tasks: Query<(&mut WalkTo, &BehaveCtx)>,
    mut folk: Query<(
        &Townsfolk,
        &Neighbour,
        &Health,
        &Position,
        &CurrentTile,
        &mut LinearVelocity,
        &mut Rotation,
        &mut CreatureAnim,
    )>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (mut task, ctx) in &mut tasks {
        let Ok((townsfolk, neighbour, health, position, tile, mut velocity, mut rotation, mut anim)) =
            folk.get_mut(ctx.target_entity())
        else {
            continue;
        };
        if neighbour.scared_for > 0.0 || health.health <= 0 {
            commands.trigger(ctx.failure());
            continue;
        }
        let dest = match task.to {
            Dest::Home => Some(townsfolk.home),
            Dest::Errand => townsfolk.errand,
        };
        let Some(dest) = dest else {
            commands.trigger(ctx.failure());
            continue;
        };
        let here = position.0.xz();
        if task.path.is_none() {
            let grid = &map_graph.path_finding_grid;
            task.path = astar(
                &tile.tile,
                |t| grid.neighbours(*t).into_iter().map(|n| (n, 1)),
                |t| grid.distance(*t, dest),
                |t| *t == dest,
            )
            .map(|(path, _)| path.into_iter().skip(1).collect());
            task.last = here;
            if task.path.is_none() {
                commands.trigger(ctx.failure());
                continue;
            }
        }
        let target = Vec2::new(dest.0 as f32, dest.1 as f32);
        if here.distance(target) < 0.6 {
            velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
            anim.set_if_neq(CreatureAnim(AnimationKey::Idle));
            commands.trigger(ctx.success());
            continue;
        }
        let path = task.path.get_or_insert_with(Vec::new);
        while path
            .first()
            .is_some_and(|&(x, y)| here.distance(Vec2::new(x as f32, y as f32)) < 0.6)
        {
            path.remove(0);
        }
        let toward = path
            .first()
            .map_or(target, |&(x, y)| Vec2::new(x as f32, y as f32))
            - here;
        let dir = toward.normalize_or_zero();
        velocity.0 = Vec3::new(dir.x * WALK_SPEED, velocity.0.y, dir.y * WALK_SPEED);
        if let Some(goal) = facing(dir) {
            rotation.0 = rotation.0.slerp(goal, (dt * 6.0).min(1.0));
        }
        anim.set_if_neq(CreatureAnim(AnimationKey::Walk));

        // Getting nowhere: give up and think again.
        if here.distance(task.last) > 0.4 {
            task.last = here;
            task.stuck_for = 0.0;
        } else {
            task.stuck_for += dt;
            if task.stuck_for > STUCK_AFTER {
                commands.trigger(ctx.failure());
            }
        }
    }
}

/// A walk that ends (done, given up, or the tree moved on) leaves its walker standing.
fn stop_walking(
    remove: On<Remove<WalkTo>>,
    tasks: Query<&BehaveCtx, With<WalkTo>>,
    mut folk: Query<(&Neighbour, &mut LinearVelocity)>,
) {
    let Ok(ctx) = tasks.get(remove.event().entity) else {
        return;
    };
    if let Ok((neighbour, mut velocity)) = folk.get_mut(ctx.target_entity())
        && neighbour.scared_for <= 0.0
    {
        velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
    }
}

fn linger(
    time: Res<Time>,
    mut tasks: Query<(&mut Linger, &BehaveCtx)>,
    mut folk: Query<(&mut Townsfolk, &Neighbour, &mut LinearVelocity, &mut CreatureAnim)>,
    mut commands: Commands,
) {
    for (mut task, ctx) in &mut tasks {
        let Ok((mut townsfolk, neighbour, mut velocity, mut anim)) =
            folk.get_mut(ctx.target_entity())
        else {
            continue;
        };
        if neighbour.scared_for > 0.0 {
            commands.trigger(ctx.failure());
            continue;
        }
        let left = task.left.get_or_insert_with(|| townsfolk.seconds(LINGER));
        *left -= time.delta_secs();
        velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
        anim.set_if_neq(CreatureAnim(AnimationKey::Idle));
        if *left <= 0.0 {
            commands.trigger(ctx.success());
        }
    }
}

fn stay_in(
    time: Res<Time>,
    mut tasks: Query<(&mut StayIn, &BehaveCtx)>,
    mut folk: Query<(&mut Townsfolk, &mut LinearVelocity, &mut Visibility, Has<Indoors>)>,
    mut commands: Commands,
) {
    for (mut task, ctx) in &mut tasks {
        let target = ctx.target_entity();
        let Ok((mut townsfolk, mut velocity, mut visibility, indoors)) = folk.get_mut(target)
        else {
            continue;
        };
        if !indoors {
            commands.entity(target).insert(Indoors);
            *visibility = Visibility::Hidden;
        }
        velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
        let left = task.left.get_or_insert_with(|| townsfolk.seconds(STAY_IN));
        *left -= time.delta_secs();
        if *left <= 0.0 {
            commands.trigger(ctx.success());
        }
    }
}

/// Leaving `StayIn` (done or cut short) brings them back out.
fn come_out(
    remove: On<Remove<StayIn>>,
    tasks: Query<&BehaveCtx, With<StayIn>>,
    mut folk: Query<&mut Visibility, With<Indoors>>,
    mut commands: Commands,
) {
    let Ok(ctx) = tasks.get(remove.event().entity) else {
        return;
    };
    let target = ctx.target_entity();
    if let Ok(mut visibility) = folk.get_mut(target) {
        *visibility = Visibility::Inherited;
        commands.entity(target).remove::<Indoors>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_is_well_formed() {
        assert!(BehaveTree::verify(&life()));
    }

    #[test]
    fn a_townsperson_rolls_their_own_dice() {
        let mut a = Townsfolk::new((1, 1), 7);
        let mut b = Townsfolk::new((1, 1), 8);
        assert_ne!(a.roll(), b.roll());
        let s = a.seconds(LINGER);
        assert!((LINGER.0..=LINGER.0 + LINGER.1).contains(&s));
    }

    #[test]
    fn only_the_out_and_about_take_fright() {
        let scared_one = Neighbour { scared_for: 1.0 };
        assert!(scared(&scared_one, false));
        assert!(!scared(&scared_one, true));
        assert!(!scared(&Neighbour::default(), false));
    }
}

