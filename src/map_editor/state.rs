use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::general::components::map_components::{MapFile, TilePlacement, WaveDef};
use crate::map::MapFeatures;
use crate::map::procgen::{HouseSpec, Polygon, apply_house, resolve_house};
use crate::map::text_format;
use crate::map_editor::modal::{History, Mode};
use bevy::prelude::Resource;

pub const TILE_SPECIAL_FLOOR: u64 = MapFeatures::Floor as u64;
pub const TILE_ALIEN_SPAWN: u64 = MapFeatures::Floor as u64 | MapFeatures::EnemySpawn as u64;
pub const TILE_ALIEN_GOAL: u64 = MapFeatures::Floor as u64 | MapFeatures::EnemyExit as u64;
pub const TILE_PLAYER_SPAWN: u64 = MapFeatures::Floor as u64 | MapFeatures::PlayerSpawn as u64;
pub const TILE_WALL_ALL: u64 = MapFeatures::Floor as u64
    | MapFeatures::ImpassableForPlayers as u64
    | MapFeatures::ImpassableForEnemies as u64;
pub const TILE_WALL_PLAYER: u64 =
    MapFeatures::Floor as u64 | MapFeatures::ImpassableForPlayers as u64;
pub const TILE_WALL_ALIEN: u64 =
    MapFeatures::Floor as u64 | MapFeatures::ImpassableForEnemies as u64;
pub const TILE_VOID: u64 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaletteTab {
    Terrain,
    Tower,
    Item,
    Enemy,
    Special,
}

impl PaletteTab {
    pub const fn all() -> &'static [Self] {
        &[
            Self::Terrain,
            Self::Tower,
            Self::Item,
            Self::Enemy,
            Self::Special,
        ]
    }
    /// Position in [`PaletteTab::all`], which is also the index into
    /// `MapEditorState::catalog`.
    pub const fn index(self) -> usize {
        match self {
            Self::Terrain => 0,
            Self::Tower => 1,
            Self::Item => 2,
            Self::Enemy => 3,
            Self::Special => 4,
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Terrain => "Terrain",
            Self::Tower => "Tower",
            Self::Item => "Item",
            Self::Enemy => "Enemy",
            Self::Special => "Special",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteItem {
    /// A model def file.
    Def { path: String, name: String },
    /// A special tile marker (spawn point, goal, player spawn).
    Special {
        label: &'static str,
        tile_value: u64,
    },
}

impl PaletteItem {
    pub fn display_name(&self) -> &str {
        match self {
            Self::Def { name, .. } => name,
            Self::Special { label, .. } => label,
        }
    }
}

#[derive(Resource)]
pub struct MapEditorState {
    pub map_name: String,
    /// The loaded map's story file (`MapFile::stories`); not editable here, but kept so a
    /// save does not drop it.
    pub stories: String,
    /// The loaded map's random loot (`MapFile::loot`); kept like `stories`.
    pub loot: Option<crate::general::components::map_components::LootSpread>,
    pub width: usize,
    pub height: usize,
    /// Row-major tile grid; each cell is a `BitFlags<MapFeatures>` stored as u64.
    pub tiles: Vec<Vec<u64>>,
    /// Editor placements.
    pub placements: Vec<TilePlacement>,
    /// Wave definitions.
    pub waves: Vec<WaveDef>,
    /// Currently selected palette tab.
    pub active_tab: PaletteTab,
    /// Every brush, per tab (indexed by `PaletteTab::index`). Scanned once, so search and
    /// the eyedropper can reach brushes in other tabs.
    pub catalog: Vec<Vec<PaletteItem>>,
    /// Items in the active tab.
    pub palette_items: Vec<PaletteItem>,
    pub selected_palette: usize,
    /// The selection each tab had when last left, so cycling tabs comes back to it.
    pub tab_selection: [usize; 5],
    /// The brush before the current one, for the swap key.
    pub previous_brush: Option<(PaletteTab, usize)>,
    /// Brush rotation in 45° steps.
    pub rotation_steps: u8,
    /// If true, palette / wave list needs UI rebuild.
    pub palette_dirty: bool,
    pub waves_dirty: bool,
    pub grid_dirty: bool,
    /// Last seed used by "Generate Map".
    pub gen_seed: u64,
    pub seed_label_dirty: bool,
    /// Enemy defs available for wave assignment: (def_path, display_name).
    pub enemy_defs: Vec<(String, String)>,
    pub selected_enemy: usize,
    pub enemy_picker_dirty: bool,
    /// The modal editor's mode (`modal.rs`).
    pub mode: Mode,
    /// The mode to go back to when a search ends.
    pub mode_before_search: Mode,
    /// The keyboard cursor, in tiles. The mouse moves it too, by hovering.
    pub cursor: (i32, i32),
    /// The search query and its hits, `(tab, index)` best first.
    pub search: String,
    pub search_hits: Vec<(PaletteTab, usize)>,
    pub search_selected: usize,
    /// Undo / redo of the tiles and placements.
    pub history: History,
    /// An edit is in progress (a run of pen-down moves, a mouse drag), so further edits
    /// join its undo step instead of making their own.
    pub stroke_open: bool,
    /// One line of feedback for the status bar: saved, nothing to undo, ...
    pub status: String,
    /// Nodes of an in-progress House polygon, in tile coordinates, in click order.
    /// Each new node snaps onto a horizontal or vertical line from the previous node, so
    /// every house drawn this way is rectilinear (arbitrary L/T/U shapes, not just rectangles).
    pub house_points: Vec<(i32, i32)>,
    /// Def path used for plain wall segments generated by the House tool.
    pub house_wall_def: String,
}

impl Default for MapEditorState {
    fn default() -> Self {
        let width = 20;
        let height = 24;
        let mut state = Self {
            map_name: "new_map".to_string(),
            stories: String::new(),
            loot: None,
            width,
            height,
            tiles: vec![vec![TILE_SPECIAL_FLOOR; width]; height],
            placements: Vec::new(),
            waves: Vec::new(),
            active_tab: PaletteTab::Special,
            catalog: scan_catalog(),
            palette_items: Vec::new(),
            selected_palette: 0,
            tab_selection: [0; 5],
            previous_brush: None,
            rotation_steps: 0,
            palette_dirty: true,
            waves_dirty: true,
            grid_dirty: true,
            gen_seed: 0,
            seed_label_dirty: false,
            enemy_defs: scan_enemy_defs(),
            selected_enemy: 0,
            enemy_picker_dirty: true,
            mode: Mode::Normal,
            mode_before_search: Mode::Normal,
            cursor: (0, 0),
            search: String::new(),
            search_hits: Vec::new(),
            search_selected: 0,
            history: History::default(),
            stroke_open: false,
            status: String::new(),
            house_points: Vec::new(),
            house_wall_def: scan_wall_def().unwrap_or_default(),
        };
        state.refresh_palette();
        state
    }
}

impl MapEditorState {
    /// Load the active tab's brushes from the catalog, selecting the one it had last.
    pub fn refresh_palette(&mut self) {
        self.palette_items = self
            .catalog
            .get(self.active_tab.index())
            .cloned()
            .unwrap_or_default();
        let remembered = self.remembered(self.active_tab);
        self.selected_palette = remembered.min(self.palette_items.len().saturating_sub(1));
        self.palette_dirty = true;
    }

    pub fn set_tab(&mut self, tab: PaletteTab) {
        self.remember(self.active_tab, self.selected_palette);
        self.active_tab = tab;
        self.refresh_palette();
    }

    /// Make `(tab, index)` the brush, remembering the old one for the swap key.
    pub fn select_brush(&mut self, tab: PaletteTab, index: usize) {
        let current = (self.active_tab, self.selected_palette);
        if current == (tab, index) {
            return;
        }
        self.previous_brush = Some(current);
        if tab != self.active_tab {
            self.set_tab(tab);
        }
        self.selected_palette = index.min(self.palette_items.len().saturating_sub(1));
        self.remember(tab, self.selected_palette);
        self.palette_dirty = true;
    }

    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.palette_items.get(self.selected_palette)
    }

    /// The brush at `(tab, index)` in the catalog.
    pub fn catalog_item(&self, tab: PaletteTab, index: usize) -> Option<&PaletteItem> {
        self.tab_items(tab).get(index)
    }

    /// Every brush in `tab`.
    pub fn tab_items(&self, tab: PaletteTab) -> &[PaletteItem] {
        self.catalog.get(tab.index()).map_or(&[], Vec::as_slice)
    }

    /// The selection `tab` had when last left.
    pub fn remembered(&self, tab: PaletteTab) -> usize {
        self.tab_selection.get(tab.index()).copied().unwrap_or(0)
    }

    fn remember(&mut self, tab: PaletteTab, index: usize) {
        if let Some(slot) = self.tab_selection.get_mut(tab.index()) {
            *slot = index;
        }
    }

    pub const fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    // ── Undo ─────────────────────────────────────────────────────────────────

    /// Start an undo step of its own for a one-off edit (a house, a generated map).
    pub fn checkpoint(&mut self) {
        self.stroke_open = false;
        self.history.checkpoint(&self.tiles, &self.placements);
    }

    /// Start an undo step unless one is already open for this stroke.
    pub fn begin_stroke(&mut self) {
        if !self.stroke_open {
            self.history.checkpoint(&self.tiles, &self.placements);
            self.stroke_open = true;
        }
    }

    /// Undo can restore a map of another size (a generated one replaced it).
    fn after_restore(&mut self) {
        self.height = self.tiles.len();
        self.width = self.tiles.first().map_or(0, Vec::len);
        self.cursor = (
            self.cursor.0.clamp(0, (self.width as i32 - 1).max(0)),
            self.cursor.1.clamp(0, (self.height as i32 - 1).max(0)),
        );
        self.grid_dirty = true;
    }

    pub fn undo(&mut self) {
        self.stroke_open = false;
        if self.history.undo(&mut self.tiles, &mut self.placements) {
            self.after_restore();
            self.status = "undo".into();
        } else {
            self.status = "nothing to undo".into();
        }
    }

    pub fn redo(&mut self) {
        self.stroke_open = false;
        if self.history.redo(&mut self.tiles, &mut self.placements) {
            self.after_restore();
            self.status = "redo".into();
        } else {
            self.status = "nothing to redo".into();
        }
    }
}

/// Every tab's brushes, in `PaletteTab::all` order.
fn scan_catalog() -> Vec<Vec<PaletteItem>> {
    PaletteTab::all()
        .iter()
        .map(|tab| match tab {
            PaletteTab::Special => special_items(),
            _ => scan_defs_for_tab(tab),
        })
        .collect()
}

fn special_items() -> Vec<PaletteItem> {
    vec![
        PaletteItem::Special {
            label: "Floor",
            tile_value: TILE_SPECIAL_FLOOR,
        },
        PaletteItem::Special {
            label: "Wall (all)",
            tile_value: TILE_WALL_ALL,
        },
        PaletteItem::Special {
            label: "Wall (player)",
            tile_value: TILE_WALL_PLAYER,
        },
        PaletteItem::Special {
            label: "Wall (alien)",
            tile_value: TILE_WALL_ALIEN,
        },
        PaletteItem::Special {
            label: "Void",
            tile_value: TILE_VOID,
        },
        PaletteItem::Special {
            label: "Alien Spawn",
            tile_value: TILE_ALIEN_SPAWN,
        },
        PaletteItem::Special {
            label: "Alien Goal",
            tile_value: TILE_ALIEN_GOAL,
        },
        PaletteItem::Special {
            label: "Player Spawn",
            tile_value: TILE_PLAYER_SPAWN,
        },
    ]
}

impl MapEditorState {
    pub fn place_at(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let (ux, uy) = (x as usize, y as usize);

        // Resolve what to place before taking mutable borrows.
        let action = match self.palette_items.get(self.selected_palette) {
            Some(PaletteItem::Special { tile_value, .. }) => Some((*tile_value, None::<String>)),
            Some(PaletteItem::Def { path, .. }) => Some((TILE_SPECIAL_FLOOR, Some(path.clone()))),
            None => None,
        };

        if let Some((tile_v, maybe_path)) = action {
            let Some(row) = self.tiles.get_mut(uy) else {
                return;
            };
            let Some(cell) = row.get_mut(ux) else { return };
            *cell = tile_v;
            if let Some(path) = maybe_path {
                let rot = self.rotation_steps;
                self.placements.retain(|p| !(p.x == x && p.y == y));
                self.placements.push(TilePlacement {
                    x,
                    y,
                    def_path: path,
                    rotation_steps: rot,
                });
            }
            self.grid_dirty = true;
        }
    }

    /// Erase both tile value and any placement at (x, y).
    pub fn erase_at(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        if let Some(cell) = self
            .tiles
            .get_mut(y as usize)
            .and_then(|row| row.get_mut(x as usize))
        {
            *cell = 0;
        }
        self.placements.retain(|p| !(p.x == x && p.y == y));
        self.grid_dirty = true;
    }

    pub fn add_wave(&mut self) {
        let enemy_def = self
            .enemy_defs
            .get(self.selected_enemy)
            .map_or_default(|(path, _)| path.clone());
        self.waves.push(WaveDef {
            enemy_def,
            ..WaveDef::default()
        });
        self.waves_dirty = true;
    }

    pub fn remove_wave(&mut self, index: usize) {
        if index < self.waves.len() {
            self.waves.remove(index);
            self.waves_dirty = true;
        }
    }

    /// Adds a click as the next node of the in-progress House polygon. A click landing on
    /// the first node (with at least 3 nodes already placed) closes and resolves the
    /// polygon instead of adding a degenerate node. Every node after the first is snapped
    /// onto a horizontal or vertical line from the previous node, so the resulting polygon
    /// is always rectilinear (an L-shape, T-shape, etc. — not just a rectangle) and every
    /// wall segment lands on one of `procgen`'s four cardinal facings.
    pub fn house_add_point(&mut self, x: i32, y: i32) {
        if let Some(&first) = self.house_points.first()
            && self.house_points.len() >= 3
            && (x, y) == first
        {
            self.finish_house_polygon();
            return;
        }
        let point = match self.house_points.last() {
            Some(&(lx, ly)) => {
                if (x - lx).abs() >= (y - ly).abs() {
                    (x, ly)
                } else {
                    (lx, y)
                }
            }
            None => (x, y),
        };
        // Snapping can produce a no-op point (click landed back on the last node); skip it.
        if self.house_points.last() != Some(&point) {
            self.house_points.push(point);
        }
    }

    /// Resolves the in-progress polygon into a house via `crate::map::procgen` and stamps
    /// it into the grid. No-ops (and clears) if fewer than 3 nodes were placed.
    pub fn finish_house_polygon(&mut self) {
        if self.house_points.len() >= 3 {
            self.checkpoint();
            let polygon = Polygon {
                points: self.house_points.clone(),
            };
            let spec = HouseSpec::new(self.house_wall_def.clone());
            let result = resolve_house(&polygon, &spec);
            apply_house(
                &result,
                &spec,
                self.width,
                self.height,
                &mut self.tiles,
                &mut self.placements,
            );
            self.grid_dirty = true;
        }
        self.house_points.clear();
    }

    /// Discards the in-progress House polygon without stamping anything.
    pub fn cancel_house_polygon(&mut self) {
        self.house_points.clear();
    }

    /// Rotate the brush by one 45 degree step, backwards with `back`.
    pub const fn rotate_brush(&mut self, back: bool) {
        self.rotation_steps = if back {
            (self.rotation_steps + 7) % 8
        } else {
            (self.rotation_steps + 1) % 8
        };
        self.palette_dirty = true; // refresh label with new rotation angle
    }

    pub fn to_map_file(&self) -> MapFile {
        MapFile {
            name: String::new(),
            generated: false,
            seed: 0,
            map_width: self.width,
            map_height: self.height,
            tiles: self.tiles.clone(),
            decorations: Vec::new(),
            placements: self.placements.clone(),
            waves: self.waves.clone(),
            stories: self.stories.clone(),
            loot: self.loot.clone(),
        }
    }

    /// Write the map as `.ron` and say how it went in the status line.
    ///
    /// A `.map` text file beside the `.ron` is what the game loads for that level
    /// (`text_format::load_level`), so the save is called out as shadowed when one exists.
    pub fn save(&mut self) {
        let map = self.to_map_file();
        let path = format!("assets/maps/{}.ron", self.map_name);
        let written = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())
            .and_then(|text| {
                std::fs::create_dir_all("assets/maps").map_err(|e| e.to_string())?;
                std::fs::write(&path, text).map_err(|e| e.to_string())
            });
        if let Err(e) = written {
            self.status = format!("save failed: {e}");
            return;
        }
        bevy::log::info!("Map saved to {path}");
        let text_map = text_format::converted_path(std::path::Path::new(&path));
        self.status = if text_map.exists() {
            format!(
                "saved {path} - but {} exists and the game loads that instead",
                text_map.display()
            )
        } else {
            format!("saved {path}")
        };
    }

    pub fn load_from_map_file(&mut self, map: MapFile) {
        self.checkpoint();
        self.width = map.map_width;
        self.height = map.map_height;
        self.tiles = map.tiles;
        self.placements = map.placements;
        self.waves = map.waves;
        self.stories = map.stories;
        self.loot = map.loot;
        self.cursor = (
            self.cursor.0.clamp(0, self.width as i32 - 1),
            self.cursor.1.clamp(0, self.height as i32 - 1),
        );
        self.grid_dirty = true;
        self.waves_dirty = true;
    }
}

pub fn scan_enemy_defs() -> Vec<(String, String)> {
    let dir = std::path::Path::new("assets/defs");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut items: Vec<(String, String)> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? != "ron" {
                return None;
            }
            let text = std::fs::read_to_string(&p).ok()?;
            let def: AssetDefinition = ron::from_str(&text).ok()?;
            if !matches!(def.model_type, ModelType::Enemy(_)) {
                return None;
            }
            let name = p.file_stem()?.to_str()?.to_string();
            let path = p.to_string_lossy().replace('\\', "/");
            Some((path, name))
        })
        .collect();
    items.sort_by(|a, b| a.1.cmp(&b.1));
    items
}

/// Finds a reasonable default wall def for the House tool: prefers a Terrain def whose
/// name contains "wall", falls back to the first Terrain def found.
fn scan_wall_def() -> Option<String> {
    let dir = std::path::Path::new("assets/defs");
    let entries = std::fs::read_dir(dir).ok()?;
    let mut terrain_defs: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? != "ron" {
                return None;
            }
            let text = std::fs::read_to_string(&p).ok()?;
            let def: AssetDefinition = ron::from_str(&text).ok()?;
            if !matches!(def.model_type, ModelType::Terrain(_)) {
                return None;
            }
            Some(p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    terrain_defs.sort();
    terrain_defs
        .iter()
        .find(|p| p.to_lowercase().contains("wall"))
        .cloned()
        .or_else(|| terrain_defs.into_iter().next())
}

fn scan_defs_for_tab(tab: &PaletteTab) -> Vec<PaletteItem> {
    let dir = std::path::Path::new("assets/defs");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut items: Vec<PaletteItem> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? != "ron" {
                return None;
            }
            let text = std::fs::read_to_string(&p).ok()?;
            let def: AssetDefinition = ron::from_str(&text).ok()?;
            let matches = match tab {
                PaletteTab::Terrain => matches!(def.model_type, ModelType::Terrain(_)),
                PaletteTab::Tower => matches!(def.model_type, ModelType::Tower(_)),
                PaletteTab::Item => matches!(def.model_type, ModelType::Item(_)),
                PaletteTab::Enemy => matches!(def.model_type, ModelType::Enemy(_)),
                PaletteTab::Special => false,
            };
            if !matches {
                return None;
            }
            let mut name = p.file_stem()?.to_str()?.to_string();
            // Say what a pickup does, so "Medkit" vs "Medkit [+25 HP]" is visible at a glance.
            if let ModelType::Item(props) = &def.model_type
                && props.kind.is_pickup()
            {
                name = format!("{name} [{}]", props.kind.label());
            }
            Some(PaletteItem::Def {
                path: p.to_string_lossy().replace('\\', "/"),
                name,
            })
        })
        .collect();
    items.sort_by(|a, b| a.display_name().cmp(b.display_name()));
    items
}
