use std::collections::HashMap;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use crate::poly_pizza::types::PizzaModel;

const LIBRARY_PATH: &str = "assets/poly_pizza_library.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub model: PizzaModel,
    /// Relative to `assets/`, e.g. `"poly_pizza_cache/abc123.glb"`.
    pub local_glb: Option<String>,
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct ModelLibrary {
    pub entries: HashMap<String, LibraryEntry>,
}

impl ModelLibrary {
    pub fn load() -> Self {
        std::fs::read_to_string(LIBRARY_PATH)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(LIBRARY_PATH, json);
        }
    }

    pub fn is_saved(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Toggles the model in/out of the library. Returns `true` if it was added.
    pub fn toggle(&mut self, model: &PizzaModel, local_glb: Option<String>) -> bool {
        if self.entries.remove(&model.id).is_some() {
            false
        } else {
            self.entries.insert(model.id.clone(), LibraryEntry {
                model: model.clone(),
                local_glb,
            });
            true
        }
    }
}
