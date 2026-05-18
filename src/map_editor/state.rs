use bevy::prelude::Resource;
use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::general::components::map_components::{MapFile, TilePlacement, WaveDef};

pub const TILE_SPECIAL_FLOOR: u8 = 1;
pub const TILE_ALIEN_SPAWN: u8 = 5;
pub const TILE_ALIEN_GOAL: u8 = 9;
pub const TILE_PLAYER_SPAWN: u8 = 17;

#[derive(Clone, PartialEq, Debug)]
pub enum PaletteTab {
    Terrain,
    Tower,
    Item,
    Enemy,
    Special,
}

impl PaletteTab {
    pub fn all() -> &'static [PaletteTab] {
        &[PaletteTab::Terrain, PaletteTab::Tower, PaletteTab::Item, PaletteTab::Enemy, PaletteTab::Special]
    }
    pub fn label(&self) -> &'static str {
        match self {
            PaletteTab::Terrain => "Terrain",
            PaletteTab::Tower   => "Tower",
            PaletteTab::Item    => "Item",
            PaletteTab::Enemy   => "Enemy",
            PaletteTab::Special => "Special",
        }
    }
}

#[derive(Clone)]
pub enum PaletteItem {
    /// A model def file.
    Def { path: String, name: String },
    /// A special tile marker (spawn point, goal, player spawn).
    Special { label: &'static str, tile_value: u8 },
}

impl PaletteItem {
    pub fn display_name(&self) -> &str {
        match self {
            PaletteItem::Def { name, .. } => name,
            PaletteItem::Special { label, .. } => label,
        }
    }
}

#[derive(Resource)]
pub struct MapEditorState {
    pub map_name: String,
    pub width: usize,
    pub height: usize,
    /// Row-major tile grid; same encoding as MapFile.tiles.
    pub tiles: Vec<Vec<u8>>,
    /// Editor placements.
    pub placements: Vec<TilePlacement>,
    /// Wave definitions.
    pub waves: Vec<WaveDef>,
    /// Currently selected palette tab.
    pub active_tab: PaletteTab,
    /// Items in the active tab.
    pub palette_items: Vec<PaletteItem>,
    pub selected_palette: usize,
    /// Brush rotation in 45° steps.
    pub rotation_steps: u8,
    /// If true, palette / wave list needs UI rebuild.
    pub palette_dirty: bool,
    pub waves_dirty: bool,
    pub grid_dirty: bool,
}

impl Default for MapEditorState {
    fn default() -> Self {
        let width = 20;
        let height = 24;
        let mut state = Self {
            map_name: "new_map".to_string(),
            width,
            height,
            tiles: vec![vec![1u8; width]; height],
            placements: Vec::new(),
            waves: Vec::new(),
            active_tab: PaletteTab::Special,
            palette_items: Vec::new(),
            selected_palette: 0,
            rotation_steps: 0,
            palette_dirty: true,
            waves_dirty: true,
            grid_dirty: true,
        };
        state.refresh_palette();
        state
    }
}

impl MapEditorState {
    pub fn refresh_palette(&mut self) {
        self.palette_items = match self.active_tab {
            PaletteTab::Special => vec![
                PaletteItem::Special { label: "Floor",        tile_value: TILE_SPECIAL_FLOOR },
                PaletteItem::Special { label: "Alien Spawn",  tile_value: TILE_ALIEN_SPAWN },
                PaletteItem::Special { label: "Alien Goal",   tile_value: TILE_ALIEN_GOAL },
                PaletteItem::Special { label: "Player Spawn", tile_value: TILE_PLAYER_SPAWN },
                PaletteItem::Special { label: "Erase Tile",   tile_value: 0 },
            ],
            _ => scan_defs_for_tab(&self.active_tab),
        };
        self.selected_palette = 0;
        self.palette_dirty = true;
    }

    pub fn set_tab(&mut self, tab: PaletteTab) {
        self.active_tab = tab;
        self.refresh_palette();
    }

    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.palette_items.get(self.selected_palette)
    }

    pub fn place_at(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return; }
        let (ux, uy) = (x as usize, y as usize);

        // Resolve what to place before taking mutable borrows.
        let action = match self.palette_items.get(self.selected_palette) {
            Some(PaletteItem::Special { tile_value, .. }) => Some((*tile_value, None::<String>)),
            Some(PaletteItem::Def { path, .. }) => Some((1u8, Some(path.clone()))),
            None => None,
        };

        if let Some((tile_v, maybe_path)) = action {
            self.tiles[uy][ux] = tile_v;
            if let Some(path) = maybe_path {
                let rot = self.rotation_steps;
                self.placements.retain(|p| !(p.x == x && p.y == y));
                self.placements.push(TilePlacement { x, y, def_path: path, rotation_steps: rot });
            }
            self.grid_dirty = true;
        }
    }

    pub fn erase_placement_at(&mut self, x: i32, y: i32) {
        self.placements.retain(|p| !(p.x == x && p.y == y));
        self.grid_dirty = true;
    }

    pub fn add_wave(&mut self) {
        self.waves.push(WaveDef::default());
        self.waves_dirty = true;
    }

    pub fn remove_wave(&mut self, index: usize) {
        if index < self.waves.len() {
            self.waves.remove(index);
            self.waves_dirty = true;
        }
    }

    pub fn rotate_brush(&mut self) {
        self.rotation_steps = (self.rotation_steps + 1) % 8;
    }

    pub fn to_map_file(&self) -> MapFile {
        MapFile {
            generated: false,
            seed: 0,
            map_width: self.width,
            map_height: self.height,
            tiles: self.tiles.clone(),
            decorations: Vec::new(),
            placements: self.placements.clone(),
            waves: self.waves.clone(),
        }
    }

    pub fn save(&self) {
        let map = self.to_map_file();
        let path = format!("assets/maps/{}.ron", self.map_name);
        if let Ok(text) = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::create_dir_all("assets/maps");
            let _ = std::fs::write(&path, text);
            bevy::log::info!("Map saved to {path}");
        }
    }

    pub fn load_from_file(&mut self, path: &str) {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(map) = ron::from_str::<MapFile>(&text) {
                self.width = map.map_width;
                self.height = map.map_height;
                self.tiles = map.tiles;
                self.placements = map.placements;
                self.waves = map.waves;
                self.grid_dirty = true;
                self.waves_dirty = true;
            }
        }
    }
}

fn scan_defs_for_tab(tab: &PaletteTab) -> Vec<PaletteItem> {
    let dir = std::path::Path::new("assets/defs");
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut items: Vec<PaletteItem> = entries.flatten().filter_map(|e| {
        let p = e.path();
        if p.extension()?.to_str()? != "ron" { return None; }
        let text = std::fs::read_to_string(&p).ok()?;
        let def: AssetDefinition = ron::from_str(&text).ok()?;
        let matches = match tab {
            PaletteTab::Terrain => matches!(def.model_type, ModelType::Terrain(_)),
            PaletteTab::Tower   => matches!(def.model_type, ModelType::Tower(_)),
            PaletteTab::Item    => matches!(def.model_type, ModelType::Item(_)),
            PaletteTab::Enemy   => matches!(def.model_type, ModelType::Enemy(_)),
            PaletteTab::Special => false,
        };
        if !matches { return None; }
        let name = p.file_stem()?.to_str()?.to_string();
        Some(PaletteItem::Def { path: p.to_string_lossy().replace('\\', "/"), name })
    }).collect();
    items.sort_by(|a, b| a.display_name().cmp(b.display_name()));
    items
}
