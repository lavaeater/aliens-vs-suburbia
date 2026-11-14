//! The selected level checked as the game would play it: composed from the base
//! (`map::compose`), then walked from the goal. Recomputed when the level changes (at
//! most a few times a second while dragging).
// Grid code: indices come from the grid's own ranges.
#![allow(clippy::indexing_slicing)]

use bevy::prelude::*;

use super::doc::CampaignDoc;
use super::view::Drag;
use crate::general::components::map_components::{MapFile, MarkerKind};
use crate::map::compose::{compose_level, effective_region};
use crate::general::components::map_components::Marker;
use crate::map::reach::{
    distances, enemy_passable, inward, perimeter, player_passable, suggest_spawns,
};

/// What the check found, in base map tiles.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Analysis {
    /// Markers that don't work: spawns that can't reach the goal, player starts the
    /// players can't walk from to the goal.
    pub bad: Vec<(i32, i32)>,
    /// Where roads leave the region, widest first.
    pub suggested_spawns: Vec<(i32, i32)>,
    /// Tiles aliens can walk on, and how many of them can't reach the goal.
    pub open: usize,
    pub cut_off: usize,
    /// One line per problem, for the panel.
    pub problems: Vec<String>,
    /// The doc revision and level this is for.
    for_revision: u64,
    for_level: Option<usize>,
}

/// How many alien spawns "suggest" offers.
pub const SUGGESTED: usize = 4;

/// Check a level on `base` (pure).
pub fn analyse(base: &MapFile, level: &MapFile) -> Analysis {
    let mut out = Analysis::default();
    let region = match effective_region(base, level) {
        Ok(r) => r,
        Err(e) => {
            out.problems.push(e);
            return out;
        }
    };
    let map = match compose_level(base, level) {
        Ok(map) => map,
        Err(e) => {
            out.problems.push(e);
            return out;
        }
    };
    let local = |x: i32, y: i32| ((x - region.x) as usize, (y - region.y) as usize);
    out.suggested_spawns = suggest_spawns(&map.tiles, &map.roads, SUGGESTED)
        .into_iter()
        .map(|(c, r)| (c as i32 + region.x, r as i32 + region.y))
        .collect();
    out.open = map
        .tiles
        .iter()
        .flatten()
        .filter(|&&t| enemy_passable(t))
        .count();

    let count = |kind| level.markers.iter().filter(|m| m.kind == kind).count();
    if count(MarkerKind::Player) == 0 {
        out.problems.push("No player start (tool 1)".into());
    }
    let classic = level.infestation.is_none();
    if classic && count(MarkerKind::Spawn) == 0 {
        out.problems.push("No alien spawn (tool 2, or S to suggest)".into());
    }
    let goal = level.markers.iter().find(|m| m.kind == MarkerKind::Goal);
    let Some(goal) = goal else {
        if classic {
            out.problems.push("No goal (tool 3)".into());
        }
        return out;
    };
    let aliens = distances(&map.tiles, local(goal.x, goal.y), enemy_passable);
    let players = distances(&map.tiles, local(goal.x, goal.y), player_passable);
    out.cut_off = map
        .tiles
        .iter()
        .zip(&aliens)
        .flat_map(|(t, d)| t.iter().zip(d))
        .filter(|(t, d)| enemy_passable(**t) && d.is_none())
        .count();
    for m in &level.markers {
        let (c, r) = local(m.x, m.y);
        let reach = match m.kind {
            MarkerKind::Spawn => &aliens,
            MarkerKind::Player => &players,
            MarkerKind::Goal => continue,
        };
        if reach.get(r).and_then(|row| row.get(c)).copied().flatten().is_none() {
            out.bad.push((m.x, m.y));
            out.problems.push(match m.kind {
                MarkerKind::Spawn => format!("Spawn at {}, {} can't reach the goal", m.x, m.y),
                _ => format!("Player start at {}, {} can't walk to the goal", m.x, m.y),
            });
        }
    }
    out
}

/// Markers for a level that has none yet, in base tiles: the goal on the open tile nearest
/// the middle of its region, alien spawns where roads leave it (or, with no road out, the
/// edge farthest from the goal), and the player start a few steps' walk from the goal.
/// Only spawns that reach the goal are kept.
pub fn auto_markers(base: &MapFile, level: &MapFile) -> Result<Vec<Marker>, String> {
    let bare = MapFile {
        markers: Vec::new(),
        ..level.clone()
    };
    let region = effective_region(base, &bare)?;
    let map = compose_level(base, &bare)?;
    let (width, height) = (region.width as usize, region.height as usize);
    let open = |c: usize, r: usize| {
        let t = map.tiles[r][c];
        enemy_passable(t) && player_passable(t)
    };
    let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
    let goal = (1..height - 1)
        .flat_map(|r| (1..width - 1).map(move |c| (c, r)))
        .filter(|&(c, r)| open(c, r))
        .min_by(|a, b| {
            let d = |&(c, r): &(usize, usize)| (c as f32 - cx).powi(2) + (r as f32 - cy).powi(2);
            d(a).total_cmp(&d(b))
        })
        .ok_or("no open ground in the region for a goal")?;

    let aliens = distances(&map.tiles, goal, enemy_passable);
    let reaches = |(c, r): (usize, usize)| {
        // An edge tile is a wall until the marker opens it: judge it by the tile inside.
        inward(c, r, width, height)
            .is_some_and(|(ic, ir)| aliens[ir][ic].is_some())
    };
    let mut spawns: Vec<(usize, usize)> = suggest_spawns(&map.tiles, &map.roads, SUGGESTED)
        .into_iter()
        .filter(|&s| reaches(s))
        .collect();
    if spawns.is_empty() {
        let farthest = perimeter(width, height)
            .into_iter()
            .filter_map(|(c, r)| {
                let (ic, ir) = inward(c, r, width, height)?;
                aliens[ir][ic].map(|d| (d, (c, r)))
            })
            .max_by_key(|&(d, _)| d)
            .map(|(_, at)| at);
        spawns.extend(farthest);
    }
    if spawns.is_empty() {
        return Err("no edge of the region leads to the goal".into());
    }

    let walk = distances(&map.tiles, goal, player_passable);
    let start = (1..height - 1)
        .flat_map(|r| (1..width - 1).map(move |c| (c, r)))
        .filter(|&(c, r)| (c, r) != goal && open(c, r))
        .filter_map(|(c, r)| walk[r][c].map(|d| (d, (c, r))))
        .min_by_key(|&(d, _)| d.abs_diff(4))
        .map(|(_, at)| at)
        .ok_or("no room for a player start next to the goal")?;

    let base_tile = |(c, r): (usize, usize)| (c as i32 + region.x, r as i32 + region.y);
    let mut markers = vec![
        Marker {
            kind: MarkerKind::Goal,
            x: base_tile(goal).0,
            y: base_tile(goal).1,
        },
        Marker {
            kind: MarkerKind::Player,
            x: base_tile(start).0,
            y: base_tile(start).1,
        },
    ];
    markers.extend(spawns.into_iter().map(|s| Marker {
        kind: MarkerKind::Spawn,
        x: base_tile(s).0,
        y: base_tile(s).1,
    }));
    Ok(markers)
}

/// Re-check the selected level when it changed.
pub fn update_analysis(
    doc: Res<CampaignDoc>,
    drag: Res<Drag>,
    time: Res<Time>,
    mut analysis: ResMut<Analysis>,
    mut last: Local<f32>,
) {
    if analysis.for_revision == doc.revision && analysis.for_level == doc.selected {
        return;
    }
    let dragging = !matches!(*drag, Drag::None | Drag::Pan(_));
    let now = time.elapsed_secs();
    if dragging && now - *last < 0.3 {
        return;
    }
    *last = now;
    let mut fresh = match (&doc.base, doc.selected_level()) {
        (Some(base), Some(level)) if level.on_base() => analyse(base, &level.map),
        _ => Analysis::default(),
    };
    fresh.for_revision = doc.revision;
    fresh.for_level = doc.selected;
    if *analysis != fresh {
        *analysis = fresh;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::{Marker, Region};
    use crate::map::{BitFlags, MapFeatures};

    fn base() -> MapFile {
        let grass = BitFlags::from(MapFeatures::Grass).bits();
        let wall = (MapFeatures::Floor
            | MapFeatures::ImpassableForEnemies
            | MapFeatures::ImpassableForPlayers)
            .bits();
        // A wall down the middle, x = 10.
        let tiles = (0..20)
            .map(|_| (0..20).map(|x| if x == 10 { wall } else { grass }).collect())
            .collect();
        MapFile {
            tiles,
            ..MapFile::default()
        }
    }

    fn level(markers: &[(MarkerKind, i32, i32)]) -> MapFile {
        MapFile {
            base: "b.map".into(),
            region: Some(Region::new(0, 0, 20, 20)),
            markers: markers
                .iter()
                .map(|&(kind, x, y)| Marker { kind, x, y })
                .collect(),
            ..MapFile::default()
        }
    }

    #[test]
    fn a_spawn_behind_a_wall_is_bad() {
        let a = analyse(
            &base(),
            &level(&[
                (MarkerKind::Goal, 5, 5),
                (MarkerKind::Player, 6, 5),
                (MarkerKind::Spawn, 0, 8),
                (MarkerKind::Spawn, 19, 8),
            ]),
        );
        assert_eq!(a.bad, vec![(19, 8)]);
        assert!(a.cut_off > 0);
        assert_eq!(a.problems.len(), 1, "{:?}", a.problems);
    }

    #[test]
    fn auto_markers_make_a_level_that_checks_out() {
        // The wall down x = 10 leaves the west half; with no roads, the spawn is the
        // edge tile farthest from the goal.
        let mut level = level(&[]);
        level.region = Some(Region::new(0, 0, 10, 20));
        let markers = auto_markers(&base(), &level).unwrap();
        let count = |kind| markers.iter().filter(|m| m.kind == kind).count();
        assert_eq!((count(MarkerKind::Goal), count(MarkerKind::Player)), (1, 1));
        assert!(count(MarkerKind::Spawn) >= 1);
        let a = analyse(&base(), &MapFile { markers, ..level });
        assert!(a.problems.is_empty(), "{:?}", a.problems);
    }

    /// On the real GIS town: every classic level's region gets markers that check out.
    #[test]
    fn auto_markers_work_on_the_gis_levels() {
        let base = crate::map::text_format::read_map_file("assets/campaigns/gis/world/world.map")
            .expect("the gis base map");
        for region in [
            Region::new(78, 78, 80, 80),
            Region::new(60, 20, 100, 90),
            Region::new(40, 130, 96, 96),
            Region::new(130, 100, 100, 100),
        ] {
            let level = MapFile {
                base: "campaigns/gis/world/world.map".into(),
                region: Some(region),
                ..MapFile::default()
            };
            let markers = auto_markers(&base, &level).unwrap();
            let a = analyse(&base, &MapFile { markers, ..level });
            assert!(a.problems.is_empty(), "{region:?}: {:?}", a.problems);
        }
    }

    #[test]
    fn a_level_without_markers_says_what_it_needs() {
        let a = analyse(&base(), &level(&[]));
        assert_eq!(a.problems.len(), 3, "{:?}", a.problems);
    }
}
