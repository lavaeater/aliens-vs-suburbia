//! The keyboard side of the map editor: a modal editor, vim-style.
//!
//! A cursor sits on the grid (arrows move it, Shift moves five; hovering the mouse moves
//! it too). The mode decides what moving and the action keys do:
//!
//! | Key | Mode | |
//! |-----|------|-|
//! | Esc | Normal | moving does nothing; Space stamps one tile, Delete erases one |
//! | P | Paint | pen down: every tile the cursor enters is painted |
//! | X | Erase | the same, erasing |
//! | V | Select | a rectangle from where V was pressed; Enter/Space/F fills it, X/Delete erases it, H makes it a house |
//! | H | House | Space drops a corner, Backspace takes one back, Enter closes the shape |
//! | / | Search | type to filter every brush in every tab; Up/Down, Enter picks, Esc backs out |
//!
//! Pressing a mode's key again goes back to Normal. Picking the brush, in any mode: Tab /
//! Shift+Tab cycles the category, `[` `]` (or `,` `.`) steps through it, 1-9 picks directly,
//! `/` searches, I takes the brush from the tile under the cursor and `'` swaps back to the
//! brush before. R / Shift+R rotates. U or Ctrl+Z undoes, Ctrl+Y (or Ctrl+Shift+Z) redoes,
//! Ctrl+S saves, Ctrl+Q quits; Esc never leaves the editor.
//!
//! The House tab's brushes are the houses saved in the house editor (`assets/houses`).
//! Stamping one puts the whole house with its top-left tile at the cursor, replacing any
//! house it overlaps (so painting with it drags the house along); it turns in quarters
//! with R; erasing any of its tiles removes all of it, and I picks it up. Houses are kept
//! whole in the map (`MapFile::houses`); the game stamps them into the tiles on load. (H,
//! the House mode, is the older tool that stamps a drawn outline straight into the tiles.)
//!
//! Pure: keys arrive as [`EditorKey`], already translated from Bevy's input by
//! `ui::handle_editor_keys`, so all of it is testable without an app.

use crate::general::components::map_components::{HousePlacement, TilePlacement};
use crate::map_editor::state::{MapEditorState, PaletteItem, PaletteTab};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Paint,
    Erase,
    /// A rectangle from `anchor` to the cursor.
    Select {
        anchor: (i32, i32),
    },
    House,
    Search,
}

impl Mode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Paint => "PAINT",
            Self::Erase => "ERASE",
            Self::Select { .. } => "SELECT",
            Self::House => "HOUSE",
            Self::Search => "SEARCH",
        }
    }

    /// The same mode, ignoring a selection's anchor.
    const fn same_kind(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Normal, Self::Normal)
                | (Self::Paint, Self::Paint)
                | (Self::Erase, Self::Erase)
                | (Self::Select { .. }, Self::Select { .. })
                | (Self::House, Self::House)
                | (Self::Search, Self::Search)
        )
    }
}

/// A key as the editor sees it. Letters arrive lowercase; Shift is in [`Mods`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorKey {
    Up,
    Down,
    Left,
    Right,
    Space,
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Char(char),
}

impl EditorKey {
    /// Cursor step for an arrow key. Row 0 is the top of the grid.
    pub const fn direction(self) -> Option<(i32, i32)> {
        match self {
            Self::Up => Some((0, -1)),
            Self::Down => Some((0, 1)),
            Self::Left => Some((-1, 0)),
            Self::Right => Some((1, 0)),
            _ => None,
        }
    }

    /// Whether holding the key down should repeat it. Mode switches and one-off actions
    /// must not, or holding P would flicker between Paint and Normal.
    pub const fn repeats(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::Backspace
                | Self::Char('[' | ']' | ',' | '.')
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    Handled,
    Quit,
}

/// Shift+arrow moves this many tiles.
const FAST_STEP: i32 = 5;

// ── Undo ────────────────────────────────────────────────────────────────────

/// Undo steps kept; the oldest is dropped past this.
const UNDO_LIMIT: usize = 200;

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    tiles: Vec<Vec<u64>>,
    placements: Vec<TilePlacement>,
    houses: Vec<HousePlacement>,
}

/// Undo / redo of the tiles, placements and houses (the waves are edited in their own panel and
/// are not part of it).
#[derive(Default, Debug)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    /// Remember the map as it is, before an edit.
    pub fn checkpoint(
        &mut self,
        tiles: &[Vec<u64>],
        placements: &[TilePlacement],
        houses: &[HousePlacement],
    ) {
        let snapshot = Snapshot {
            tiles: tiles.to_vec(),
            placements: placements.to_vec(),
            houses: houses.to_vec(),
        };
        if self.undo.last() != Some(&snapshot) {
            self.undo.push(snapshot);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Step back. `false` when there is nothing that differs from the map as it is.
    pub fn undo(
        &mut self,
        tiles: &mut Vec<Vec<u64>>,
        placements: &mut Vec<TilePlacement>,
        houses: &mut Vec<HousePlacement>,
    ) -> bool {
        Self::step(&mut self.undo, &mut self.redo, tiles, placements, houses)
    }

    pub fn redo(
        &mut self,
        tiles: &mut Vec<Vec<u64>>,
        placements: &mut Vec<TilePlacement>,
        houses: &mut Vec<HousePlacement>,
    ) -> bool {
        Self::step(&mut self.redo, &mut self.undo, tiles, placements, houses)
    }

    fn step(
        from: &mut Vec<Snapshot>,
        to: &mut Vec<Snapshot>,
        tiles: &mut Vec<Vec<u64>>,
        placements: &mut Vec<TilePlacement>,
        houses: &mut Vec<HousePlacement>,
    ) -> bool {
        let current = Snapshot {
            tiles: tiles.clone(),
            placements: placements.clone(),
            houses: houses.clone(),
        };
        // A step that changed nothing (an edit that put back what was there) is skipped,
        // so undo always visibly does something.
        while let Some(snapshot) = from.pop() {
            if snapshot != current {
                *tiles = snapshot.tiles;
                *placements = snapshot.placements;
                *houses = snapshot.houses;
                to.push(current);
                return true;
            }
        }
        false
    }
}

// ── Search ──────────────────────────────────────────────────────────────────

/// How well `name` matches `query`, lower is better: a prefix, then anywhere in the name,
/// then the query's letters in order with gaps ("bw" finds "brick-wall"). `None` is no
/// match. An empty query matches everything.
pub fn match_score(name: &str, query: &str) -> Option<u8> {
    let name = name.to_lowercase();
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(3);
    }
    if name.starts_with(&query) {
        return Some(0);
    }
    if name.contains(&query) {
        return Some(1);
    }
    let mut letters = name.chars();
    query
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|q| letters.any(|n| n == q))
        .then_some(2)
}

// ── The editor ──────────────────────────────────────────────────────────────

impl MapEditorState {
    pub fn handle_key(&mut self, key: EditorKey, mods: Mods) -> KeyOutcome {
        // Anything but a move ends the current pen stroke, so it is one undo step.
        if key.direction().is_none() {
            self.stroke_open = false;
        }
        if self.mode == Mode::Search {
            self.search_key(key);
            return KeyOutcome::Handled;
        }
        if mods.ctrl {
            match key {
                EditorKey::Char('s') => self.save(),
                EditorKey::Char('z') if mods.shift => self.redo(),
                EditorKey::Char('z') => self.undo(),
                EditorKey::Char('y') => self.redo(),
                EditorKey::Char('q') => return KeyOutcome::Quit,
                _ => {}
            }
            return KeyOutcome::Handled;
        }
        if let Some((dx, dy)) = key.direction() {
            let step = if mods.shift { FAST_STEP } else { 1 };
            self.move_cursor(dx * step, dy * step);
            return KeyOutcome::Handled;
        }
        if !self.mode_key(key) {
            self.common_key(key, mods);
        }
        KeyOutcome::Handled
    }

    /// Keys that mean something particular in the current mode. `false` lets the key
    /// fall through to the ones that work everywhere.
    fn mode_key(&mut self, key: EditorKey) -> bool {
        match (self.mode, key) {
            (
                Mode::Select { anchor },
                EditorKey::Enter | EditorKey::Space | EditorKey::Char('f'),
            ) => {
                self.fill_rect(anchor, self.cursor);
                self.mode = Mode::Normal;
            }
            (
                Mode::Select { anchor },
                EditorKey::Char('x') | EditorKey::Delete | EditorKey::Backspace,
            ) => {
                self.erase_rect(anchor, self.cursor);
                self.mode = Mode::Normal;
            }
            (Mode::Select { anchor }, EditorKey::Char('h')) => {
                self.house_from_rect(anchor, self.cursor);
                self.mode = Mode::Normal;
            }
            (Mode::House, EditorKey::Space) => {
                let (x, y) = self.cursor;
                let before = self.house_points.len();
                self.house_add_point(x, y);
                if before > 0 && self.house_points.is_empty() {
                    self.status = "house built".into();
                }
            }
            (Mode::House, EditorKey::Backspace) => {
                self.house_points.pop();
            }
            (Mode::House, EditorKey::Enter) => {
                if self.house_points.len() >= 3 {
                    self.finish_house_polygon();
                    self.status = "house built".into();
                } else {
                    self.status = "a house needs at least 3 corners".into();
                }
            }
            (Mode::Normal | Mode::Paint, EditorKey::Space) => self.paint_cursor(),
            (Mode::Erase, EditorKey::Space) => self.erase_cursor(),
            _ => return false,
        }
        true
    }

    fn common_key(&mut self, key: EditorKey, mods: Mods) {
        match key {
            EditorKey::Escape => {
                if self.mode == Mode::Normal {
                    self.status = "Ctrl+Q quits to the menu".into();
                }
                self.set_mode(Mode::Normal);
            }
            EditorKey::Delete => self.erase_cursor(),
            EditorKey::Tab => self.cycle_tab(if mods.shift { -1 } else { 1 }),
            EditorKey::Char(c) => match c {
                'p' => self.toggle_mode(Mode::Paint),
                'x' => self.toggle_mode(Mode::Erase),
                'h' => self.toggle_mode(Mode::House),
                'v' => self.toggle_mode(Mode::Select {
                    anchor: self.cursor,
                }),
                'i' => self.eyedrop(),
                'r' => self.rotate_brush(mods.shift),
                '[' | ',' => self.step_brush(-1),
                ']' | '.' => self.step_brush(1),
                '/' => self.start_search(),
                '\'' => self.swap_brush(),
                'u' => self.undo(),
                '1'..='9' => self.pick_numbered(c as usize - '1' as usize),
                _ => {}
            },
            _ => {}
        }
    }

    /// Enter `mode`, or go back to Normal if already in it.
    fn toggle_mode(&mut self, mode: Mode) {
        if self.mode.same_kind(mode) {
            self.set_mode(Mode::Normal);
            return;
        }
        self.set_mode(mode);
        // Pen down: the tile under the cursor is the first one painted.
        match mode {
            Mode::Paint => self.paint_cursor(),
            Mode::Erase => self.erase_cursor(),
            _ => {}
        }
    }

    /// Switch mode, dropping a half-drawn house when leaving the House mode.
    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == Mode::House && mode != Mode::House && !self.house_points.is_empty() {
            self.cancel_house_polygon();
            self.status = "house cancelled".into();
        }
        self.mode = mode;
    }

    // ── Cursor and painting ─────────────────────────────────────────────────

    /// Move the cursor, painting (or erasing) every tile it passes in Paint (Erase).
    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        let steps = dx.abs().max(dy.abs());
        for _ in 0..steps {
            let next = (self.cursor.0 + dx.signum(), self.cursor.1 + dy.signum());
            if !self.in_bounds(next.0, next.1) {
                break;
            }
            self.cursor = next;
            match self.mode {
                Mode::Paint => self.paint_cursor(),
                Mode::Erase => self.erase_cursor(),
                _ => {}
            }
        }
    }

    pub fn paint_cursor(&mut self) {
        let (x, y) = self.cursor;
        if self.in_bounds(x, y) {
            self.begin_stroke();
            self.place_at(x, y);
        }
    }

    pub fn erase_cursor(&mut self) {
        let (x, y) = self.cursor;
        if self.in_bounds(x, y) {
            self.begin_stroke();
            self.erase_at(x, y);
        }
    }

    /// The tiles from `a` to `b` inclusive, as `(min, max)` corners.
    pub fn rect(a: (i32, i32), b: (i32, i32)) -> ((i32, i32), (i32, i32)) {
        ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
    }

    fn fill_rect(&mut self, a: (i32, i32), b: (i32, i32)) {
        self.checkpoint();
        let ((x0, y0), (x1, y1)) = Self::rect(a, b);
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.place_at(x, y);
            }
        }
    }

    fn erase_rect(&mut self, a: (i32, i32), b: (i32, i32)) {
        self.checkpoint();
        let ((x0, y0), (x1, y1)) = Self::rect(a, b);
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.erase_at(x, y);
            }
        }
    }

    fn house_from_rect(&mut self, a: (i32, i32), b: (i32, i32)) {
        let ((x0, y0), (x1, y1)) = Self::rect(a, b);
        if x0 == x1 || y0 == y1 {
            self.status = "a house needs a rectangle at least 2 tiles each way".into();
            return;
        }
        self.house_points = vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        self.finish_house_polygon();
        self.status = "house built".into();
    }

    // ── Brushes ─────────────────────────────────────────────────────────────

    fn step_brush(&mut self, delta: i32) {
        let len = self.palette_items.len() as i32;
        if len == 0 {
            return;
        }
        let index = (self.selected_palette as i32 + delta).rem_euclid(len) as usize;
        self.select_brush(self.active_tab, index);
    }

    fn cycle_tab(&mut self, delta: i32) {
        let tabs = PaletteTab::all();
        let index = (self.active_tab.index() as i32 + delta).rem_euclid(tabs.len() as i32);
        if let Some(&tab) = tabs.get(index as usize) {
            self.select_brush(tab, self.remembered(tab));
        }
    }

    fn pick_numbered(&mut self, index: usize) {
        if index < self.palette_items.len() {
            self.select_brush(self.active_tab, index);
        } else {
            self.status = format!("no brush {} in {}", index + 1, self.active_tab.label());
        }
    }

    fn swap_brush(&mut self) {
        match self.previous_brush {
            Some((tab, index)) => self.select_brush(tab, index),
            None => self.status = "no previous brush".into(),
        }
    }

    /// The catalog entry painting a def, matched by path, or by file name when the map
    /// spells the path differently.
    fn find_def(&self, def_path: &str) -> Option<(PaletteTab, usize)> {
        let stem = |p: &str| {
            std::path::Path::new(p)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        };
        let wanted = stem(def_path);
        PaletteTab::all().iter().find_map(|&tab| {
            self.tab_items(tab)
                .iter()
                .position(|item| match item {
                    PaletteItem::Def { path, .. } => path == def_path || stem(path) == wanted,
                    PaletteItem::Special { .. } | PaletteItem::House { .. } => false,
                })
                .map(|index| (tab, index))
        })
    }

    /// Take the brush from the tile under the cursor: its placement (and rotation) if it
    /// has one, else the special tile that paints its value.
    fn eyedrop(&mut self) {
        let (x, y) = self.cursor;
        if let Some(placed) = self.house_at(x, y).cloned() {
            let found = self.tab_items(PaletteTab::House).iter().position(
                |item| matches!(item, PaletteItem::House { name } if *name == placed.house),
            );
            match found {
                Some(index) => {
                    self.select_brush(PaletteTab::House, index);
                    self.rotation_steps = placed.turns.saturating_mul(2) % 8;
                    self.palette_dirty = true;
                }
                None => self.status = format!("house {} is not in the palette", placed.house),
            }
            return;
        }
        if let Some(placement) = self
            .placements
            .iter()
            .rev()
            .find(|p| p.x == x && p.y == y)
            .cloned()
        {
            match self.find_def(&placement.def_path) {
                Some((tab, index)) => {
                    self.select_brush(tab, index);
                    self.rotation_steps = placement.rotation_steps;
                    self.palette_dirty = true;
                }
                None => self.status = format!("{} is not in the palette", placement.def_path),
            }
            return;
        }
        let Some(&value) = self
            .tiles
            .get(y as usize)
            .and_then(|row| row.get(x as usize))
        else {
            return;
        };
        let special = PaletteTab::Special;
        let found = self.tab_items(special).iter().position(
            |item| matches!(item, PaletteItem::Special { tile_value, .. } if *tile_value == value),
        );
        match found {
            Some(index) => self.select_brush(special, index),
            None => self.status = format!("no brush paints this tile (flags {value:#x})"),
        }
    }

    // ── Search ──────────────────────────────────────────────────────────────

    fn start_search(&mut self) {
        // Not `set_mode`: a half-drawn house survives a search for the wall brush.
        self.mode_before_search = self.mode;
        self.mode = Mode::Search;
        self.search.clear();
        self.refresh_search();
    }

    fn end_search(&mut self) {
        self.mode = self.mode_before_search;
        self.palette_dirty = true;
    }

    fn search_key(&mut self, key: EditorKey) {
        match key {
            EditorKey::Escape => self.end_search(),
            EditorKey::Enter => self.pick_search_hit(self.search_selected),
            EditorKey::Up => {
                self.search_selected = self.search_selected.saturating_sub(1);
                self.palette_dirty = true;
            }
            EditorKey::Down | EditorKey::Tab => {
                self.search_selected =
                    (self.search_selected + 1).min(self.search_hits.len().saturating_sub(1));
                self.palette_dirty = true;
            }
            EditorKey::Backspace => {
                self.search.pop();
                self.refresh_search();
            }
            EditorKey::Space => {
                self.search.push(' ');
                self.refresh_search();
            }
            // ASCII only: the default font draws nothing else.
            EditorKey::Char(c) if c.is_ascii_graphic() => {
                self.search.push(c);
                self.refresh_search();
            }
            _ => {}
        }
    }

    /// Choose the `index`th hit as the brush and end the search.
    pub fn pick_search_hit(&mut self, index: usize) {
        let Some(&(tab, item)) = self.search_hits.get(index) else {
            self.status = "nothing matches".into();
            return;
        };
        self.end_search();
        self.select_brush(tab, item);
    }

    fn refresh_search(&mut self) {
        let query = self.search.as_str();
        let mut hits: Vec<(u8, String, PaletteTab, usize)> = PaletteTab::all()
            .iter()
            .flat_map(|&tab| {
                self.tab_items(tab)
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, item)| {
                        let name = item.display_name();
                        match_score(name, query)
                            .map(|score| (score, name.to_lowercase(), tab, index))
                    })
            })
            .collect();
        hits.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        self.search_hits = hits.into_iter().map(|(_, _, tab, i)| (tab, i)).collect();
        self.search_selected = 0;
        self.palette_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_editor::state::{TILE_SPECIAL_FLOOR, TILE_VOID, TILE_WALL_ALL};

    const NONE: Mods = Mods {
        shift: false,
        ctrl: false,
    };
    const SHIFT: Mods = Mods {
        shift: true,
        ctrl: false,
    };
    const CTRL: Mods = Mods {
        shift: false,
        ctrl: true,
    };

    fn def(name: &str) -> PaletteItem {
        PaletteItem::Def {
            path: format!("assets/defs/{name}.ron"),
            name: name.into(),
        }
    }

    /// A small editor with a known catalog instead of whatever is in `assets/defs`.
    fn editor() -> MapEditorState {
        let mut s = MapEditorState::default();
        s.catalog = vec![
            vec![def("brick-wall"), def("hedge"), def("fence")],
            vec![def("cannon")],
            vec![def("medkit")],
            vec![def("golem")],
            vec![PaletteItem::House {
                name: "box".into(),
            }],
            s.catalog[PaletteTab::Special.index()].clone(),
        ];
        s.house_library = [(
            "box".to_string(),
            crate::map::house::HouseDef {
                outline: vec![(0, 0), (3, 0), (3, 2), (0, 2)],
                ..Default::default()
            },
        )]
        .into();
        s.width = 8;
        s.height = 6;
        s.tiles = vec![vec![TILE_SPECIAL_FLOOR; 8]; 6];
        s.set_tab(PaletteTab::Special);
        s
    }

    fn press(s: &mut MapEditorState, keys: &[EditorKey]) {
        for &k in keys {
            s.handle_key(k, NONE);
        }
    }

    fn type_text(s: &mut MapEditorState, text: &str) {
        for c in text.chars() {
            s.handle_key(EditorKey::Char(c), NONE);
        }
    }

    fn select_special(s: &mut MapEditorState, label: &str) {
        let index = s.catalog[PaletteTab::Special.index()]
            .iter()
            .position(|i| i.display_name() == label)
            .unwrap();
        s.select_brush(PaletteTab::Special, index);
    }

    use EditorKey::{Backspace, Char, Down, Enter, Escape, Right, Space, Tab};

    #[test]
    fn paint_mode_paints_every_tile_the_cursor_enters_and_undoes_as_one_step() {
        let mut s = editor();
        select_special(&mut s, "Wall (all)");
        press(&mut s, &[Char('p'), Right, Right, Down]);
        for (x, y) in [(0, 0), (1, 0), (2, 0), (2, 1)] {
            assert_eq!(s.tiles[y][x], TILE_WALL_ALL, "({x},{y}) painted");
        }
        assert_eq!(
            s.tiles[0][3], TILE_SPECIAL_FLOOR,
            "only where the cursor went"
        );

        // Lifting the pen and moving paints nothing.
        press(&mut s, &[Char('p'), Right]);
        assert_eq!(s.mode, Mode::Normal);
        assert_eq!(s.tiles[1][3], TILE_SPECIAL_FLOOR);

        s.handle_key(Char('u'), NONE);
        assert!(
            s.tiles.iter().flatten().all(|&t| t == TILE_SPECIAL_FLOOR),
            "the whole stroke goes in one undo"
        );
        s.handle_key(Char('y'), CTRL);
        assert_eq!(s.tiles[1][2], TILE_WALL_ALL, "and comes back with redo");
    }

    #[test]
    fn shift_moves_five_and_stops_at_the_edge() {
        let mut s = editor();
        s.handle_key(Right, SHIFT);
        assert_eq!(s.cursor, (5, 0));
        s.handle_key(Right, SHIFT);
        assert_eq!(s.cursor, (7, 0), "clamped to the 8-wide grid");
    }

    #[test]
    fn erase_mode_clears_tiles_and_placements() {
        let mut s = editor();
        s.select_brush(PaletteTab::Terrain, 1);
        press(&mut s, &[Space]);
        assert_eq!(s.placements.len(), 1, "Space stamps in Normal");
        press(&mut s, &[Char('x'), Right]);
        assert!(s.placements.is_empty());
        assert_eq!(s.tiles[0][0], TILE_VOID);
        assert_eq!(s.tiles[0][1], TILE_VOID);
    }

    #[test]
    fn a_selection_fills_or_erases_its_rectangle() {
        let mut s = editor();
        select_special(&mut s, "Wall (all)");
        press(&mut s, &[Right, Char('v'), Right, Right, Down, Enter]);
        assert_eq!(s.mode, Mode::Normal);
        let walls = s
            .tiles
            .iter()
            .flatten()
            .filter(|&&t| t == TILE_WALL_ALL)
            .count();
        assert_eq!(walls, 6, "a 3x2 rectangle");
        assert_eq!(
            s.tiles[0][0], TILE_SPECIAL_FLOOR,
            "left of the anchor untouched"
        );

        press(&mut s, &[Char('v'), Escape]);
        assert_eq!(s.mode, Mode::Normal, "Esc drops a selection");

        s.cursor = (1, 0);
        press(&mut s, &[Char('v'), Right, Char('x')]);
        assert_eq!(s.tiles[0][1], TILE_VOID);
        assert_eq!(s.tiles[0][2], TILE_VOID);
        assert_eq!(s.tiles[0][3], TILE_WALL_ALL);
    }

    #[test]
    fn a_house_can_be_drawn_from_the_keyboard() {
        let mut s = editor();
        s.width = 12;
        s.height = 12;
        s.tiles = vec![vec![TILE_SPECIAL_FLOOR; 12]; 12];
        s.house_wall_def = "assets/defs/brick-wall.ron".into();
        let before = (s.tiles.clone(), s.placements.clone());
        s.cursor = (1, 1);
        press(&mut s, &[Char('v')]);
        for _ in 0..6 {
            press(&mut s, &[Right, Down]);
        }
        press(&mut s, &[Char('h')]);
        assert_ne!(
            (s.tiles.clone(), s.placements.clone()),
            before,
            "walls went in"
        );
        s.handle_key(Char('u'), NONE);
        assert_eq!((s.tiles.clone(), s.placements.clone()), before, "one undo");

        // Corner by corner in House mode, with Backspace taking one back.
        press(
            &mut s,
            &[
                Char('h'),
                Space,
                Right,
                Right,
                Space,
                Down,
                Space,
                Backspace,
            ],
        );
        assert_eq!(s.house_points.len(), 2);
        press(&mut s, &[Escape]);
        assert!(
            s.house_points.is_empty(),
            "leaving cancels the half-drawn house"
        );
    }

    #[test]
    fn search_finds_brushes_in_any_tab_and_returns_to_the_mode() {
        let mut s = editor();
        press(&mut s, &[Char('p'), Char('p')]); // back in Normal, nothing painted over
        s.mode = Mode::Paint;
        s.handle_key(Char('/'), NONE);
        assert_eq!(s.mode, Mode::Search);
        type_text(&mut s, "gol");
        assert_eq!(
            s.search_hits[0],
            (PaletteTab::Enemy, 0),
            "the prefix match first"
        );
        s.handle_key(Enter, NONE);
        assert_eq!(
            s.mode,
            Mode::Paint,
            "back to the mode the search started from"
        );
        assert_eq!(s.active_tab, PaletteTab::Enemy);
        assert_eq!(
            s.selected_item().map(PaletteItem::display_name),
            Some("golem")
        );

        // Typed letters are text, not commands.
        s.handle_key(Char('/'), NONE);
        type_text(&mut s, "px");
        assert_eq!(s.search, "px");
        s.handle_key(Escape, NONE);
        assert_eq!(s.mode, Mode::Paint);
    }

    #[test]
    fn match_score_prefers_prefixes_then_substrings_then_scattered_letters() {
        assert_eq!(match_score("brick-wall", "bri"), Some(0));
        assert_eq!(match_score("brick-wall", "wall"), Some(1));
        assert_eq!(match_score("brick-wall", "bw"), Some(2));
        assert_eq!(match_score("brick-wall", "wb"), None);
        assert_eq!(match_score("Alien Spawn", "SPAWN"), Some(1));
    }

    #[test]
    fn a_house_brush_places_moves_turns_and_erases_whole_houses() {
        let mut s = editor();
        s.select_brush(PaletteTab::House, 0);
        s.cursor = (1, 1);
        press(&mut s, &[Space]);
        assert_eq!(s.houses.len(), 1);
        assert_eq!(s.house_cells(&s.houses[0]).len(), 6, "3 x 2");
        assert_eq!(s.tiles[1][1], TILE_SPECIAL_FLOOR, "the tiles stay as painted");

        // Overlapping the first replaces it: a drag moves the house along.
        s.cursor = (2, 1);
        press(&mut s, &[Space]);
        assert_eq!(s.houses.len(), 1);
        assert_eq!((s.houses[0].x, s.houses[0].y), (2, 1));

        // A house turns in quarters.
        s.handle_key(Char('r'), NONE);
        assert_eq!(s.house_turns(), 1);
        s.cursor = (6, 3);
        press(&mut s, &[Space]);
        assert_eq!(s.houses.len(), 2, "no overlap, both stay");
        assert!(s.house_cells(&s.houses[1]).contains(&(7, 5)), "2 wide, 3 deep");

        // Erasing any tile of a house removes all of it; undo brings it back.
        s.cursor = (4, 2);
        press(&mut s, &[EditorKey::Delete]);
        assert_eq!(s.houses.len(), 1);
        press(&mut s, &[Char('u')]);
        assert_eq!(s.houses.len(), 2);

        // The eyedropper picks a house and its turn.
        s.select_brush(PaletteTab::Terrain, 0);
        s.rotation_steps = 0;
        s.cursor = (7, 5);
        s.handle_key(Char('i'), NONE);
        assert_eq!(s.active_tab, PaletteTab::House);
        assert_eq!(s.house_turns(), 1);

        let map = s.to_map_file();
        assert_eq!(map.houses, s.houses, "saved with the map");
    }

    #[test]
    fn brushes_cycle_by_tab_step_number_and_swap() {
        let mut s = editor();
        s.handle_key(Tab, NONE);
        assert_eq!(
            s.active_tab,
            PaletteTab::Terrain,
            "Special wraps round to Terrain"
        );
        press(&mut s, &[Char(']'), Char(']')]);
        assert_eq!(s.selected_palette, 2);
        s.handle_key(Char('.'), NONE);
        assert_eq!(s.selected_palette, 0, "wraps");
        s.handle_key(Char('3'), NONE);
        assert_eq!(s.selected_palette, 2);

        s.handle_key(Tab, SHIFT);
        assert_eq!(s.active_tab, PaletteTab::Special);
        s.handle_key(Tab, NONE);
        assert_eq!(s.selected_palette, 2, "a tab remembers its selection");

        s.handle_key(Char('1'), NONE);
        s.handle_key(Char('\''), NONE);
        assert_eq!(s.selected_palette, 2, "swap goes back to the brush before");
        s.handle_key(Char('9'), NONE);
        assert_eq!(s.selected_palette, 2, "no 9th brush: unchanged");
    }

    #[test]
    fn the_eyedropper_takes_the_placement_and_its_rotation_or_the_tile() {
        let mut s = editor();
        s.select_brush(PaletteTab::Terrain, 1);
        s.rotation_steps = 3;
        press(&mut s, &[Space]);
        select_special(&mut s, "Player Spawn");
        s.rotation_steps = 0;
        press(&mut s, &[Right, Space, Char('x'), Char('x')]);

        s.cursor = (0, 0);
        s.handle_key(Char('i'), NONE);
        assert_eq!((s.active_tab, s.selected_palette), (PaletteTab::Terrain, 1));
        assert_eq!(s.rotation_steps, 3);

        s.cursor = (1, 0);
        s.handle_key(Char('i'), NONE);
        assert_eq!(
            s.selected_item().map(PaletteItem::display_name),
            Some("Void"),
            "erased tile picks the Void brush"
        );
    }

    #[test]
    fn escape_never_quits_but_ctrl_q_does() {
        let mut s = editor();
        assert_eq!(s.handle_key(Escape, NONE), KeyOutcome::Handled);
        assert_eq!(s.handle_key(Escape, NONE), KeyOutcome::Handled);
        assert_eq!(s.handle_key(Char('q'), CTRL), KeyOutcome::Quit);
    }

    #[test]
    fn rotation_goes_both_ways() {
        let mut s = editor();
        s.handle_key(Char('r'), SHIFT);
        assert_eq!(s.rotation_steps, 7);
        s.handle_key(Char('r'), NONE);
        assert_eq!(s.rotation_steps, 0);
    }

    #[test]
    fn undo_with_nothing_done_says_so() {
        let mut s = editor();
        s.handle_key(Char('u'), NONE);
        assert_eq!(s.status, "nothing to undo");
    }
}
