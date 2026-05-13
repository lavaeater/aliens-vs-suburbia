use bevy::prelude::*;

const WINDOW_SIZE: usize = 40;

#[derive(Resource)]
pub struct AssetBrowserState {
    pub files: Vec<String>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub viewer_entity: Option<Entity>,
    pub load_requested: bool,
    pub list_dirty: bool,
    pub toon_shader: bool,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        Self {
            files: scan_assets(),
            selected: 0,
            scroll_offset: 0,
            viewer_entity: None,
            load_requested: false,
            list_dirty: true,
            toon_shader: false,
        }
    }
}

impl AssetBrowserState {
    pub fn reset_for_enter(&mut self) {
        self.viewer_entity = None;
        self.load_requested = false;
        self.list_dirty = true;
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
            self.list_dirty = true;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.files.len() {
            self.selected += 1;
            if self.selected >= self.scroll_offset + WINDOW_SIZE {
                self.scroll_offset = self.selected + 1 - WINDOW_SIZE;
            }
            self.list_dirty = true;
        }
    }

    pub fn page_up(&mut self) {
        let step = WINDOW_SIZE / 2;
        self.selected = self.selected.saturating_sub(step);
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        self.list_dirty = true;
    }

    pub fn page_down(&mut self) {
        let step = WINDOW_SIZE / 2;
        let max = self.files.len().saturating_sub(1);
        self.selected = (self.selected + step).min(max);
        if self.selected >= self.scroll_offset + WINDOW_SIZE {
            self.scroll_offset = self.selected + 1 - WINDOW_SIZE;
        }
        self.list_dirty = true;
    }

    pub fn visible_files(&self) -> impl Iterator<Item = (usize, &str)> {
        let end = (self.scroll_offset + WINDOW_SIZE).min(self.files.len());
        self.files[self.scroll_offset..end]
            .iter()
            .enumerate()
            .map(move |(i, s)| (self.scroll_offset + i, s.as_str()))
    }

    pub fn selected_path(&self) -> Option<&str> {
        self.files.get(self.selected).map(|s| s.as_str())
    }
}

fn scan_assets() -> Vec<String> {
    let assets_dir = std::path::PathBuf::from("assets/packs/toon-shooter");
    let mut files = Vec::new();
    scan_dir(&assets_dir, &assets_dir, &mut files);
    files.sort();
    files
}

fn scan_dir(base: &std::path::Path, dir: &std::path::Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(base, &path, files);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if (ext == "gltf" || ext == "glb")
                && let Ok(rel) = path.strip_prefix(base)
                    && let Some(s) = rel.to_str() {
                        files.push(s.replace('\\', "/"));
                    }
        }
    }
}
