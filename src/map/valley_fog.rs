//! Fog banks in the valleys: on a level with hills (`MapFile::elevation`), pale banks of
//! fog (`assets/fx/ground_fog.ron`) lie in the lowest dips of the ground. They show the
//! shape of the land the near top-down camera hides, and they hang over the streets the
//! aliens come up. Off with the fog (`camera::fog::FogLevel::Off`, Ctrl+O).
//!
//! [`plan_banks`] is pure: lattice points in the lowest part of the level's range, lowest
//! first, spaced out, on open ground - so the banks follow the valley floors.
// Grid code: indices come from the grid's own ranges or are checked with `get`.
#![allow(clippy::indexing_slicing)]

use bevy::prelude::*;
use bevy_sprinkles::prelude::{Particles3d, ParticlesAsset};

use crate::camera::fog::FogLevel;
use crate::general::components::map_components::{ELEVATION_STEP, MapFile};
use crate::general::events::map_events::LoadMap;
use crate::general::systems::map_systems::TileDefinitions;
use crate::map::ground::{GroundHeight, Hills};
use crate::map::reach::enemy_passable;

/// Banks only in the lowest this share of the level's height range.
const LOW_SHARE: f32 = 0.3;
/// Banks at least this far apart (tiles).
const SPACING: usize = 14;
/// At most this many banks on a level.
const MAX_BANKS: usize = 48;

/// A fog bank (its own entity, with the particles).
#[derive(Component)]
pub struct FogBank;

/// The banks' effect, loaded once.
#[derive(Resource)]
pub struct GroundFogAsset(Handle<ParticlesAsset>);

pub fn load_ground_fog(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GroundFogAsset(assets.load("fx/ground_fog.ron")));
}

/// Where the banks go on `map`: map tiles, lowest first.
pub fn plan_banks(map: &MapFile) -> Vec<(usize, usize)> {
    let lattice = &map.elevation;
    let (Some(lowest), Some(highest)) = (
        lattice.iter().flatten().copied().min(),
        lattice.iter().flatten().copied().max(),
    ) else {
        return Vec::new();
    };
    let span = f32::from(highest - lowest);
    if span < 50.0 {
        // Under half a unit of relief: no valleys to speak of.
        return Vec::new();
    }
    let ceiling = f32::from(lowest) + span * LOW_SHARE;
    let mut lows: Vec<(u16, (usize, usize))> = Vec::new();
    for (j, row) in lattice.iter().enumerate() {
        for (i, &h) in row.iter().enumerate() {
            if f32::from(h) > ceiling {
                continue;
            }
            let (x, y) = (i * ELEVATION_STEP, j * ELEVATION_STEP);
            let open = map
                .tiles
                .get(y)
                .and_then(|r| r.get(x))
                .is_some_and(|&t| enemy_passable(t));
            if open {
                lows.push((h, (x, y)));
            }
        }
    }
    lows.sort_by_key(|&(h, _)| h);
    let mut banks: Vec<(usize, usize)> = Vec::new();
    for (_, at) in lows {
        if banks.len() >= MAX_BANKS {
            break;
        }
        if banks
            .iter()
            .all(|&(x, y)| x.abs_diff(at.0).max(y.abs_diff(at.1)) >= SPACING)
        {
            banks.push(at);
        }
    }
    banks
}

/// Lay the banks when a hilly map loads (the old ones go with the old map).
#[allow(clippy::too_many_arguments)]
pub fn spawn_fog_banks(
    mut loads: MessageReader<LoadMap>,
    mut commands: Commands,
    old: Query<Entity, With<FogBank>>,
    fog: Option<Res<GroundFogAsset>>,
    hills: Option<Res<Hills>>,
    tile_defs: Res<TileDefinitions>,
    level: Res<FogLevel>,
) {
    for load in loads.read() {
        for bank in &old {
            commands.entity(bank).despawn();
        }
        let (Some(fog), Some(_)) = (fog.as_ref(), hills.as_ref()) else {
            continue;
        };
        let ground = GroundHeight::new(&load.map, tile_defs.tile_width);
        let visibility = if *level == FogLevel::Off {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let banks = plan_banks(&load.map);
        info!("valley fog: {} bank(s)", banks.len());
        for (x, y) in banks {
            // Map tile (x, y) is padded tile (x + 1, y + 1).
            let (wx, wz) = (
                (x + 1) as f32 * tile_defs.tile_width,
                (y + 1) as f32 * tile_defs.tile_width,
            );
            commands.spawn((
                Name::new("Fog bank"),
                FogBank,
                Particles3d(fog.0.clone()),
                Transform::from_xyz(
                    wx,
                    tile_defs.floor_level + ground.height(wx, wz) + 0.15,
                    wz,
                ),
                visibility,
            ));
        }
    }
}

/// Show the banks with the fog, hide them without.
pub fn show_fog_banks(level: Res<FogLevel>, mut banks: Query<&mut Visibility, With<FogBank>>) {
    if !level.is_changed() {
        return;
    }
    for mut visibility in &mut banks {
        *visibility = if *level == FogLevel::Off {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{BitFlags, MapFeatures};

    fn map(lattice: Vec<Vec<u16>>) -> MapFile {
        let size = (lattice.len() - 1) * ELEVATION_STEP + 1;
        MapFile {
            tiles: vec![vec![BitFlags::from(MapFeatures::Grass).bits(); size]; size],
            elevation: lattice,
            ..MapFile::default()
        }
    }

    #[test]
    fn banks_lie_in_the_dips() {
        // A bowl: high at the rim, lowest in the middle.
        let n = 11;
        let lattice = (0..n)
            .map(|j: i32| {
                (0..n)
                    .map(|i: i32| (((i - 5).pow(2) + (j - 5).pow(2)) * 20) as u16)
                    .collect()
            })
            .collect();
        let banks = plan_banks(&map(lattice));
        assert_eq!(banks, vec![(10, 10)], "the bottom of the bowl");
    }

    #[test]
    fn flat_ground_has_no_banks() {
        assert!(plan_banks(&map(vec![vec![100; 6]; 6])).is_empty());
        assert!(plan_banks(&MapFile::default()).is_empty());
    }
}
