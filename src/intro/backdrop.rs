//! The town round the level, for the intro to fly over.
//!
//! A level on a base map (`map::compose`) is a small cut of a much larger place; the copter
//! comes in from far outside it. While it flies, the whole base map is drawn around and
//! under the level, lined up with it: the same ground, walls, roads, houses and trees, built
//! at once by `streaming::spawn_scenery`, with nothing to collide with. The level's own
//! chunk scenery (`ChunkScenery`: its border wall among it) is hidden meanwhile; its
//! lamps, grass and the stand-ins stay. At the hand-over the backdrop goes and the level
//! shows again.

use bevy::prelude::*;

use super::Intro;
use crate::assets::paths::disk_path;
use crate::campaign::Campaign;
use crate::general::components::map_components::{MapFile, Region};
use crate::general::systems::map_systems::TileDefinitions;
use crate::map::ground::{GroundHeight, Hills};
use crate::map::streaming::{ChunkScenery, spawn_scenery};
use crate::settings::resources::GameSettings;

/// The base map drawn round the level.
#[derive(Clone)]
pub struct Backdrop {
    pub root: Entity,
    /// The base map's ground, in its own world coordinates.
    pub ground: GroundHeight,
    /// Added to a base world position to get the level's.
    pub offset: Vec3,
}

impl Backdrop {
    /// The ground's height over `floor_level` at a level world position, anywhere in the
    /// base, not only on the level.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.ground.height(x - self.offset.x, z - self.offset.z) + self.offset.y
    }
}

/// How far the base map is moved to line up with the level: back by the region's corner,
/// down by the ground the level was lowered by.
pub fn offset_of(region: Region, lowest_cm: u16, tile: f32) -> Vec3 {
    Vec3::new(
        -(region.x as f32) * tile,
        -f32::from(lowest_cm) / 100.0,
        -(region.y as f32) * tile,
    )
}

/// The base map under a level file (inside `assets/`), and where the level lies in it.
/// `None` for a level that is not on a base map.
pub fn surroundings(level: &str) -> Result<Option<(MapFile, Region, u16)>, String> {
    let path = disk_path(level);
    let path = if path.with_extension("map").is_file() {
        path.with_extension("map")
    } else {
        path
    };
    let level = crate::map::text_format::read_map_file(&path)?;
    if level.base.is_empty() {
        return Ok(None);
    }
    let base = crate::map::text_format::read_map_file(disk_path(&level.base))?;
    let (region, lowest) = crate::map::compose::level_offset(&base, &level)?;
    Ok(Some((base, region, lowest)))
}

/// Draw the base map round the current level, once, before the copter takes off.
#[allow(clippy::too_many_arguments)]
pub fn build_backdrop(
    mut commands: Commands,
    mut intro: ResMut<Intro>,
    campaign: Res<Campaign>,
    tile_defs: Option<Res<TileDefinitions>>,
    hills: Option<Res<Hills>>,
    settings: Res<GameSettings>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if intro.backdrop_tried || intro.level.is_none() {
        return;
    }
    intro.backdrop_tried = true;
    let Some(tile_defs) = tile_defs else {
        return;
    };
    let Some(level) = campaign.file.levels.get(campaign.current) else {
        return;
    };
    let (base, region, lowest) = match surroundings(&level.map) {
        Ok(Some(found)) => found,
        Ok(None) => return,
        Err(e) => {
            warn!("intro: no town round the level ({e})");
            return;
        }
    };
    let tile = tile_defs.tile_width;
    let offset = offset_of(region, lowest, tile);
    let ground = GroundHeight::for_level(&base, hills.is_some(), tile);
    let root = commands
        .spawn((
            Name::new("Intro backdrop"),
            Transform::from_translation(offset),
            Visibility::default(),
        ))
        .id();
    spawn_scenery(
        &mut commands,
        root,
        base,
        &tile_defs,
        &ground,
        settings.player_unit,
        &asset_server,
        &mut meshes,
        &mut materials,
    );
    info!(
        "intro: the town round the level ({}x{} at {}, {})",
        region.width, region.height, region.x, region.y
    );
    intro.backdrop = Some(Backdrop {
        root,
        ground,
        offset,
    });
}

/// Hide the level's chunk scenery while the backdrop draws the place, show it after.
pub fn level_scenery(intro: Res<Intro>, mut scenery: Query<&mut Visibility, With<ChunkScenery>>) {
    let wanted = if intro.backdrop.is_some() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut scenery {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Take the backdrop away (the hand-over, or leaving the match).
pub fn remove_backdrop(commands: &mut Commands, intro: &mut Intro) {
    if let Some(backdrop) = intro.backdrop.take() {
        commands.entity(backdrop.root).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_lines_up_with_the_level() {
        let region = Region::new(40, 20, 80, 80);
        let offset = offset_of(region, 250, 1.0);
        // The level's tile (0, 0) is the base's (40, 20): base world x 41 is level world x 1.
        assert_eq!(Vec3::new(41.0, 3.0, 21.0) + offset, Vec3::new(1.0, 0.5, 1.0));
    }

    #[test]
    fn the_gis_level_ones_town_is_found() {
        let found = surroundings("campaigns/gis/map_01/map_01.map").unwrap();
        let (base, region, _) = found.expect("map_01 is on a base map");
        assert!(base.tiles.len() > region.height as usize);
        assert!(!base.buildings.is_empty());
    }
}
