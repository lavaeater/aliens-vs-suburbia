//! Prop scatter — dresses a tile grid with decoration models so generated maps read as
//! a ruined, ultraviolent suburb instead of bare floor. Decorations are collider-free
//! (`SceneRoot` only, see `map_systems`), so scattering them on walkable floor never
//! breaks pathfinding.
//!
//! The `ULTRAVIOLENCE` palette leans on the `city` + `post-apocalypse` + `toon-shooter`
//! packs (burned cars, barricades, barrels, dumpsters, sandbags, debris, blood). Used by
//! the chunk stitcher; the older `map_generator` can call it too.

use crate::general::components::map_components::DecorationItem;
use crate::map::MapFeatures;
use enumflags2::BitFlags;

/// `(model_path, scale_in_player_units)`.
type Prop = (&'static str, f32);

/// Ground-level gore/debris — the common scatter that dirties the floor.
const GROUND: &[Prop] = &[
    ("packs/post-apocalypse/Blood Splat.glb", 0.05),
    ("packs/post-apocalypse/Trash Bag.glb", 0.4),
    ("packs/post-apocalypse/Trash Bags.glb", 0.5),
    ("packs/post-apocalypse/Cinder Block.glb", 0.3),
    ("packs/post-apocalypse/Wheel.glb", 0.25),
    ("packs/post-apocalypse/Pallet.glb", 0.2),
    ("packs/post-apocalypse/Pallet Broken.glb", 0.2),
    ("packs/toon-shooter/Debris Pile.glb", 0.2),
    ("packs/toon-shooter/Debris Papers.glb", 0.1),
    ("packs/toon-shooter/Tires.glb", 0.6),
    ("packs/survival/Gas Can.glb", 0.2),
];

/// Waist-high cover — barrels, barricades, sandbags.
const COVER: &[Prop] = &[
    ("packs/post-apocalypse/Barrel.glb", 0.8),
    ("packs/post-apocalypse/Plastic Barrier.glb", 0.9),
    ("packs/toon-shooter/Barrier Single.glb", 0.9),
    ("packs/toon-shooter/Sack Trench Small.glb", 0.8),
    ("packs/toon-shooter/Crate.glb", 0.7),
    ("packs/toon-shooter/Gas Tank.glb", 0.6),
    ("packs/city/Cone.glb", 0.35),
];

/// Big landmarks — wrecked vehicles, dumpsters, the odd water tower.
const LANDMARK: &[Prop] = &[
    ("packs/toon-shooter/Broken Car.glb", 0.9),
    ("packs/city/Car.glb", 0.9),
    ("packs/city/SUV.glb", 1.1),
    ("packs/city/Van.glb", 1.2),
    ("packs/city/Dumpster.glb", 1.0),
    ("packs/toon-shooter/Dumpster.glb", 1.0),
    ("packs/post-apocalypse/Water Tower.glb", 2.5),
    ("packs/post-apocalypse/Damaged Couch.glb", 0.6),
];

/// Weighting knobs for the scatter. Probabilities are per floor tile.
#[derive(Clone, Copy)]
pub struct ScatterOptions {
    /// Chance a floor tile gets a ground-clutter/gore prop.
    pub ground_chance: f32,
    /// Chance a floor tile gets a piece of cover.
    pub cover_chance: f32,
    /// Chance a floor tile gets a big landmark (kept low — they're large).
    pub landmark_chance: f32,
}

impl Default for ScatterOptions {
    fn default() -> Self {
        Self { ground_chance: 0.16, cover_chance: 0.05, landmark_chance: 0.015 }
    }
}

// ── Seeded RNG (xorshift64, matching the other map modules) ───────────────────

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
    fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn pick<'a>(&mut self, slice: &'a [Prop]) -> &'a Prop {
        &slice[(self.next() as usize) % slice.len()]
    }
    fn rotation(&mut self) -> f32 {
        ((self.next() as usize) % 8) as f32 * 45.0
    }
}

/// True if a tile is plain walkable floor we're allowed to dress — not void, not a wall,
/// and not a spawn/goal/player marker (we keep those clear).
fn is_dressable(bits: u64) -> bool {
    if bits == 0 {
        return false;
    }
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(bits);
    flags.contains(MapFeatures::Floor)
        && !flags.contains(MapFeatures::ImpassableForPlayers)
        && !flags.contains(MapFeatures::ImpassableForEnemies)
        && !flags.contains(MapFeatures::EnemySpawn)
        && !flags.contains(MapFeatures::EnemyExit)
        && !flags.contains(MapFeatures::PlayerSpawn)
}

/// Scatter decorations across the dressable floor of `tiles`. Deterministic for a seed.
/// At most one prop per tile (landmark > cover > ground precedence, so big things win
/// their tile). `x = col`, `y = row` to match `DecorationItem` / the tile layout.
pub fn scatter_decorations(seed: u64, tiles: &[Vec<u64>], opts: ScatterOptions) -> Vec<DecorationItem> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();

    for (row, cols) in tiles.iter().enumerate() {
        for (col, &bits) in cols.iter().enumerate() {
            if !is_dressable(bits) {
                continue;
            }
            // One roll, split across the three tiers by precedence.
            let roll = rng.f32();
            let palette = if roll < opts.landmark_chance {
                LANDMARK
            } else if roll < opts.landmark_chance + opts.cover_chance {
                COVER
            } else if roll < opts.landmark_chance + opts.cover_chance + opts.ground_chance {
                GROUND
            } else {
                continue;
            };

            let (model, scale) = *rng.pick(palette);
            out.push(DecorationItem {
                x: col as i32,
                y: row as i32,
                model: model.to_string(),
                rotation_y: rng.rotation(),
                scale,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor() -> u64 {
        BitFlags::from(MapFeatures::Floor).bits()
    }
    fn wall() -> u64 {
        (MapFeatures::Floor | MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies)
            .bits()
    }
    fn spawn() -> u64 {
        (MapFeatures::Floor | MapFeatures::EnemySpawn).bits()
    }

    /// A grid that's all floor except a wall ring, a spawn and a goal.
    fn test_grid() -> Vec<Vec<u64>> {
        let mut g = vec![vec![floor(); 10]; 10];
        for c in 0..10 {
            g[0][c] = wall();
            g[9][c] = wall();
        }
        for r in 0..10 {
            g[r][0] = wall();
            g[r][9] = wall();
        }
        g[5][0] = spawn();
        g[5][9] = (MapFeatures::Floor | MapFeatures::EnemyExit).bits();
        g
    }

    #[test]
    fn scatter_is_deterministic_for_a_seed() {
        let g = test_grid();
        let a = scatter_decorations(9, &g, ScatterOptions::default());
        let b = scatter_decorations(9, &g, ScatterOptions::default());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.model, y.model);
            assert_eq!((x.x, x.y), (y.x, y.y));
        }
    }

    #[test]
    fn nothing_lands_on_walls_void_or_spawn_goal() {
        let g = test_grid();
        // High density to stress it.
        let opts = ScatterOptions { ground_chance: 0.9, cover_chance: 0.05, landmark_chance: 0.02 };
        let decs = scatter_decorations(3, &g, opts);
        assert!(!decs.is_empty(), "something should be placed on the interior floor");
        for d in &decs {
            let bits = g[d.y as usize][d.x as usize];
            assert!(is_dressable(bits), "decoration landed on a non-dressable tile at {:?}", (d.x, d.y));
        }
    }

    #[test]
    fn zero_chance_scatters_nothing() {
        let g = test_grid();
        let opts = ScatterOptions { ground_chance: 0.0, cover_chance: 0.0, landmark_chance: 0.0 };
        assert!(scatter_decorations(1, &g, opts).is_empty());
    }

    #[test]
    fn higher_density_places_more_props() {
        let g = test_grid();
        let sparse = scatter_decorations(5, &g, ScatterOptions { ground_chance: 0.05, cover_chance: 0.0, landmark_chance: 0.0 });
        let dense = scatter_decorations(5, &g, ScatterOptions { ground_chance: 0.8, cover_chance: 0.0, landmark_chance: 0.0 });
        assert!(dense.len() > sparse.len(), "more density -> more props ({} vs {})", dense.len(), sparse.len());
    }

    #[test]
    fn every_palette_entry_has_a_real_path_and_positive_scale() {
        for palette in [GROUND, COVER, LANDMARK] {
            for &(model, scale) in palette {
                assert!(model.ends_with(".glb"), "'{model}' should be a glb path");
                assert!(model.starts_with("packs/"), "'{model}' should be assets-relative");
                assert!(scale > 0.0, "'{model}' needs a positive scale");
            }
        }
    }
}
