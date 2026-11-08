//! The engine-neutral scene: a walled level, a crowd of aliens walking to a goal, four players
//! shooting rays, and a burst of gibs every half second. Both engines build and drive exactly
//! this, so the only difference left is the physics.

use bevy::prelude::*;

/// The game's fixed step (`game_state_plugin.rs`: `Time::<Fixed>::from_seconds(0.05)`).
pub const STEP: f32 = 0.05;
pub const MAP_TILES: i32 = 48;
pub const TILE: f32 = 2.0;
/// `map_systems.rs`: walls are `4/3` half-height cubes.
pub const WALL_HALF_HEIGHT: f32 = 4.0 / 3.0;
pub const ALIEN_RADIUS: f32 = 0.3;
pub const ALIEN_LENGTH: f32 = 0.8;
pub const ALIEN_SPEED: f32 = 3.0;
/// Rays per alien per tick, like `avoid_walls_systems.rs`: forward, left, right.
pub const WALL_RAY_LENGTH: f32 = 2.0;
pub const SHOT_LENGTH: f32 = 40.0;
pub const PLAYERS: usize = 4;
/// One kill every `GIB_EVERY` ticks bursts `GIBS_PER_KILL` gibs; the oldest go past the budget.
pub const GIB_EVERY: u64 = 10;
pub const GIBS_PER_KILL: usize = 12;
pub const GIB_BUDGET: usize = 240;

#[derive(Clone, Copy, Debug)]
pub struct WallBox {
    pub center: Vec3,
    pub half: Vec3,
}

#[derive(Resource, Clone)]
pub struct Scene {
    pub walls: Vec<WallBox>,
    pub floor: WallBox,
    pub spawns: Vec<Vec3>,
    pub goal: Vec3,
    pub players: Vec<Vec3>,
    pub aliens: usize,
}

/// SplitMix64: no dependency, the same numbers for both engines.
#[derive(Resource, Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[-1, 1)`.
    pub fn signed(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

fn tile_center(x: i32, z: i32) -> Vec3 {
    let half = MAP_TILES as f32 * TILE / 2.0;
    Vec3::new(x as f32 * TILE - half + TILE / 2.0, 0.0, z as f32 * TILE - half + TILE / 2.0)
}

impl Scene {
    pub fn new(aliens: usize) -> Self {
        let mut rng = Rng(7);
        let n = MAP_TILES;
        let mid = n / 2;
        let corners = [(3, 3), (n - 4, 3), (3, n - 4), (n - 4, n - 4)];
        let near = |x: i32, z: i32, (cx, cz): (i32, i32), r: i32| (x - cx).abs() <= r && (z - cz).abs() <= r;
        let mut solid = vec![vec![false; n as usize]; n as usize];
        for z in 0..n {
            for x in 0..n {
                let edge = x == 0 || z == 0 || x == n - 1 || z == n - 1;
                let clear = near(x, z, (mid, mid), 3) || corners.iter().any(|&c| near(x, z, c, 2));
                solid[z as usize][x as usize] =
                    edge || (!clear && (rng.next_u64() % 100) < 12);
            }
        }
        // Greedy merge per row, like the game's merged wall colliders.
        let mut walls = Vec::new();
        for z in 0..n {
            let mut x = 0;
            while x < n {
                if !solid[z as usize][x as usize] {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < n && solid[z as usize][x as usize] {
                    x += 1;
                }
                let a = tile_center(start, z);
                let b = tile_center(x - 1, z);
                walls.push(WallBox {
                    center: Vec3::new((a.x + b.x) / 2.0, WALL_HALF_HEIGHT, a.z),
                    half: Vec3::new((x - start) as f32 * TILE / 2.0, WALL_HALF_HEIGHT, TILE / 2.0),
                });
            }
        }
        let size = n as f32 * TILE / 2.0;
        Self {
            walls,
            floor: WallBox { center: Vec3::new(0.0, -0.5, 0.0), half: Vec3::new(size, 0.5, size) },
            spawns: corners.iter().map(|&(x, z)| tile_center(x, z) + Vec3::Y).collect(),
            goal: tile_center(mid, mid),
            players: (0..PLAYERS)
                .map(|i| tile_center(mid - 2 + i as i32, mid + 2) + Vec3::Y)
                .collect(),
            aliens,
        }
    }

    /// Where alien `i` starts, and goes back to after reaching the goal.
    pub fn spawn_of(&self, i: usize, rng: &mut Rng) -> Vec3 {
        self.spawns[i % self.spawns.len()] + Vec3::new(rng.signed() * 1.5, 0.0, rng.signed() * 1.5)
    }

    /// The horizontal velocity alien `i` asks for: to the goal with a wobble, turned away
    /// from a wall closer than a metre ahead.
    pub fn steer(&self, i: usize, tick: u64, position: Vec3, wall_ahead: Option<f32>) -> Vec3 {
        let to_goal = (self.goal - position).with_y(0.0).normalize_or_zero();
        let wobble = (tick as f32 * 0.1 + i as f32).sin() * 0.6;
        let mut dir = (to_goal + to_goal.cross(Vec3::Y) * wobble).normalize_or_zero();
        if wall_ahead.is_some_and(|d| d < 1.0) {
            dir = dir.cross(Vec3::Y);
        }
        dir * ALIEN_SPEED
    }

    pub fn reached_goal(&self, position: Vec3) -> bool {
        (self.goal - position).with_y(0.0).length() < 3.0
    }
}

/// The three ray directions of an alien walking along `forward`.
pub fn wall_rays(forward: Vec3) -> [Vec3; 3] {
    let f = if forward.length_squared() > 1e-6 { forward.normalize() } else { Vec3::Z };
    let left = Quat::from_rotation_y(0.6) * f;
    let right = Quat::from_rotation_y(-0.6) * f;
    [f, left, right]
}

/// Player `p`'s shot this tick: a slowly sweeping direction.
pub fn shot_dir(p: usize, tick: u64) -> Vec3 {
    let a = tick as f32 * 0.07 + p as f32 * 1.57;
    Vec3::new(a.cos(), -0.02, a.sin()).normalize()
}

/// Eight points of a gib hull, about 0.3 m across.
pub fn gib_points(rng: &mut Rng) -> Vec<Vec3> {
    (0..8).map(|_| Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 0.15).collect()
}

pub fn gib_velocity(rng: &mut Rng) -> Vec3 {
    Vec3::new(rng.signed() * 3.0, 3.0 + rng.signed().abs() * 3.0, rng.signed() * 3.0)
}

/// What every alien carries in both engines.
#[derive(Component)]
pub struct Alien {
    pub index: usize,
    pub forward: Vec3,
    pub wall_ahead: Option<f32>,
}

#[derive(Component)]
pub struct Gib;

/// Tick count, gib queue and hit counters (the counters keep the queries from being dead code
/// and let the two engines be compared for sanity).
#[derive(Resource, Default)]
pub struct Stats {
    pub tick: u64,
    pub gibs: std::collections::VecDeque<Entity>,
    pub wall_hits: u64,
    pub shot_hits: u64,
    pub arrivals: u64,
    pub physics_time: std::time::Duration,
}
