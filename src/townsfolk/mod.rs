//! Townsfolk: the people who live on a level (`townsfolk: N` in its `.map`). Each has a
//! home (a front door, [`places`]) and a life run by a behaviour tree (`bevy_behave`):
//! go somewhere near (a neighbour's door, a spot down the street), stay a while, walk
//! home, go in for a bit, and again. Each has a [`Temper`] (plain, brave, timid, curious)
//! and the time of day ([`LightingPreset`]) decides how often they go out at all
//! ([`temper`]). When monsters come near every task gives up and the tree turns to
//! safety: run for the nearest door the monster is not nearer to and hide there until the
//! street has been clear for a while, or, with no door to hand (or a brave one with a
//! player near), just run (the neighbours' own flight, `infestation::neighbours::flee`).
//!
//! ```text
//! Forever
//! `- Fallback
//!    |- Sequence: IsScared?
//!    |  `- Fallback
//!    |     |- Sequence: PickShelter, WalkTo(shelter, running), Hide (until clear)
//!    |     `- Flee (until calm)
//!    |- Sequence: PickErrand (or stay home), WalkTo(errand), Linger, WalkTo(home), StayIn
//!    `- Sequence: WalkTo(home), StayIn
//! ```
//!
//! A townsperson is an `infestation::Neighbour` (body, health, animation, flight) with a
//! [`Townsfolk`] home; the tree is a child entity, so it goes with them. Indoors (at home
//! or hiding) they are hidden ([`Indoors`]): monsters don't hunt them and they don't take
//! fright, and they don't come out while a monster is near the door.
//!
//! [`TownCensus`] counts them, and the facts in [`keys`] tell the stories (and the card
//! between levels) how many made it.

pub mod places;
pub mod temper;

use bevy::prelude::*;
use bevy_behave::prelude::*;
use pathfinding::directed::astar::astar;

use crate::game_state::GameState;
use crate::alien::components::general::Alien;
use crate::facts::facts_resource::Facts;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use crate::general::events::map_events::LoadMap;
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::lights_systems::LightingPreset;
use crate::infestation::hunters::{CreatureAnim, facing};
use crate::infestation::{Neighbour, SpawnNeighbour};
use crate::animation::animation_plugin::AnimationKey;
use crate::physics::prelude::{LinearVelocity, Position, Rotation};
use crate::player::components::{Player, PlayerDead};
use places::{Places, find_places, mix, pick_errand, pick_homes, pick_shelter};
pub use temper::Temper;
use temper::{errand_chance, stay_in_by_light};

/// A walk that gets nowhere for this long (seconds) gives up.
const STUCK_AFTER: f32 = 3.0;
/// Seconds a walk may take at most.
const WALK_TIMEOUT: f32 = 120.0;
/// Seconds a run for a door may take at most.
const SHELTER_TIMEOUT: f32 = 20.0;
/// No monster this close (metres) counts as the street being clear.
const ALL_CLEAR: f32 = 10.0;
/// Seconds of clear street before someone hiding comes out (times their temper's).
const HIDE_UNTIL_CLEAR: f32 = 6.0;
/// A player this close (metres) is someone the brave run to rather than hide.
const PLAYER_NEAR: f32 = 15.0;
/// Seconds lingering at an errand, and indoors at home: a base and a random extra.
const LINGER: (f32, f32) = (4.0, 8.0);
const STAY_IN: (f32, f32) = (8.0, 25.0);

pub struct TownsfolkPlugin;

impl Plugin for TownsfolkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BehavePlugin::default())
            .init_resource::<TownPlaces>()
            .init_resource::<TownCensus>()
            .add_observer(give_a_life)
            .add_observer(is_scared)
            .add_observer(pick_an_errand)
            .add_observer(pick_a_shelter)
            .add_observer(stop_walking)
            .add_observer(come_out)
            .add_observer(come_out_of_hiding)
            .add_systems(
                Update,
                (
                    plan_town.after(crate::general::systems::map_systems::map_loader),
                    flee_until_calm,
                    walk,
                    linger,
                    stay_in,
                    hide,
                    count_townsfolk,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// The facts the townsfolk keep for the stories.
pub mod keys {
    /// Townsfolk the level started with (the most seen alive at once).
    pub const TOWNSFOLK_TOTAL: &str = "TownsfolkTotal";
    /// Townsfolk alive.
    pub const TOWNSFOLK_ALIVE: &str = "TownsfolkAlive";
    /// Townsfolk killed.
    pub const TOWNSFOLK_LOST: &str = "TownsfolkLost";
    /// Townsfolk indoors right now (at home or hiding).
    pub const TOWNSFOLK_INDOORS: &str = "TownsfolkIndoors";
    /// Every one of them, for `crate::validate`.
    pub const ALL: &[&str] = &[
        TOWNSFOLK_TOTAL,
        TOWNSFOLK_ALIVE,
        TOWNSFOLK_LOST,
        TOWNSFOLK_INDOORS,
    ];
}

/// The current level's places.
#[derive(Resource, Debug, Default)]
pub struct TownPlaces(pub Places);

/// How the current level's townsfolk are doing; reset when a map loads.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TownCensus {
    /// The most seen alive at once: the level's count, once they have spawned.
    pub total: u32,
    pub alive: u32,
    pub indoors: u32,
}

impl TownCensus {
    pub const fn lost(&self) -> u32 {
        self.total.saturating_sub(self.alive)
    }
}

/// Someone who lives here: their front door (padded-grid tile), where they are off to,
/// the door they are running for, their temper and their own dice.
#[derive(Component, Debug, Clone)]
pub struct Townsfolk {
    pub home: (usize, usize),
    pub errand: Option<(usize, usize)>,
    /// Set while they run for a door; the flight (`neighbours::flee`) leaves them alone.
    pub shelter: Option<(usize, usize)>,
    pub temper: Temper,
    roll: u64,
}

impl Townsfolk {
    pub const fn new(home: (usize, usize), seed: u64) -> Self {
        Self {
            home,
            errand: None,
            shelter: None,
            temper: Temper::roll(mix(seed, 0x7E3)),
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

/// Trigger: pick where to go next; fails when there is nowhere, or they would rather
/// stay home (the time of day, their temper).
#[derive(Clone)]
pub struct PickErrand;

/// Trigger: pick a door to run for; fails when there is none to hand, or a brave one has
/// a player to run to.
#[derive(Clone)]
pub struct PickShelter;

/// Task: runs (the flight is `neighbours::flee`'s) until the person has calmed down.
#[derive(Component, Clone)]
pub struct Flee;

/// Where a walk goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    Errand,
    Home,
    /// The door they are running for (at a run, scared or not).
    Shelter,
}

/// Task: walk to the errand, home or a shelter along the pathfinding grid. Fails when
/// scared (except on the way to a shelter), stuck or there is no way.
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

/// Task: go in and stay in for a while (counting only while no monster is near).
#[derive(Component, Clone, Default)]
pub struct StayIn {
    left: Option<f32>,
}

/// Task: go in where they are and stay hidden until the street has been clear for a while.
#[derive(Component, Clone, Default)]
pub struct Hide {
    clear_for: f32,
}

/// A townsperson's life.
pub fn life() -> Tree<Behave> {
    behave! {
        Behave::Forever => {
            Behave::Fallback => {
                Behave::Sequence => {
                    Behave::trigger(IsScared),
                    Behave::Fallback => {
                        Behave::Sequence => {
                            Behave::trigger(PickShelter),
                            Behave::spawn_named(
                                "Run for a door",
                                (WalkTo::new(Dest::Shelter), BehaveTimeout::from_secs(SHELTER_TIMEOUT, false))
                            ),
                            Behave::spawn_named("Hide", Hide::default()),
                        },
                        Behave::spawn_named("Flee", Flee),
                    },
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
                Behave::Sequence => {
                    Behave::spawn_named(
                        "Stay home",
                        (WalkTo::new(Dest::Home), BehaveTimeout::from_secs(WALK_TIMEOUT, false))
                    ),
                    Behave::spawn_named("Stay in", StayIn::default()),
                },
            }
        }
    }
}

fn give_a_life(
    add: On<Add<Townsfolk>>,
    mut folk: Query<(&Townsfolk, &mut Name)>,
    mut commands: Commands,
) {
    let entity = add.event().entity;
    if let Ok((townsfolk, mut name)) = folk.get_mut(entity) {
        let named = format!("{} ({})", name.as_str(), townsfolk.temper.label());
        name.set(named);
    }
    commands.entity(entity).with_child((
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
    light: Option<Res<LightingPreset>>,
    mut folk: Query<(&mut Townsfolk, &CurrentTile)>,
    mut commands: Commands,
) {
    let ctx = trigger.event().ctx();
    let light = light.map_or_else(LightingPreset::default, |l| *l);
    let picked = folk.get_mut(ctx.target_entity()).ok().and_then(|(mut t, tile)| {
        if t.roll() % 100 >= errand_chance(light, t.temper) {
            t.errand = None;
            return None;
        }
        let roll = t.roll();
        t.errand = pick_errand(&places.0, tile.tile, t.temper.errand_reach(), roll);
        t.errand
    });
    commands.trigger(if picked.is_some() {
        ctx.success()
    } else {
        ctx.failure()
    });
}

/// The nearest of `points` to `here`, and how far it is.
fn nearest(here: Vec2, points: impl Iterator<Item = Vec2>) -> Option<(Vec2, f32)> {
    points
        .map(|p| (p, p.distance(here)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn pick_a_shelter(
    trigger: On<BehaveTrigger<PickShelter>>,
    places: Res<TownPlaces>,
    mut folk: Query<(&mut Townsfolk, &Position)>,
    monsters: Query<&Position, With<Alien>>,
    players: Query<&Position, (With<Player>, Without<PlayerDead>)>,
    mut commands: Commands,
) {
    let ctx = trigger.event().ctx();
    let picked = folk.get_mut(ctx.target_entity()).ok().and_then(|(mut t, position)| {
        let here = position.0.xz();
        let (monster, _) = nearest(here, monsters.iter().map(|p| p.0.xz()))?;
        let player_near = nearest(here, players.iter().map(|p| p.0.xz()))
            .is_some_and(|(_, d)| d < PLAYER_NEAR);
        if !t.temper.seeks_shelter(player_near) {
            return None;
        }
        t.shelter = pick_shelter(&places.0.doors, here, monster, t.temper.shelter_reach());
        debug!("townsfolk: a {} one at {here} runs for {:?}", t.temper.label(), t.shelter);
        t.shelter
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
    mut census: ResMut<TownCensus>,
    mut spawn: MessageWriter<SpawnNeighbour>,
) {
    for load in loads.read() {
        places.0 = find_places(&load.map);
        *census = TownCensus::default();
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
        let running = task.to == Dest::Shelter;
        if (neighbour.scared_for > 0.0 && !running) || health.health <= 0 {
            commands.trigger(ctx.failure());
            continue;
        }
        let dest = match task.to {
            Dest::Home => Some(townsfolk.home),
            Dest::Errand => townsfolk.errand,
            Dest::Shelter => townsfolk.shelter,
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
        let speed = if running {
            townsfolk.temper.run_speed()
        } else {
            townsfolk.temper.walk_speed()
        };
        velocity.0 = Vec3::new(dir.x * speed, velocity.0.y, dir.y * speed);
        if let Some(goal) = facing(dir) {
            rotation.0 = rotation.0.slerp(goal, (dt * if running { 8.0 } else { 6.0 }).min(1.0));
        }
        anim.set_if_neq(CreatureAnim(if running {
            AnimationKey::Run
        } else {
            AnimationKey::Walk
        }));

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

/// A walk that ends (done, given up, or the tree moved on) leaves its walker standing,
/// and a run for a door hands them back to the flight (or to `Hide`).
fn stop_walking(
    remove: On<Remove<WalkTo>>,
    tasks: Query<(&BehaveCtx, &WalkTo)>,
    mut folk: Query<(&mut Townsfolk, &Neighbour, &mut LinearVelocity)>,
) {
    let Ok((ctx, task)) = tasks.get(remove.event().entity) else {
        return;
    };
    let Ok((mut townsfolk, neighbour, mut velocity)) = folk.get_mut(ctx.target_entity()) else {
        return;
    };
    if task.to == Dest::Shelter {
        townsfolk.shelter = None;
    }
    if neighbour.scared_for <= 0.0 || task.to == Dest::Shelter {
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
        let factor = townsfolk.temper.linger_factor();
        let left = task
            .left
            .get_or_insert_with(|| townsfolk.seconds((LINGER.0 * factor, LINGER.1 * factor)));
        *left -= time.delta_secs();
        velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
        anim.set_if_neq(CreatureAnim(AnimationKey::Idle));
        if *left <= 0.0 {
            commands.trigger(ctx.success());
        }
    }
}

/// Whether any monster is within [`ALL_CLEAR`] of `here`.
fn monster_near(here: Vec2, monsters: &Query<&Position, With<Alien>>) -> bool {
    monsters
        .iter()
        .any(|p| p.0.xz().distance(here) < ALL_CLEAR)
}

/// In and out of sight.
fn go_in(
    commands: &mut Commands,
    target: Entity,
    visibility: &mut Visibility,
    velocity: &mut LinearVelocity,
    indoors: bool,
) {
    if !indoors {
        commands.entity(target).insert(Indoors);
        *visibility = Visibility::Hidden;
    }
    velocity.0 = Vec3::new(0.0, velocity.0.y, 0.0);
}

#[allow(clippy::type_complexity)]
fn stay_in(
    time: Res<Time>,
    light: Option<Res<LightingPreset>>,
    mut tasks: Query<(&mut StayIn, &BehaveCtx)>,
    mut folk: Query<(
        &mut Townsfolk,
        &Position,
        &mut LinearVelocity,
        &mut Visibility,
        Has<Indoors>,
    )>,
    monsters: Query<&Position, With<Alien>>,
    mut commands: Commands,
) {
    let light = light.map_or_else(LightingPreset::default, |l| *l);
    for (mut task, ctx) in &mut tasks {
        let target = ctx.target_entity();
        let Ok((mut townsfolk, position, mut velocity, mut visibility, indoors)) =
            folk.get_mut(target)
        else {
            continue;
        };
        go_in(&mut commands, target, &mut visibility, &mut velocity, indoors);
        let factor = townsfolk.temper.stay_in_factor() * stay_in_by_light(light);
        let left = task
            .left
            .get_or_insert_with(|| townsfolk.seconds((STAY_IN.0 * factor, STAY_IN.1 * factor)));
        // Nobody opens the door with a monster outside.
        if !monster_near(position.0.xz(), &monsters) {
            *left -= time.delta_secs();
        }
        if *left <= 0.0 {
            commands.trigger(ctx.success());
        }
    }
}

#[allow(clippy::type_complexity)]
fn hide(
    time: Res<Time>,
    mut tasks: Query<(&mut Hide, &BehaveCtx)>,
    mut folk: Query<(
        &Townsfolk,
        &Position,
        &mut LinearVelocity,
        &mut Visibility,
        Has<Indoors>,
    )>,
    monsters: Query<&Position, With<Alien>>,
    mut commands: Commands,
) {
    for (mut task, ctx) in &mut tasks {
        let target = ctx.target_entity();
        let Ok((townsfolk, position, mut velocity, mut visibility, indoors)) = folk.get_mut(target)
        else {
            continue;
        };
        go_in(&mut commands, target, &mut visibility, &mut velocity, indoors);
        if monster_near(position.0.xz(), &monsters) {
            task.clear_for = 0.0;
        } else {
            task.clear_for += time.delta_secs();
        }
        if task.clear_for >= HIDE_UNTIL_CLEAR * townsfolk.temper.stay_in_factor() {
            debug!("townsfolk: all clear at {}, out they come", position.0.xz());
            commands.trigger(ctx.success());
        }
    }
}

/// Back out where they went in.
fn come_out_of(
    ctx: &BehaveCtx,
    folk: &mut Query<&mut Visibility, With<Indoors>>,
    commands: &mut Commands,
) {
    let target = ctx.target_entity();
    if let Ok(mut visibility) = folk.get_mut(target) {
        *visibility = Visibility::Inherited;
        commands.entity(target).remove::<Indoors>();
    }
}

/// Leaving `StayIn` (done or cut short) brings them back out.
fn come_out(
    remove: On<Remove<StayIn>>,
    tasks: Query<&BehaveCtx, With<StayIn>>,
    mut folk: Query<&mut Visibility, With<Indoors>>,
    mut commands: Commands,
) {
    if let Ok(ctx) = tasks.get(remove.event().entity) {
        come_out_of(ctx, &mut folk, &mut commands);
    }
}

/// And so does leaving `Hide`.
fn come_out_of_hiding(
    remove: On<Remove<Hide>>,
    tasks: Query<&BehaveCtx, With<Hide>>,
    mut folk: Query<&mut Visibility, With<Indoors>>,
    mut commands: Commands,
) {
    if let Ok(ctx) = tasks.get(remove.event().entity) {
        come_out_of(ctx, &mut folk, &mut commands);
    }
}

/// Count the townsfolk and keep the facts in [`keys`].
fn count_townsfolk(
    mut census: ResMut<TownCensus>,
    mut facts: ResMut<Facts>,
    folk: Query<(&Health, Has<Indoors>), With<Townsfolk>>,
) {
    let (alive, indoors) = folk
        .iter()
        .filter(|(h, _)| h.health > 0)
        .fold((0u32, 0u32), |(a, i), (_, inside)| {
            (a.saturating_add(1), i.saturating_add(u32::from(inside)))
        });
    let now = TownCensus {
        total: census.total.max(alive),
        alive,
        indoors,
    };
    census.set_if_neq(now);
    let ints = [
        (keys::TOWNSFOLK_TOTAL, now.total),
        (keys::TOWNSFOLK_ALIVE, now.alive),
        (keys::TOWNSFOLK_LOST, now.lost()),
        (keys::TOWNSFOLK_INDOORS, now.indoors),
    ];
    for (key, value) in ints {
        if facts.try_int(key) != Some(i64::from(value)) {
            facts.set_int(key, i64::from(value));
        }
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
    fn tempers_come_from_the_seed() {
        let tempers: Vec<Temper> = (0..200).map(|i| Townsfolk::new((1, 1), i).temper).collect();
        assert!(tempers.contains(&Temper::Brave) && tempers.contains(&Temper::Timid));
        assert_eq!(Townsfolk::new((1, 1), 9).temper, Townsfolk::new((2, 2), 9).temper);
    }

    #[test]
    fn the_lost_are_the_total_less_the_living() {
        let census = TownCensus {
            total: 30,
            alive: 24,
            indoors: 3,
        };
        assert_eq!(census.lost(), 6);
        assert_eq!(TownCensus::default().lost(), 0);
    }

    #[test]
    fn only_the_out_and_about_take_fright() {
        let scared_one = Neighbour { scared_for: 1.0 };
        assert!(scared(&scared_one, false));
        assert!(!scared(&scared_one, true));
        assert!(!scared(&Neighbour::default(), false));
    }
}


/// The tree on a small headless app: no physics (position follows velocity) and no flight
/// (`Neighbour::scared_for` is set by the test).
#[cfg(test)]
mod tree_tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;
    use pathfinding::grid::Grid;

    use super::*;
    use crate::ai::flow_field::FlowField;

    fn integrate(
        time: Res<Time>,
        mut bodies: Query<(&mut Position, &LinearVelocity, &mut CurrentTile)>,
    ) {
        for (mut position, velocity, mut tile) in &mut bodies {
            position.0 += velocity.0 * time.delta_secs();
            tile.tile = (
                position.0.x.round().max(0.0) as usize,
                position.0.z.round().max(0.0) as usize,
            );
        }
    }

    fn app() -> App {
        let step = Duration::from_secs_f32(0.05);
        let mut grid = Grid::new(20, 20);
        grid.fill();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, BehavePlugin::default()))
            .insert_resource(Time::<Fixed>::from_duration(step))
            .insert_resource(TimeUpdateStrategy::ManualDuration(step))
            .insert_resource(TownPlaces(Places {
                doors: vec![(10, 5), (2, 12)],
                streets: Vec::new(),
            }))
            .insert_resource(MapGraph {
                path_finding_grid: grid,
                occupied_tiles: HashSet::new(),
                goal: (0, 0),
                path_reopened: false,
                flow: FlowField::default(),
            })
            .insert_resource(LightingPreset::Day)
            .init_resource::<TownCensus>()
            .init_resource::<Facts>()
            .add_observer(give_a_life)
            .add_observer(is_scared)
            .add_observer(pick_an_errand)
            .add_observer(pick_a_shelter)
            .add_observer(stop_walking)
            .add_observer(come_out)
            .add_observer(come_out_of_hiding)
            .add_systems(
                Update,
                (flee_until_calm, walk, linger, stay_in, hide, count_townsfolk, integrate).chain(),
            );
        app
    }

    fn hiding(app: &mut App) -> usize {
        app.world_mut().query::<&Hide>().iter(app.world()).count()
    }

    #[test]
    fn scared_townsfolk_run_for_a_door_hide_and_come_out_when_it_is_clear() {
        let mut app = app();
        // Their own home is far off (2, 12); the monster is west of them, the door east.
        let person = app
            .world_mut()
            .spawn((
                Name::new("Neighbour test"),
                Townsfolk::new((2, 12), 1),
                Neighbour { scared_for: 2.5 },
                Health::full(60),
                Position(Vec3::new(5.0, 0.0, 5.0)),
                CurrentTile { tile: (5, 5) },
                LinearVelocity::default(),
                Rotation::default(),
                CreatureAnim(AnimationKey::Idle),
                Visibility::default(),
            ))
            .id();
        let monster = app
            .world_mut()
            .spawn((Alien, Position(Vec3::new(3.0, 0.0, 5.0))))
            .id();

        let mut inside = false;
        for _ in 0..200 {
            app.world_mut().get_mut::<Neighbour>(person).expect("person").scared_for = 2.5;
            app.update();
            if app.world().get::<Indoors>(person).is_some() {
                inside = true;
                break;
            }
        }
        assert!(inside, "never got indoors");
        let at = app.world().get::<Position>(person).expect("person").0.xz();
        assert!(at.distance(Vec2::new(10.0, 5.0)) < 1.0, "hid at {at}, not the east door");
        assert_eq!(app.world().get::<Visibility>(person), Some(&Visibility::Hidden));
        assert!(app.world().get::<Townsfolk>(person).expect("person").shelter.is_none());
        assert_eq!(app.world().resource::<TownCensus>().indoors, 1);

        // The monster lingers within ALL_CLEAR of the door: they stay in.
        for _ in 0..400 {
            app.update();
        }
        assert_eq!(hiding(&mut app), 1, "came out with a monster at the door");

        // It goes; a little while later they come out.
        app.world_mut().get_mut::<Position>(monster).expect("monster").0 =
            Vec3::new(60.0, 0.0, 60.0);
        app.world_mut().get_mut::<Neighbour>(person).expect("person").scared_for = 0.0;
        let mut out = false;
        for _ in 0..400 {
            app.update();
            if hiding(&mut app) == 0 {
                out = true;
                break;
            }
        }
        assert!(out, "still hiding with the street clear");
        let census = *app.world().resource::<TownCensus>();
        assert_eq!((census.total, census.alive, census.lost()), (1, 1, 0));
        let name = app.world().get::<Name>(person).expect("person").as_str().to_owned();
        assert!(name.ends_with(')'), "{name}");
    }
}
