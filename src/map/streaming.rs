//! Streaming: what you can see of a map exists only near the camera.
//!
//! A map's lights, loot bags, decorations and floor and wall meshes scale with its area: 800 x 800 tiles
//! plan about 17 000 lights (many casting shadows) and 29 000 bags, each a glTF scene. So
//! they are planned once, when the map loads, and kept per [`Chunk`] in [`MapStream`]: the
//! lights and bags as plain data, the surfaces as built meshes. The chunks the camera can
//! see get entities, under one root per chunk; the ones it has left lose them. A bag picked
//! up in the meantime is gone from the plan when its chunk unloads, so it stays gone.
//!
//! The tile grid, pathfinding and colliders are **not** streamed: aliens walk the whole map
//! off screen and need them everywhere.
//!
//! [`to_load`] and [`to_unload`] are the pure part. Chunks load inside the view plus
//! [`LOAD_MARGIN`] and unload outside it plus [`KEEP_MARGIN`], so a camera sitting on a
//! chunk's edge doesn't make it flicker.

use std::collections::{HashMap, HashSet};

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};

use crate::camera::components::GameCamera;
use crate::general::components::map_components::Floor;
use crate::general::events::map_events::LoadMap;
use crate::general::systems::map_systems::TileDefinitions;
use crate::loot::bags::{self, PlannedBag, StreamedBag};
use crate::map::floor_mesh::{TerrainColor, build_floors};
use crate::map::ground::{GroundHeight, HillsPreview};
use crate::map::map_lights::{Fixtures, LightPlan, plan_lights, spawn_light};
use crate::map::walls::{WallKind, padded_tiles, plan_walls};
use crate::settings::resources::GameSettings;

/// A chunk's edge, in tiles.
pub const CHUNK_TILES: i32 = 32;

/// Chunks load this many tiles outside the view. More than the longest light's reach, so
/// a lamp just off screen still lights the edge of the picture.
const LOAD_MARGIN: f32 = 12.0;
/// Chunks stay until this many tiles outside the view.
const KEEP_MARGIN: f32 = LOAD_MARGIN + CHUNK_TILES as f32;
/// The view never reaches further than this from the point under the camera, however
/// shallow the camera looks.
const MAX_VIEW_REACH: f32 = 128.0;
/// Chunks spawned per frame, so walking into new ground doesn't hitch.
const CHUNKS_PER_FRAME: usize = 2;

/// `(column, row)` of a chunk, in chunks.
pub type Chunk = (i32, i32);

/// The chunk a tile of the padded grid is in.
pub const fn chunk_of((col, row): (i32, i32)) -> Chunk {
    (col.div_euclid(CHUNK_TILES), row.div_euclid(CHUNK_TILES))
}

/// A rectangle of the map, in tiles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub min: Vec2,
    pub max: Vec2,
}

impl View {
    /// Everything: for a camera that isn't there, so the map is still lit.
    pub const EVERYTHING: Self = Self {
        min: Vec2::splat(f32::MIN),
        max: Vec2::splat(f32::MAX),
    };

    pub fn around(point: Vec2, reach: f32) -> Self {
        Self {
            min: point - reach,
            max: point + reach,
        }
    }

    pub fn containing(points: impl IntoIterator<Item = Vec2>) -> Option<Self> {
        points.into_iter().fold(None, |view: Option<Self>, p| {
            Some(view.map_or(Self { min: p, max: p }, |v| Self {
                min: v.min.min(p),
                max: v.max.max(p),
            }))
        })
    }

    #[must_use]
    pub fn grown(self, by: f32) -> Self {
        Self {
            min: self.min - by,
            max: self.max + by,
        }
    }

    /// The part of this view that is also in `other`.
    #[must_use]
    pub fn clipped_to(self, other: Self) -> Self {
        Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        }
    }

    pub fn touches(&self, (col, row): Chunk) -> bool {
        let size = CHUNK_TILES as f32;
        let (min_x, min_y) = (col as f32 * size, row as f32 * size);
        min_x < self.max.x
            && min_x + size > self.min.x
            && min_y < self.max.y
            && min_y + size > self.min.y
    }
}

/// The planned chunks that should exist for this view and don't yet, nearest `to` first.
pub fn to_load(
    view: View,
    to: Vec2,
    planned: impl IntoIterator<Item = Chunk>,
    live: &HashSet<Chunk>,
) -> Vec<Chunk> {
    let wanted = view.grown(LOAD_MARGIN);
    let mut chunks: Vec<Chunk> = planned
        .into_iter()
        .filter(|chunk| wanted.touches(*chunk) && !live.contains(chunk))
        .collect();
    let size = CHUNK_TILES as f32;
    let distance = |&(col, row): &Chunk| {
        (Vec2::new(col as f32 + 0.5, row as f32 + 0.5) * size).distance_squared(to)
    };
    chunks.sort_by(|a, b| distance(a).total_cmp(&distance(b)).then(a.cmp(b)));
    chunks
}

/// The live chunks this view has left behind.
pub fn to_unload(view: View, live: impl IntoIterator<Item = Chunk>) -> Vec<Chunk> {
    let kept = view.grown(KEEP_MARGIN);
    live.into_iter().filter(|c| !kept.touches(*c)).collect()
}

/// Drop the bags of a chunk that aren't lying there any more. A bag's id is its place in
/// the plan, so this has to run before the chunk spawns again, which renumbers them.
fn forget_taken_bags(bags: &mut Vec<PlannedBag>, chunk: Chunk, left: &HashSet<StreamedBag>) {
    let mut id = 0;
    bags.retain(|_| {
        id += 1;
        left.contains(&StreamedBag { chunk, id: id - 1 })
    });
}

/// One built mesh of a chunk.
struct Surface {
    kind: SurfaceKind,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Clone, Copy)]
enum SurfaceKind {
    Floor,
    Wall(WallKind),
}

/// What stands in one chunk.
#[derive(Default)]
struct ChunkPlan {
    /// A light and its animation seed.
    lights: Vec<(LightPlan, f32)>,
    bags: Vec<PlannedBag>,
    surfaces: Vec<Surface>,
    decorations: Vec<Decoration>,
}

/// A map decoration, a tree say. The plan holds its scene, so walking back and forth doesn't
/// load the model from disk each time.
struct Decoration {
    name: String,
    transform: Transform,
    scene: Handle<WorldAsset>,
    foliage: bool,
}

/// A plant, drawn without an outline: its leaves are see-through cards, and an outline
/// follows the cards, not the leaves.
#[derive(Component)]
pub struct Foliage;

/// The nature pack is the plants' (and a few rocks').
fn is_foliage(model: &str) -> bool {
    model.starts_with("packs/nature/")
}

/// The current map's scatter: all of it as data, the chunks near the camera as entities.
#[derive(Resource, Default)]
pub struct MapStream {
    planned: HashMap<Chunk, ChunkPlan>,
    live: HashMap<Chunk, Entity>,
    fixtures: Option<Fixtures>,
    bag_looks: Vec<bags::BagLook>,
    bag_table: String,
}

/// The root a chunk's entities hang under.
#[derive(Component)]
pub struct MapChunk;

/// A floor's look. Terrain of one colour shares it, so its chunks can batch.
fn floor_material(
    [r, g, b]: TerrainColor,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb_u8(r, g, b),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    })
}

/// Build a map's floors and walls into the chunks' plans.
fn plan_surfaces(
    load: &LoadMap,
    tiles: &[Vec<u64>],
    tile_defs: &TileDefinitions,
    ground: &GroundHeight,
    planned: &mut HashMap<Chunk, ChunkPlan>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let (tile, floor_y) = (tile_defs.tile_width, tile_defs.floor_level);

    let mut floor_looks: HashMap<TerrainColor, Handle<StandardMaterial>> = HashMap::new();
    for ((chunk, color), floor) in build_floors(tiles, tile, floor_y, ground) {
        let material = floor_looks
            .entry(color)
            .or_insert_with(|| floor_material(color, materials))
            .clone();
        planned.entry(chunk).or_default().surfaces.push(Surface {
            kind: SurfaceKind::Floor,
            mesh: meshes.add(floor.into_mesh()),
            material,
        });
    }

    let mut wall_looks: HashMap<WallKind, Handle<StandardMaterial>> = HashMap::new();
    for (chunk, kind, wall) in plan_walls(&load.map, tiles, tile, floor_y, ground) {
        let material = wall_looks
            .entry(kind)
            .or_insert_with(|| materials.add(kind.material()))
            .clone();
        planned.entry(chunk).or_default().surfaces.push(Surface {
            kind: SurfaceKind::Wall(kind),
            mesh: meshes.add(wall.into_mesh()),
            material,
        });
    }
}

fn spawn_surface(parent: &mut ChildSpawnerCommands, surface: &Surface) {
    let mut spawned = parent.spawn((
        Mesh3d(surface.mesh.clone()),
        MeshMaterial3d(surface.material.clone()),
    ));
    match surface.kind {
        // Nothing lies below the floor, and every shadow-casting lamp in view would
        // otherwise redraw it into its shadow map.
        SurfaceKind::Floor => {
            spawned.insert((Name::from("Floor"), Floor {}, NotShadowCaster));
        }
        SurfaceKind::Wall(WallKind::Solid) => {
            spawned.insert(Name::from(WallKind::Solid.label()));
        }
        SurfaceKind::Wall(kind) => {
            spawned.insert((Name::from(kind.label()), NotShadowCaster));
        }
    }
}

/// Plan a map's chunks when it loads.
#[allow(clippy::too_many_arguments)]
pub fn plan_map_stream(
    mut loads: MessageReader<LoadMap>,
    mut stream: ResMut<MapStream>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    tile_defs: Res<TileDefinitions>,
    game_settings: Res<GameSettings>,
    hills: Option<Res<HillsPreview>>,
    mut ground: ResMut<GroundHeight>,
) {
    for load in loads.read() {
        *ground = GroundHeight::for_level(&load.map, hills.is_some(), tile_defs.tile_width);
        // Leaving a match sweeps these already, hence `try_`.
        for root in stream.live.values() {
            commands.entity(*root).try_despawn();
        }
        let tiles = padded_tiles(&load.map);
        let lights = plan_lights(&tiles);
        let spread = bags::plan_spread(&load.map, &tiles, &asset_server);

        let mut planned: HashMap<Chunk, ChunkPlan> = HashMap::new();
        for (i, light) in lights.iter().enumerate() {
            let chunk = planned.entry(chunk_of(light.tile)).or_default();
            chunk.lights.push((*light, i as f32));
        }
        let (bag_looks, bag_table, bag_count) = match spread {
            Some(spread) => {
                let count = spread.bags.len();
                for bag in spread.bags {
                    let chunk = planned.entry(chunk_of(bag.tile)).or_default();
                    chunk.bags.push(bag);
                }
                (spread.looks, spread.table, count)
            }
            None => (Vec::new(), String::new(), 0),
        };
        for dec in &load.map.decorations {
            // Map coordinates, like placements: one tile in from the padded grid.
            let tile = (dec.x + 1, dec.y + 1);
            let (x, z) = (
                tile_defs.tile_width * tile.0 as f32,
                tile_defs.tile_width * tile.1 as f32,
            );
            let floor = tile_defs.floor_level + tile_defs.tile_depth;
            let at = Vec3::new(x, floor + ground.height(x, z), z);
            // `dec.scale` is in player units.
            let scale = dec.scale * game_settings.player_unit;
            planned
                .entry(chunk_of(tile))
                .or_default()
                .decorations
                .push(Decoration {
                    name: format!("Decoration {}:{} {}", dec.x, dec.y, dec.model),
                    transform: Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_y(dec.rotation_y.to_radians()))
                        .with_scale(Vec3::splat(scale)),
                    scene: asset_server.load(format!("{}#Scene0", dec.model)),
                    foliage: is_foliage(&dec.model),
                });
        }
        plan_surfaces(
            load,
            &tiles,
            &tile_defs,
            &ground,
            &mut planned,
            &mut meshes,
            &mut materials,
        );
        info!(
            "planned {} map lights, {bag_count} loot bags and {} decorations in {} chunks",
            lights.len(),
            load.map.decorations.len(),
            planned.len()
        );
        *stream = MapStream {
            planned,
            live: HashMap::new(),
            fixtures: Some(Fixtures::new(&mut meshes, &mut materials)),
            bag_looks,
            bag_table,
        };
    }
}

/// The ground the camera sees, in tiles, from where its corners meet the floor. `None` when
/// a corner looks above the horizon. On hills that is measured at the ground under the camera.
fn camera_view(
    camera: &Camera,
    at: &GlobalTransform,
    tiles: &TileDefinitions,
    ground: &GroundHeight,
) -> Option<View> {
    let size = camera.logical_viewport_size()?;
    let under = at.translation();
    let floor = Vec3::Y * (tiles.floor_level + ground.height(under.x, under.z));
    let on_floor = |corner: Vec2| {
        let ray = camera.viewport_to_world(at, corner).ok()?;
        let along = ray.intersect_plane(floor, InfinitePlane3d::new(Vec3::Y))?;
        Some(ray.get_point(along).xz() / tiles.tile_width)
    };
    View::containing([
        on_floor(Vec2::ZERO)?,
        on_floor(Vec2::new(size.x, 0.0))?,
        on_floor(Vec2::new(0.0, size.y))?,
        on_floor(size)?,
    ])
}

/// Bring the chunks around the camera to life and retire the ones it has left.
pub fn stream_map_chunks(
    mut stream: ResMut<MapStream>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    bags_left: Query<&StreamedBag>,
    tile_defs: Res<TileDefinitions>,
    ground: Res<GroundHeight>,
) {
    if stream.planned.is_empty() {
        return;
    }
    let (view, centre) = cameras
        .single()
        .map_or((View::EVERYTHING, Vec2::ZERO), |(camera, at)| {
            let under = at.translation().xz() / tile_defs.tile_width;
            let reach = View::around(under, MAX_VIEW_REACH);
            let view = camera_view(camera, at, &tile_defs, &ground).map_or(reach, |v| v.clipped_to(reach));
            (view, under)
        });

    let stream = &mut *stream;
    let leaving = to_unload(view, stream.live.keys().copied());
    if !leaving.is_empty() {
        let left: HashSet<StreamedBag> = bags_left.iter().copied().collect();
        for chunk in leaving {
            if let Some(root) = stream.live.remove(&chunk) {
                commands.entity(root).try_despawn();
            }
            if let Some(plan) = stream.planned.get_mut(&chunk) {
                forget_taken_bags(&mut plan.bags, chunk, &left);
            }
        }
    }

    let live: HashSet<Chunk> = stream.live.keys().copied().collect();
    let arriving = to_load(view, centre, stream.planned.keys().copied(), &live);
    let Some(fixtures) = &stream.fixtures else {
        return;
    };
    for chunk in arriving.into_iter().take(CHUNKS_PER_FRAME) {
        let Some(plan) = stream.planned.get(&chunk) else {
            continue;
        };
        let root = commands
            .spawn((
                Name::new(format!("Map Chunk {}:{}", chunk.0, chunk.1)),
                MapChunk,
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|parent| {
                for surface in &plan.surfaces {
                    spawn_surface(parent, surface);
                }
                for decoration in &plan.decorations {
                    let mut scene = parent.spawn((
                        Name::new(decoration.name.clone()),
                        WorldAssetRoot(decoration.scene.clone()),
                        decoration.transform,
                    ));
                    if decoration.foliage {
                        scene.insert(Foliage);
                    }
                }
                let tw = tile_defs.tile_width;
                for (light, seed) in &plan.lights {
                    let (x, z) = (tw * light.tile.0 as f32, tw * light.tile.1 as f32);
                    let at = Vec3::new(x, tile_defs.floor_level + ground.height(x, z), z);
                    spawn_light(parent, &mut materials, fixtures, light.kind, at, tw, *seed);
                }
                for (id, bag) in plan.bags.iter().enumerate() {
                    let id = StreamedBag { chunk, id };
                    bags::spawn_bag(
                        parent,
                        bag,
                        id,
                        &stream.bag_looks,
                        &stream.bag_table,
                        &tile_defs,
                        &ground,
                    );
                }
            })
            .id();
        stream.live.insert(chunk, root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: f32 = CHUNK_TILES as f32;

    #[test]
    fn tiles_fall_in_their_chunk() {
        assert_eq!(chunk_of((0, 0)), (0, 0));
        assert_eq!(chunk_of((31, 32)), (0, 1));
        assert_eq!(chunk_of((-1, 64)), (-1, 2));
    }

    #[test]
    fn a_view_covers_the_points_it_is_made_of() {
        let view = View::containing([Vec2::new(5.0, 9.0), Vec2::new(-2.0, 20.0)]).unwrap();
        assert_eq!(view.min, Vec2::new(-2.0, 9.0));
        assert_eq!(view.max, Vec2::new(5.0, 20.0));
        assert_eq!(View::containing([]), None);
    }

    #[test]
    fn a_view_never_reaches_past_what_it_is_clipped_to() {
        let reach = View::around(Vec2::ZERO, 10.0);
        let view = View::around(Vec2::new(8.0, 0.0), 5.0).clipped_to(reach);
        assert_eq!(view.min, Vec2::new(3.0, -5.0));
        assert_eq!(view.max, Vec2::new(10.0, 5.0));
    }

    #[test]
    fn chunks_load_near_the_view_nearest_first_and_only_once() {
        let view = View::around(Vec2::splat(SIZE * 5.5), 20.0);
        let planned: Vec<Chunk> = (0..12)
            .flat_map(|row| (0..12).map(move |col| (col, row)))
            .collect();
        let loaded = to_load(
            view,
            Vec2::splat(SIZE * 5.5),
            planned.clone(),
            &HashSet::new(),
        );
        assert_eq!(loaded.first(), Some(&(5, 5)));
        assert!(loaded.contains(&(6, 6)) && loaded.contains(&(4, 4)));
        assert!(!loaded.contains(&(7, 5)) && !loaded.contains(&(3, 5)));

        let live = HashSet::from([(5, 5)]);
        assert!(!to_load(view, Vec2::ZERO, planned, &live).contains(&(5, 5)));
    }

    #[test]
    fn only_planned_chunks_load() {
        let view = View::around(Vec2::splat(SIZE * 0.5), 1.0);
        assert_eq!(
            to_load(view, Vec2::ZERO, [(0, 0), (40, 40)], &HashSet::new()),
            vec![(0, 0)]
        );
    }

    #[test]
    fn a_chunk_on_the_edge_neither_flickers_nor_stays_forever() {
        // Just out of load range: not loaded, but kept if it is already there.
        let chunk = (5, 0);
        let view = View {
            min: Vec2::ZERO,
            max: Vec2::new(5.0 * SIZE - LOAD_MARGIN - 1.0, SIZE),
        };
        assert!(to_load(view, Vec2::ZERO, [chunk], &HashSet::new()).is_empty());
        assert!(to_unload(view, [chunk]).is_empty());
        // Far enough away, it goes.
        let view = View {
            min: Vec2::ZERO,
            max: Vec2::new(5.0 * SIZE - KEEP_MARGIN - 1.0, SIZE),
        };
        assert_eq!(to_unload(view, [chunk]), vec![chunk]);
    }

    #[test]
    fn a_taken_bag_stays_gone_and_the_others_stay() {
        let bag = |col| PlannedBag {
            tile: (col, 0),
            look: 0,
            offset: Vec2::ZERO,
            yaw: 0.0,
        };
        let mut bags = vec![bag(1), bag(2), bag(3)];
        // Bag 1 was picked up; another chunk's bag with the same id doesn't count.
        let left = HashSet::from([
            StreamedBag {
                chunk: (0, 0),
                id: 0,
            },
            StreamedBag {
                chunk: (0, 0),
                id: 2,
            },
            StreamedBag {
                chunk: (9, 9),
                id: 1,
            },
        ]);
        forget_taken_bags(&mut bags, (0, 0), &left);
        assert_eq!(bags, vec![bag(1), bag(3)]);
    }

    #[test]
    fn without_a_camera_everything_loads_and_stays() {
        let planned = [(0, 0), (500, -3)];
        assert_eq!(
            to_load(View::EVERYTHING, Vec2::ZERO, planned, &HashSet::new()).len(),
            2
        );
        assert!(to_unload(View::EVERYTHING, planned).is_empty());
    }
}
