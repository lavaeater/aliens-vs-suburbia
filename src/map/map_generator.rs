use crate::general::components::map_components::{DecorationItem, MapFile};

// Generated map dimensions
const W: usize = 14;
const H: usize = 24;

// ── Seeded RNG (xorshift64) ──────────────────────────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_add(1).wrapping_mul(0x9e3779b97f4a7c15))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next() as usize % (hi - lo))
    }

    fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn prob(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        &slice[self.range(0, slice.len())]
    }

    fn rotation(&mut self) -> f32 {
        self.range(0, 8) as f32 * 45.0
    }
}

// ── Model palettes ───────────────────────────────────────────────────────────
// (path relative to assets/, visual scale)
// Scale values are approximate for Quaternius poly-pizza models at tile_width=1.0.
// Adjust per-model in level_01.ron if a specific prop looks wrong in-game.

type Prop = (&'static str, f32);

const TREES: &[Prop] = &[
    ("packs/nature/Pine.glb", 1.0),
    ("packs/nature/Pine-79gmlLnweB.glb", 1.0),
    ("packs/nature/Pine-Zt62gceKXZ.glb", 1.0),
    ("packs/nature/Tree.glb", 1.0),
    ("packs/nature/Tree-aVOxaHRPWe.glb", 1.0),
    ("packs/nature/Twisted Tree.glb", 1.0),
    ("packs/nature/Twisted Tree-8oraKn9m0x.glb", 1.0),
    ("packs/nature/Dead Tree.glb", 0.9),
    ("packs/nature/Dead Tree-MlmK5488ou.glb", 0.9),
    ("packs/city/Tree.glb", 1.0),
    ("packs/toon-shooter/Tree.glb", 1.0),
    ("packs/toon-shooter/Tree-1BkD9JnKrE.glb", 1.0),
];

const BUSHES: &[Prop] = &[
    ("packs/nature/Bush.glb", 0.8),
    ("packs/nature/Bush with Flowers.glb", 0.8),
    ("packs/nature/Fern.glb", 0.7),
    ("packs/nature/Plant.glb", 0.7),
    ("packs/nature/Plant Big.glb", 0.9),
    ("packs/nature/Plant Big-MbhbP7JrTI.glb", 0.9),
    ("packs/city/Planter & Bushes.glb", 0.8),
    ("packs/city/Flower Pot.glb", 0.7),
    ("packs/nature/Flower Group.glb", 0.6),
    ("packs/nature/Tall Grass.glb", 0.7),
];

// Street/yard props that sell the "suburb under siege" atmosphere
const SUBURBAN: &[Prop] = &[
    ("packs/city/Bench.glb", 0.8),
    ("packs/city/Trash Can.glb", 0.7),
    ("packs/city/Fire hydrant.glb", 0.7),
    ("packs/city/Mailbox.glb", 0.7),
    ("packs/city/Bicycle.glb", 0.8),
    ("packs/city/Cone.glb", 0.6),
    ("packs/city/Dumpster.glb", 0.9),
    ("packs/city/Bus Stop.glb", 0.9),
    ("packs/city/Traffic Light.glb", 0.8),
    ("packs/city/Stop sign.glb", 0.7),
    ("packs/city/Washing Line.glb", 0.8),
    ("packs/survival/Tent.glb", 0.8),
    ("packs/survival/Bonfire.glb", 0.7),
    ("packs/post-apocalypse/Damaged Couch.glb", 0.8),
    ("packs/interiors/Couch Medium.glb", 0.8),
    // Parked vehicles – large props, but decorative only
    ("packs/city/Car.glb", 1.0),
    ("packs/city/Car-unqqkULtRU.glb", 1.0),
    ("packs/city/SUV.glb", 1.0),
    ("packs/city/Pickup Truck.glb", 1.0),
    ("packs/city/Van.glb", 1.0),
    ("packs/city/Motorcycle.glb", 0.8),
    ("packs/toon-shooter/Broken Car.glb", 1.0),
];

// Alien-side dressing: sci-fi plants, invasion debris
const ALIEN_DRESSING: &[Prop] = &[
    ("packs/space/Rock.glb", 0.8),
    ("packs/space/Rock Large.glb", 1.0),
    ("packs/space/Rock Large-d2VWOdthtR.glb", 1.0),
    ("packs/space/Tree Blob.glb", 1.0),
    ("packs/space/Tree Blob-j0byyoIGOv.glb", 1.0),
    ("packs/space/Tree Lava.glb", 1.0),
    ("packs/space/Tree Spikes.glb", 1.0),
    ("packs/space/Tree Spiral.glb", 1.0),
    ("packs/space/Plant.glb", 0.8),
    ("packs/space/Plant-VwXvoIpCHP.glb", 0.8),
    ("packs/space/Bush.glb", 0.8),
    ("packs/space/Bush-RfUP3gXj69.glb", 0.8),
    ("packs/space/Grass.glb", 0.7),
    ("packs/post-apocalypse/Traffic Cone.glb", 0.6),
    ("packs/post-apocalypse/Trash Bag.glb", 0.7),
    ("packs/post-apocalypse/Trash Bags.glb", 0.7),
    ("packs/space/Pickup Crate.glb", 0.8),
    ("packs/space/Solar Panel.glb", 0.9),
    ("packs/space/Roof Antenna.glb", 0.8),
];

// Battle debris – mid-map where fighting has occurred
const COMBAT: &[Prop] = &[
    ("packs/toon-shooter/Crate.glb", 0.8),
    ("packs/toon-shooter/Cardboard Boxes.glb", 0.8),
    ("packs/toon-shooter/Cardboard Boxes-rdKKO0DvMG.glb", 0.8),
    ("packs/post-apocalypse/Barrel.glb", 0.8),
    ("packs/post-apocalypse/Pallet.glb", 0.8),
    ("packs/post-apocalypse/Pallet Broken.glb", 0.8),
    ("packs/toon-shooter/Barrier Single.glb", 0.9),
    ("packs/toon-shooter/Sack Trench Small.glb", 0.9),
    ("packs/toon-shooter/Gas Tank.glb", 0.7),
    ("packs/toon-shooter/Dumpster.glb", 0.9),
    ("packs/toon-shooter/Pallet.glb", 0.8),
    ("packs/toon-shooter/Tires.glb", 0.8),
    ("packs/post-apocalypse/Cinder Block.glb", 0.7),
    ("packs/post-apocalypse/Plastic Barrier.glb", 0.9),
    ("packs/post-apocalypse/Water Tower.glb", 1.0),
];

// Small ground-level scatter — fills empty floor without dominating the view
const CLUTTER: &[Prop] = &[
    ("packs/nature/Rock Medium.glb", 0.7),
    ("packs/nature/Rock Medium-JQxF95498B.glb", 0.7),
    ("packs/nature/Pebble Round.glb", 0.6),
    ("packs/nature/Pebble Square.glb", 0.6),
    ("packs/nature/Mushroom.glb", 0.6),
    ("packs/nature/Mushroom Laetiporus.glb", 0.6),
    ("packs/nature/Flower Petal.glb", 0.5),
    ("packs/nature/Clover.glb", 0.5),
    ("packs/city/Box.glb", 0.6),
    ("packs/post-apocalypse/Blood Splat.glb", 0.5),
    ("packs/toon-shooter/Debris Papers.glb", 0.6),
    ("packs/toon-shooter/Debris Pile.glb", 0.7),
    ("packs/city/Debris Papers.glb", 0.6),
    ("packs/survival/Wood Log.glb", 0.7),
    ("packs/survival/Can.glb", 0.5),
    ("packs/survival/Gas Can.glb", 0.6),
    ("packs/post-apocalypse/Wheel.glb", 0.7),
];

// ── Zone classification ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Zone {
    PlayerArea,  // near player spawn — suburban props, parked cars
    AlienArea,   // near alien spawns — sci-fi/invasion dressing
    Perimeter,   // map edge and house-adjacent — trees, hedges
    Open,        // mid-map — mixed combat debris and suburban clutter
}

fn classify(
    row: usize,
    col: usize,
    grid: &[Vec<u8>],
    player: (usize, usize),
    aliens: &[(usize, usize)],
) -> Zone {
    let dp = row.abs_diff(player.0) + col.abs_diff(player.1);
    let da = aliens
        .iter()
        .map(|&(ar, ac)| row.abs_diff(ar) + col.abs_diff(ac))
        .min()
        .unwrap_or(99);
    let h = grid.len();
    let w = grid[0].len();
    let on_edge = row == 0 || row == h - 1 || col == 0 || col == w - 1;
    let near_void = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|&(dr, dc)| {
        let nr = row as i32 + dr;
        let nc = col as i32 + dc;
        nr >= 0 && (nr as usize) < h && nc >= 0 && (nc as usize) < w
            && grid[nr as usize][nc as usize] == 0
    });

    if dp <= 3 {
        Zone::PlayerArea
    } else if da <= 3 {
        Zone::AlienArea
    } else if on_edge || near_void {
        Zone::Perimeter
    } else {
        Zone::Open
    }
}

fn pick_prop(rng: &mut Rng, zone: Zone) -> (&'static str, f32) {
    let palette: &[Prop] = match zone {
        Zone::PlayerArea => match rng.range(0, 5) {
            0 | 1 => SUBURBAN,
            2     => TREES,
            3     => BUSHES,
            _     => CLUTTER,
        },
        Zone::AlienArea => match rng.range(0, 4) {
            0 | 1 => ALIEN_DRESSING,
            2     => COMBAT,
            _     => CLUTTER,
        },
        Zone::Perimeter => {
            if rng.prob(0.65) { TREES } else { BUSHES }
        }
        Zone::Open => match rng.range(0, 6) {
            0     => TREES,
            1     => BUSHES,
            2     => SUBURBAN,
            3     => ALIEN_DRESSING,
            4     => COMBAT,
            _     => CLUTTER,
        },
    };
    *rng.pick(palette)
}

// ── Connectivity check (BFS) ─────────────────────────────────────────────────

fn is_connected(grid: &[Vec<u8>], from: (usize, usize), to: (usize, usize)) -> bool {
    if grid[from.0][from.1] == 0 || grid[to.0][to.1] == 0 {
        return false;
    }
    let h = grid.len();
    let w = grid[0].len();
    let mut visited = vec![vec![false; w]; h];
    let mut stack = vec![from];
    visited[from.0][from.1] = true;
    while let Some((r, c)) = stack.pop() {
        if (r, c) == to {
            return true;
        }
        for (dr, dc) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nr = r as i32 + dr;
            let nc = c as i32 + dc;
            if nr >= 0 && (nr as usize) < h && nc >= 0 && (nc as usize) < w {
                let (nr, nc) = (nr as usize, nc as usize);
                if !visited[nr][nc] && grid[nr][nc] != 0 {
                    visited[nr][nc] = true;
                    stack.push((nr, nc));
                }
            }
        }
    }
    false
}

// ── House placement ──────────────────────────────────────────────────────────

fn try_place_house(
    grid: &mut Vec<Vec<u8>>,
    rng: &mut Rng,
    player: (usize, usize),
    aliens: &[(usize, usize)],
    goal: (usize, usize),
) -> bool {
    let h = grid.len();
    let w = grid[0].len();

    for _ in 0..30 {
        let hh = rng.range(2, 5); // house height (rows)
        let hw = rng.range(2, 4); // house width (cols)

        if h < hh + 4 || w < hw + 2 { continue; }
        let row = rng.range(2, h - hh - 2);
        let col = rng.range(1, w - hw - 1);

        // All cells must be plain floor (value 1)
        let clear = (row..row + hh).all(|r| (col..col + hw).all(|c| grid[r][c] == 1));
        if !clear { continue; }

        // Tentatively place
        for r in row..row + hh {
            for c in col..col + hw {
                grid[r][c] = 0;
            }
        }

        // All alien spawns and player must still reach the goal
        let connected = is_connected(grid, player, goal)
            && aliens.iter().all(|&sp| is_connected(grid, sp, goal));

        if connected {
            return true;
        }

        // Roll back
        for r in row..row + hh {
            for c in col..col + hw {
                grid[r][c] = 1;
            }
        }
    }
    false
}

// ── Public entry point ───────────────────────────────────────────────────────

pub fn generate_suburb_map(seed: u64) -> MapFile {
    let mut rng = Rng::new(seed);
    let mut grid: Vec<Vec<u8>> = vec![vec![1u8; W]; H];

    // Anchor points
    let player = (0usize, 0usize);
    grid[player.0][player.1] = 17;

    // 1–3 alien spawn points spread across the far end of row 0
    let spawn_candidates: &[(usize, usize)] = &[
        (0, W - 1),
        (0, W - 3),
        (2, W - 1),
        (0, W - 5),
        (4, W - 1),
    ];
    let spawn_count = rng.range(1, 4);
    let alien_spawns: Vec<(usize, usize)> = spawn_candidates[..spawn_count].to_vec();
    for &(r, c) in &alien_spawns {
        grid[r][c] = 5;
    }

    // Alien goal deep in the map, slightly off-center for asymmetry
    let goal_col = W / 2 + rng.range(0, 3).wrapping_sub(1);
    let goal = (H - 5, goal_col.min(W - 2).max(1));
    grid[goal.0][goal.1] = 9;

    // Place houses — 3–5 attempts, each verified against pathfinding
    let num_houses = rng.range(3, 6);
    for _ in 0..num_houses {
        try_place_house(&mut grid, &mut rng, player, &alien_spawns, goal);
    }

    // Decorate every plain-floor tile according to its zone
    let mut decorations: Vec<DecorationItem> = Vec::new();
    for row in 0..H {
        for col in 0..W {
            // Only decorate plain floor; skip specials and void
            if grid[row][col] != 1 { continue; }
            // Keep the tile immediately around goal clear for readability
            if row.abs_diff(goal.0) + col.abs_diff(goal.1) <= 1 { continue; }

            let zone = classify(row, col, &grid, player, &alien_spawns);
            let prob = match zone {
                Zone::PlayerArea => 0.30,
                Zone::AlienArea  => 0.35,
                Zone::Perimeter  => 0.42,
                Zone::Open       => 0.13,
            };
            if !rng.prob(prob) { continue; }

            let (model, scale) = pick_prop(&mut rng, zone);
            decorations.push(DecorationItem {
                x: col as i32,
                y: row as i32,
                model: model.to_string(),
                rotation_y: rng.rotation(),
                scale,
            });
        }
    }

    MapFile {
        generated: false,
        seed,
        tiles: grid,
        decorations,
    }
}
