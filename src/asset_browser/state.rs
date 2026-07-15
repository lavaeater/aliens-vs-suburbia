use bevy::animation::graph::AnimationNodeIndex;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

// Re-export from shared location so asset browser code keeps using the same name.
pub use crate::assets::asset_definition::{AssetDefinition, ModelType};

const WINDOW_SIZE: usize = 36;
pub const CHARACTER_NODE_PREFIX: &str = "Character_";
const ROOT: &str = "assets";

// ── Game-state animation key names (order matches display) ────────────────────

pub const ANIM_KEY_NAMES: &[&str] = &[
    "idle", "idle_shoot", "walk", "walk_shoot", "run", "run_gun",
    "jump", "jump_idle", "jump_land",
    "punch", "wave", "yes", "no",
    "death", "hitreact", "throwing", "building",
];

// ── Browser state ─────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct AssetBrowserState {
    // ── Folder navigation ──────────────────────────────────────────────────────
    pub current_folder: String,
    pub folders: Vec<String>,
    pub folder_list_dirty: bool,
    pub folder_scroll: usize,
    #[allow(dead_code)]
    pub selected_folder: usize,

    // ── File list ──────────────────────────────────────────────────────────────
    pub files: Vec<String>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub viewer_entity: Option<Entity>,
    pub load_requested: bool,
    pub list_dirty: bool,

    // ── GLTF / animation ──────────────────────────────────────────────────────
    pub gltf_handle: Option<Handle<Gltf>>,
    pub anim_index: usize,
    pub anim_count: usize,
    pub anim_dirty: bool,
    pub anim_node_indices: Vec<AnimationNodeIndex>,
    pub anim_names: Vec<String>,
    pub anim_player_entity: Option<Entity>,
    pub viewer_graph_handle: Option<Handle<AnimationGraph>>,

    // ── Node visibility ────────────────────────────────────────────────────────
    pub mesh_nodes: Vec<String>,
    pub hidden_nodes: HashSet<String>,
    pub nodes_dirty: bool,     // consumed by apply_node_visibility
    pub nodes_ui_dirty: bool,  // consumed by rebuild_node_list

    // ── Model type ────────────────────────────────────────────────────────────
    pub model_type: ModelType,
    pub type_dirty: bool,

    // ── Clip tagging + game-key bindings (for import) ──────────────────────────
    /// Free-form hierarchical tag per clip. Key = clip name as it appears in
    /// `anim_names`, value = "/"-separated path (e.g. "Combat/Ranged/Shoot").
    pub clip_tags: HashMap<String, String>,
    /// Binds a game-state key (ANIM_KEY_NAMES entry) → a tag path from `clip_tags`.
    pub animation_bindings: HashMap<String, String>,
    /// Rebuild flag for the clip-tag list and the bindings list.
    pub tags_dirty: bool,
    /// While a tag text-box is open: the clip being edited and the current buffer.
    pub tag_edit_clip: Option<String>,
    pub tag_edit_buffer: String,

    // ── External animation sources ─────────────────────────────────────────────
    /// Paths of extra GLB/GLTF files whose clips supplement the model's own clips.
    pub animation_sources: Vec<String>,
    /// Loaded GLTF handles for each entry in animation_sources (same order).
    pub extra_gltf_handles: Vec<Handle<Gltf>>,
    pub sources_dirty: bool,

    // ── Height / scale ─────────────────────────────────────────────────────────
    /// Raw AABB height (max_y − min_y) of the loaded mesh, in mesh units. 0 = not yet measured.
    pub model_raw_height: f32,
    /// Desired real-world height in metres. Default 2.0.
    pub target_height_m: f32,
    /// When a def is loaded before the mesh AABB is known, stash the stored scale here.
    pub pending_scale: Option<f32>,
    pub height_dirty: bool,
    /// Counts up each frame after a model loads; AABB is measured once this reaches a threshold.
    pub aabb_settle_frames: u32,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        let current_folder = "packs".to_string();
        let (folders, files) = scan_folder(&current_folder);
        Self {
            current_folder,
            folders,
            folder_list_dirty: true,
            folder_scroll: 0,
            selected_folder: 0,
            files,
            selected: 0,
            scroll_offset: 0,
            viewer_entity: None,
            load_requested: false,
            list_dirty: true,
            gltf_handle: None,
            anim_index: 0,
            anim_count: 0,
            anim_dirty: false,
            anim_node_indices: Vec::new(),
            anim_names: Vec::new(),
            anim_player_entity: None,
            viewer_graph_handle: None,
            mesh_nodes: Vec::new(),
            hidden_nodes: HashSet::new(),
            nodes_dirty: false,
            nodes_ui_dirty: false,
            clip_tags: HashMap::new(),
            animation_bindings: HashMap::new(),
            tags_dirty: false,
            tag_edit_clip: None,
            tag_edit_buffer: String::new(),
            animation_sources: Vec::new(),
            extra_gltf_handles: Vec::new(),
            sources_dirty: false,
            model_type: ModelType::default(),
            type_dirty: false,
            model_raw_height: 0.0,
            target_height_m: 2.0,
            pending_scale: None,
            height_dirty: false,
            aabb_settle_frames: 0,
        }
    }
}

impl AssetBrowserState {
    pub fn reset_for_enter(&mut self) {
        self.viewer_entity = None;
        self.load_requested = false;
        self.list_dirty = true;
        self.folder_list_dirty = true;
        self.type_dirty = true;
    }

    pub fn reset_anim(&mut self) {
        self.gltf_handle = None;
        self.anim_index = 0;
        self.anim_count = 0;
        self.anim_dirty = false;
        self.anim_node_indices.clear();
        self.anim_names.clear();
        self.mesh_nodes.clear();
        self.nodes_dirty = false;
        self.nodes_ui_dirty = false;
        self.anim_player_entity = None;
        self.viewer_graph_handle = None;
        self.animation_sources.clear();
        self.extra_gltf_handles.clear();
        self.sources_dirty = false;
        self.model_type = ModelType::default();
        self.type_dirty = true;
        self.model_raw_height = 0.0;
        self.target_height_m = 2.0;
        self.pending_scale = None;
        self.height_dirty = false;
        self.aabb_settle_frames = 0;
        // Keep clip_tags / animation_bindings across loads (same-pack models share
        // clip names, like the old anim_mapping did) but close any open text box.
        self.tag_edit_clip = None;
        self.tag_edit_buffer.clear();
    }

    pub fn computed_scale(&self) -> f32 {
        if self.model_raw_height > 0.0 {
            self.target_height_m / self.model_raw_height
        } else {
            1.0
        }
    }

    pub fn add_animation_source(&mut self, path: String) {
        if !self.animation_sources.contains(&path) {
            self.animation_sources.push(path);
            self.sources_dirty = true;
        }
    }

    pub fn remove_animation_source(&mut self, idx: usize) {
        if idx < self.animation_sources.len() {
            self.animation_sources.remove(idx);
            self.extra_gltf_handles.truncate(self.animation_sources.len());
            self.sources_dirty = true;
        }
    }

    pub fn set_model_type(&mut self, label: &str) {
        self.model_type = ModelType::from_label(label);
        self.type_dirty = true;
    }

    pub fn height_up(&mut self) {
        self.target_height_m = (self.target_height_m + 0.1).min(50.0);
        self.height_dirty = true;
    }

    pub fn height_down(&mut self) {
        self.target_height_m = (self.target_height_m - 0.1).max(0.1);
        self.height_dirty = true;
    }

    /// Navigate into a sub-folder by name.
    pub fn enter_folder(&mut self, name: &str) {
        self.current_folder = format!("{}/{}", self.current_folder, name);
        let (folders, files) = scan_folder(&self.current_folder);
        self.folders = folders;
        self.files = files;
        self.selected = 0;
        self.scroll_offset = 0;
        self.folder_scroll = 0;
        self.folder_list_dirty = true;
        self.list_dirty = true;
    }

    /// Navigate up one level.
    pub fn leave_folder(&mut self) {
        if let Some(parent) = std::path::Path::new(&self.current_folder).parent()
            .and_then(|p| p.to_str())
        {
            self.current_folder = parent.to_string();
        }
        let (folders, files) = scan_folder(&self.current_folder);
        self.folders = folders;
        self.files = files;
        self.selected = 0;
        self.scroll_offset = 0;
        self.folder_scroll = 0;
        self.folder_list_dirty = true;
        self.list_dirty = true;
    }

    pub fn toggle_node(&mut self, name: &str) {
        if self.hidden_nodes.contains(name) {
            self.hidden_nodes.remove(name);
        } else {
            self.hidden_nodes.insert(name.to_string());
        }
        self.nodes_dirty = true;
        self.nodes_ui_dirty = true;
    }

    // ── Clip tagging ────────────────────────────────────────────────────────

    /// Distinct tag paths currently in use, sorted. Used for binding cycling and
    /// as the autocomplete pool while typing a tag.
    pub fn distinct_tag_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.clip_tags.values()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        paths.sort();
        paths
    }

    /// Open the tag text-box for `clip`, seeded with its existing tag.
    pub fn begin_tag_edit(&mut self, clip: &str) {
        self.tag_edit_buffer = self.clip_tags.get(clip).cloned().unwrap_or_default();
        self.tag_edit_clip = Some(clip.to_string());
        self.tags_dirty = true;
    }

    pub fn tag_edit_push(&mut self, s: &str) {
        if self.tag_edit_clip.is_some() {
            self.tag_edit_buffer.push_str(s);
            self.tags_dirty = true;
        }
    }

    pub fn tag_edit_backspace(&mut self) {
        if self.tag_edit_clip.is_some() {
            self.tag_edit_buffer.pop();
            self.tags_dirty = true;
        }
    }

    pub fn tag_edit_cancel(&mut self) {
        self.tag_edit_clip = None;
        self.tag_edit_buffer.clear();
        self.tags_dirty = true;
    }

    /// Commit the buffer as `clip`'s tag. Trims slashes/whitespace; an empty
    /// result clears the tag.
    pub fn tag_edit_commit(&mut self) {
        if let Some(clip) = self.tag_edit_clip.take() {
            let path = normalize_tag_path(&self.tag_edit_buffer);
            if path.is_empty() {
                self.clip_tags.remove(&clip);
            } else {
                self.clip_tags.insert(clip, path);
            }
        }
        self.tag_edit_buffer.clear();
        self.tags_dirty = true;
    }

    /// Fill the open text-box with an autocomplete suggestion.
    pub fn tag_edit_accept_suggestion(&mut self, path: &str) {
        if self.tag_edit_clip.is_some() {
            self.tag_edit_buffer = path.to_string();
            self.tags_dirty = true;
        }
    }

    /// Existing tag paths matching the current buffer (case-insensitive substring),
    /// excluding an exact match. Empty while the buffer is empty.
    pub fn tag_suggestions(&self) -> Vec<String> {
        let buf = self.tag_edit_buffer.trim().to_lowercase();
        if buf.is_empty() { return Vec::new(); }
        self.distinct_tag_paths().into_iter()
            .filter(|p| {
                let pl = p.to_lowercase();
                pl != buf && pl.contains(&buf)
            })
            .take(6)
            .collect()
    }

    // ── Game-key → tag bindings ─────────────────────────────────────────────

    /// Cycle `key`'s binding through the in-use tag paths (plus an unbound slot).
    pub fn cycle_binding(&mut self, key: &str, delta: i32) {
        let mut options = self.distinct_tag_paths();
        options.insert(0, String::new()); // unbound
        let current = self.animation_bindings.get(key).cloned().unwrap_or_default();
        let idx = options.iter().position(|o| o == &current).unwrap_or(0) as i32;
        let next = options[(idx + delta).rem_euclid(options.len() as i32) as usize].clone();
        if next.is_empty() {
            self.animation_bindings.remove(key);
        } else {
            self.animation_bindings.insert(key.to_string(), next);
        }
        self.tags_dirty = true;
    }

    /// Write the current viewer state to `assets/defs/<model>.ron`.
    pub fn export_definition(&self) {
        let Some(path) = self.selected_path() else { return };
        let def = AssetDefinition {
            model_path: path.to_string(),
            scale: self.computed_scale(),
            model_type: self.model_type.clone(),
            hidden_nodes: self.hidden_nodes.iter().cloned().collect(),
            // Legacy field retired in favour of clip_tags + animation_bindings.
            animation_mapping: HashMap::new(),
            clip_tags: self.clip_tags.clone(),
            animation_bindings: self.animation_bindings.clone(),
            animation_sources: self.animation_sources.clone(),
        };
        def.save();
    }

    /// Load an existing definition (if any) for the selected model.
    pub fn load_definition(&mut self) {
        let Some(path) = self.selected_path() else { return };
        if let Some(def) = AssetDefinition::load(path) {
            self.hidden_nodes = def.hidden_nodes.into_iter().collect();
            self.clip_tags = def.clip_tags;
            self.animation_bindings = def.animation_bindings;
            // Migrate legacy flat mappings: each game-key → clip becomes a flat tag
            // (named after the key) on that clip, plus a binding to it. Lossless and
            // gives the user a starting point to reorganize hierarchically.
            if self.animation_bindings.is_empty() && !def.animation_mapping.is_empty() {
                for (key, clip) in &def.animation_mapping {
                    if clip.is_empty() { continue; }
                    self.clip_tags.entry(clip.clone()).or_insert_with(|| key.clone());
                    self.animation_bindings.insert(key.clone(), self.clip_tags[clip].clone());
                }
            }
            // Normalize paths: strip leading "assets/" if present (legacy wrong prefix).
            self.animation_sources = def.animation_sources.into_iter()
                .map(|p| p.strip_prefix("assets/").unwrap_or(&p).to_string())
                .collect();
            self.sources_dirty = true;
            self.model_type = def.model_type;
            self.type_dirty = true;
            // Stash the stored scale; target_height_m is resolved once the mesh AABB is measured.
            self.pending_scale = Some(def.scale);
        } else {
            // No def for this model. Clear node visibility but KEEP existing clip_tags /
            // bindings — same-pack models share clip names so the user's work carries over.
            self.hidden_nodes.clear();
            self.animation_sources.clear();
            self.sources_dirty = true;
        }
        self.tag_edit_clip = None;
        self.tag_edit_buffer.clear();
        self.nodes_dirty = true;
        self.nodes_ui_dirty = true;
        self.tags_dirty = true;
    }

    /// Reset tags, bindings and hidden nodes back to a clean state.
    pub fn clear_all(&mut self) {
        self.clip_tags.clear();
        self.animation_bindings.clear();
        self.tag_edit_clip = None;
        self.tag_edit_buffer.clear();
        self.hidden_nodes.clear();
        self.animation_sources.clear();
        self.sources_dirty = true;
        self.nodes_dirty = true;
        self.nodes_ui_dirty = true;
        self.tags_dirty = true;
    }

    pub fn anim_next(&mut self) {
        if self.anim_count > 0 {
            self.anim_index = (self.anim_index + 1) % self.anim_count;
            self.anim_dirty = true;
        }
    }

    pub fn anim_prev(&mut self) {
        if self.anim_count > 0 {
            self.anim_index = (self.anim_index + self.anim_count - 1) % self.anim_count;
            self.anim_dirty = true;
        }
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

// ── Scanning helpers ──────────────────────────────────────────────────────────

/// Returns (subdirs, glb/gltf files) directly inside `folder` (non-recursive).
/// Paths are relative to `assets/`.
pub fn scan_folder(folder: &str) -> (Vec<String>, Vec<String>) {
    let base = std::path::Path::new(ROOT);
    let dir = base.join(folder);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (vec![], vec![]);
    };

    let mut folders = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                folders.push(name.to_string());
            }
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if (ext == "glb" || ext == "gltf")
                && let Ok(rel) = path.strip_prefix(base)
                && let Some(s) = rel.to_str()
            {
                files.push(s.replace('\\', "/"));
            }
        }
    }

    folders.sort();
    files.sort();
    (folders, files)
}

/// Normalize a free-form tag path: split on '/', trim each segment, drop empties,
/// rejoin with '/'. "  Combat // Ranged /Shoot " → "Combat/Ranged/Shoot".
fn normalize_tag_path(raw: &str) -> String {
    raw.split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
