/// Curated layer options available in the character creator.
/// Each option lists which body types it supports (based on what exists in lpc/spritesheets/).

pub struct LayerOption {
    pub label: &'static str,
    /// Sprite sheet directory path (no trailing slash, no animation filename).
    /// Use `{body}` to substitute the body type (male/female/muscular).
    pub path_template: &'static str,
    /// Which body types have sprites for this option.
    pub supported_bodies: &'static [&'static str],
}

impl LayerOption {
    /// Returns the resolved path for the given body type, or None if unsupported.
    pub fn resolve(&self, body_type: &str) -> Option<String> {
        if self.supported_bodies.contains(&body_type) {
            let path = self.path_template.replace("{body}", body_type);
            Some(path)
        } else if self.supported_bodies.contains(&"male") && body_type == "muscular" {
            // Many layers only have male/female; fall back to male for muscular.
            let path = self.path_template.replace("{body}", "male");
            Some(path)
        } else {
            None
        }
    }
}

pub const HAIR_OPTIONS: &[LayerOption] = &[
    LayerOption {
        label: "Afro",
        path_template: "lpc/spritesheets/hair/afro/adult",
        supported_bodies: &["male", "female", "muscular"],
    },
    LayerOption {
        label: "Bangs",
        path_template: "lpc/spritesheets/hair/bangs/adult",
        supported_bodies: &["male", "female", "muscular"],
    },
    LayerOption {
        label: "Bob",
        path_template: "lpc/spritesheets/hair/bob/adult",
        supported_bodies: &["male", "female", "muscular"],
    },
    LayerOption {
        label: "Buzzcut",
        path_template: "lpc/spritesheets/hair/buzzcut/adult",
        supported_bodies: &["male", "female", "muscular"],
    },
    LayerOption {
        label: "Cornrows",
        path_template: "lpc/spritesheets/hair/cornrows/adult",
        supported_bodies: &["male", "female", "muscular"],
    },
    LayerOption {
        label: "Curly Long",
        path_template: "lpc/spritesheets/hair/curly_long/adult",
        supported_bodies: &["male", "female", "muscular"],
    },
];

pub const TORSO_OPTIONS: &[LayerOption] = &[
    LayerOption {
        label: "T-Shirt",
        path_template: "lpc/spritesheets/torso/clothes/shortsleeve/shortsleeve/{body}",
        supported_bodies: &["male", "female"],
    },
    LayerOption {
        label: "Longsleeve",
        path_template: "lpc/spritesheets/torso/clothes/longsleeve/longsleeve/{body}",
        supported_bodies: &["male", "female"],
    },
    LayerOption {
        label: "Plate Armour",
        path_template: "lpc/spritesheets/torso/armour/plate/{body}",
        supported_bodies: &["male", "female"],
    },
];

pub const LEGS_OPTIONS: &[LayerOption] = &[
    LayerOption {
        label: "Pants",
        path_template: "lpc/spritesheets/legs/pants/{body}",
        supported_bodies: &["male", "muscular"],
    },
    LayerOption {
        label: "Hose",
        path_template: "lpc/spritesheets/legs/hose/{body}",
        supported_bodies: &["male"],
    },
    LayerOption {
        label: "Leg Armour",
        path_template: "lpc/spritesheets/legs/armour/plate/{body}",
        supported_bodies: &["male", "female"],
    },
];

pub const BODY_TYPES: &[(&str, &str)] = &[
    ("male", "Male"),
    ("female", "Female"),
    ("muscular", "Muscular"),
];
