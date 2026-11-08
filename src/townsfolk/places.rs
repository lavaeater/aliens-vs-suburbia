//! Where the townsfolk go: front doors (an open tile next to a building) and spots along
//! the streets, worked out from the map once per level. Pure, no ECS.
//!
//! Tiles are in the **padded grid** the game uses (map tile `(x, y)` is `(x + 1, y + 1)`),
//! the coordinates of `CurrentTile`, the pathfinding grid and world positions.
// Grid code: indices come from the grid's own ranges or are checked with `get`.
#![allow(clippy::indexing_slicing)]

use crate::general::components::map_components::{MapFile, RoadKind};
use crate::map::reach::{enemy_passable, player_passable};

/// Doors closer than this (tiles) to one already kept are dropped: one per house or so.
const DOOR_SPACING: usize = 7;
/// A street spot every this many tiles along a road.
const STREET_STEP: f32 = 8.0;

/// A level's places, padded-grid tiles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places {
    /// Open tiles in front of buildings: homes, and places to visit.
    pub doors: Vec<(usize, usize)>,
    /// Open tiles along streets and paths.
    pub streets: Vec<(usize, usize)>,
}

fn open(tile: u64) -> bool {
    enemy_passable(tile) && player_passable(tile)
}

fn far_from_all(kept: &[(usize, usize)], at: (usize, usize), spacing: usize) -> bool {
    kept.iter()
        .all(|&(x, y)| x.abs_diff(at.0).max(y.abs_diff(at.1)) >= spacing)
}

/// The places on `map` (an ordinary map, as `LoadMap` carries it).
pub fn find_places(map: &MapFile) -> Places {
    let height = map.tiles.len();
    let width = map.tiles.first().map_or(0, Vec::len);
    let building = |x: usize, y: usize| {
        map.storeys
            .get(y)
            .and_then(|r| r.get(x))
            .is_some_and(|&s| s > 0)
            && !open(map.tiles[y][x])
    };
    let mut doors: Vec<(usize, usize)> = Vec::new();
    let inner_rows = map.tiles.iter().enumerate().skip(1).take(height.saturating_sub(2));
    for (y, row) in inner_rows {
        for (x, &tile) in row.iter().enumerate().skip(1).take(width.saturating_sub(2)) {
            if !open(tile) {
                continue;
            }
            let by_a_building = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                .iter()
                .any(|&(bx, by)| building(bx, by));
            if by_a_building && far_from_all(&doors, (x, y), DOOR_SPACING) {
                doors.push((x, y));
            }
        }
    }

    let mut streets: Vec<(usize, usize)> = Vec::new();
    for road in map.roads.iter().filter(|r| r.kind != RoadKind::Major) {
        for pair in road.points.windows(2) {
            let ([ax, ay], [bx, by]) = (pair[0], pair[1]);
            let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
            let steps = (length / STREET_STEP).floor() as usize;
            for i in 0..=steps {
                let t = if steps == 0 { 0.0 } else { i as f32 / steps as f32 };
                let (x, y) = ((ax + (bx - ax) * t).round(), (ay + (by - ay) * t).round());
                if x < 1.0 || y < 1.0 {
                    continue;
                }
                let (x, y) = (x as usize, y as usize);
                if map.tiles.get(y).and_then(|r| r.get(x)).is_some_and(|&t| open(t))
                    && far_from_all(&streets, (x, y), 4)
                {
                    streets.push((x, y));
                }
            }
        }
    }

    let pad = |v: Vec<(usize, usize)>| v.into_iter().map(|(x, y)| (x + 1, y + 1)).collect();
    Places {
        doors: pad(doors),
        streets: pad(streets),
    }
}

/// A number that looks random, the same for the same inputs.
pub const fn mix(a: u64, b: u64) -> u64 {
    let mixed = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    mixed ^ (mixed >> 29)
}

/// `count` homes from the doors, spread over the whole list.
pub fn pick_homes(places: &Places, count: usize, seed: u64) -> Vec<(usize, usize)> {
    let doors = &places.doors;
    if doors.is_empty() || count == 0 {
        return Vec::new();
    }
    let offset = (mix(seed, 1) as usize) % doors.len();
    (0..count)
        .map(|i| doors[(offset + i * doors.len() / count.max(1)) % doors.len()])
        .collect()
}

/// Somewhere to go from `from`: a door or a street spot within `reach` tiles, picked by
/// `roll`. `None` when there is nowhere.
pub fn pick_errand(
    places: &Places,
    from: (usize, usize),
    reach: usize,
    roll: u64,
) -> Option<(usize, usize)> {
    let near: Vec<(usize, usize)> = places
        .doors
        .iter()
        .chain(&places.streets)
        .copied()
        .filter(|&(x, y)| {
            let d = x.abs_diff(from.0).max(y.abs_diff(from.1));
            d > 3 && d <= reach
        })
        .collect();
    if near.is_empty() {
        return None;
    }
    near.get((roll as usize) % near.len()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::RoadLine;
    use crate::map::{BitFlags, MapFeatures};

    /// 30 x 20 of grass, two 3 x 3 buildings, a street along row 15.
    fn map() -> MapFile {
        let grass = BitFlags::from(MapFeatures::Grass).bits();
        let wall = (MapFeatures::Floor
            | MapFeatures::ImpassableForPlayers
            | MapFeatures::ImpassableForEnemies)
            .bits();
        let in_house = |x: usize, y: usize| (4..7).contains(&y) && ((4..7).contains(&x) || (20..23).contains(&x));
        MapFile {
            tiles: (0..20)
                .map(|y| (0..30).map(|x| if in_house(x, y) { wall } else { grass }).collect())
                .collect(),
            storeys: (0..20)
                .map(|y| (0..30).map(|x| u8::from(in_house(x, y))).collect())
                .collect(),
            roads: vec![RoadLine {
                kind: RoadKind::Street,
                width: 2.0,
                points: vec![[1.0, 15.0], [28.0, 15.0]],
            }],
            ..MapFile::default()
        }
    }

    #[test]
    fn each_house_gets_a_door_and_the_street_gets_spots() {
        let places = find_places(&map());
        assert_eq!(places.doors.len(), 2, "{:?}", places.doors);
        // Padded: next to the first house (map 4..7 -> padded 5..8).
        let (x, y) = places.doors[0];
        assert!((4..=8).contains(&x) && (4..=8).contains(&y), "{x}, {y}");
        assert!(places.streets.len() >= 3, "{:?}", places.streets);
        assert!(places.streets.iter().all(|&(_, y)| y == 16));
    }

    #[test]
    fn homes_and_errands_come_from_the_places() {
        let places = find_places(&map());
        let homes = pick_homes(&places, 3, 7);
        assert_eq!(homes.len(), 3);
        assert!(homes.iter().all(|h| places.doors.contains(h)));
        let from = places.doors[0];
        let errand = pick_errand(&places, from, 40, 12345).expect("somewhere");
        assert_ne!(errand, from);
        assert!(pick_errand(&places, from, 1, 1).is_none(), "nothing that close");
        assert!(pick_homes(&Places::default(), 3, 1).is_empty());
    }
}
