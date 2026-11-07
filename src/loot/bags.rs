//! Random loot bags: a map's `loot` spread ([`LootSpread`]) scatters bags over its open
//! floor when it loads. Each bag is an `ItemKind::Loot` pickup: walking into it rolls its
//! table and spills the result around it (`items::pickup_items`), so a bag can hold a
//! gun, a med-kit, some ammo, or nothing worth having.
//!
//! [`plan_bags`] is the pure part (which tiles), [`plan_spread`] dresses those tiles up, and
//! `map::streaming` keeps the result per chunk and calls [`spawn_bag`] for the chunks near
//! the camera.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};

use crate::map::ground::GroundHeight;
use crate::assets::asset_definition::{AssetDefinition, ItemKind};
use crate::general::components::map_components::{LootSpread, MapFile};
use crate::general::systems::map_systems::TileDefinitions;
use crate::items::{Item, Pickup};
use crate::loot::unit_random;
use crate::map::streaming::Chunk;
use crate::map::{BitFlags, MapFeatures};

/// How far from the tile centre a bag may lie, as a share of the tile, so a row of bags
/// does not line up on the grid.
const JITTER: f32 = 0.3;

/// Whether a bag may lie on this tile value: floor anyone can walk on, and not a spawn,
/// a goal or water.
fn takes_a_bag(raw: u64) -> bool {
    if raw == 0 {
        return false;
    }
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(raw);
    !flags.intersects(
        MapFeatures::ImpassableForPlayers
            | MapFeatures::ImpassableForEnemies
            | MapFeatures::PlayerSpawn
            | MapFeatures::EnemySpawn
            | MapFeatures::EnemyExit
            | MapFeatures::Water,
    )
}

/// How many bags for `open` tiles: `min..=max` for every `per_tiles` of them (rounded,
/// at least one stretch on a map with any floor at all).
pub fn bag_count(open: usize, spread: &LootSpread, rng: &mut impl FnMut() -> f32) -> usize {
    if open == 0 {
        return 0;
    }
    let per = spread.per_tiles.max(1) as usize;
    let stretches = ((open + per / 2) / per).max(1);
    let (low, high) = (spread.min.min(spread.max), spread.min.max(spread.max));
    let span = (high - low + 1) as f32;
    (0..stretches)
        .map(|_| low + ((rng().clamp(0.0, 0.999_999) * span) as u32).min(high - low))
        .sum::<u32>() as usize
}

/// The `(col, row)` tiles that get a bag, at most one each, never on a `taken` tile.
pub fn plan_bags(
    tiles: &[Vec<u64>],
    taken: &HashSet<(usize, usize)>,
    spread: &LootSpread,
    rng: &mut impl FnMut() -> f32,
) -> Vec<(usize, usize)> {
    let mut open: Vec<(usize, usize)> = tiles
        .iter()
        .enumerate()
        .flat_map(|(row, cells)| {
            cells
                .iter()
                .enumerate()
                .filter(|(_, raw)| takes_a_bag(**raw))
                .map(move |(col, _)| (col, row))
        })
        .collect();
    let count = bag_count(open.len(), spread, rng);
    open.retain(|tile| !taken.contains(tile));
    // Partial Fisher-Yates: the first `count` are a fair random pick.
    let count = count.min(open.len());
    for i in 0..count {
        let j = i + ((rng().clamp(0.0, 0.999_999) * (open.len() - i) as f32) as usize);
        open.swap(i, j);
    }
    open.truncate(count);
    open
}

/// A model a bag can be drawn with.
pub type BagLook = (AssetDefinition, Handle<WorldAsset>);

/// One bag of a map's spread, decided when the map loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannedBag {
    /// Grid `(col, row)` of the padded grid.
    pub tile: (i32, i32),
    /// Index into [`Spread::looks`].
    pub look: usize,
    /// Off the tile centre, in tiles.
    pub offset: Vec2,
    pub yaw: f32,
}

/// A map's bags, as data.
pub struct Spread {
    pub looks: Vec<BagLook>,
    pub table: String,
    pub bags: Vec<PlannedBag>,
}

/// Marks a live bag with its place in its chunk's plan, so the plan can tell which bags are
/// still lying there when the chunk unloads.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamedBag {
    pub chunk: Chunk,
    pub id: usize,
}

/// Decide a map's loot bags. `None` when it has no spread or no bag model can be read.
pub fn plan_spread(
    map: &MapFile,
    tiles: &[Vec<u64>],
    asset_server: &AssetServer,
) -> Option<Spread> {
    let spread = map.loot.as_ref()?;
    let looks: Vec<BagLook> = spread
        .bags
        .iter()
        .filter_map(|path| {
            let def = AssetDefinition::load_from_def_path(path);
            if def.is_none() {
                warn!("loot bag def {path} not found");
            }
            def
        })
        .map(|def| {
            let scene = asset_server
                .load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(def.model_path.clone()));
            (def, scene)
        })
        .collect();
    if looks.is_empty() {
        warn!("no loot bag defs could be read; no loot bags");
        return None;
    }
    // A fresh spread every time the level is played.
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos());
    let mut rng = || unit_random(&mut seed);
    // Placements are in map coordinates, one tile in from the padded grid.
    let taken: HashSet<(usize, usize)> = map
        .placements
        .iter()
        .filter_map(|p| {
            Some((
                usize::try_from(p.x + 1).ok()?,
                usize::try_from(p.y + 1).ok()?,
            ))
        })
        .collect();
    let bags = plan_bags(tiles, &taken, spread, &mut rng)
        .into_iter()
        .map(|(col, row)| PlannedBag {
            tile: (col as i32, row as i32),
            look: ((rng() * looks.len() as f32) as usize).min(looks.len() - 1),
            offset: Vec2::new(rng() - 0.5, rng() - 0.5) * 2.0 * JITTER,
            yaw: rng() * std::f32::consts::TAU,
        })
        .collect();
    Some(Spread {
        looks,
        table: spread.table.clone(),
        bags,
    })
}

/// Build one planned bag.
pub fn spawn_bag(
    parent: &mut ChildSpawnerCommands,
    bag: &PlannedBag,
    id: StreamedBag,
    looks: &[BagLook],
    table: &str,
    tile_defs: &TileDefinitions,
    ground: &GroundHeight,
) {
    let Some((def, scene)) = looks.get(bag.look) else {
        return;
    };
    let tw = tile_defs.tile_width;
    let floor = tile_defs.floor_level + tile_defs.tile_depth;
    let (col, row) = bag.tile;
    let (x, z) = (tw * (col as f32 + bag.offset.x), tw * (row as f32 + bag.offset.y));
    let at = Vec3::new(x, floor + ground.height(x, z) + def.lift_world(), z);
    parent.spawn((
        Name::new(format!("Loot bag {col}:{row}")),
        WorldAssetRoot(scene.clone()),
        Transform::from_translation(at)
            .with_rotation(Quat::from_rotation_y(bag.yaw))
            .with_scale(Vec3::splat(def.scale)),
        Item(ItemKind::Loot {
            table: table.to_string(),
        }),
        Pickup::default(),
        id,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: u64 = MapFeatures::Floor as u64;

    fn spread(min: u32, max: u32, per_tiles: u32) -> LootSpread {
        LootSpread {
            min,
            max,
            per_tiles,
            ..LootSpread::default()
        }
    }

    #[test]
    fn the_count_is_min_to_max_per_stretch_of_tiles() {
        // 100 open tiles, 1-5 per 20: five stretches.
        assert_eq!(bag_count(100, &spread(1, 5, 20), &mut || 0.0), 5);
        assert_eq!(bag_count(100, &spread(1, 5, 20), &mut || 0.999), 25);
        // A small map still gets its one stretch; no floor, no bags.
        assert_eq!(bag_count(3, &spread(2, 2, 20), &mut || 0.5), 2);
        assert_eq!(bag_count(0, &spread(2, 2, 20), &mut || 0.5), 0);
        // Written backwards, or with a zero stretch, it still means something.
        assert_eq!(bag_count(10, &spread(3, 1, 0), &mut || 0.0), 10);
    }

    #[test]
    fn bags_go_on_open_floor_only_one_per_tile() {
        let wall = FLOOR
            | MapFeatures::ImpassableForPlayers as u64
            | MapFeatures::ImpassableForEnemies as u64;
        let spawn = FLOOR | MapFeatures::PlayerSpawn as u64;
        let tiles = vec![vec![0, wall, spawn], vec![FLOOR, FLOOR, FLOOR]];
        let taken = HashSet::from([(2, 1)]);
        let mut n = 0.0;
        let mut rng = || {
            n += 0.37;
            n % 1.0
        };
        // Asks for far more than there is room for.
        let plan = plan_bags(&tiles, &taken, &spread(9, 9, 1), &mut rng);
        let mut got = plan.clone();
        got.sort_unstable();
        assert_eq!(got, vec![(0, 1), (1, 1)]);
    }
}
