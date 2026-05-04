/// A single selectable variant within a layer category.
pub struct LayerOption {
    pub label: &'static str,
    /// Path valid for ALL body types (no substitution needed).
    pub default_path: Option<&'static str>,
    /// Per-body-type paths. Checked first.
    /// Fallbacks applied automatically: muscular→male, female→thin.
    pub body_paths: &'static [(&'static str, &'static str)],
}

impl LayerOption {
    /// Returns the resolved sprite directory path for the given body type, or None.
    pub fn resolve(&self, body_type: &str) -> Option<String> {
        for &(body, path) in self.body_paths {
            if body == body_type { return Some(path.to_string()); }
        }
        if body_type == "muscular" {
            for &(body, path) in self.body_paths {
                if body == "male" { return Some(path.to_string()); }
            }
        }
        if body_type == "female" {
            for &(body, path) in self.body_paths {
                if body == "thin" { return Some(path.to_string()); }
            }
        }
        self.default_path.map(|p| p.to_string())
    }
}

/// UI grouping for the character creator panels.
#[derive(Clone, Copy, PartialEq)]
pub enum CategoryGroup {
    Face,
    HairHead,
    Clothes,
    Accessories,
}

/// A named collection of `LayerOption`s representing one slot in the character.
pub struct LayerCategory {
    pub id: &'static str,
    pub label: &'static str,
    pub group: CategoryGroup,
    pub options: &'static [LayerOption],
}

// ── Shared helpers ───────────────────────────────────────────────────────────

macro_rules! adult_only {
    ($label:literal, $path:literal) => {
        LayerOption {
            label: $label,
            default_path: Some($path),
            body_paths: &[],
        }
    };
}

macro_rules! male_thin {
    ($label:literal, $male:literal, $thin:literal) => {
        LayerOption {
            label: $label,
            default_path: None,
            body_paths: &[
                ("male", $male),
                ("thin", $thin),
                ("female", $thin),
                ("muscular", $male),
            ],
        }
    };
}

macro_rules! gendered {
    ($label:literal, $male:literal, $female:literal) => {
        LayerOption {
            label: $label,
            default_path: None,
            body_paths: &[
                ("male", $male),
                ("female", $female),
                ("muscular", $male),
            ],
        }
    };
}

// ── Category data ─────────────────────────────────────────────────────────────

pub static FACE_OPTIONS: &[LayerOption] = &[
    gendered!("Neutral",
        "lpc/spritesheets/head/faces/male/neutral",
        "lpc/spritesheets/head/faces/female/neutral"),
    gendered!("Happy",
        "lpc/spritesheets/head/faces/male/happy",
        "lpc/spritesheets/head/faces/female/happy"),
    gendered!("Sad",
        "lpc/spritesheets/head/faces/male/sad",
        "lpc/spritesheets/head/faces/female/sad"),
    gendered!("Angry",
        "lpc/spritesheets/head/faces/male/anger",
        "lpc/spritesheets/head/faces/female/anger"),
    gendered!("Blush",
        "lpc/spritesheets/head/faces/male/blush",
        "lpc/spritesheets/head/faces/female/blush"),
    gendered!("Shocked",
        "lpc/spritesheets/head/faces/male/shock",
        "lpc/spritesheets/head/faces/female/shock"),
    gendered!("Look Left",
        "lpc/spritesheets/head/faces/male/look_l",
        "lpc/spritesheets/head/faces/female/look_l"),
    gendered!("Look Right",
        "lpc/spritesheets/head/faces/male/look_r",
        "lpc/spritesheets/head/faces/female/look_r"),
    gendered!("Closed Eyes",
        "lpc/spritesheets/head/faces/male/closed",
        "lpc/spritesheets/head/faces/female/closed"),
    adult_only!("Happy (Alt)", "lpc/spritesheets/head/faces/global/happy2"),
    adult_only!("Sad (Alt)", "lpc/spritesheets/head/faces/global/sad2"),
    adult_only!("Tears", "lpc/spritesheets/head/faces/global/tears"),
    adult_only!("Elderly Neutral",
        "lpc/spritesheets/head/faces/elderly/neutral"),
    adult_only!("Elderly Happy",
        "lpc/spritesheets/head/faces/elderly/happy"),
];

pub static EYES_OPTIONS: &[LayerOption] = &[
    adult_only!("Default",     "lpc/spritesheets/eyes/human/adult/default"),
    adult_only!("Neutral",     "lpc/spritesheets/eyes/human/adult/neutral"),
    adult_only!("Angry",       "lpc/spritesheets/eyes/human/adult/anger"),
    adult_only!("Sad",         "lpc/spritesheets/eyes/human/adult/sad"),
    adult_only!("Look Left",   "lpc/spritesheets/eyes/human/adult/look_l"),
    adult_only!("Look Right",  "lpc/spritesheets/eyes/human/adult/look_r"),
    adult_only!("Eye Roll",    "lpc/spritesheets/eyes/human/adult/eyeroll"),
    adult_only!("Shame",       "lpc/spritesheets/eyes/human/adult/shame"),
    adult_only!("Shocked",     "lpc/spritesheets/eyes/human/adult/shock"),
    adult_only!("Closing",     "lpc/spritesheets/eyes/human/adult/closing"),
    adult_only!("Child",       "lpc/spritesheets/eyes/human/child"),
    adult_only!("Elderly Def", "lpc/spritesheets/eyes/human/elderly/default"),
];

pub static EYEBROWS_OPTIONS: &[LayerOption] = &[
    adult_only!("Thick", "lpc/spritesheets/eyes/eyebrows/thick/adult"),
    adult_only!("Thin",  "lpc/spritesheets/eyes/eyebrows/thin/adult"),
];

pub static EARS_OPTIONS: &[LayerOption] = &[
    adult_only!("Big",        "lpc/spritesheets/head/ears/big/adult"),
    adult_only!("Medium",     "lpc/spritesheets/head/ears/medium/adult"),
    adult_only!("Long",       "lpc/spritesheets/head/ears/long/adult"),
    adult_only!("Down",       "lpc/spritesheets/head/ears/down/adult"),
    adult_only!("Hang",       "lpc/spritesheets/head/ears/hang/adult"),
    adult_only!("Elven",      "lpc/spritesheets/head/ears/elven/adult"),
    adult_only!("Avyon",      "lpc/spritesheets/head/ears/avyon/adult"),
    adult_only!("Avyon Skin", "lpc/spritesheets/head/ears/avyon/skin/adult"),
    adult_only!("Cat FG",     "lpc/spritesheets/head/ears/cat/skin/adult/fg"),
    adult_only!("Cat BG",     "lpc/spritesheets/head/ears/cat/skin/adult/bg"),
    adult_only!("Dragon",     "lpc/spritesheets/head/ears/dragon/adult"),
    adult_only!("Wolf FG",    "lpc/spritesheets/head/ears/wolf/skin/adult/fg"),
    adult_only!("Wolf BG",    "lpc/spritesheets/head/ears/wolf/skin/adult/bg"),
    adult_only!("Lykon",      "lpc/spritesheets/head/ears/lykon/adult"),
    adult_only!("Zabos",      "lpc/spritesheets/head/ears/zabos/adult"),
];

pub static NOSE_OPTIONS: &[LayerOption] = &[
    adult_only!("Button",   "lpc/spritesheets/head/nose/button/adult"),
    adult_only!("Straight", "lpc/spritesheets/head/nose/straight/adult"),
    adult_only!("Big",      "lpc/spritesheets/head/nose/big/adult"),
    adult_only!("Large",    "lpc/spritesheets/head/nose/large/adult"),
    adult_only!("Elderly",  "lpc/spritesheets/head/nose/elderly/adult"),
];

pub static ALT_HEAD_OPTIONS: &[LayerOption] = &[
    adult_only!("Alien",       "lpc/spritesheets/head/heads/alien/adult"),
    adult_only!("Boarman",     "lpc/spritesheets/head/heads/boarman/adult"),
    adult_only!("Frankenstein","lpc/spritesheets/head/heads/frankenstein/adult"),
    adult_only!("Goblin",      "lpc/spritesheets/head/heads/goblin/adult"),
    adult_only!("Jack-o",      "lpc/spritesheets/head/heads/jack/adult"),
    gendered!("Lizard",
        "lpc/spritesheets/head/heads/lizard/male",
        "lpc/spritesheets/head/heads/lizard/female"),
    gendered!("Minotaur",
        "lpc/spritesheets/head/heads/minotaur/male",
        "lpc/spritesheets/head/heads/minotaur/female"),
    adult_only!("Mouse",    "lpc/spritesheets/head/heads/mouse/adult"),
    gendered!("Orc",
        "lpc/spritesheets/head/heads/orc/male",
        "lpc/spritesheets/head/heads/orc/female"),
    adult_only!("Pig",      "lpc/spritesheets/head/heads/pig/adult"),
    adult_only!("Rabbit",   "lpc/spritesheets/head/heads/rabbit/adult"),
    adult_only!("Rat",      "lpc/spritesheets/head/heads/rat/adult"),
    adult_only!("Sheep",    "lpc/spritesheets/head/heads/sheep/adult"),
    adult_only!("Troll",    "lpc/spritesheets/head/heads/troll/adult"),
    adult_only!("Vampire",  "lpc/spritesheets/head/heads/vampire/adult"),
    gendered!("Wolf",
        "lpc/spritesheets/head/heads/wolf/male",
        "lpc/spritesheets/head/heads/wolf/female"),
    adult_only!("Zombie",   "lpc/spritesheets/head/heads/zombie/adult"),
];

pub static HORNS_OPTIONS: &[LayerOption] = &[
    adult_only!("Curled", "lpc/spritesheets/head/horns/curled/adult"),
];

// ── Hair ─────────────────────────────────────────────────────────────────────

pub static HAIR_OPTIONS: &[LayerOption] = &[
    adult_only!("Afro",       "lpc/spritesheets/hair/afro/adult"),
    adult_only!("Bangs",      "lpc/spritesheets/hair/bangs/adult"),
    adult_only!("Bangs Long", "lpc/spritesheets/hair/bangslong/adult"),
    adult_only!("Bangs Long2","lpc/spritesheets/hair/bangslong2/adult"),
    adult_only!("Bangs Short","lpc/spritesheets/hair/bangsshort/adult"),
    adult_only!("Bed Head",   "lpc/spritesheets/hair/bedhead/adult"),
    adult_only!("Bob",        "lpc/spritesheets/hair/bob/adult"),
    adult_only!("Bob Side",   "lpc/spritesheets/hair/bob_side_part/adult"),
    adult_only!("Braid",      "lpc/spritesheets/hair/braid/adult"),
    adult_only!("Braid2",     "lpc/spritesheets/hair/braid2/adult"),
    adult_only!("Bunches",    "lpc/spritesheets/hair/bunches/adult"),
    adult_only!("Buzzcut",    "lpc/spritesheets/hair/buzzcut/adult"),
    adult_only!("Cornrows",   "lpc/spritesheets/hair/cornrows/adult"),
    adult_only!("Cowlick",    "lpc/spritesheets/hair/cowlick/adult"),
    adult_only!("Curly Long", "lpc/spritesheets/hair/curly_long/adult"),
];

// ── Beard & Mustache ──────────────────────────────────────────────────────────

pub static BEARD_OPTIONS: &[LayerOption] = &[
    adult_only!("5 O'Clock",  "lpc/spritesheets/beards/beard/5oclock_shadow"),
    adult_only!("Basic",      "lpc/spritesheets/beards/beard/basic"),
    adult_only!("Medium",     "lpc/spritesheets/beards/beard/medium"),
    adult_only!("Trimmed",    "lpc/spritesheets/beards/beard/trimmed"),
    gendered!("Winter",
        "lpc/spritesheets/beards/beard/winter/male",
        "lpc/spritesheets/beards/beard/winter/female"),
];

pub static MUSTACHE_OPTIONS: &[LayerOption] = &[
    adult_only!("Basic",     "lpc/spritesheets/beards/mustache/basic"),
    adult_only!("Big Stache","lpc/spritesheets/beards/mustache/bigstache"),
    adult_only!("Chevron",   "lpc/spritesheets/beards/mustache/chevron"),
    adult_only!("French",    "lpc/spritesheets/beards/mustache/french"),
    adult_only!("Handlebar", "lpc/spritesheets/beards/mustache/handlebar"),
    adult_only!("Horseshoe", "lpc/spritesheets/beards/mustache/horseshoe"),
    adult_only!("Lampshade", "lpc/spritesheets/beards/mustache/lampshade"),
    adult_only!("Walrus",    "lpc/spritesheets/beards/mustache/walrus"),
];

// ── Hats ─────────────────────────────────────────────────────────────────────

pub static HAT_OPTIONS: &[LayerOption] = &[
    adult_only!("Bandana",          "lpc/spritesheets/hat/cloth/bandana/adult"),
    adult_only!("Bandana 2",        "lpc/spritesheets/hat/cloth/bandana2/adult"),
    adult_only!("Hood",             "lpc/spritesheets/hat/cloth/hood/adult"),
    adult_only!("Leather Cap",      "lpc/spritesheets/hat/cloth/leather_cap/adult"),
    adult_only!("Leather Cap+Feather", "lpc/spritesheets/hat/cloth/leather_cap/feather/adult"),
    adult_only!("Bowler",           "lpc/spritesheets/hat/formal/bowler/adult"),
    adult_only!("Crown",            "lpc/spritesheets/hat/formal/crown/adult"),
    adult_only!("Tiara",            "lpc/spritesheets/hat/formal/tiara/adult"),
    adult_only!("Top Hat",          "lpc/spritesheets/hat/formal/tophat/adult"),
    adult_only!("Headband",         "lpc/spritesheets/hat/headband/thick/adult"),
    adult_only!("Hair Tie",         "lpc/spritesheets/hat/headband/hairtie/adult"),
    adult_only!("Helmet: Armet",    "lpc/spritesheets/hat/helmet/armet/adult"),
    adult_only!("Helmet: Barbarian","lpc/spritesheets/hat/helmet/barbarian/adult"),
    adult_only!("Helmet: Barbarian Viking","lpc/spritesheets/hat/helmet/barbarian_viking/adult"),
    gendered!("Helmet: Barbuta",
        "lpc/spritesheets/hat/helmet/barbuta/male",
        "lpc/spritesheets/hat/helmet/barbuta/female"),
    adult_only!("Helmet: Bascinet", "lpc/spritesheets/hat/helmet/bascinet/adult"),
    adult_only!("Helmet: Kettle",   "lpc/spritesheets/hat/helmet/kettle/adult"),
    adult_only!("Helmet: Legion",   "lpc/spritesheets/hat/helmet/legion/adult"),
    adult_only!("Helmet: Mail",     "lpc/spritesheets/hat/helmet/mail/adult"),
    adult_only!("Helmet: Horned",   "lpc/spritesheets/hat/helmet/horned/adult"),
    adult_only!("Wizard Hat",       "lpc/spritesheets/hat/magic/wizard/base/adult"),
    adult_only!("Celestial Hat",    "lpc/spritesheets/hat/magic/celestial/adult"),
    adult_only!("Pirate Bandana",   "lpc/spritesheets/hat/pirate/bandana/adult"),
    adult_only!("Pirate Tricorne",  "lpc/spritesheets/hat/pirate/tricorne/basic/adult"),
    adult_only!("Pirate Bicorne",   "lpc/spritesheets/hat/pirate/bicorne/foreaft/adult"),
    adult_only!("Pirate Bonnie",    "lpc/spritesheets/hat/pirate/bonnie/adult"),
    adult_only!("Visor: Grated",    "lpc/spritesheets/hat/visor/grated/adult"),
    adult_only!("Visor: Pigface",   "lpc/spritesheets/hat/visor/pigface/adult"),
    adult_only!("Visor: Slit",      "lpc/spritesheets/hat/visor/slit/adult"),
    adult_only!("Xmas",             "lpc/spritesheets/hat/holiday/christmas/adult"),
    adult_only!("Santa",            "lpc/spritesheets/hat/holiday/santa/adult"),
    adult_only!("Crest",            "lpc/spritesheets/hat/accessory/crest/adult"),
    adult_only!("Plumage",          "lpc/spritesheets/hat/accessory/plumage/adult"),
    adult_only!("Horn Acc. Up",     "lpc/spritesheets/hat/accessory/horns_upward/fg/adult"),
    adult_only!("Wings Acc.",       "lpc/spritesheets/hat/accessory/wings/fg/adult"),
];

// ── Torso ─────────────────────────────────────────────────────────────────────

pub static TORSO_OPTIONS: &[LayerOption] = &[
    gendered!("T-Shirt",
        "lpc/spritesheets/torso/clothes/shortsleeve/shortsleeve/male",
        "lpc/spritesheets/torso/clothes/shortsleeve/shortsleeve/female"),
    gendered!("Longsleeve",
        "lpc/spritesheets/torso/clothes/longsleeve/longsleeve/male",
        "lpc/spritesheets/torso/clothes/longsleeve/longsleeve/female"),
    gendered!("Plate Armour",
        "lpc/spritesheets/torso/armour/plate/male",
        "lpc/spritesheets/torso/armour/plate/female"),
    gendered!("Leather Armour",
        "lpc/spritesheets/torso/armour/leather/male",
        "lpc/spritesheets/torso/armour/leather/female"),
];

// ── Legs ─────────────────────────────────────────────────────────────────────

pub static LEGS_OPTIONS: &[LayerOption] = &[
    LayerOption {
        label: "Pants",
        default_path: None,
        body_paths: &[
            ("male",     "lpc/spritesheets/legs/pants/male"),
            ("muscular", "lpc/spritesheets/legs/pants/muscular"),
            ("female",   "lpc/spritesheets/legs/pants/male"),
        ],
    },
    male_thin!("Hose",
        "lpc/spritesheets/legs/hose/male",
        "lpc/spritesheets/legs/hose/thin"),
    gendered!("Leg Armour",
        "lpc/spritesheets/legs/armour/plate/male",
        "lpc/spritesheets/legs/armour/plate/female"),
];

// ── Feet ─────────────────────────────────────────────────────────────────────

pub static FEET_OPTIONS: &[LayerOption] = &[
    male_thin!("Boots Basic",
        "lpc/spritesheets/feet/boots/basic/male",
        "lpc/spritesheets/feet/boots/basic/thin"),
    male_thin!("Boots Fold",
        "lpc/spritesheets/feet/boots/fold/male",
        "lpc/spritesheets/feet/boots/fold/thin"),
    male_thin!("Boots Rimmed",
        "lpc/spritesheets/feet/boots/rimmed/male",
        "lpc/spritesheets/feet/boots/rimmed/thin"),
    male_thin!("Shoes Basic",
        "lpc/spritesheets/feet/shoes/basic/male",
        "lpc/spritesheets/feet/shoes/basic/thin"),
    male_thin!("Shoes Ghillies",
        "lpc/spritesheets/feet/shoes/ghillies/male",
        "lpc/spritesheets/feet/shoes/ghillies/thin"),
    male_thin!("Sandals",
        "lpc/spritesheets/feet/sandals/male",
        "lpc/spritesheets/feet/sandals/thin"),
    male_thin!("Slippers",
        "lpc/spritesheets/feet/slippers/male",
        "lpc/spritesheets/feet/slippers/thin"),
    male_thin!("Socks High",
        "lpc/spritesheets/feet/socks/high/male",
        "lpc/spritesheets/feet/socks/high/thin"),
];

// ── Shoulders ────────────────────────────────────────────────────────────────

pub static SHOULDERS_OPTIONS: &[LayerOption] = &[
    male_thin!("Bauldron",
        "lpc/spritesheets/shoulders/bauldron/male",
        "lpc/spritesheets/shoulders/bauldron/thin"),
    gendered!("Legion",
        "lpc/spritesheets/shoulders/legion/male",
        "lpc/spritesheets/shoulders/legion/female"),
    male_thin!("Mantal",
        "lpc/spritesheets/shoulders/mantal/male",
        "lpc/spritesheets/shoulders/mantal/thin"),
    male_thin!("Pauldrons",
        "lpc/spritesheets/shoulders/pauldrons/male",
        "lpc/spritesheets/shoulders/pauldrons/thin"),
    male_thin!("Epaulets",
        "lpc/spritesheets/shoulders/epaulets/male",
        "lpc/spritesheets/shoulders/epaulets/thin"),
];

// ── Neck ─────────────────────────────────────────────────────────────────────

pub static NECK_OPTIONS: &[LayerOption] = &[
    adult_only!("Bowtie",   "lpc/spritesheets/neck/tie/bowtie/adult"),
    adult_only!("Bowtie 2", "lpc/spritesheets/neck/tie/bowtie2/adult"),
    gendered!("Necktie",
        "lpc/spritesheets/neck/tie/necktie/male",
        "lpc/spritesheets/neck/tie/necktie/female"),
];

// ── Master category list (order = Z compositing order, bottom→top) ───────────

pub static CATEGORIES: &[LayerCategory] = &[
    // Face
    LayerCategory { id: "face",       label: "Face Expression", group: CategoryGroup::Face,        options: FACE_OPTIONS },
    LayerCategory { id: "eyes",       label: "Eyes",            group: CategoryGroup::Face,        options: EYES_OPTIONS },
    LayerCategory { id: "eyebrows",   label: "Eyebrows",        group: CategoryGroup::Face,        options: EYEBROWS_OPTIONS },
    LayerCategory { id: "ears",       label: "Ears",            group: CategoryGroup::Face,        options: EARS_OPTIONS },
    LayerCategory { id: "nose",       label: "Nose",            group: CategoryGroup::Face,        options: NOSE_OPTIONS },
    LayerCategory { id: "alt_head",   label: "Alt Head",        group: CategoryGroup::Face,        options: ALT_HEAD_OPTIONS },
    LayerCategory { id: "horns",      label: "Horns",           group: CategoryGroup::Face,        options: HORNS_OPTIONS },
    // Clothes
    LayerCategory { id: "legs",       label: "Legs",            group: CategoryGroup::Clothes,     options: LEGS_OPTIONS },
    LayerCategory { id: "torso",      label: "Torso",           group: CategoryGroup::Clothes,     options: TORSO_OPTIONS },
    LayerCategory { id: "feet",       label: "Feet",            group: CategoryGroup::Clothes,     options: FEET_OPTIONS },
    // Accessories
    LayerCategory { id: "shoulders",  label: "Shoulders",       group: CategoryGroup::Accessories, options: SHOULDERS_OPTIONS },
    LayerCategory { id: "neck",       label: "Neck",            group: CategoryGroup::Accessories, options: NECK_OPTIONS },
    // Hair & headgear (on top of face)
    LayerCategory { id: "hair",       label: "Hair",            group: CategoryGroup::HairHead,    options: HAIR_OPTIONS },
    LayerCategory { id: "beard",      label: "Beard",           group: CategoryGroup::HairHead,    options: BEARD_OPTIONS },
    LayerCategory { id: "mustache",   label: "Mustache",        group: CategoryGroup::HairHead,    options: MUSTACHE_OPTIONS },
    LayerCategory { id: "hat",        label: "Hat / Helmet",    group: CategoryGroup::HairHead,    options: HAT_OPTIONS },
];

// ── Body types ───────────────────────────────────────────────────────────────

pub const BODY_TYPES: &[(&str, &str)] = &[
    ("male",     "Male"),
    ("female",   "Female"),
    ("muscular", "Muscular"),
];
