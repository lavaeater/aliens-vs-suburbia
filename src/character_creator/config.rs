use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Saved character configuration. Persists across game states.
#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct CharacterConfig {
    /// Base body type: "male", "female", or "muscular".
    pub body_type: String,
    /// Ordered list of additional layer paths (e.g. "lpc/spritesheets/hair/afro/adult").
    /// Each path is the directory prefix; the composer appends "/{anim}.png".
    pub extra_layers: Vec<String>,
}

impl CharacterConfig {
    pub fn new_male() -> Self {
        Self {
            body_type: "male".into(),
            extra_layers: vec![],
        }
    }

    pub fn body_path(&self) -> String {
        format!("lpc/spritesheets/body/bodies/{}", self.body_type)
    }
}

// ── Composed sprite sheet resource ───────────────────────────────────────────

/// Holds the composed billboard sprite sheet handle.
/// Created/updated by the composer whenever CharacterConfig changes.
#[derive(Resource, Default)]
pub struct ComposedSpriteSheet {
    /// Full 704×256 combined (idle+walk) sprite sheet for the billboard.
    pub billboard_handle: Option<bevy::prelude::Handle<bevy::prelude::Image>>,
    /// 64×64 portrait (idle row 2 col 0) for the character creator preview.
    pub portrait_handle: Option<bevy::prelude::Handle<bevy::prelude::Image>>,
}
