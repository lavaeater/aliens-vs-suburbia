//! Grass on the lawns, from `bevy_grass`: blades built on the GPU near the camera each frame,
//! swaying in the wind and pressed flat where people walk.
//!
//! - **Where**: [`LawnMask`] (pure, tested) - grass tiles nothing blocks, at half a tile, with
//!   the roads and their pavements (`road_mesh::half_width`) cut out. Water, mud, snow, rock,
//!   floors, walls, hedges and trees' own tiles stay bare.
//! - **On the hills**: each field lies on [`GroundHeight`], sampled once on load.
//! - **Darker low, lighter high**: the lawn is split into [`BANDS`] altitude bands, one field
//!   each (their masks don't overlap), tinted by `floor_mesh::altitude_tint` at the band's
//!   middle, so the blades follow the floor's own height tint. A flat map has one.
//! - **Trampling**: players, aliens and neighbours carry a [`GrassInteractor`].
//! - **The camera**: the game camera gets a [`GrassCamera`] whose blades are at least a pixel
//!   wide (it renders into the retro look's small image).
//!
//! **Ctrl+Y** turns it on and off ([`GrassOn`]); `--no-grass` starts with it off. Planted on
//! every `LoadMap`, after `streaming::plan_map_stream` has set the ground; swept with the rest
//! of a match on leaving it.

use bevy::prelude::*;
use bevy_grass::prelude::*;

use crate::alien::components::general::Alien;
use crate::camera::components::GameCamera;
use crate::game_state::GameState;
use crate::general::components::map_components::RoadLine;
use crate::general::events::map_events::LoadMap;
use crate::general::systems::map_systems::TileDefinitions;
use crate::infestation::Neighbour;
use crate::map::floor_mesh::altitude_tint;
use crate::map::ground::GroundHeight;
use crate::map::road_mesh::half_width;
use crate::map::walls::padded_tiles;
use crate::map::{BitFlags, MapFeatures};
use crate::player::components::Player;

/// Mask cells per tile, along each side.
const CELLS_PER_TILE: usize = 2;
/// Altitude bands on a hilly level.
pub const BANDS: usize = 5;
/// How wide (metres) a trampling foot is, and how flat it presses.
const FOOT_RADIUS: f32 = 0.45;
const FOOT_STRENGTH: f32 = 0.85;

/// Whether the grass shows (Ctrl+Y, `--no-grass`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrassOn(pub bool);

impl Default for GrassOn {
    fn default() -> Self {
        Self(true)
    }
}

/// A field of the current level's grass.
#[derive(Component)]
pub struct LevelGrass;

/// The suburban lawn: short, dense enough to read from the camera, greens close to the
/// floor's own grass colour (`floor_mesh::terrain_color`).
pub fn lawn_style() -> GrassStyle {
    GrassStyle {
        blade: GrassBlade {
            height: Vec2::new(0.08, 0.20),
            width: 0.045,
            root_color: Color::srgb(0.10, 0.26, 0.07),
            tip_color: Color::srgb(0.42, 0.72, 0.28),
            translucency: 0.6,
            ..GrassStyle::LAWN.blade
        },
        density: 160.0,
        ..GrassStyle::LAWN
    }
}

/// Whether grass grows on a tile: grass, no other terrain over it, nothing blocking it.
pub fn is_lawn(tile: u64) -> bool {
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(tile);
    let other_ground = MapFeatures::Water | MapFeatures::Mud | MapFeatures::Snow | MapFeatures::Rock;
    let blocked = MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies;
    flags.contains(MapFeatures::Grass) && !flags.intersects(other_ground) && !flags.intersects(blocked)
}

/// Where the lawn is, over the padded grid, [`CELLS_PER_TILE`] cells a tile side. Tile
/// `(c, r)` is centred on world `tile * (c, r)`, so the grid starts half a tile west and north
/// of the origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawnMask {
    cols: usize,
    rows: usize,
    cells: Vec<bool>,
    /// World size of a tile.
    tile_milli: u32,
}

/// Distance from `p` to the segment `a`-`b`.
fn to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > f32::EPSILON {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

impl LawnMask {
    /// The lawn of a padded grid (`walls::padded_tiles`) and its roads (map coordinates).
    pub fn plan(tiles: &[Vec<u64>], roads: &[RoadLine], tile: f32) -> Self {
        let rows = tiles.len() * CELLS_PER_TILE;
        let cols = tiles.iter().map(Vec::len).max().unwrap_or(0) * CELLS_PER_TILE;
        let mut cells = vec![false; cols * rows];
        for (r, row) in tiles.iter().enumerate() {
            for (c, &t) in row.iter().enumerate() {
                if !is_lawn(t) {
                    continue;
                }
                for dr in 0..CELLS_PER_TILE {
                    for dc in 0..CELLS_PER_TILE {
                        let (x, y) = (c * CELLS_PER_TILE + dc, r * CELLS_PER_TILE + dr);
                        if let Some(cell) = cells.get_mut(y * cols + x) {
                            *cell = true;
                        }
                    }
                }
            }
        }
        let mut mask = Self {
            cols,
            rows,
            cells,
            tile_milli: (tile * 1000.0).round() as u32,
        };
        for road in roads {
            let reach = half_width(road, tile);
            let line: Vec<Vec2> = road
                .points
                .iter()
                .map(|p| Vec2::new(tile * (p[0] + 1.0), tile * (p[1] + 1.0)))
                .collect();
            for pair in line.windows(2) {
                if let [a, b] = *pair {
                    mask.clear_near(a, b, reach);
                }
            }
            if let [only] = line.as_slice() {
                mask.clear_near(*only, *only, reach);
            }
        }
        mask
    }

    fn tile(&self) -> f32 {
        self.tile_milli as f32 / 1000.0
    }

    fn cell(&self) -> f32 {
        self.tile() / CELLS_PER_TILE as f32
    }

    /// The world point at the centre of cell `(x, y)`.
    fn centre(&self, x: usize, y: usize) -> Vec2 {
        let origin = Vec2::splat(-self.tile() / 2.0);
        origin + (Vec2::new(x as f32, y as f32) + 0.5) * self.cell()
    }

    /// The cell a world point is in.
    fn index(&self, world: Vec2) -> Option<usize> {
        let at = (world + Vec2::splat(self.tile() / 2.0)) / self.cell();
        if at.x < 0.0 || at.y < 0.0 {
            return None;
        }
        let (x, y) = (at.x as usize, at.y as usize);
        (x < self.cols && y < self.rows).then_some(y * self.cols + x)
    }

    /// Bare every cell whose centre lies within `reach` of the segment `a`-`b`.
    fn clear_near(&mut self, a: Vec2, b: Vec2, reach: f32) {
        let cell = self.cell();
        let lo = a.min(b) - Vec2::splat(reach);
        let hi = a.max(b) + Vec2::splat(reach);
        let span = |v: f32, n: usize| {
            let first = ((v + self.tile() / 2.0) / cell).floor().max(0.0) as usize;
            first.min(n)
        };
        let (x0, x1) = (span(lo.x, self.cols), (span(hi.x, self.cols) + 1).min(self.cols));
        let (y0, y1) = (span(lo.y, self.rows), (span(hi.y, self.rows) + 1).min(self.rows));
        for y in y0..y1 {
            for x in x0..x1 {
                if to_segment(self.centre(x, y), a, b) <= reach
                    && let Some(c) = self.cells.get_mut(y * self.cols + x)
                {
                    *c = false;
                }
            }
        }
    }

    /// Whether grass grows at a world point.
    pub fn at(&self, world: Vec2) -> bool {
        self.index(world)
            .and_then(|i| self.cells.get(i))
            .copied()
            .unwrap_or(false)
    }

    /// Lawn cells in all.
    pub fn count(&self) -> usize {
        self.cells.iter().filter(|c| **c).count()
    }

    /// The world rectangle the grid covers: its centre and size.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let size = Vec2::new(self.cols as f32, self.rows as f32) * self.cell();
        (size / 2.0 - Vec2::splat(self.tile() / 2.0), size)
    }

    /// The centres of the lawn cells.
    pub fn lawn(&self) -> impl Iterator<Item = Vec2> + '_ {
        (0..self.rows).flat_map(move |y| {
            (0..self.cols)
                .filter(move |&x| self.cells.get(y * self.cols + x).copied().unwrap_or(false))
                .map(move |x| self.centre(x, y))
        })
    }
}

/// How many bands a level with ground `range` gets: one when flat.
pub fn band_count(range: (f32, f32)) -> usize {
    if range.1 - range.0 > f32::EPSILON { BANDS } else { 1 }
}

/// The band of `height`, `0..bands`.
pub fn band_of(height: f32, (lowest, highest): (f32, f32), bands: usize) -> usize {
    let span = highest - lowest;
    if span <= f32::EPSILON || bands <= 1 {
        return 0;
    }
    let at = ((height - lowest) / span).clamp(0.0, 1.0);
    ((at * bands as f32) as usize).min(bands - 1)
}

/// The tint of band `band`: the floor's altitude tint at its middle.
pub fn band_tint(band: usize, range: (f32, f32), bands: usize) -> f32 {
    let (lowest, highest) = range;
    let middle = lowest + (highest - lowest) * (band as f32 + 0.5) / bands.max(1) as f32;
    altitude_tint(middle, range)
}

pub struct LawnGrassPlugin {
    /// Whether the grass starts on (`--no-grass` turns it off).
    pub on: bool,
}

impl Plugin for LawnGrassPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GrassPlugins.build().disable::<FarGrassPlugin>())
            .insert_resource(GrassSettings {
                // The moon casts no shadows, and the lamps' shadows don't take grass.
                shadows: GrassShadows::Off,
                ..GrassSettings::HIGH
            })
            .insert_resource(GrassOn(self.on))
            .add_observer(grass_camera)
            .add_observer(tread::<Player>)
            .add_observer(tread::<Alien>)
            .add_observer(tread::<Neighbour>)
            .add_systems(
                Update,
                (
                    plant_grass.after(crate::map::streaming::plan_map_stream),
                    toggle_grass,
                    show_grass,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

fn grass_camera(add: On<Add<GameCamera>>, mut commands: Commands) {
    commands.entity(add.event().entity).insert(GrassCamera {
        min_width_px: Some(1.5),
        ..default()
    });
}

fn tread<T: Component>(add: On<Add<T>>, mut commands: Commands) {
    commands.entity(add.event().entity).try_insert(GrassInteractor {
        radius: FOOT_RADIUS,
        strength: FOOT_STRENGTH,
        ..default()
    });
}

fn visibility(on: GrassOn) -> Visibility {
    if on.0 { Visibility::Inherited } else { Visibility::Hidden }
}

/// Plant a level's grass when its map loads (the old fields go with the old map).
#[allow(clippy::too_many_arguments)]
fn plant_grass(
    mut loads: MessageReader<LoadMap>,
    mut commands: Commands,
    old: Query<Entity, With<LevelGrass>>,
    ground: Res<GroundHeight>,
    tile_defs: Res<TileDefinitions>,
    on: Res<GrassOn>,
    mut styles: ResMut<Assets<GrassStyle>>,
    mut images: ResMut<Assets<Image>>,
) {
    for load in loads.read() {
        for field in &old {
            commands.entity(field).despawn();
        }
        let tile = tile_defs.tile_width;
        let mask = LawnMask::plan(&padded_tiles(&load.map), &load.map.roads, tile);
        if mask.count() == 0 {
            continue;
        }
        let (centre, size) = mask.bounds();
        let range = ground.range();
        let bands = band_count(range);
        let height = |p: Vec2| ground.height(centre.x + p.x, centre.y + p.y);
        let mut in_band = vec![0usize; bands];
        for at in mask.lawn() {
            if let Some(n) = in_band.get_mut(band_of(ground.height(at.x, at.y), range, bands)) {
                *n += 1;
            }
        }
        let style = styles.add(lawn_style());
        let cell = tile / CELLS_PER_TILE as f32;
        let mut ground_image: Option<GrassGround> = None;
        let mut fields = 0;
        for (band, _) in in_band.iter().enumerate().filter(|(_, n)| **n > 0) {
            let field = GrassField {
                size,
                style: style.clone(),
                ..default()
            };
            let field = match &ground_image {
                Some(shared) => GrassField {
                    ground: shared.clone(),
                    ..field
                },
                None => field.with_ground_from_fn(&mut images, cell, height),
            };
            ground_image.get_or_insert_with(|| field.ground.clone());
            let field = field.with_density_from_fn(&mut images, cell, |p| {
                let world = centre + p;
                let here = ground.height(world.x, world.y);
                f32::from(u8::from(mask.at(world) && band_of(here, range, bands) == band))
            });
            let tint = band_tint(band, range, bands);
            commands.spawn((
                Name::new(format!("Lawn grass {}/{bands}", band + 1)),
                LevelGrass,
                field,
                GrassFieldOverrides {
                    tint: (bands > 1).then(|| Color::linear_rgb(tint, tint, tint)),
                    ..default()
                },
                Transform::from_xyz(centre.x, tile_defs.floor_level, centre.y),
                visibility(*on),
            ));
            fields += 1;
        }
        info!(
            "grass: {} m2 of lawn in {fields} field(s), {} x {} m",
            mask.count() as f32 * cell * cell,
            size.x,
            size.y
        );
    }
}

fn toggle_grass(keys: Res<ButtonInput<KeyCode>>, mut on: ResMut<GrassOn>) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        on.0 = !on.0;
        info!("grass: {}", if on.0 { "on" } else { "off" });
    }
}

fn show_grass(on: Res<GrassOn>, mut fields: Query<&mut Visibility, With<LevelGrass>>) {
    if !on.is_changed() {
        return;
    }
    for mut v in &mut fields {
        *v = visibility(*on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::RoadKind;

    fn bits(f: impl Into<BitFlags<MapFeatures>>) -> u64 {
        f.into().bits()
    }

    #[test]
    fn grass_grows_on_open_grass_only() {
        assert!(is_lawn(bits(MapFeatures::Grass)));
        assert!(!is_lawn(bits(MapFeatures::Floor)));
        assert!(!is_lawn(bits(MapFeatures::Grass | MapFeatures::Water)));
        assert!(!is_lawn(bits(MapFeatures::Grass | MapFeatures::Rock)));
        assert!(!is_lawn(bits(MapFeatures::Grass | MapFeatures::ImpassableForPlayers)));
        assert!(!is_lawn(bits(MapFeatures::Nothing)));
    }

    /// 10 x 6 of lawn, a wall at map (2, 2), a street along map row 4.
    fn mask() -> LawnMask {
        let grass = bits(MapFeatures::Grass);
        let wall = bits(MapFeatures::Grass | MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies);
        let mut tiles = vec![vec![grass; 12]; 8];
        if let Some(t) = tiles.get_mut(3).and_then(|row| row.get_mut(3)) {
            *t = wall; // map (2, 2), padded (3, 3)
        }
        let road = RoadLine {
            kind: RoadKind::Street,
            width: 1.0,
            points: vec![[0.0, 4.0], [9.0, 4.0]],
        };
        LawnMask::plan(&tiles, &[road], 1.0)
    }

    #[test]
    fn the_mask_follows_the_tiles_and_leaves_the_road_bare() {
        let mask = mask();
        assert!(mask.at(Vec2::new(1.0, 1.0)), "open lawn");
        assert!(mask.at(Vec2::new(1.3, 0.8)), "anywhere on the tile");
        assert!(!mask.at(Vec2::new(3.0, 3.0)), "the wall");
        // The street's centreline is padded row 5; 1 wide plus pavement is 0.72 either side.
        assert!(!mask.at(Vec2::new(4.0, 5.0)), "on the road");
        assert!(!mask.at(Vec2::new(4.0, 5.4)), "on the pavement");
        assert!(mask.at(Vec2::new(4.0, 6.5)), "beyond it");
        assert!(!mask.at(Vec2::new(-5.0, 1.0)), "off the map");
        assert!(!mask.at(Vec2::new(50.0, 1.0)), "off the map");
        let (centre, size) = mask.bounds();
        assert_eq!(size, Vec2::new(12.0, 8.0));
        assert_eq!(centre, Vec2::new(5.5, 3.5));
        assert_eq!(mask.lawn().count(), mask.count());
        assert!(mask.lawn().all(|p| mask.at(p)));
    }

    #[test]
    fn bands_split_the_height_and_follow_the_floor_tint() {
        let range = (0.0, 10.0);
        assert_eq!(band_count(range), BANDS);
        assert_eq!(band_count((2.0, 2.0)), 1);
        assert_eq!(band_of(0.0, range, BANDS), 0);
        assert_eq!(band_of(10.0, range, BANDS), BANDS - 1);
        assert_eq!(band_of(5.0, range, BANDS), BANDS / 2);
        assert_eq!(band_of(3.0, (2.0, 2.0), 1), 0);
        let tints: Vec<f32> = (0..BANDS).map(|b| band_tint(b, range, BANDS)).collect();
        assert!(tints.windows(2).all(|w| w[0] < w[1]), "{tints:?}");
        assert!(tints.first().is_some_and(|t| *t < 1.0));
        assert!(tints.last().is_some_and(|t| *t > 1.0));
        assert!(tints.get(BANDS / 2).is_some_and(|t| (t - 1.0).abs() < 1e-5));
    }
}
