//! Random loot bags: a map's `loot` spread ([`LootSpread`]) scatters bags over its open
//! floor when it loads. Each bag is an `ItemKind::Loot` pickup: walking into it rolls its
//! table and spills the result around it (`items::pickup_items`), so a bag can hold a
//! gun, a med-kit, some ammo, or nothing worth having.
//!
//! [`plan_bags`] is the pure part (which tiles), [`spawn_loot_bags`] builds them.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};

use crate::assets::asset_definition::{AssetDefinition, ItemKind};
use crate::general::components::map_components::LootSpread;
use crate::general::events::map_events::LoadMap;
use crate::general::systems::map_systems::TileDefinitions;
use crate::items::{Item, Pickup};
use crate::loot::unit_random;
use crate::map::walls::padded_tiles;
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

/// Scatter the map's loot bags when it loads.
pub fn spawn_loot_bags(
    mut loads: MessageReader<LoadMap>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_defs: Res<TileDefinitions>,
) {
    for load in loads.read() {
        let Some(spread) = &load.map.loot else {
            continue;
        };
        let looks: Vec<(AssetDefinition, Handle<WorldAsset>)> = spread
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
            continue;
        }
        // A fresh spread every time the level is played.
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos());
        let mut rng = || unit_random(&mut seed);
        // Placements are in map coordinates, one tile in from the padded grid.
        let taken: HashSet<(usize, usize)> = load
            .map
            .placements
            .iter()
            .filter_map(|p| {
                Some((
                    usize::try_from(p.x + 1).ok()?,
                    usize::try_from(p.y + 1).ok()?,
                ))
            })
            .collect();
        let plan = plan_bags(&padded_tiles(&load.map), &taken, spread, &mut rng);

        let tw = tile_defs.tile_width;
        let floor = tile_defs.floor_level + tile_defs.tile_depth;
        for &(col, row) in &plan {
            let pick = (rng() * looks.len() as f32) as usize;
            let Some((def, scene)) = looks.get(pick).or(looks.last()) else {
                continue;
            };
            let jitter = Vec3::new(rng() - 0.5, 0.0, rng() - 0.5) * 2.0 * JITTER * tw;
            let at = Vec3::new(tw * col as f32, floor + def.lift_world(), tw * row as f32) + jitter;
            commands.spawn((
                Name::new(format!("Loot bag {col}:{row}")),
                WorldAssetRoot(scene.clone()),
                Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_y(rng() * std::f32::consts::TAU))
                    .with_scale(Vec3::splat(def.scale)),
                Item(ItemKind::Loot {
                    table: spread.table.clone(),
                }),
                Pickup::default(),
            ));
        }
        info!("scattered {} loot bags ({})", plan.len(), spread.table);
    }
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
