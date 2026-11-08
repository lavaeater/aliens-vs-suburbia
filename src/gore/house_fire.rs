//! House fires: a fire started next to a building (a molotov, a fire grenade, a burning
//! barrel) sets it alight. Flames on the roof, a thick smoke column leaning downwind
//! (`assets/fx/smoke_column.ron`) and a flickering orange glow, for most of a minute.
//!
//! Only the look: the building stays standing and blocking. Buildings are the wall tiles
//! with storeys (`MapFile::storeys`, the OpenStreetMap import's buildings), kept per level
//! in [`Buildings`].

use bevy::prelude::*;
use bevy_sprinkles::prelude::Particles3d;

use crate::fx::{FxAssets, StopFx};
use crate::general::events::map_events::LoadMap;
use crate::general::systems::lights_systems::{LightAnimation, LightStyle};
use crate::general::systems::map_systems::TileDefinitions;
use crate::gore::fire::SpawnFire;
use crate::map::ground::GroundHeight;
use crate::map::walls::WallKind;

/// Seconds a house burns.
const BURNS_FOR: f32 = 50.0;
/// How far (tiles) from a fire a building catches.
const CATCHES_WITHIN: usize = 2;
/// No new house fire this close (tiles) to one already burning.
const APART: usize = 5;
/// House fires at once, at most.
const MAX_FIRES: usize = 6;
/// The glow's brightness, lumens.
const GLOW: f32 = 60_000.0;
/// Seconds the smoke goes on after the fire is out: its longest puff.
const SMOKE_LINGER: f32 = 6.5;

/// The level's buildings: storeys per padded-grid tile, 0 = not a building.
#[derive(Resource, Debug, Default)]
pub struct Buildings {
    storeys: Vec<Vec<u8>>,
}

impl Buildings {
    /// From a map as `LoadMap` carries it: only solid walls count.
    pub fn of(map: &crate::general::components::map_components::MapFile) -> Self {
        let height = map.tiles.len() + 2;
        let width = map.tiles.first().map_or(0, Vec::len) + 2;
        let mut storeys = vec![vec![0u8; width]; height];
        for (y, row) in map.tiles.iter().enumerate() {
            for (x, &tile) in row.iter().enumerate() {
                let s = map.storeys.get(y).and_then(|r| r.get(x)).copied().unwrap_or(0);
                if s > 0
                    && WallKind::of(tile) == Some(WallKind::Solid)
                    && let Some(cell) = storeys.get_mut(y + 1).and_then(|r| r.get_mut(x + 1))
                {
                    *cell = s;
                }
            }
        }
        Self { storeys }
    }

    fn at(&self, x: usize, y: usize) -> u8 {
        self.storeys.get(y).and_then(|r| r.get(x)).copied().unwrap_or(0)
    }

    /// The building tile nearest `(x, y)` within `reach` tiles, and its storeys.
    pub fn nearest(&self, (x, y): (usize, usize), reach: usize) -> Option<((usize, usize), u8)> {
        let mut best: Option<(usize, (usize, usize), u8)> = None;
        for by in y.saturating_sub(reach)..=y + reach {
            for bx in x.saturating_sub(reach)..=x + reach {
                let s = self.at(bx, by);
                if s == 0 {
                    continue;
                }
                let d = bx.abs_diff(x).pow(2) + by.abs_diff(y).pow(2);
                if best.is_none_or(|(bd, ..)| d < bd) {
                    best = Some((d, (bx, by), s));
                }
            }
        }
        best.map(|(_, at, s)| (at, s))
    }
}

/// A burning house.
#[derive(Component, Debug)]
pub struct HouseFire {
    tile: (usize, usize),
    left: f32,
    /// The flames, the smoke and the glow.
    parts: [Option<Entity>; 3],
}

pub fn remember_buildings(mut loads: MessageReader<LoadMap>, mut buildings: ResMut<Buildings>) {
    for load in loads.read() {
        *buildings = Buildings::of(&load.map);
    }
}

/// A fire next to a building sets it alight.
#[allow(clippy::too_many_arguments)]
pub fn catch_fire(
    mut fires: MessageReader<SpawnFire>,
    buildings: Res<Buildings>,
    burning: Query<&HouseFire>,
    fx: Option<Res<FxAssets>>,
    tile_defs: Res<TileDefinitions>,
    ground: Res<GroundHeight>,
    mut commands: Commands,
) {
    let Some(fx) = fx else {
        fires.clear();
        return;
    };
    let mut lit: Vec<(usize, usize)> = burning.iter().map(|f| f.tile).collect();
    let width = tile_defs.tile_width.max(f32::EPSILON);
    for fire in fires.read() {
        if lit.len() >= MAX_FIRES {
            break;
        }
        let at = fire.position;
        let tile = (
            (at.x / width + 0.5).max(0.0) as usize,
            (at.z / width + 0.5).max(0.0) as usize,
        );
        let Some((house, storeys)) = buildings.nearest(tile, CATCHES_WITHIN + fire.radius as usize)
        else {
            continue;
        };
        if lit
            .iter()
            .any(|&(x, y)| x.abs_diff(house.0).max(y.abs_diff(house.1)) < APART)
        {
            continue;
        }
        lit.push(house);
        let (wx, wz) = (house.0 as f32 * width, house.1 as f32 * width);
        let roof = tile_defs.floor_level
            + ground.height(wx, wz)
            + WallKind::Solid.style_at(storeys).height;
        let top = Vec3::new(wx, roof, wz);
        let flames = commands
            .spawn((
                Name::new("House fire flames"),
                Particles3d(fx.fire.clone()),
                Transform::from_translation(top).with_scale(Vec3::new(1.6, 1.4, 1.6)),
            ))
            .id();
        let smoke = commands
            .spawn((
                Name::new("House fire smoke"),
                Particles3d(fx.smoke_column.clone()),
                Transform::from_translation(top + Vec3::Y * 0.3),
            ))
            .id();
        let glow = commands
            .spawn((
                Name::new("House fire glow"),
                PointLight {
                    color: Color::srgb(1.0, 0.5, 0.15),
                    intensity: GLOW,
                    range: 6.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                LightAnimation {
                    base: GLOW,
                    style: LightStyle::Flicker {
                        seed: (house.0 * 31 + house.1) as f32 * 0.37,
                    },
                },
                Transform::from_translation(top + Vec3::Y * 0.4),
            ))
            .id();
        commands.spawn((
            Name::new("House fire"),
            HouseFire {
                tile: house,
                left: BURNS_FOR,
                parts: [Some(flames), Some(smoke), Some(glow)],
            },
        ));
    }
}

/// Burn down the clock; then the flames die, the smoke thins and the glow goes out.
pub fn burn_houses(
    time: Res<Time>,
    mut fires: Query<(Entity, &mut HouseFire)>,
    mut commands: Commands,
) {
    for (entity, mut fire) in &mut fires {
        fire.left -= time.delta_secs();
        if fire.left > 0.0 {
            continue;
        }
        let [flames, smoke, glow] = fire.parts;
        if let Some(flames) = flames {
            commands.entity(flames).insert(StopFx::after(1.2));
        }
        if let Some(smoke) = smoke {
            commands.entity(smoke).insert(StopFx::after(SMOKE_LINGER));
        }
        if let Some(glow) = glow {
            commands.entity(glow).despawn();
        }
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::MapFile;
    use crate::map::{BitFlags, MapFeatures};

    #[test]
    fn only_buildings_with_storeys_burn() {
        let wall = (MapFeatures::Floor
            | MapFeatures::ImpassableForPlayers
            | MapFeatures::ImpassableForEnemies)
            .bits();
        let fence = BitFlags::from(MapFeatures::ImpassableForEnemies).bits() | 4;
        let grass = BitFlags::from(MapFeatures::Grass).bits();
        // A house at map (3, 3) with 2 storeys, a fence at (6, 3), a plain wall at (0, 0).
        let mut tiles = vec![vec![grass; 8]; 6];
        tiles[3][3] = wall;
        tiles[3][6] = fence;
        tiles[0][0] = wall;
        let mut storeys = vec![vec![0; 8]; 6];
        storeys[3][3] = 2;
        storeys[3][6] = 2;
        let b = Buildings::of(&MapFile {
            tiles,
            storeys,
            ..MapFile::default()
        });
        // Padded: the house is (4, 4).
        assert_eq!(b.nearest((5, 5), 2), Some(((4, 4), 2)));
        assert_eq!(b.nearest((7, 4), 1), None, "the fence is no house");
        assert_eq!(b.nearest((1, 1), 1), None, "a wall without storeys is no house");
    }
}

