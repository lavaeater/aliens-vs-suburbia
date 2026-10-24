use crate::map::house::{self, HOUSE_DIR, HouseDef};
use crate::map::procgen::{HouseResolveResult, resolve_house};
use bevy::prelude::Resource;

/// State for the standalone House Editor (`GameState::HouseEditor`): one house at a
/// time, drawn as a polygon on the canvas, its rules edited in the side panel, saved as
/// `assets/houses/<name>.ron` (`map::house`).
///
/// `points` is the outline being drawn; once closed it is the house's outline (a click
/// after that starts a new house). The resolved preview is re-run on every change.
#[derive(Resource)]
pub struct HouseEditorState {
    /// Nodes of the polygon, in click order. The house's outline once closed.
    pub points: Vec<(i32, i32)>,
    /// The resolved house while the outline is closed.
    pub resolved: Option<HouseResolveResult>,
    /// The rules; its `outline` is synced from `points` (see [`Self::current`]).
    pub house: HouseDef,
    /// The def drawn for each wall tile when a house is stamped into a map.
    pub wall_def: String,
    /// The file the house was opened from or last saved to.
    pub path: Option<String>,
    /// The house as last saved or opened, to tell whether there are unsaved edits.
    pub saved: Option<HouseDef>,
    /// A line for the panel: what just happened.
    pub status: String,
    /// Set when New/Open was refused over unsaved edits; the next one goes ahead.
    pub confirm_discard: bool,
    /// True whenever spec/points/resolved changed and the side-panel label needs a refresh.
    pub info_dirty: bool,
}

impl Default for HouseEditorState {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            resolved: None,
            house: HouseDef::default(),
            wall_def: house::default_wall_def().unwrap_or_default(),
            path: None,
            saved: None,
            status: String::new(),
            confirm_discard: false,
            info_dirty: true,
        }
    }
}

impl HouseEditorState {
    /// Adds a click as the next polygon node. Closing happens when a click lands back on
    /// the first node with at least 3 nodes already placed (mirrors the map editor's House
    /// tool), or via `close()`/Enter. Once closed, the next click starts a new outline
    /// (the rules are kept).
    pub fn add_point(&mut self, x: i32, y: i32) {
        if self.resolved.is_some() {
            self.points.clear();
            self.resolved = None;
        }
        if let Some(&first) = self.points.first()
            && self.points.len() >= 3
            && (x, y) == first
        {
            self.close();
            return;
        }
        if self.points.last() != Some(&(x, y)) {
            self.points.push((x, y));
        }
        self.info_dirty = true;
    }

    /// Closes the outline (no-op below 3 points) and resolves it. `points` stays, so the
    /// outline shows next to the result and rule changes re-resolve the same shape.
    pub fn close(&mut self) {
        if self.points.len() >= 3 {
            self.house.outline = self.points.clone();
            self.resolve();
        }
        self.info_dirty = true;
    }

    fn resolve(&mut self) {
        self.resolved = Some(resolve_house(
            &self.house.polygon(),
            &self.house.spec(&self.wall_def),
        ));
    }

    /// Takes back the last node. On a closed house it reopens the polygon for editing.
    pub fn remove_last_point(&mut self) {
        self.points.pop();
        self.resolved = None;
        self.info_dirty = true;
    }

    /// Cancels the in-progress polygon without touching any previously resolved result.
    pub fn cancel_points(&mut self) {
        self.points.clear();
        self.info_dirty = true;
    }

    /// Clears the outline, keeping the rules and the file.
    pub fn clear_all(&mut self) {
        self.points.clear();
        self.resolved = None;
        self.info_dirty = true;
    }

    /// Change one property a step (`map::house::step`) and show the result at once.
    pub fn step_field(&mut self, id: &str, forward: bool) {
        house::step(&mut self.house, id, forward);
        if self.resolved.is_some() {
            self.resolve();
        }
        self.info_dirty = true;
    }

    /// The house as it would be saved: the rules with the drawn outline.
    pub fn current(&self) -> HouseDef {
        HouseDef {
            outline: self.points.clone(),
            ..self.house.clone()
        }
    }

    /// Edits that are not in a file. An empty, never-saved editor has none.
    pub fn is_dirty(&self) -> bool {
        match &self.saved {
            Some(saved) => *saved != self.current(),
            None => !self.points.is_empty() || self.house != HouseDef::default(),
        }
    }

    /// The house's name: its file stem, or "(new house)".
    pub fn name(&self) -> String {
        self.path
            .as_deref()
            .map_or_else(|| "(new house)".to_string(), house::house_name)
    }

    /// Whether a New/Open may drop the current house: yes when nothing is unsaved, or on
    /// the second try after a warning.
    fn may_discard(&mut self) -> bool {
        if !self.is_dirty() || self.confirm_discard {
            self.confirm_discard = false;
            return true;
        }
        self.confirm_discard = true;
        self.status = "Unsaved changes - again to discard them".into();
        self.info_dirty = true;
        false
    }

    /// Start a fresh house with default rules.
    pub fn new_house(&mut self) {
        if !self.may_discard() {
            return;
        }
        let wall_def = std::mem::take(&mut self.wall_def);
        *self = Self {
            wall_def,
            status: "New house".into(),
            ..Self::default()
        };
    }

    /// Open a house file, replacing the current one.
    pub fn open(&mut self, path: &str) {
        if !self.may_discard() {
            return;
        }
        match HouseDef::load(path) {
            Ok(house) => {
                self.points = house.outline.clone();
                self.saved = Some(house.clone());
                self.house = house;
                self.path = Some(path.to_string());
                self.resolved = None;
                if self.points.len() >= 3 {
                    self.resolve();
                }
                self.status = format!("Opened {}", self.name());
            }
            Err(e) => self.status = e,
        }
        self.info_dirty = true;
    }

    /// Save to the file it came from, or to the next free `house_<n>.ron`. Only a closed
    /// outline is saved.
    pub fn save(&mut self) {
        self.save_in(HOUSE_DIR);
    }

    fn save_in(&mut self, dir: &str) {
        self.info_dirty = true;
        if self.resolved.is_none() {
            self.status = "Close the outline before saving".into();
            return;
        }
        let path = self
            .path
            .clone()
            .unwrap_or_else(|| house::free_house_path(dir));
        let house = self.current();
        match house.save(&path) {
            Ok(()) => {
                self.saved = Some(house);
                self.path = Some(path);
                self.confirm_discard = false;
                self.status = format!("Saved {}", self.name());
            }
            Err(e) => self.status = e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> HouseEditorState {
        HouseEditorState {
            wall_def: "wall.ron".into(),
            info_dirty: false,
            ..HouseEditorState::default()
        }
    }

    fn draw_rect(s: &mut HouseEditorState) {
        for (x, y) in [(0, 0), (6, 0), (6, 4), (0, 4), (0, 0)] {
            s.add_point(x, y);
        }
    }

    #[test]
    fn clicking_the_first_node_closes_and_the_next_click_starts_over() {
        let mut s = state();
        draw_rect(&mut s);
        assert_eq!(s.points.len(), 4);
        let house = s.resolved.as_ref().expect("closed");
        assert_eq!(house.floor_cells.len() + house.wall_cells.len(), 24);
        s.add_point(10, 10);
        assert_eq!(s.points, vec![(10, 10)]);
        assert!(s.resolved.is_none());
    }

    #[test]
    fn backspace_reopens_a_closed_house() {
        let mut s = state();
        for (x, y) in [(0, 0), (4, 0), (4, 4)] {
            s.add_point(x, y);
        }
        s.close();
        s.remove_last_point();
        assert_eq!(s.points, vec![(0, 0), (4, 0)]);
        assert!(s.resolved.is_none());
    }

    #[test]
    fn a_rule_change_re_resolves_the_closed_house() {
        let mut s = state();
        draw_rect(&mut s);
        assert_eq!(s.resolved.as_ref().unwrap().door_count(), 1);
        s.step_field("doors", true);
        assert_eq!(s.resolved.as_ref().unwrap().door_count(), 2);
    }

    #[test]
    fn save_writes_a_file_that_opens_the_same() {
        let dir = std::env::temp_dir().join(format!("avs-houses-{}", std::process::id()));
        let dir = dir.to_string_lossy().into_owned();
        let mut s = state();
        s.save_in(&dir);
        assert!(s.path.is_none(), "an open outline is not saved");

        draw_rect(&mut s);
        s.step_field("roof", true);
        assert!(s.is_dirty());
        s.save_in(&dir);
        let path = s.path.clone().expect("saved");
        assert!(path.ends_with("house_1.ron"), "{path}");
        assert!(!s.is_dirty());

        let mut other = state();
        other.open(&path);
        assert_eq!(other.current(), s.current());
        assert!(!other.is_dirty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsaved_edits_need_a_second_try_to_drop() {
        let mut s = state();
        draw_rect(&mut s);
        s.new_house();
        assert!(!s.points.is_empty(), "first try only warns");
        s.new_house();
        assert!(s.points.is_empty());
        assert_eq!(s.wall_def, "wall.ron", "the wall def is kept");
    }
}
