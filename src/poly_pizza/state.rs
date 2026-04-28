use std::collections::HashSet;
use std::path::PathBuf;
use bevy::prelude::{Entity, Resource};
use crate::poly_pizza::types::PizzaModel;

#[derive(Default, PartialEq, Clone, Copy)]
pub enum InputFocus {
    #[default]
    Keyword,
    Username,
    Tags,
}

#[derive(Resource, Default)]
pub struct PolyPizzaState {
    // Search
    pub search_term: String,
    pub username_term: String,
    pub input_focus: InputFocus,
    pub page: u32,
    pub category_filter: Option<u32>,
    pub animated_only: bool,
    pub cc0_only: bool,

    // Results
    pub results: Vec<PizzaModel>,
    pub total: u32,
    pub pending: bool,
    pub status: String,

    // Flags for system coordination
    pub search_requested: bool,
    pub user_search_requested: bool,
    pub results_dirty: bool,

    // UI entity refs (for runtime rebuild)
    pub results_container: Option<Entity>,
    pub search_label: Option<Entity>,
    pub username_label: Option<Entity>,
    pub status_label: Option<Entity>,
    pub attribution_label: Option<Entity>,

    // Thumbnails
    pub downloading_thumbnails: HashSet<String>,

    // Viewer
    pub selected_model: Option<PizzaModel>,
    pub viewer_entity: Option<Entity>,
    pub viewer_needs_load: bool,
    pub viewer_downloading: bool,
    pub toon_shader: bool,

    // Tag editing
    pub tag_input: String,
}

impl PolyPizzaState {
    pub fn reset_for_enter(&mut self) {
        self.results_container = None;
        self.search_label = None;
        self.username_label = None;
        self.status_label = None;
        self.attribution_label = None;
        self.viewer_entity = None;
        self.viewer_needs_load = false;
        self.viewer_downloading = false;
        self.downloading_thumbnails.clear();
        self.pending = false;
        self.search_requested = false;
        self.results_dirty = false;
        self.status = String::new();
        self.tag_input = String::new();
    }

    pub fn glb_cache_path(&self, id: &str) -> PathBuf {
        PathBuf::from("assets/poly_pizza_cache").join(format!("{id}.glb"))
    }

    pub fn glb_asset_path(&self, id: &str) -> String {
        format!("poly_pizza_cache/{id}.glb#Scene0")
    }

    /// Returns the on-disk path for a thumbnail, using the correct extension.
    pub fn thumb_cache_path(&self, id: &str, url: &str) -> PathBuf {
        let ext = thumb_ext(url);
        PathBuf::from("assets/poly_pizza_cache/thumbs").join(format!("{id}.{ext}"))
    }

    /// Scans for a cached thumbnail with any supported extension.
    pub fn find_thumb_asset_path(&self, id: &str) -> Option<String> {
        for ext in ["webp", "jpg", "png"] {
            let p = PathBuf::from("assets/poly_pizza_cache/thumbs").join(format!("{id}.{ext}"));
            if p.exists() {
                return Some(format!("poly_pizza_cache/thumbs/{id}.{ext}"));
            }
        }
        None
    }

    pub fn has_cached_thumb(&self, id: &str) -> bool {
        for ext in ["webp", "jpg", "png"] {
            if PathBuf::from("assets/poly_pizza_cache/thumbs")
                .join(format!("{id}.{ext}"))
                .exists()
            {
                return true;
            }
        }
        false
    }
}

pub fn thumb_ext(url: &str) -> &'static str {
    if url.contains(".webp") || url.contains("webp") {
        "webp"
    } else if url.contains(".png") {
        "png"
    } else {
        "jpg"
    }
}
