//! The campaign being edited: its `campaign.ron`, its levels' `.map` files, and the base
//! map new levels are cut from. No ECS beyond the resource itself; every panel and tool
//! edits through [`CampaignDoc`]'s methods.
//!
//! Dirty is computed: a level is dirty when its `.map` text differs from what was read
//! (or last saved), the campaign when its file differs. Save writes only what changed.
//! Removing a level takes it out of the campaign; its files stay on disk.

use std::path::Path;

use bevy::prelude::Resource;

use crate::assets::paths::{content_ref, disk_path};
use crate::campaign::{CampaignFile, CampaignLevel, campaign_file};
use crate::general::components::map_components::{
    ELEVATION_STEP, MapFile, Marker, MarkerKind, Region, TilePlacement,
};
use crate::map::compose::MIN_SIZE;
use crate::map::text_format;

/// One level of the campaign, as a file.
#[derive(Debug, Clone)]
pub struct LevelDoc {
    /// The level's `.map`, inside `assets/`.
    pub path: String,
    pub map: MapFile,
    /// The `.map` text as read or last saved; empty for a level never saved.
    saved: String,
}

impl LevelDoc {
    fn text(&self) -> String {
        text_format::to_text(&self.map).unwrap_or_default()
    }

    pub fn is_dirty(&self) -> bool {
        self.saved.is_empty() || self.text() != self.saved
    }

    /// Whether the level is cut from a base map (only those are edited here).
    pub fn on_base(&self) -> bool {
        !self.map.base.is_empty()
    }

    pub fn count(&self, kind: MarkerKind) -> usize {
        self.map.markers.iter().filter(|m| m.kind == kind).count()
    }
}

/// The campaign being edited.
#[derive(Resource, Debug, Default)]
pub struct CampaignDoc {
    /// The campaign's folder name under `assets/campaigns`.
    pub name: String,
    pub file: CampaignFile,
    saved_file: CampaignFile,
    /// One per `file.levels`, in the same order.
    pub levels: Vec<LevelDoc>,
    /// The base map new levels are cut from, inside `assets/`. Empty = none.
    pub base_ref: String,
    pub base: Option<MapFile>,
    pub selected: Option<usize>,
    /// Bumped on every change, so views rebuild only when something changed.
    pub revision: u64,
    /// The last thing that happened, for the status line.
    pub status: String,
    /// Undo and redo (`undo` / `redo`): the states before and after, newest last.
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The state the last recorded edit left; what an undo goes back past.
    last: Option<Snapshot>,
    /// While set (a drag in progress), edits are not recorded one by one; releasing it
    /// records them as one step.
    hold: bool,
}

/// How many steps undo keeps.
const UNDO_DEPTH: usize = 100;

/// What undo restores: the campaign file and each level's map.
#[derive(Debug, Clone)]
struct Snapshot {
    file: CampaignFile,
    levels: Vec<(String, MapFile)>,
    selected: Option<usize>,
}

impl Snapshot {
    /// Whether two snapshots hold the same content (the selection aside).
    fn same(&self, other: &Self) -> bool {
        let text = |m: &MapFile| text_format::to_text(m).unwrap_or_default();
        self.file == other.file
            && self.levels.len() == other.levels.len()
            && self
                .levels
                .iter()
                .zip(&other.levels)
                .all(|((pa, ma), (pb, mb))| pa == pb && text(ma) == text(mb))
    }
}

/// A new level's story file (shows its intro crawl when the level starts, sets the
/// objective line) and the crawl, for a level whose files are named `stem`.
pub fn story_templates(stem: &str, title: &str) -> (String, String) {
    let title: String = title
        .chars()
        .filter(|c| c.is_ascii() && *c != '"' && *c != '\\')
        .collect();
    let story = format!(
        r#"// {stem}'s own stories, loaded on top of the base level-flow ones (`MapFile::stories`).
// Made by the campaign editor; format: assets/campaigns/main/level_01/level_01_story.ron.
[
    (
        name: "{stem} intro",
        repeat: false,
        init_facts: [
            ("ObjectiveText", Text("Do not let them reach the goal")),
        ],
        rules: [
            (name: "started", criteria: [BoolIs(key: "LevelStarted", expected: true)]),
        ],
        consequences: [Emit(effect: "crawl:{stem}_crawl_intro")],
    ),
]
"#
    );
    let crawl = format!(
        r#"// {stem}'s intro crawl, made by the campaign editor. Fields: see
// assets/campaigns/main/level_01/level_01_crawl_intro.ron. Plain ASCII only.
(
    title: "{upper}",
    lines: [
        "Placeholder: what happens here, and why it matters.",
        "",
        "Hold the line...",
    ],
    style: Scroll(tilt: 0.0),
    speed: 55.0,
    backdrop: 0.85,
)
"#,
        upper = title.to_uppercase()
    );
    (story, crawl)
}

/// The rectangle spanned by two tiles, both inside it.
pub fn spanning(a: (i32, i32), b: (i32, i32)) -> Region {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    Region::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// `region` made playable on a `width` x `height` base: inside it, its origin on an even
/// tile (the elevation lattice; the far edges stay), at least [`MIN_SIZE`] a side.
pub fn fit_region(region: Region, width: i32, height: i32) -> Region {
    let step = ELEVATION_STEP as i32;
    let fit = |start: i32, size: i32, limit: i32| {
        let end = (start + size).clamp(0, limit);
        let start = (start.clamp(0, limit) / step) * step;
        let end = end.max(start + MIN_SIZE).min(limit);
        let start = start.min(((end - MIN_SIZE).max(0) / step) * step);
        (start, end - start)
    };
    let (x, w) = fit(region.x, region.width, width);
    let (y, h) = fit(region.y, region.height, height);
    Region::new(x, y, w, h)
}

impl CampaignDoc {
    /// Read campaign `name`: its file, every level's `.map` (not composed), and the base.
    pub fn load(name: &str) -> Result<Self, String> {
        let path = campaign_file(name);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let file: CampaignFile =
            ron::from_str(&text).map_err(|e| format!("cannot parse {path}: {e}"))?;
        let mut levels = Vec::new();
        for level in &file.levels {
            let map = text_format::read_map_file(disk_path(&level.map))?;
            let saved = text_format::to_text(&map)?;
            levels.push(LevelDoc {
                path: content_ref(&level.map),
                map,
                saved,
            });
        }
        let base_ref = if file.base.is_empty() {
            levels
                .iter()
                .find(|l| l.on_base())
                .map_or_else(String::new, |l| l.map.base.clone())
        } else {
            content_ref(&file.base)
        };
        let base = if base_ref.is_empty() {
            None
        } else {
            Some(text_format::read_map_file(disk_path(&base_ref))?)
        };
        let status = if base.is_some() {
            format!("Opened {name}: {} level(s) on {base_ref}", levels.len())
        } else {
            format!(
                "Opened {name}: no base map. Make one with --split-level from a whole level."
            )
        };
        let mut doc = Self {
            name: name.to_string(),
            saved_file: file.clone(),
            file,
            levels,
            base_ref,
            base,
            selected: None,
            revision: 1,
            status,
            ..Self::default()
        };
        doc.last = Some(doc.snapshot());
        Ok(doc)
    }

    /// An empty editor that says why `name` could not be opened.
    pub fn failed(name: &str, why: String) -> Self {
        Self {
            name: name.to_string(),
            status: why,
            ..Self::default()
        }
    }

    /// The base map's size in tiles.
    pub fn base_size(&self) -> (i32, i32) {
        self.base.as_ref().map_or((0, 0), |b| {
            (
                b.tiles.first().map_or(0, Vec::len) as i32,
                b.tiles.len() as i32,
            )
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.file != self.saved_file || self.levels.iter().any(LevelDoc::is_dirty)
    }

    /// Something changed: views rebuild, and the edit is recorded for undo.
    fn touch(&mut self, status: impl Into<String>) {
        self.bump(status);
        if !self.hold {
            self.record();
        }
    }

    /// Views rebuild; nothing to undo (selection, status).
    fn bump(&mut self, status: impl Into<String>) {
        self.revision = self.revision.wrapping_add(1);
        self.status = status.into();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            file: self.file.clone(),
            levels: self
                .levels
                .iter()
                .map(|l| (l.path.clone(), l.map.clone()))
                .collect(),
            selected: self.selected,
        }
    }

    /// Keep the state as it is now as the next undo step, unless nothing changed.
    fn record(&mut self) {
        let now = self.snapshot();
        match self.last.take() {
            Some(before) if !before.same(&now) => {
                self.undo.push(before);
                if self.undo.len() > UNDO_DEPTH {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
            _ => {}
        }
        self.last = Some(now);
    }

    /// Put a snapshot back. Each level keeps what its file holds (`saved`), so dirty
    /// stays true to the disk.
    fn restore(&mut self, snapshot: &Snapshot) {
        self.file = snapshot.file.clone();
        self.levels = snapshot
            .levels
            .iter()
            .map(|(path, map)| {
                let saved = self
                    .levels
                    .iter()
                    .find(|l| &l.path == path)
                    .map_or_else(String::new, |l| l.saved.clone());
                LevelDoc {
                    path: path.clone(),
                    map: map.clone(),
                    saved,
                }
            })
            .collect();
        self.selected = snapshot.selected.filter(|&i| i < self.levels.len());
    }

    /// Hold recording while a drag goes on (`true`), then record it as one step.
    pub fn hold_history(&mut self, hold: bool) {
        self.hold = hold;
        if !hold {
            self.record();
        }
    }

    /// Go back one step. Whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some(before) = self.undo.pop() else {
            self.bump("Nothing to undo");
            return false;
        };
        let now = self.last.take().unwrap_or_else(|| self.snapshot());
        self.redo.push(now);
        self.restore(&before);
        self.last = Some(before);
        self.bump(format!("Undone ({} more)", self.undo.len()));
        true
    }

    /// Go forward one step. Whether there was one.
    pub fn redo(&mut self) -> bool {
        let Some(after) = self.redo.pop() else {
            self.bump("Nothing to redo");
            return false;
        };
        if let Some(now) = self.last.take() {
            self.undo.push(now);
        }
        self.restore(&after);
        self.last = Some(after);
        self.bump(format!("Redone ({} more)", self.redo.len()));
        true
    }

    pub const fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub const fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn select(&mut self, index: Option<usize>) {
        let index = index.filter(|&i| i < self.levels.len());
        if self.selected != index {
            self.selected = index;
            self.bump(String::new());
        }
    }

    pub fn selected_level(&self) -> Option<&LevelDoc> {
        self.selected.and_then(|i| self.levels.get(i))
    }

    /// The region a level is played on (all of the base without one), in base tiles.
    pub fn region_of(&self, index: usize) -> Option<Region> {
        let level = self.levels.get(index).filter(|l| l.on_base())?;
        let (w, h) = self.base_size();
        Some(level.map.region.unwrap_or(Region::new(0, 0, w, h)))
    }

    /// A level folder name not used yet, after the campaign: `gis_01`, `gis_02`, ... (crawl ids
    /// are file stems, shared by every campaign, so `level_02` would clash with `main`'s).
    fn free_slug(&self) -> String {
        let campaign_dir = Path::new(crate::assets::content::CAMPAIGNS).join(&self.name);
        (1..1000)
            .map(|n| format!("{}_{n:02}", self.name))
            .find(|slug| {
                let path = format!("campaigns/{}/{slug}/{slug}.map", self.name);
                !campaign_dir.join(slug).exists() && !self.levels.iter().any(|l| l.path == path)
            })
            .unwrap_or_else(|| format!("{}_new", self.name))
    }

    /// A new level on the base, played on `region`, added last and selected.
    pub fn add_level(&mut self, region: Region) -> Option<usize> {
        if self.base.is_none() {
            self.touch("No base map to cut a level from");
            return None;
        }
        let (w, h) = self.base_size();
        let slug = self.free_slug();
        let path = format!("campaigns/{}/{slug}/{slug}.map", self.name);
        let number = self.levels.len() + 1;
        let map = MapFile {
            base: self.base_ref.clone(),
            region: Some(fit_region(region, w, h)),
            map_width: 0,
            map_height: 0,
            ..MapFile::default()
        };
        self.file.levels.push(CampaignLevel {
            map: path.clone(),
            title: format!("Level {number}"),
            tagline: String::new(),
        });
        self.levels.push(LevelDoc {
            path: path.clone(),
            map,
            saved: String::new(),
        });
        let index = self.levels.len() - 1;
        self.selected = Some(index);
        self.touch(format!("Added level {number} ({path})"));
        Some(index)
    }

    /// Take a level out of the campaign (its files stay).
    pub fn remove_level(&mut self, index: usize) {
        if index >= self.levels.len() {
            return;
        }
        let level = self.levels.remove(index);
        self.file.levels.remove(index);
        self.selected = None;
        self.touch(format!(
            "Removed {} from the campaign (its files are kept)",
            level.path
        ));
    }

    /// Move a level one place earlier (`up`) or later in the campaign.
    pub fn move_level(&mut self, index: usize, up: bool) {
        let other = if up {
            index.checked_sub(1)
        } else {
            Some(index + 1)
        };
        let Some(other) = other.filter(|&o| o < self.levels.len() && index < self.levels.len())
        else {
            return;
        };
        self.levels.swap(index, other);
        self.file.levels.swap(index, other);
        self.selected = Some(other);
        self.touch(format!("Moved to place {}", other + 1));
    }

    /// Play level `index` on `region` (fitted to the base).
    pub fn set_region(&mut self, index: usize, region: Region) {
        let (w, h) = self.base_size();
        let fitted = fit_region(region, w, h);
        let Some(level) = self.levels.get_mut(index).filter(|l| l.on_base()) else {
            return;
        };
        if level.map.region != Some(fitted) {
            level.map.region = Some(fitted);
            self.touch(String::new());
        }
    }

    pub fn set_title(&mut self, index: usize, title: String) {
        if let Some(level) = self.file.levels.get_mut(index) {
            level.title = title;
            self.touch(String::new());
        }
    }

    pub fn set_tagline(&mut self, index: usize, tagline: String) {
        if let Some(level) = self.file.levels.get_mut(index) {
            level.tagline = tagline;
            self.touch(String::new());
        }
    }

    pub fn set_victory_title(&mut self, title: String) {
        self.file.victory_title = title;
        self.touch(String::new());
    }

    /// Put a marker on tile `(x, y)` of level `index`. A goal replaces the level's goal,
    /// a fifth player start the oldest. Outside the region it is refused.
    pub fn place_marker(&mut self, index: usize, kind: MarkerKind, x: i32, y: i32) {
        let Some(region) = self.region_of(index) else {
            return;
        };
        if !region.contains(x, y) {
            self.touch("Markers go inside the level's region");
            return;
        }
        let Some(level) = self.levels.get_mut(index) else {
            return;
        };
        let markers = &mut level.map.markers;
        markers.retain(|m| (m.x, m.y) != (x, y));
        match kind {
            MarkerKind::Goal => markers.retain(|m| m.kind != MarkerKind::Goal),
            MarkerKind::Player if markers.iter().filter(|m| m.kind == kind).count() >= 4 => {
                if let Some(first) = markers.iter().position(|m| m.kind == kind) {
                    markers.remove(first);
                }
            }
            _ => {}
        }
        markers.push(Marker { kind, x, y });
        self.touch(format!("{} at {x}, {y}", kind.name()));
    }

    /// Move the marker on tile `from` of level `index` to `to` (inside the region).
    pub fn move_marker(&mut self, index: usize, from: (i32, i32), to: (i32, i32)) {
        let Some(region) = self.region_of(index) else {
            return;
        };
        if from == to || !region.contains(to.0, to.1) {
            return;
        }
        let Some(level) = self.levels.get_mut(index) else {
            return;
        };
        let markers = &mut level.map.markers;
        if !markers.iter().any(|m| (m.x, m.y) == from) {
            return;
        }
        markers.retain(|m| (m.x, m.y) != to);
        if let Some(m) = markers.iter_mut().find(|m| (m.x, m.y) == from) {
            (m.x, m.y) = to;
        }
        self.touch(String::new());
    }

    /// Remove the marker on tile `(x, y)` of level `index`. Whether there was one.
    pub fn remove_marker(&mut self, index: usize, x: i32, y: i32) -> bool {
        let Some(level) = self.levels.get_mut(index) else {
            return false;
        };
        let before = level.map.markers.len();
        level.map.markers.retain(|m| (m.x, m.y) != (x, y));
        let removed = level.map.markers.len() != before;
        if removed {
            self.touch(format!("Removed the marker at {x}, {y}"));
        }
        removed
    }

    /// Put def `def_path` on tile `(x, y)` of level `index` (base tiles), turned
    /// `rotation_steps` eighths; it replaces whatever was placed there.
    pub fn place_def(&mut self, index: usize, def_path: &str, (x, y): (i32, i32), rotation_steps: u8) {
        let Some(region) = self.region_of(index) else {
            return;
        };
        if !region.contains(x, y) || region.on_edge(x, y) {
            self.touch("Placements go inside the level's walls");
            return;
        }
        let Some(level) = self.levels.get_mut(index) else {
            return;
        };
        level.map.placements.retain(|p| (p.x, p.y) != (x, y));
        level.map.placements.push(TilePlacement {
            x,
            y,
            def_path: def_path.to_string(),
            rotation_steps: rotation_steps % 8,
        });
        let name = std::path::Path::new(def_path)
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        self.touch(format!("{name} at {x}, {y}"));
    }

    /// Take the placement off tile `(x, y)` of level `index`. Whether there was one.
    pub fn remove_placement(&mut self, index: usize, x: i32, y: i32) -> bool {
        let Some(level) = self.levels.get_mut(index) else {
            return false;
        };
        let before = level.map.placements.len();
        level.map.placements.retain(|p| (p.x, p.y) != (x, y));
        let removed = level.map.placements.len() != before;
        if removed {
            self.touch(format!("Removed the placement at {x}, {y}"));
        }
        removed
    }

    /// Put markers on level `index` where `analysis::auto_markers` says (all of its
    /// markers are replaced).
    pub fn auto_markers(&mut self, index: usize) {
        let (Some(base), Some(level)) = (self.base.as_ref(), self.levels.get(index)) else {
            return;
        };
        match super::analysis::auto_markers(base, &level.map) {
            Ok(markers) => {
                let spawns = markers.iter().filter(|m| m.kind == MarkerKind::Spawn).count();
                if let Some(level) = self.levels.get_mut(index) {
                    level.map.markers = markers;
                }
                self.touch(format!("Goal, player start and {spawns} spawn(s) placed"));
            }
            Err(e) => self.bump(format!("No markers: {e}")),
        }
    }

    /// Replace level `index`'s alien spawns with `spawns` (base tiles).
    pub fn set_spawns(&mut self, index: usize, spawns: &[(i32, i32)]) {
        let Some(level) = self.levels.get_mut(index) else {
            return;
        };
        level.map.markers.retain(|m| m.kind != MarkerKind::Spawn);
        level
            .map
            .markers
            .extend(spawns.iter().map(|&(x, y)| Marker {
                kind: MarkerKind::Spawn,
                x,
                y,
            }));
        self.touch(format!("{} alien spawn(s) where roads leave", spawns.len()));
    }

    /// Edit level `index`'s map (rules and the like) in place.
    pub fn edit_map(&mut self, index: usize, edit: impl FnOnce(&mut MapFile)) {
        if let Some(level) = self.levels.get_mut(index) {
            edit(&mut level.map);
            self.touch(String::new());
        }
    }

    /// Give level `index` a story file and an intro crawl next to its map (from a template,
    /// titled after the level), and name the story in the level. Files that exist are kept.
    pub fn create_story(&mut self, index: usize) -> Result<(), String> {
        let (Some(level), Some(entry)) = (self.levels.get(index), self.file.levels.get(index))
        else {
            return Ok(());
        };
        let map_path = disk_path(&level.path);
        let stem = map_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .ok_or("the level has no file name")?;
        let dir = map_path.parent().ok_or("the level has no folder")?;
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let (story_file, crawl_file) = story_templates(&stem, &entry.title);
        let story = dir.join(format!("{stem}_story.ron"));
        let crawl = dir.join(format!("{stem}_crawl_intro.ron"));
        for (path, text) in [(&story, story_file), (&crawl, crawl_file)] {
            if !path.exists() {
                std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        let story_ref = content_ref(&story.to_string_lossy());
        self.edit_map(index, |map| map.stories = story_ref);
        self.status = format!("Story and intro crawl in {}", dir.display());
        Ok(())
    }

    /// Write every dirty level and the campaign file.
    pub fn save(&mut self) -> Result<usize, String> {
        let mut written = 0;
        for level in &mut self.levels {
            if !level.is_dirty() {
                continue;
            }
            let path = disk_path(&level.path);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            text_format::write_map_file(&path, &level.map)?;
            level.saved = level.text();
            written += 1;
        }
        if self.file != self.saved_file {
            let path = campaign_file(&self.name);
            let body = ron::ser::to_string_pretty(&self.file, ron::ser::PrettyConfig::new())
                .map_err(|e| e.to_string())?;
            let text = format!(
                "// The {} campaign (`campaign::CampaignFile`), written by the campaign editor:\n\
                 // cargo run -- --campaign-editor {}. Levels in order; plain ASCII only.\n{body}\n",
                self.name, self.name
            );
            std::fs::write(&path, text).map_err(|e| format!("cannot write {path}: {e}"))?;
            self.saved_file = self.file.clone();
            written += 1;
        }
        self.touch(format!("Saved {written} file(s)"));
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> CampaignDoc {
        let base = MapFile {
            tiles: vec![vec![1; 40]; 30],
            ..MapFile::default()
        };
        CampaignDoc {
            name: "test_campaign_that_does_not_exist".into(),
            base_ref: "campaigns/x/world/world.map".into(),
            base: Some(base),
            // The default file has a level of its own; the doc's lists go in step.
            file: CampaignFile {
                levels: Vec::new(),
                ..CampaignFile::default()
            },
            ..CampaignDoc::default()
        }
    }

    #[test]
    fn a_region_is_fitted_to_the_base_and_the_lattice() {
        assert_eq!(
            fit_region(Region::new(3, 5, 10, 10), 40, 30),
            Region::new(2, 4, 11, 11)
        );
        assert_eq!(
            fit_region(Region::new(-4, 25, 10, 10), 40, 30),
            Region::new(0, 24, 6, 6),
            "clipped, and still MIN_SIZE"
        );
        assert_eq!(
            fit_region(Region::new(38, 0, 1, 1), 40, 30),
            Region::new(34, 0, 6, 5)
        );
        assert_eq!(spanning((9, 2), (3, 7)), Region::new(3, 2, 7, 6));
    }

    #[test]
    fn levels_are_added_moved_and_removed_in_step_with_the_file() {
        let mut doc = doc();
        let a = doc.add_level(Region::new(0, 0, 10, 10)).unwrap();
        let b = doc.add_level(Region::new(10, 10, 10, 10)).unwrap();
        assert_eq!((a, b), (0, 1));
        assert_ne!(doc.levels[0].path, doc.levels[1].path);
        assert_eq!(doc.file.levels[1].map, doc.levels[1].path);
        assert!(doc.is_dirty() && doc.levels[0].is_dirty());
        doc.move_level(1, true);
        assert_eq!(doc.file.levels[0].title, "Level 2");
        assert_eq!(doc.file.levels[0].map, doc.levels[0].path);
        assert_eq!(doc.selected, Some(0));
        doc.remove_level(0);
        assert_eq!(doc.levels.len(), 1);
        assert_eq!(doc.file.levels[0].title, "Level 1");
    }

    #[test]
    fn markers_stay_inside_and_a_level_has_one_goal() {
        let mut doc = doc();
        let i = doc.add_level(Region::new(10, 10, 10, 10)).unwrap();
        doc.place_marker(i, MarkerKind::Goal, 12, 12);
        doc.place_marker(i, MarkerKind::Goal, 13, 13);
        doc.place_marker(i, MarkerKind::Player, 1, 1);
        assert_eq!(doc.levels[i].count(MarkerKind::Goal), 1);
        assert_eq!(doc.levels[i].count(MarkerKind::Player), 0, "outside");
        for x in 11..16 {
            doc.place_marker(i, MarkerKind::Player, x, 15);
        }
        assert_eq!(doc.levels[i].count(MarkerKind::Player), 4);
        doc.move_marker(i, (13, 13), (14, 14));
        assert!(doc.levels[i].map.markers.iter().any(|m| m.kind == MarkerKind::Goal && (m.x, m.y) == (14, 14)));
        assert!(doc.remove_marker(i, 14, 14));
        assert_eq!(doc.levels[i].count(MarkerKind::Goal), 0);
    }

    #[test]
    fn the_story_templates_parse() {
        let (story, crawl) = story_templates("gis_09", "The \"Mall\"");
        let stories = turbofacts::persistence::stories_from_ron(&story).expect("story parses");
        assert_eq!(stories.len(), 1);
        assert!(story.contains("crawl:gis_09_crawl_intro"));
        let crawl: lava_ui_builder::crawl::CrawlSpec = ron::from_str(&crawl).expect("crawl parses");
        assert_eq!(crawl.title, "THE MALL");
    }

    #[test]
    fn placements_go_inside_the_walls_one_per_tile() {
        let mut doc = doc();
        let i = doc.add_level(Region::new(10, 10, 10, 10)).unwrap();
        doc.place_def(i, "defs/Barrel.ron", (12, 12), 9);
        doc.place_def(i, "defs/Crate.ron", (12, 12), 0);
        doc.place_def(i, "defs/Barrel.ron", (10, 12), 0);
        doc.place_def(i, "defs/Barrel.ron", (1, 1), 0);
        let placed = &doc.levels[i].map.placements;
        assert_eq!(placed.len(), 1, "one per tile, none on the wall or outside");
        assert_eq!(placed[0].def_path, "defs/Crate.ron");
        assert!(doc.remove_placement(i, 12, 12));
        assert!(!doc.remove_placement(i, 12, 12));
    }

    #[test]
    fn undo_and_redo_step_through_the_edits() {
        let mut doc = doc();
        doc.last = Some(doc.snapshot());
        let i = doc.add_level(Region::new(0, 0, 10, 10)).unwrap();
        doc.set_title(i, "Mall".into());
        doc.select(None);
        assert_eq!(doc.undo.len(), 2, "add and title; selecting is no step");
        assert!(doc.undo());
        assert_eq!(doc.file.levels[0].title, "Level 1");
        assert!(doc.undo());
        assert!(doc.levels.is_empty() && doc.file.levels.is_empty());
        assert!(!doc.undo());
        assert!(doc.redo() && doc.redo());
        assert_eq!(doc.file.levels[0].title, "Mall");
        assert!(!doc.redo());
        // A new edit after an undo drops what could be redone.
        doc.undo();
        doc.set_tagline(i, "x".into());
        assert!(!doc.can_redo());
    }

    #[test]
    fn a_held_drag_is_one_step() {
        let mut doc = doc();
        let i = doc.add_level(Region::new(0, 0, 10, 10)).unwrap();
        let steps = doc.undo.len();
        doc.hold_history(true);
        for x in 0..5 {
            doc.set_region(i, Region::new(x * 2, 0, 10, 10));
        }
        doc.hold_history(false);
        assert_eq!(doc.undo.len(), steps + 1);
        doc.undo();
        assert_eq!(doc.region_of(i), Some(Region::new(0, 0, 10, 10)));
    }

    #[test]
    fn dirty_follows_the_text() {
        let mut level = LevelDoc {
            path: "x.map".into(),
            map: MapFile {
                base: "b.map".into(),
                ..MapFile::default()
            },
            saved: String::new(),
        };
        assert!(level.is_dirty(), "never saved");
        level.saved = level.text();
        assert!(!level.is_dirty());
        level.map.escape_limit = Some(2);
        assert!(level.is_dirty());
    }
}
