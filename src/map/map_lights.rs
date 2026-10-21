//! Lights placed around the map, now that there is no ambient light to see by.
//!
//! [`plan_lights`] is pure: it reads the tile grid and decides what goes where. The map's
//! features get their own lights (a sickly green throb at each alien spawn, a red beacon at
//! the goal, a soft lamp at each player spawn); the rest of the floor is covered by lights
//! kept at least [`SPACING`] tiles apart, the ones against a wall first, since that is where
//! a lamp post or a lantern would be. Those cycle through every kind of light on purpose,
//! so one map shows them all side by side: street lamps (a spot pointing down from a post),
//! lanterns (a flickering low point light), floodlights (a spot on a wall, aimed out across
//! the floor) and neon globes (a coloured, pulsing point light).
//!
//! [`spawn_map_lights`] turns the plan into entities when a map loads.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::general::events::map_events::LoadMap;
use crate::general::systems::lights_systems::{LightAnimation, LightStyle};
use crate::general::systems::map_systems::TileDefinitions;
use crate::map::walls::padded_tiles;
use crate::map::{BitFlags, MapFeatures};

/// Minimum distance between two placed lights, in tiles.
pub const SPACING: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    StreetLamp,
    Lantern,
    /// Mounted on the wall at `wall`, shining out over the tile.
    Floodlight {
        wall: (i32, i32),
    },
    /// `hue` in degrees.
    Neon {
        hue: f32,
    },
    AlienSpawn,
    Goal,
    PlayerSpawn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightPlan {
    pub kind: LightKind,
    /// Grid `(col, row)` of the tile it stands on.
    pub tile: (i32, i32),
}

fn flags(tiles: &[Vec<u64>], (col, row): (i32, i32)) -> Option<BitFlags<MapFeatures>> {
    let raw = *tiles
        .get(usize::try_from(row).ok()?)?
        .get(usize::try_from(col).ok()?)?;
    (raw != 0).then(|| BitFlags::from_bits_truncate(raw))
}

fn is_open(tiles: &[Vec<u64>], tile: (i32, i32)) -> bool {
    flags(tiles, tile).is_some_and(|f| {
        !f.intersects(MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies)
    })
}

fn is_solid_wall(tiles: &[Vec<u64>], tile: (i32, i32)) -> bool {
    flags(tiles, tile).is_some_and(|f| {
        f.contains(MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies)
    })
}

/// A stable shuffle key, so lights spread over the map instead of piling into the first
/// rows the scan reaches.
fn scatter_key((col, row): (i32, i32)) -> u32 {
    let mut h = (col as u32).wrapping_mul(0x9e37_79b9) ^ (row as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

const NEIGHBOURS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Decide every light for a (padded) tile grid. Deterministic: the same map always gets
/// the same lights.
pub fn plan_lights(tiles: &[Vec<u64>]) -> Vec<LightPlan> {
    let mut plan = Vec::new();
    let mut open = Vec::new();

    for (row, cols) in tiles.iter().enumerate() {
        for (col, &raw) in cols.iter().enumerate() {
            if raw == 0 {
                continue;
            }
            let tile = (col as i32, row as i32);
            let f = BitFlags::<MapFeatures>::from_bits_truncate(raw);
            let feature = if f.contains(MapFeatures::EnemySpawn) {
                Some(LightKind::AlienSpawn)
            } else if f.contains(MapFeatures::EnemyExit) {
                Some(LightKind::Goal)
            } else if f.contains(MapFeatures::PlayerSpawn) {
                Some(LightKind::PlayerSpawn)
            } else {
                None
            };
            match feature {
                Some(kind) => plan.push(LightPlan { kind, tile }),
                None if is_open(tiles, tile) => open.push(tile),
                None => {}
            }
        }
    }

    let wall_beside = |(col, row): (i32, i32)| {
        NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (col + dx, row + dy))
            .find(|&n| is_solid_wall(tiles, n))
    };
    open.sort_by_key(|&t| (wall_beside(t).is_none(), scatter_key(t)));

    let far_enough = |plan: &[LightPlan], (col, row): (i32, i32)| {
        plan.iter().all(|p| {
            let (dx, dy) = ((p.tile.0 - col) as f32, (p.tile.1 - row) as f32);
            dx * dx + dy * dy >= SPACING * SPACING
        })
    };

    let (mut along_walls, mut in_the_open) = (0usize, 0usize);
    for tile in open {
        if !far_enough(&plan, tile) {
            continue;
        }
        let kind = if let Some(wall) = wall_beside(tile) {
            along_walls += 1;
            match along_walls % 3 {
                1 => LightKind::StreetLamp,
                2 => LightKind::Lantern,
                _ => LightKind::Floodlight { wall },
            }
        } else {
            in_the_open += 1;
            if in_the_open % 2 == 1 {
                LightKind::Neon {
                    hue: (scatter_key(tile) % 360) as f32,
                }
            } else {
                LightKind::StreetLamp
            }
        };
        plan.push(LightPlan { kind, tile });
    }
    plan
}

// ── Spawning ────────────────────────────────────────────────────────────────

/// Light levels, in lumens (Bevy's unit for point and spot lights alike). Tuned for the
/// default camera exposure, against the 5000 lux the old sun gave.
mod lumens {
    pub const STREET_LAMP: f32 = 80_000.0;
    pub const LANTERN: f32 = 14_000.0;
    pub const FLOODLIGHT: f32 = 120_000.0;
    pub const NEON: f32 = 25_000.0;
    pub const ALIEN_SPAWN: f32 = 45_000.0;
    pub const GOAL: f32 = 45_000.0;
    pub const PLAYER_SPAWN: f32 = 30_000.0;
}

const STREET_LAMP_HEIGHT: f32 = 1.4;

/// A placed light's fixture, so it can be found (and filtered out of the inspector).
#[derive(Component)]
pub struct MapLight;

struct Fixtures {
    pole: Handle<Mesh>,
    bulb: Handle<Mesh>,
    lantern: Handle<Mesh>,
    flood: Handle<Mesh>,
    metal: Handle<StandardMaterial>,
}

fn glow(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    strength: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * strength,
        unlit: true,
        ..default()
    })
}

pub fn spawn_map_lights(
    mut loads: MessageReader<LoadMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_defs: Res<TileDefinitions>,
) {
    for load in loads.read() {
        let tiles = padded_tiles(&load.map);
        let plan = plan_lights(&tiles);
        let fixtures = Fixtures {
            pole: meshes.add(Cylinder::new(0.025, STREET_LAMP_HEIGHT)),
            bulb: meshes.add(Sphere::new(0.06)),
            lantern: meshes.add(Cuboid::new(0.08, 0.12, 0.08)),
            flood: meshes.add(Cuboid::new(0.14, 0.08, 0.06)),
            metal: materials.add(StandardMaterial {
                base_color: Color::srgb(0.12, 0.12, 0.13),
                metallic: 0.6,
                perceptual_roughness: 0.5,
                ..default()
            }),
        };
        let floor = tile_defs.floor_level;
        let tw = tile_defs.tile_width;
        for (i, light) in plan.iter().enumerate() {
            let at = Vec3::new(tw * light.tile.0 as f32, floor, tw * light.tile.1 as f32);
            spawn_light(
                &mut commands,
                &mut materials,
                &fixtures,
                light.kind,
                at,
                tw,
                i as f32,
            );
        }
        info!("placed {} map lights", plan.len());
    }
}

fn point(color: Color, intensity: f32, range: f32) -> PointLight {
    PointLight {
        color,
        intensity,
        range,
        shadow_maps_enabled: false,
        ..default()
    }
}

#[allow(clippy::too_many_lines)]
fn spawn_light(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    fx: &Fixtures,
    kind: LightKind,
    at: Vec3,
    tile: f32,
    seed: f32,
) {
    match kind {
        LightKind::StreetLamp => {
            let warm = Color::srgb(1.0, 0.78, 0.5);
            let top = at + Vec3::Y * STREET_LAMP_HEIGHT;
            commands.spawn((
                Name::from("Street Lamp"),
                MapLight,
                Mesh3d(fx.pole.clone()),
                MeshMaterial3d(fx.metal.clone()),
                Transform::from_translation(at + Vec3::Y * STREET_LAMP_HEIGHT * 0.5),
            ));
            commands.spawn((
                Name::from("Street Lamp Bulb"),
                MapLight,
                Mesh3d(fx.bulb.clone()),
                MeshMaterial3d(glow(materials, warm, 8.0)),
                NotShadowCaster,
                Transform::from_translation(top),
            ));
            commands.spawn((
                Name::from("Street Lamp Light"),
                MapLight,
                SpotLight {
                    color: warm,
                    intensity: lumens::STREET_LAMP,
                    range: 6.0,
                    outer_angle: 0.95,
                    inner_angle: 0.6,
                    shadow_maps_enabled: true,
                    ..default()
                },
                Transform::from_translation(top - Vec3::Y * 0.05).looking_at(at, Vec3::Z),
            ));
        }
        LightKind::Lantern => {
            let flame = Color::srgb(1.0, 0.55, 0.2);
            let spot = at + Vec3::Y * 0.3;
            commands.spawn((
                Name::from("Lantern"),
                MapLight,
                Mesh3d(fx.lantern.clone()),
                MeshMaterial3d(glow(materials, flame, 6.0)),
                NotShadowCaster,
                Transform::from_translation(spot),
            ));
            commands.spawn((
                Name::from("Lantern Light"),
                MapLight,
                point(flame, lumens::LANTERN, 3.5),
                LightAnimation {
                    base: lumens::LANTERN,
                    style: LightStyle::Flicker { seed },
                },
                Transform::from_translation(spot + Vec3::Y * 0.1),
            ));
        }
        LightKind::Floodlight { wall } => {
            let cold = Color::srgb(0.85, 0.92, 1.0);
            // On the face of the wall, a little over head height, aimed away from it and
            // down at the floor two tiles out.
            let wall_at = Vec3::new(tile * wall.0 as f32, at.y, tile * wall.1 as f32);
            let out = (at - wall_at).normalize_or(Vec3::X);
            let mount = wall_at + out * (tile * 0.55) + Vec3::Y * 1.0;
            let target = at + out * (tile * 2.0);
            commands.spawn((
                Name::from("Floodlight"),
                MapLight,
                Mesh3d(fx.flood.clone()),
                MeshMaterial3d(glow(materials, cold, 5.0)),
                NotShadowCaster,
                Transform::from_translation(mount).looking_at(target, Vec3::Y),
            ));
            commands.spawn((
                Name::from("Floodlight Light"),
                MapLight,
                SpotLight {
                    color: cold,
                    intensity: lumens::FLOODLIGHT,
                    range: 8.0,
                    outer_angle: 0.7,
                    inner_angle: 0.45,
                    shadow_maps_enabled: true,
                    ..default()
                },
                Transform::from_translation(mount + out * 0.05).looking_at(target, Vec3::Y),
            ));
        }
        LightKind::Neon { hue } => {
            let color = Color::hsl(hue, 0.9, 0.55);
            let spot = at + Vec3::Y * 0.6;
            commands.spawn((
                Name::from("Neon Globe"),
                MapLight,
                Mesh3d(fx.bulb.clone()),
                MeshMaterial3d(glow(materials, color, 10.0)),
                NotShadowCaster,
                Transform::from_translation(spot).with_scale(Vec3::splat(1.5)),
            ));
            commands.spawn((
                Name::from("Neon Light"),
                MapLight,
                point(color, lumens::NEON, 4.0),
                LightAnimation {
                    base: lumens::NEON,
                    style: LightStyle::Pulse {
                        speed: 0.25 + (seed * 0.37).fract() * 0.3,
                        depth: 0.4,
                    },
                },
                Transform::from_translation(spot),
            ));
        }
        LightKind::AlienSpawn => {
            commands.spawn((
                Name::from("Alien Spawn Glow"),
                MapLight,
                point(Color::srgb(0.45, 1.0, 0.3), lumens::ALIEN_SPAWN, 4.5),
                LightAnimation {
                    base: lumens::ALIEN_SPAWN,
                    style: LightStyle::Pulse {
                        speed: 0.8,
                        depth: 0.6,
                    },
                },
                Transform::from_translation(at + Vec3::Y * 0.6),
            ));
        }
        LightKind::Goal => {
            commands.spawn((
                Name::from("Goal Beacon"),
                MapLight,
                point(Color::srgb(1.0, 0.2, 0.15), lumens::GOAL, 4.5),
                LightAnimation {
                    base: lumens::GOAL,
                    style: LightStyle::Pulse {
                        speed: 1.2,
                        depth: 0.7,
                    },
                },
                Transform::from_translation(at + Vec3::Y * 0.7),
            ));
        }
        LightKind::PlayerSpawn => {
            commands.spawn((
                Name::from("Player Spawn Lamp"),
                MapLight,
                point(Color::srgb(0.95, 0.95, 1.0), lumens::PLAYER_SPAWN, 4.0),
                Transform::from_translation(at + Vec3::Y * 1.2),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: u64 = MapFeatures::Floor as u64;
    const WALL: u64 =
        FLOOR | MapFeatures::ImpassableForPlayers as u64 | MapFeatures::ImpassableForEnemies as u64;

    /// A walled yard, `w` x `h` inside, with an alien spawn in one corner.
    fn yard(w: usize, h: usize) -> Vec<Vec<u64>> {
        let mut tiles = vec![vec![WALL; w + 2]; h + 2];
        for row in tiles.iter_mut().take(h + 1).skip(1) {
            for cell in row.iter_mut().take(w + 1).skip(1) {
                *cell = FLOOR;
            }
        }
        tiles[1][1] = FLOOR | MapFeatures::EnemySpawn as u64;
        tiles
    }

    #[test]
    fn features_get_their_own_lights() {
        let plan = plan_lights(&yard(10, 10));
        assert!(
            plan.iter()
                .any(|p| p.kind == LightKind::AlienSpawn && p.tile == (1, 1))
        );
    }

    #[test]
    fn lights_keep_their_distance_and_stand_on_open_floor() {
        let tiles = yard(30, 20);
        let plan = plan_lights(&tiles);
        assert!(
            plan.len() > 6,
            "a big yard gets plenty of lights: {}",
            plan.len()
        );
        for (i, a) in plan.iter().enumerate() {
            assert!(is_open(&tiles, a.tile), "{a:?} is on a wall");
            for b in plan.iter().skip(i + 1) {
                let d = ((a.tile.0 - b.tile.0).pow(2) + (a.tile.1 - b.tile.1).pow(2)) as f32;
                assert!(d >= SPACING * SPACING, "{a:?} and {b:?} too close");
            }
        }
    }

    #[test]
    fn every_kind_shows_up_on_a_big_enough_map() {
        let plan = plan_lights(&yard(30, 20));
        let has = |f: fn(&LightKind) -> bool| plan.iter().any(|p| f(&p.kind));
        assert!(has(|k| matches!(k, LightKind::StreetLamp)));
        assert!(has(|k| matches!(k, LightKind::Lantern)));
        assert!(has(|k| matches!(k, LightKind::Floodlight { .. })));
        assert!(has(|k| matches!(k, LightKind::Neon { .. })));
    }

    #[test]
    fn a_floodlight_hangs_on_a_wall_next_to_its_tile() {
        let tiles = yard(30, 20);
        for p in plan_lights(&tiles) {
            if let LightKind::Floodlight { wall } = p.kind {
                assert!(is_solid_wall(&tiles, wall));
                assert_eq!((wall.0 - p.tile.0).abs() + (wall.1 - p.tile.1).abs(), 1);
            }
        }
    }

    #[test]
    fn the_plan_is_the_same_every_time() {
        let tiles = yard(25, 18);
        assert_eq!(plan_lights(&tiles), plan_lights(&tiles));
    }
}
