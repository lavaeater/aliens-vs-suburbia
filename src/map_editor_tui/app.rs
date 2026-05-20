use crate::general::components::map_components::{MapFile, WaveDef};
use ron::ser::PrettyConfig;

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Alt,
    Paint,
    Command,
    WaveEditor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromptKind {
    NewWidth,
    NewHeight,
    LoadPath,
    SavePath,
    ConfirmQuit,
    WaveEnemyDef,
    WaveCount,
    WaveSpawnRate,
    WaveEditEnemyDef(usize),
    WaveEditCount(usize),
    WaveEditSpawnRate(usize),
}

#[derive(Debug, Clone)]
pub struct Prompt {
    pub kind: PromptKind,
    pub input: String,
    pub label: &'static str,
}

pub struct App {
    pub map: MapFile,
    pub file_path: Option<String>,
    pub cursor: (usize, usize), // (col, row)
    pub mode: Mode,
    pub paint_tile: u8,
    pub dirty: bool,
    pub viewport: (usize, usize), // (col_offset, row_offset)
    pub prompt: Option<Prompt>,
    pub status_msg: Option<String>,
    pub wave_selected: usize,
    // scratch space while building a new wave
    pub new_wave_scratch: (String, String, String),
}

pub const TILE_VOID: u8 = 0;
pub const TILE_FLOOR: u8 = 1;
pub const TILE_SPAWN: u8 = 5;
pub const TILE_GOAL: u8 = 9;
pub const TILE_PLAYER: u8 = 17;

impl App {
    pub fn new(file_path: Option<String>) -> Self {
        let (map, path) = match &file_path {
            Some(p) => match Self::load_from(p) {
                Ok(m) => (m, Some(p.clone())),
                Err(e) => {
                    eprintln!("Failed to load {p}: {e}");
                    (Self::blank_map(20, 20), None)
                }
            },
            None => (Self::blank_map(20, 20), None),
        };
        App {
            map,
            file_path: path,
            cursor: (0, 0),
            mode: Mode::Normal,
            paint_tile: TILE_FLOOR,
            dirty: false,
            viewport: (0, 0),
            prompt: None,
            status_msg: None,
            wave_selected: 0,
            new_wave_scratch: (String::new(), String::new(), String::new()),
        }
    }

    pub fn blank_map(width: usize, height: usize) -> MapFile {
        MapFile {
            generated: false,
            seed: 0,
            map_width: width,
            map_height: height,
            tiles: vec![vec![TILE_VOID; width]; height],
            decorations: vec![],
            placements: vec![],
            waves: vec![],
        }
    }

    fn load_from(path: &str) -> Result<MapFile, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        ron::from_str::<MapFile>(&text).map_err(|e| e.to_string())
    }

    pub fn map_width(&self) -> usize {
        self.map.tiles.first().map(|r| r.len()).unwrap_or(0)
    }

    pub fn map_height(&self) -> usize {
        self.map.tiles.len()
    }

    pub fn paint(&mut self, tile: u8) {
        let (col, row) = self.cursor;
        if row < self.map_height() && col < self.map_width() {
            self.map.tiles[row][col] = tile;
            self.dirty = true;
        }
    }

    pub fn move_cursor(&mut self, dcol: i32, drow: i32, viewport_cols: usize, viewport_rows: usize) {
        let new_col = (self.cursor.0 as i32 + dcol)
            .clamp(0, self.map_width().saturating_sub(1) as i32) as usize;
        let new_row = (self.cursor.1 as i32 + drow)
            .clamp(0, self.map_height().saturating_sub(1) as i32) as usize;
        self.cursor = (new_col, new_row);

        // scroll viewport so cursor stays visible
        if new_col < self.viewport.0 {
            self.viewport.0 = new_col;
        } else if new_col >= self.viewport.0 + viewport_cols {
            self.viewport.0 = new_col + 1 - viewport_cols;
        }
        if new_row < self.viewport.1 {
            self.viewport.1 = new_row;
        } else if new_row >= self.viewport.1 + viewport_rows {
            self.viewport.1 = new_row + 1 - viewport_rows;
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self.file_path.clone().ok_or("No file path set")?;
        let pretty = PrettyConfig::new().depth_limit(4);
        let out = ron::ser::to_string_pretty(&self.map, pretty)
            .map_err(|e| e.to_string())?;
        std::fs::write(&path, out).map_err(|e| e.to_string())?;
        self.dirty = false;
        self.status_msg = Some(format!("Saved {path}"));
        Ok(())
    }

    pub fn load(&mut self, path: &str) -> Result<(), String> {
        let map = Self::load_from(path)?;
        self.map = map;
        self.file_path = Some(path.to_string());
        self.cursor = (0, 0);
        self.viewport = (0, 0);
        self.dirty = false;
        self.status_msg = Some(format!("Loaded {path}"));
        Ok(())
    }

    pub fn new_map(&mut self, width: usize, height: usize) {
        self.map = Self::blank_map(width, height);
        self.cursor = (0, 0);
        self.viewport = (0, 0);
        self.dirty = false;
        self.status_msg = Some(format!("New map {width}x{height}"));
    }

    pub fn delete_wave(&mut self) {
        if self.map.waves.is_empty() { return; }
        self.map.waves.remove(self.wave_selected);
        if self.wave_selected > 0 && self.wave_selected >= self.map.waves.len() {
            self.wave_selected -= 1;
        }
        self.dirty = true;
    }

    pub fn commit_new_wave(&mut self) {
        let (ref def, ref cnt, ref rate) = self.new_wave_scratch.clone();
        let count = cnt.parse::<u32>().unwrap_or(10);
        let spawn_rate = rate.parse::<f32>().unwrap_or(20.0);
        self.map.waves.push(WaveDef {
            enemy_def: def.clone(),
            count,
            spawn_rate_per_minute: spawn_rate,
        });
        self.wave_selected = self.map.waves.len() - 1;
        self.new_wave_scratch = (String::new(), String::new(), String::new());
        self.dirty = true;
    }

    pub fn commit_wave_edit(&mut self, idx: usize, field: u8, value: &str) {
        if let Some(wave) = self.map.waves.get_mut(idx) {
            match field {
                0 => wave.enemy_def = value.to_string(),
                1 => { if let Ok(v) = value.parse() { wave.count = v; } }
                2 => { if let Ok(v) = value.parse() { wave.spawn_rate_per_minute = v; } }
                _ => {}
            }
            self.dirty = true;
        }
    }
}
