//! The neighbourhood generator (`generator: neighbourhood` in a `.map`): a suburb of
//! lots and streets, laid out by `ghx_proc_gen`'s model synthesis (wave function
//! collapse) and then drawn into tiles.
//!
//! The map is a grid of [`LOT`]-tile cells. Each cell is one of a handful of models -
//! a lot, or a piece of street (straight, corner, T, crossing, cul-de-sac) - and two
//! sockets decide what may sit next to what: a `road` edge meets a `road` edge, a
//! `verge` edge a `verge` edge. So streets always join up, and lots sit beside them.
//! The middle cell is pinned to a crossing so there is always a street to start on.
//!
//! The layout is then drawn: asphalt (`Rock`) down the middle of a street cell, paving
//! (`Floor`) along it, lawn (`Grass`) on the lots. A few lots that face a street get a
//! house (`MapFile::houses`, baked into walls when the level loads), its long side along
//! the street and its yard on the side its door opens to (a front yard, or a back garden),
//! with one or two neighbours standing in it: placements of the people defs in
//! [`PEOPLE_DIR`], which the game spawns as `infestation::Neighbour`s. The other lots are
//! gardens with trees and bushes. A wall runs round the edge.
//!
//! [`generate`] is pure and seeded; [`generate_from_assets`] reads the houses and people
//! it may use from `assets/`.

// Grid/procgen math: coordinates and offsets here are structurally bounded by
// loop ranges and lot dimensions checked elsewhere, so the blanket
// arithmetic/indexing lints mostly flag noise in this module. Scoped allow;
// genuinely risky spots are fixed individually.
#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use ghx_proc_gen::generator::RngMode;
use ghx_proc_gen::generator::builder::GeneratorBuilder;
use ghx_proc_gen::generator::model::{ModelCollection, ModelIndex, ModelRotation};
use ghx_proc_gen::generator::rules::RulesBuilder;
use ghx_proc_gen::generator::socket::{SocketCollection, SocketsCartesian2D};
use ghx_proc_gen::ghx_grid::cartesian::coordinates::{Cartesian2D, CartesianPosition};
use ghx_proc_gen::ghx_grid::cartesian::grid::CartesianGrid;

use crate::general::components::map_components::{
    DecorationItem, HousePlacement, MapFile, TilePlacement,
};
use crate::map::MapFeatures;
use crate::map::map_generator::{BUSHES, TREES};

/// Tiles per side of a cell.
pub const LOT: usize = 10;
/// The asphalt band down a street cell: tiles `ROAD_FROM..ROAD_TO` across it.
const ROAD_FROM: usize = 3;
const ROAD_TO: usize = 7;
/// How many houses a neighbourhood gets, at most and at least (fewer only when there
/// are not enough lots on a street).
pub const HOUSES: (usize, usize) = (4, 8);
/// Layouts tried (seed, seed + 1, ...) before giving up.
const ATTEMPTS: u64 = 24;

/// Street directions as bits, in `ghx_proc_gen`'s side order: x+, y+, x-, y-.
const EAST: u8 = 1;
const SOUTH: u8 = 2; // y+ is down the map
const WEST: u8 = 4;
const NORTH: u8 = 8;

/// A cell of the layout: a lot, or street leaving the cell on the sides in `roads`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Lot,
    Street { roads: u8 },
}

/// A house the generator may place: its name, footprint unturned (tiles), and which side
/// its door ends up on when placed with 0-3 turns (a side bit; 0 = no door). The house
/// resolver picks the door's wall afresh on the turned outline, so turning a house does
/// not simply turn its door.
#[derive(Clone, Debug)]
pub struct HouseSize {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub doors: [u8; 4],
}

impl HouseSize {
    /// The footprint placed with `turns`.
    const fn size(&self, turns: u8) -> (usize, usize) {
        if turns.is_multiple_of(2) {
            (self.width, self.height)
        } else {
            (self.height, self.width)
        }
    }

    fn door(&self, turns: u8) -> u8 {
        self.doors.get(usize::from(turns % 4)).copied().unwrap_or(0)
    }
}

/// Turn a side mask by `quarters` quarter turns, x+ towards y+: counter-clockwise as
/// `ghx_proc_gen` turns a model's sockets, and clockwise on the map (y grows down), as
/// a `HousePlacement` turns a house.
const fn turn(mask: u8, quarters: u8) -> u8 {
    let quarters = quarters % 4;
    ((mask << quarters) | (mask >> (4 - quarters))) & 0b1111
}

const fn rotate(mask: u8, rotation: ModelRotation) -> u8 {
    turn(
        mask,
        match rotation {
            ModelRotation::Rot0 => 0,
            ModelRotation::Rot90 => 1,
            ModelRotation::Rot180 => 2,
            ModelRotation::Rot270 => 3,
        },
    )
}

/// The street layout for a `width` x `height` cell grid, or `None` if model synthesis
/// failed for this seed.
pub fn layout(seed: u64, width: usize, height: usize) -> Option<Vec<Vec<Cell>>> {
    let mut sockets = SocketCollection::new();
    let (road, verge) = (sockets.create(), sockets.create());
    sockets.add_connection(road, vec![road]);
    sockets.add_connection(verge, vec![verge]);

    // (model, its streets unturned). Weights: mostly lots, streets that mostly run on.
    let mut models = ModelCollection::<Cartesian2D>::new();
    let mut street_masks: Vec<(ModelIndex, u8)> = Vec::new();
    let lot = models
        .create(SocketsCartesian2D::Mono(verge))
        .with_weight(9.0_f32)
        .with_name("lot")
        .index();
    let side = |mask: u8, bit: u8| if mask & bit != 0 { road } else { verge };
    let streets: [(&'static str, u8, f32); 5] = [
        ("straight", EAST | WEST, 1.6),
        ("corner", EAST | SOUTH, 0.5),
        ("t-junction", EAST | WEST | SOUTH, 0.35),
        ("crossing", EAST | WEST | SOUTH | NORTH, 0.15),
        ("cul-de-sac", EAST, 0.25),
    ];
    for (name, mask, weight) in streets {
        let index = models
            .create(SocketsCartesian2D::Simple {
                x_pos: side(mask, EAST),
                y_pos: side(mask, SOUTH),
                x_neg: side(mask, WEST),
                y_neg: side(mask, NORTH),
            })
            .with_all_rotations()
            .with_weight(weight)
            .with_name(name)
            .index();
        street_masks.push((index, mask));
    }
    let crossing = street_masks.get(3).map(|&(index, _)| index)?;

    let rules = RulesBuilder::new_cartesian_2d(models, sockets)
        .build()
        .ok()?;
    let grid = CartesianGrid::new_cartesian_2d(
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        false,
        false,
    );
    let middle = CartesianPosition::new_xy(
        u32::try_from(width / 2).ok()?,
        u32::try_from(height / 2).ok()?,
    );
    let mut generator = GeneratorBuilder::new()
        .with_rules(rules)
        .with_grid(grid)
        .with_rng(RngMode::Seeded(seed))
        .with_max_retry_count(20)
        .with_initial_nodes(vec![(middle, (crossing, ModelRotation::Rot0))])
        .ok()?
        .build()
        .ok()?;
    let (_, data) = generator.generate_grid().ok()?;

    let mut cells = vec![vec![Cell::Lot; width]; height];
    for (y, row) in cells.iter_mut().enumerate() {
        for (x, cell) in row.iter_mut().enumerate() {
            let instance = data.get_2d(x as u32, y as u32);
            if instance.model_index == lot {
                continue;
            }
            if let Some(&(_, mask)) = street_masks
                .iter()
                .find(|(index, _)| *index == instance.model_index)
            {
                *cell = Cell::Street {
                    roads: rotate(mask, instance.rotation),
                };
            }
        }
    }
    Some(cells)
}

// ── Seeded RNG (xorshift64, matching the other map modules) ───────────────────

struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15))
    }

    const fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// `lo..hi`; `lo` when the range is empty.
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() as usize % (hi - lo))
    }

    fn chance(&mut self, p: f32) -> bool {
        ((self.next() >> 40) as f32 / (1u64 << 24) as f32) < p
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.range(0, items.len()))
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.range(0, i + 1));
        }
    }
}

fn flags(features: &[MapFeatures]) -> u64 {
    features.iter().fold(0, |acc, f| acc | *f as u64)
}

/// A neighbourhood of `lots_x` x `lots_y` cells, with houses from `houses` and the
/// people in the yards drawn from `characters` (def paths). Tries a few seeds from `seed`
/// until the layout has a street and a lot on it for every house it wants; `None` only if
/// none of them do.
pub fn generate(
    seed: u64,
    lots_x: usize,
    lots_y: usize,
    houses: &[HouseSize],
    characters: &[String],
) -> Option<MapFile> {
    let lots_x = lots_x.max(3);
    let lots_y = lots_y.max(3);
    let houses: Vec<&HouseSize> = houses
        .iter()
        .filter(|h| h.width.max(h.height) <= LOT - 2 && h.width.min(h.height) <= LOT - 5)
        .collect();
    (0..ATTEMPTS).find_map(|attempt| {
        let seed = seed.wrapping_add(attempt);
        let cells = layout(seed, lots_x, lots_y)?;
        draw(seed, &cells, &houses, characters)
    })
}

/// The layout drawn into a map; `None` when it has too few lots on a street.
fn draw(
    seed: u64,
    cells: &[Vec<Cell>],
    houses: &[&HouseSize],
    characters: &[String],
) -> Option<MapFile> {
    let mut rng = Rng::new(seed ^ 0x005E_ED0F_5B0B);
    let lots_y = cells.len();
    let lots_x = cells.first()?.len();
    let (width, height) = (lots_x * LOT + 2, lots_y * LOT + 2);

    let grass = flags(&[MapFeatures::Grass]);
    let asphalt = flags(&[MapFeatures::Rock]);
    let paving = flags(&[MapFeatures::Floor]);
    let wall = flags(&[
        MapFeatures::Floor,
        MapFeatures::ImpassableForPlayers,
        MapFeatures::ImpassableForEnemies,
    ]);
    let mut tiles = vec![vec![grass; width]; height];

    // Streets: asphalt from the middle of the cell out to each side it leaves by.
    let cell_at = |cx: usize, cy: usize| cells.get(cy).and_then(|row| row.get(cx)).copied();
    for (cy, row) in cells.iter().enumerate() {
        for (cx, cell) in row.iter().enumerate() {
            let Cell::Street { roads } = *cell else {
                continue;
            };
            let (ox, oy) = (1 + cx * LOT, 1 + cy * LOT);
            let mut lay = |x0: usize, x1: usize, y0: usize, y1: usize| {
                for y in y0..y1 {
                    for x in x0..x1 {
                        tiles[oy + y][ox + x] = asphalt;
                    }
                }
            };
            lay(ROAD_FROM, ROAD_TO, ROAD_FROM, ROAD_TO);
            if roads & EAST != 0 {
                lay(ROAD_FROM, LOT, ROAD_FROM, ROAD_TO);
            }
            if roads & WEST != 0 {
                lay(0, ROAD_TO, ROAD_FROM, ROAD_TO);
            }
            if roads & SOUTH != 0 {
                lay(ROAD_FROM, ROAD_TO, ROAD_FROM, LOT);
            }
            if roads & NORTH != 0 {
                lay(ROAD_FROM, ROAD_TO, 0, ROAD_TO);
            }
        }
    }
    // Paving: a tile wide along every street.
    let road_tiles: Vec<(usize, usize)> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| tiles[y][x] == asphalt)
        .collect();
    for &(x, y) in &road_tiles {
        for (dx, dy) in [
            (-1i32, -1i32),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 1 || ny < 1 || nx >= width as i32 - 1 || ny >= height as i32 - 1 {
                continue;
            }
            let tile = &mut tiles[ny as usize][nx as usize];
            if *tile == grass {
                *tile = paving;
            }
        }
    }

    // The edge.
    for (y, row) in tiles.iter_mut().enumerate() {
        for (x, tile) in row.iter_mut().enumerate() {
            if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                *tile = wall;
            }
        }
    }

    // The players start in the middle of the crossing.
    let (mx, my) = (lots_x / 2, lots_y / 2);
    let spawn = (1 + mx * LOT + LOT / 2, 1 + my * LOT + LOT / 2);
    tiles[spawn.1][spawn.0] = flags(&[MapFeatures::Rock, MapFeatures::PlayerSpawn]);

    // Lots on a street, and which way the street is.
    let mut fronting: Vec<((usize, usize), u8)> = Vec::new();
    for (cy, row) in cells.iter().enumerate() {
        for (cx, cell) in row.iter().enumerate() {
            if *cell != Cell::Lot {
                continue;
            }
            let mut sides = Vec::new();
            if cx + 1 < lots_x && matches!(cell_at(cx + 1, cy), Some(Cell::Street { .. })) {
                sides.push(EAST);
            }
            if cx > 0 && matches!(cell_at(cx - 1, cy), Some(Cell::Street { .. })) {
                sides.push(WEST);
            }
            if cy + 1 < lots_y && matches!(cell_at(cx, cy + 1), Some(Cell::Street { .. })) {
                sides.push(SOUTH);
            }
            if cy > 0 && matches!(cell_at(cx, cy - 1), Some(Cell::Street { .. })) {
                sides.push(NORTH);
            }
            if let Some(&side) = rng.pick(&sides) {
                fronting.push(((cx, cy), side));
            }
        }
    }
    let wanted = rng.range(HOUSES.0, HOUSES.1 + 1);
    if houses.is_empty() || fronting.len() < HOUSES.0.min(wanted) {
        return None;
    }
    rng.shuffle(&mut fronting);
    let built: Vec<((usize, usize), u8)> = fronting.iter().take(wanted).copied().collect();

    let mut map_houses = Vec::new();
    let mut placements = Vec::new();
    let mut decorations = Vec::new();
    for &((cx, cy), front) in &built {
        let Some(house) = rng.pick(houses) else {
            continue;
        };
        // Map coordinates: one tile in for the wall round the edge.
        let (ox, oy) = ((1 + cx * LOT) as i32, (1 + cy * LOT) as i32);
        // The house's long side along the street, turned so its door opens onto the
        // street (a front yard) or away from it (a back garden). The yard is on the door's
        // side and the house stands against the other.
        let back = turn(front, 2);
        let across = front == NORTH || front == SOUTH;
        let lengthwise = |t: &u8| {
            let (w, h) = house.size(*t);
            if across { w >= h } else { h >= w }
        };
        let turns = (0..4u8)
            .filter(lengthwise)
            .find(|&t| house.door(t) == front || house.door(t) == back)
            .or_else(|| (0..4u8).find(lengthwise))
            .unwrap_or(0);
        let yard_side = if house.door(turns) == back {
            back
        } else {
            front
        };
        let (w, h) = house.size(turns);
        let (w, h) = (w as i32, h as i32);
        let lot = LOT as i32;
        let (hx, hy) = match yard_side {
            NORTH => ((lot - w) / 2, lot - 1 - h),
            SOUTH => ((lot - w) / 2, 1),
            WEST => (lot - 1 - w, (lot - h) / 2),
            _ => (1, (lot - h) / 2),
        };
        map_houses.push(HousePlacement {
            house: house.name.clone(),
            x: ox + hx,
            y: oy + hy,
            turns,
        });
        // The yard: the lot's tiles on the door's side of the house.
        let yard: Vec<(i32, i32)> = (1..lot - 1)
            .flat_map(|y| (1..lot - 1).map(move |x| (x, y)))
            .filter(|&(x, y)| match yard_side {
                NORTH => y < hy - 1,
                SOUTH => y > hy + h,
                WEST => x < hx - 1,
                _ => x > hx + w,
            })
            .collect();
        let people = rng.range(1, 3);
        let mut spots = yard.clone();
        rng.shuffle(&mut spots);
        for &(x, y) in spots.iter().take(people) {
            let Some(def) = rng.pick(characters) else {
                break;
            };
            placements.push(TilePlacement {
                x: ox + x,
                y: oy + y,
                def_path: def.clone(),
                rotation_steps: rng.range(0, 8) as u8,
            });
        }
        if let Some(&(x, y)) = spots.get(people) {
            let (model, scale) = BUSHES[rng.range(0, BUSHES.len())];
            decorations.push(decoration(ox + x, oy + y, model, scale, &mut rng));
        }
    }

    // Gardens: every other lot, trees round the edge and bushes in between.
    for (cy, row) in cells.iter().enumerate() {
        for (cx, cell) in row.iter().enumerate() {
            if *cell != Cell::Lot || built.iter().any(|&(at, _)| at == (cx, cy)) {
                continue;
            }
            let (ox, oy) = ((1 + cx * LOT) as i32, (1 + cy * LOT) as i32);
            for _ in 0..rng.range(2, 5) {
                let (x, y) = (rng.range(1, LOT - 1) as i32, rng.range(1, LOT - 1) as i32);
                let (model, scale) = TREES[rng.range(0, TREES.len())];
                decorations.push(decoration(ox + x, oy + y, model, scale, &mut rng));
            }
            for _ in 0..rng.range(1, 4) {
                let (x, y) = (rng.range(1, LOT - 1) as i32, rng.range(1, LOT - 1) as i32);
                let (model, scale) = BUSHES[rng.range(0, BUSHES.len())];
                decorations.push(decoration(ox + x, oy + y, model, scale, &mut rng));
            }
        }
    }
    // A tree here and there along the paving.
    for (y, row) in tiles.iter().enumerate() {
        for (x, &tile) in row.iter().enumerate() {
            if tile == paving && rng.chance(0.015) {
                let (model, scale) = TREES[rng.range(0, TREES.len())];
                decorations.push(decoration(x as i32, y as i32, model, scale, &mut rng));
            }
        }
    }

    Some(MapFile {
        seed,
        map_width: width,
        map_height: height,
        tiles,
        decorations,
        placements,
        houses: map_houses,
        ..MapFile::default()
    })
}

fn decoration(x: i32, y: i32, model: &str, scale: f32, rng: &mut Rng) -> DecorationItem {
    DecorationItem {
        x,
        y,
        model: model.to_string(),
        rotation_y: rng.range(0, 8) as f32 * 45.0,
        scale,
    }
}

/// The houses that fit a lot, from `assets/houses`.
pub fn house_sizes() -> Vec<HouseSize> {
    crate::map::house::house_library()
        .into_iter()
        .filter_map(|(name, house)| {
            // Inclusive cell bounds.
            let (x0, y0, x1, y1) = house.polygon().bounds()?;
            let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut doors = [0u8; 4];
            for (turns, door) in (0u8..).zip(doors.iter_mut()) {
                let (w, h) = if turns % 2 == 0 {
                    (width, height)
                } else {
                    (height, width)
                };
                let placement = crate::general::components::map_components::HousePlacement {
                    house: name.clone(),
                    x: 0,
                    y: 0,
                    turns,
                };
                *door = house
                    .resolve_placed(&placement, "")
                    .wall_cells
                    .iter()
                    .find(|c| matches!(c.kind, crate::map::procgen::WallCellKind::Door))
                    .map_or(0, |c| match (c.x, c.y) {
                        (_, 0) => NORTH,
                        (_, y) if y == h - 1 => SOUTH,
                        (0, _) => WEST,
                        (x, _) if x == w - 1 => EAST,
                        _ => 0,
                    });
            }
            Some(HouseSize {
                name,
                width: usize::try_from(width).ok()?,
                height: usize::try_from(height).ok()?,
                doors,
            })
        })
        .collect()
}

/// Where the neighbours' defs are: character defs kept apart from `assets/defs` so they
/// are not offered as player characters.
pub const PEOPLE_DIR: &str = "assets/defs/people";

/// The neighbours' defs (`PEOPLE_DIR`), as content paths: who lives here.
pub fn people_defs() -> Vec<String> {
    use crate::assets::asset_definition::{AssetDefinition, ModelType};
    let mut defs: Vec<String> = std::fs::read_dir(PEOPLE_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "ron"))
        .filter_map(|path| {
            let content = crate::assets::paths::content_ref(path.to_str()?);
            let def = AssetDefinition::load_from_def_path(&content)?;
            matches!(def.model_type, ModelType::Player(_)).then_some(content)
        })
        .collect();
    defs.sort();
    defs
}

/// [`generate`] with the houses and characters in `assets/`, for a map of about
/// `width` x `height` tiles.
pub fn generate_from_assets(seed: u64, width: usize, height: usize) -> Option<MapFile> {
    generate(
        seed,
        width.saturating_sub(2) / LOT,
        height.saturating_sub(2) / LOT,
        &house_sizes(),
        &people_defs(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bungalow() -> Vec<HouseSize> {
        vec![HouseSize {
            name: "bungalow".into(),
            width: 8,
            height: 5,
            doors: [NORTH, EAST, NORTH, EAST],
        }]
    }

    #[test]
    fn a_side_mask_turns_counter_clockwise() {
        assert_eq!(rotate(EAST, ModelRotation::Rot90), SOUTH);
        assert_eq!(rotate(EAST, ModelRotation::Rot180), WEST);
        assert_eq!(rotate(EAST | SOUTH, ModelRotation::Rot270), NORTH | EAST);
    }

    /// The sockets hold: a street that leaves a cell enters the next one, and nothing
    /// runs into a lot.
    #[test]
    fn streets_join_up() {
        for seed in 0..20 {
            let Some(cells) = layout(seed, 6, 4) else {
                continue;
            };
            let roads = |x: usize, y: usize| match cells[y][x] {
                Cell::Lot => 0,
                Cell::Street { roads } => roads,
            };
            for y in 0..4 {
                for x in 0..6 {
                    if x + 1 < 6 {
                        assert_eq!(roads(x, y) & EAST != 0, roads(x + 1, y) & WEST != 0);
                    }
                    if y + 1 < 4 {
                        assert_eq!(roads(x, y) & SOUTH != 0, roads(x, y + 1) & NORTH != 0);
                    }
                }
            }
            assert_eq!(
                cells[2][3],
                Cell::Street { roads: 0b1111 },
                "the pinned crossing"
            );
        }
    }

    #[test]
    fn a_neighbourhood_has_houses_people_a_spawn_and_a_wall_round_it() {
        let characters = vec!["defs/male-simple.ron".to_string()];
        for seed in 0..10 {
            let map = generate(seed, 6, 4, &bungalow(), &characters).expect("a neighbourhood");
            assert_eq!((map.map_width, map.map_height), (62, 42));
            assert!(
                (HOUSES.0..=HOUSES.1).contains(&map.houses.len()),
                "{}",
                map.houses.len()
            );
            assert!(
                map.placements.len() >= map.houses.len(),
                "someone in every yard"
            );
            let spawn = MapFeatures::PlayerSpawn as u64;
            assert_eq!(
                map.tiles
                    .iter()
                    .flatten()
                    .filter(|&&t| t & spawn != 0)
                    .count(),
                1
            );
            let blocks = MapFeatures::ImpassableForPlayers as u64;
            for (y, row) in map.tiles.iter().enumerate() {
                for (x, &tile) in row.iter().enumerate() {
                    let edge = x == 0 || y == 0 || x == 61 || y == 41;
                    assert_eq!(edge, tile & blocks != 0, "({x}, {y})");
                }
            }
            // Houses and people stay off the wall.
            for house in &map.houses {
                let (w, h) = if house.turns % 2 == 0 { (8, 5) } else { (5, 8) };
                assert!(house.x >= 1 && house.y >= 1, "{house:?}");
                assert!(house.x + w <= 61 && house.y + h <= 41, "{house:?}");
            }
            for person in &map.placements {
                assert!((1..61).contains(&person.x) && (1..41).contains(&person.y));
            }
        }
    }

    /// Once the houses are baked in, every open tile can still be walked to from the
    /// spawn: no yard or garden walled off.
    #[test]
    fn every_open_tile_is_reachable_once_the_houses_stand() {
        let houses = crate::map::house::house_library();
        let characters = vec!["defs/male-simple.ron".to_string()];
        for seed in 0..10 {
            let mut map = generate(seed, 6, 4, &house_sizes(), &characters).expect("map");
            let errors = crate::map::house::bake_houses(&mut map, "", |name| {
                houses.get(name).cloned().ok_or_else(|| name.to_string())
            });
            assert!(errors.is_empty(), "{errors:?}");
            let blocks = MapFeatures::ImpassableForPlayers as u64;
            let open = |x: usize, y: usize| map.tiles[y][x] & blocks == 0;
            let spawn_bit = MapFeatures::PlayerSpawn as u64;
            let start = (0..map.map_height)
                .flat_map(|y| (0..map.map_width).map(move |x| (x, y)))
                .find(|&(x, y)| map.tiles[y][x] & spawn_bit != 0)
                .expect("spawn");
            let mut seen = vec![vec![false; map.map_width]; map.map_height];
            let mut queue = vec![start];
            seen[start.1][start.0] = true;
            while let Some((x, y)) = queue.pop() {
                for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    if open(nx, ny) && !seen[ny][nx] {
                        seen[ny][nx] = true;
                        queue.push((nx, ny));
                    }
                }
            }
            let stranded = (0..map.map_height)
                .flat_map(|y| (0..map.map_width).map(move |x| (x, y)))
                .filter(|&(x, y)| open(x, y) && !seen[y][x])
                .count();
            // A house's own rooms count as open; a house with its door to the wall
            // could leave a room shut, which is fine. Yards and gardens never are.
            assert!(stranded < 40, "seed {seed}: {stranded} tiles walled off");
        }
    }

    /// A look at a layout: `cargo test print_a_neighbourhood -- --ignored --nocapture`.
    /// `#` wall, `=` street, `.` paving, `,` lawn, `@` the spawn, `p` a neighbour.
    #[test]
    #[ignore = "prints a map to look at"]
    fn print_a_neighbourhood() {
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7);
        let mut map = generate_from_assets(seed, 62, 42).expect("map");
        let houses = crate::map::house::house_library();
        crate::map::house::bake_houses(&mut map, "", |name| {
            houses.get(name).cloned().ok_or_else(|| name.to_string())
        });
        for (y, row) in map.tiles.iter().enumerate() {
            let line: String = row
                .iter()
                .enumerate()
                .map(|(x, &t)| {
                    let has = |f: MapFeatures| t & f as u64 != 0;
                    if map
                        .placements
                        .iter()
                        .any(|p| (p.x, p.y) == (x as i32, y as i32))
                    {
                        'p'
                    } else if has(MapFeatures::PlayerSpawn) {
                        '@'
                    } else if has(MapFeatures::ImpassableForPlayers) {
                        '#'
                    } else if has(MapFeatures::Rock) {
                        '='
                    } else if has(MapFeatures::Grass) {
                        ','
                    } else {
                        '.'
                    }
                })
                .collect();
            println!("{line}");
        }
        println!(
            "{} houses, {} people",
            map.houses.len(),
            map.placements.len()
        );
        for house in house_sizes() {
            println!("{house:?}");
        }
    }
}
