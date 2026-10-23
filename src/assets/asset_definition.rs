use crate::animation::animation_plugin::AnimationKey;
use crate::general::explosion::ExplosionProps;
use crate::general::projectiles::ProjectileProps;
use crate::gore::components::DamageKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

const fn default_scale() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyProps {
    pub health: f32,
    pub speed: f32,
    pub coin_drop: u32,
    /// Per-`DamageKind` multipliers (missing = 1.0, 0.0 = immune).
    #[serde(default)]
    pub resistances: HashMap<DamageKind, f32>,
    /// Loot table (file stem under `assets/loot`) rolled on death. Default `"alien"`.
    #[serde(default = "default_enemy_loot")]
    pub loot_table: Option<String>,
    /// Damage per second while overlapping a player.
    #[serde(default = "default_touch_dps")]
    pub touch_dps: f32,
    #[serde(default)]
    pub attack: EnemyAttack,
    /// Detonate on death (exploders).
    #[serde(default)]
    pub explodes_on_death: Option<ExplosionProps>,
}

/// How an enemy hurts players beyond bumping into them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum EnemyAttack {
    /// Touch damage only.
    #[default]
    Melee,
    /// Shoots the nearest player in range and line of sight while walking.
    Ranged {
        damage: i32,
        range: f32,
        fire_rate_per_minute: f32,
    },
}

// Serde default for an `Option<String>` field, so the wrapper is the point.
#[allow(clippy::unnecessary_wraps)]
fn default_enemy_loot() -> Option<String> {
    Some("alien".to_string())
}
const fn default_touch_dps() -> f32 {
    10.0
}

impl Default for EnemyProps {
    fn default() -> Self {
        Self {
            health: 100.0,
            speed: 2.0,
            coin_drop: 5,
            resistances: HashMap::new(),
            loot_table: default_enemy_loot(),
            touch_dps: default_touch_dps(),
            attack: EnemyAttack::default(),
            explodes_on_death: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TowerProps {
    pub health: f32,
    pub cost: u32,
    pub range: f32,
    pub damage: f32,
    pub fire_rate_per_minute: f32,
    /// Per-`DamageKind` multipliers (missing = 1.0, 0.0 = immune).
    #[serde(default)]
    pub resistances: HashMap<DamageKind, f32>,
    #[serde(default)]
    pub kind: TowerKind,
    /// One line for the build menu.
    #[serde(default)]
    pub description: String,
}

/// What a tower does to aliens inside its `range`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum TowerKind {
    /// Lobs balls at the nearest alien, `fire_rate_per_minute` times a minute.
    #[default]
    Shooter,
    /// Multiplies alien velocity by `factor` while inside.
    Slow { factor: f32 },
    /// Burns everything inside for `damage` per second, applied `tick_hz` times a second.
    Area { tick_hz: f32 },
}

impl Default for TowerProps {
    fn default() -> Self {
        Self {
            health: 200.0,
            cost: 50,
            range: 4.0,
            damage: 20.0,
            fire_rate_per_minute: 30.0,
            resistances: HashMap::new(),
            kind: TowerKind::default(),
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainProps {
    pub blocks_enemies: bool,
    pub blocks_players: bool,
    /// None = indestructible.
    pub health: Option<f32>,
    /// Per-`DamageKind` multipliers (missing = 1.0, 0.0 = immune). Only meaningful with
    /// `health: Some(_)`.
    #[serde(default)]
    pub resistances: HashMap<DamageKind, f32>,
    /// Loot table (file stem under `assets/loot`) rolled when this breaks, e.g. `"crate"`.
    #[serde(default)]
    pub loot_table: Option<String>,
    /// Detonate when destroyed (barrels). Needs `health: Some(_)`.
    #[serde(default)]
    pub explodes_on_death: Option<ExplosionProps>,
}

impl Default for TerrainProps {
    fn default() -> Self {
        Self {
            blocks_enemies: true,
            blocks_players: false,
            health: None,
            resistances: HashMap::new(),
            loot_table: None,
            explodes_on_death: None,
        }
    }
}

/// What a gun consumes. Each kind is a separate pool in the player's `AmmoPouch`;
/// `Infinite` weapons never reload (the character editor's default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmmoKind {
    #[default]
    Infinite,
    Pistol,
    Rifle,
    Shells,
    Grenade,
    Molotov,
}

impl AmmoKind {
    /// Most rounds of this kind a player can carry outside the magazine.
    pub const fn cap(self) -> u32 {
        match self {
            Self::Infinite => u32::MAX,
            Self::Pistol => 120,
            Self::Rifle => 240,
            Self::Shells => 48,
            Self::Grenade => 6,
            Self::Molotov => 4,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Infinite => "inf",
            Self::Pistol => "9mm",
            Self::Rifle => "5.56",
            Self::Shells => "shells",
            Self::Grenade => "grenades",
            Self::Molotov => "molotovs",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ItemKind {
    #[default]
    Decorative,
    HealthPickup {
        amount: f32,
    },
    AmmoPickup {
        kind: AmmoKind,
        rounds: u32,
    },
    /// Path to a Weapon-typed def; picking it up adds the gun to the loadout.
    WeaponPickup {
        def: String,
    },
    Coins {
        value: u32,
    },
    /// Story item / objective token: picking it up sets the `ItemFound.<id>` fact.
    Key {
        id: String,
    },
    /// A bag of something: walking into it rolls the loot table (`assets/loot/<table>.ron`)
    /// and spills what comes out around it. The random loot of a map (`MapFile::loot`).
    Loot {
        table: String,
    },
}

impl ItemKind {
    /// Short human label for editors and the HUD.
    pub fn label(&self) -> String {
        match self {
            Self::Decorative => "decorative".into(),
            Self::HealthPickup { amount } => format!("+{amount} HP"),
            Self::AmmoPickup { kind, rounds } => format!("{rounds} {}", kind.label()),
            Self::WeaponPickup { def } => std::path::Path::new(def)
                .file_stem()
                .map_or_else(|| "weapon".into(), |s| s.to_string_lossy().into_owned()),
            Self::Coins { value } => format!("{value} coins"),
            Self::Key { id } => format!("key {id}"),
            Self::Loot { table } => format!("loot ({table})"),
        }
    }

    /// Whether players can pick this up (everything except set dressing).
    pub const fn is_pickup(&self) -> bool {
        !matches!(self, Self::Decorative)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemProps {
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerAbility {
    #[default]
    Bombardment,
    Healing,
    Whirlwind,
    GoldDigger,
    Molotov,
}

const fn default_throw_rate() -> f32 {
    60.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerProps {
    #[serde(default)]
    pub ability: PlayerAbility,
    /// Balls thrown per minute. Defaults to 60 (one per second).
    #[serde(default = "default_throw_rate")]
    pub throw_rate_per_minute: f32,
    /// Def path of the weapon to equip on spawn, e.g. `"assets/defs/Pistol.ron"`.
    /// Snapped to this model's `grip` hardpoint. `None` = unarmed.
    #[serde(default)]
    pub weapon: Option<String>,
    /// Further weapon defs carried from the start (cycled with 1-4 / d-pad up-down).
    #[serde(default)]
    pub extra_weapons: Vec<String>,
    /// Rounds in the pouch at spawn, per ammo kind (magazines start full on top of this).
    #[serde(default)]
    pub starting_ammo: Vec<(AmmoKind, u32)>,
}

impl Default for PlayerProps {
    fn default() -> Self {
        Self {
            ability: PlayerAbility::default(),
            throw_rate_per_minute: default_throw_rate(),
            weapon: None,
            extra_weapons: Vec::new(),
            starting_ammo: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponHands {
    #[default]
    OneHanded,
    TwoHanded,
}

const fn default_weapon_damage() -> i32 {
    20
}
const fn default_fire_rate() -> f32 {
    300.0
}
const fn default_weapon_range() -> f32 {
    40.0
}
const fn default_spread_deg() -> f32 {
    1.5
}
const fn default_pellets() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponProps {
    #[serde(default)]
    pub hands: WeaponHands,
    /// Damage per pellet that lands.
    #[serde(default = "default_weapon_damage")]
    pub damage: i32,
    /// Shots per minute. 300 = 5/s.
    #[serde(default = "default_fire_rate")]
    pub fire_rate_per_minute: f32,
    /// Hitscan range in world units.
    #[serde(default = "default_weapon_range")]
    pub range: f32,
    /// Cone half-angle in degrees applied to each pellet.
    #[serde(default = "default_spread_deg")]
    pub spread_deg: f32,
    /// Pellets per shot (1 = pistol/rifle, 8 = shotgun).
    #[serde(default = "default_pellets")]
    pub pellets: u32,
    /// Holds-to-fire (automatic) vs one shot per trigger press.
    #[serde(default)]
    pub auto: bool,
    /// Ammo pool this weapon draws from. `Infinite` = never reloads.
    #[serde(default)]
    pub ammo: AmmoKind,
    /// Rounds per magazine. Ignored for `Infinite`.
    #[serde(default = "default_magazine")]
    pub magazine: u32,
    /// Seconds a reload takes.
    #[serde(default = "default_reload_secs")]
    pub reload_secs: f32,
    /// Fire a physics projectile (grenade launcher, rocket) instead of a hitscan ray.
    #[serde(default)]
    pub projectile: Option<ProjectileProps>,
}

const fn default_magazine() -> u32 {
    12
}
const fn default_reload_secs() -> f32 {
    1.5
}

impl Default for WeaponProps {
    fn default() -> Self {
        Self {
            hands: WeaponHands::default(),
            damage: default_weapon_damage(),
            fire_rate_per_minute: default_fire_rate(),
            range: default_weapon_range(),
            spread_deg: default_spread_deg(),
            pellets: default_pellets(),
            auto: false,
            ammo: AmmoKind::default(),
            magazine: default_magazine(),
            reload_secs: default_reload_secs(),
            projectile: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelType {
    Player(PlayerProps),
    Tower(TowerProps),
    Terrain(TerrainProps),
    Item(ItemProps),
    Enemy(EnemyProps),
    Weapon(WeaponProps),
}

impl Default for ModelType {
    fn default() -> Self {
        Self::Player(PlayerProps::default())
    }
}

impl ModelType {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Player(_) => "Player",
            Self::Tower(_) => "Tower",
            Self::Terrain(_) => "Terrain",
            Self::Item(_) => "Item",
            Self::Enemy(_) => "Enemy",
            Self::Weapon(_) => "Weapon",
        }
    }

    pub const fn all_labels() -> &'static [&'static str] {
        &["Player", "Tower", "Terrain", "Item", "Enemy", "Weapon"]
    }

    /// True if `self` and `other` are the same `ModelType` variant, ignoring their
    /// props. Used so re-selecting the current type in the picker keeps its props.
    pub fn same_variant(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Return a default instance for each label.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Tower" => Self::Tower(TowerProps::default()),
            "Terrain" => Self::Terrain(TerrainProps::default()),
            "Item" => Self::Item(ItemProps::default()),
            "Enemy" => Self::Enemy(EnemyProps::default()),
            "Weapon" => Self::Weapon(WeaponProps::default()),
            _ => Self::Player(PlayerProps::default()),
        }
    }
}

/// A named coordinate frame ("hardpoint") on a model, used to snap weapons onto
/// characters. On a character the frame is relative to a bone (`anchor = Some(bone)`);
/// on a weapon it is relative to the model origin (`anchor = None`). Roles are keyed
/// by name (e.g. "grip", "foregrip", "stock", "sight") and paired across the two
/// defs at equip time. See `docs/inverse-kinematics-hardpoints.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hardpoint {
    /// Bone the frame is relative to (characters), or `None` for the model origin (weapons).
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub translation: [f32; 3],
    /// XYZ Euler angles in degrees (authored via the asset browser).
    #[serde(default)]
    pub rotation_euler_deg: [f32; 3],
}

impl Default for Hardpoint {
    fn default() -> Self {
        Self {
            anchor: None,
            translation: [0.0; 3],
            rotation_euler_deg: [0.0; 3],
        }
    }
}

/// A model attached to one of a character's bones (a "socket"), e.g. a rifle in
/// the right hand. Positioned by a local offset relative to the bone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// Name of the bone entity to parent the attached model to (e.g. "mixamorigRightHand").
    pub bone: String,
    /// Path of the attached model, relative to `assets/` with no prefix (same as `model_path`).
    pub model_path: String,
    /// Local translation relative to the bone.
    #[serde(default)]
    pub translation: [f32; 3],
    /// Local rotation relative to the bone, as XYZ Euler angles in degrees.
    #[serde(default)]
    pub rotation_euler_deg: [f32; 3],
    /// Local uniform scale.
    #[serde(default = "default_scale")]
    pub scale: f32,
}

impl Default for Attachment {
    fn default() -> Self {
        Self {
            bone: String::new(),
            model_path: String::new(),
            translation: [0.0; 3],
            rotation_euler_deg: [0.0; 3],
            scale: 1.0,
        }
    }
}

/// Persisted definition for one imported asset. Written to `assets/defs/*.ron`
/// by the asset browser and read at runtime to drive hidden-node lists and
/// animation mappings without hard-coding them in source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDefinition {
    pub model_path: String,
    /// Uniform scale factor applied to the model so it has a meaningful real-world size.
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub model_type: ModelType,
    /// Node names that should be hidden when this model is used in-game.
    #[serde(default)]
    pub hidden_nodes: Vec<String>,
    /// The clip each game animation key plays: the model's own clip name, or
    /// `"<source stem>|<clip>"` for a clip from `animation_sources`. Only what differs
    /// from the default needs listing: a key left out is resolved by
    /// `animation::clips::ClipSet::resolve` (Mesh2Motion rig preset, then name search).
    #[serde(default)]
    pub animations: BTreeMap<AnimationKey, String>,
    /// Paths of external GLB/GLTF files that supply additional animation clips.
    /// Same convention as model_path: relative to the assets/ folder, no "assets/" prefix.
    /// e.g. "packs/AnimPack.glb"
    #[serde(default)]
    pub animation_sources: Vec<String>,
    /// Models attached to the character's bones (e.g. a held rifle). Manual, fixed
    /// props. Superseded for dynamic weapon-holding by `hardpoints` (see below).
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Named connection frames for dynamic weapon snapping (role -> frame), on both
    /// characters (grip in the hand) and weapons (grip/foregrip/stock/sight).
    #[serde(default)]
    pub hardpoints: HashMap<String, Hardpoint>,
    /// Spine bones the torso-twist aim offset rotates, with the share of the total
    /// twist each one takes (see `src/player/systems/torso_twist.rs`). Spreading the
    /// angle over several joints avoids tearing the skinning at one waist bone.
    /// Empty = use the default mixamo-style spine chain.
    #[serde(default)]
    pub aim_bones: Vec<AimBone>,
    /// How far to raise the model so it stands on the floor, in the model's own units
    /// (so it scales with `scale`; [`Self::lift_world`] is the distance in metres). For
    /// models whose origin is not at their base, such as a bag centred on its middle.
    /// Applied wherever the model is put on the ground: map placements, random loot
    /// bags, item drops and the character editor's prop. The editor's Model tab can set
    /// it from the mesh ("sit on floor").
    #[serde(default)]
    pub lift: f32,
}

/// One link in the torso-twist chain: a bone name and its share of the twist.
/// Weights are normalized at use, so they can be written as any proportions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AimBone {
    pub bone: String,
    pub weight: f32,
}

impl Default for AssetDefinition {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            scale: 1.0,
            model_type: ModelType::default(),
            hidden_nodes: Vec::new(),
            animations: BTreeMap::new(),
            animation_sources: Vec::new(),
            attachments: Vec::new(),
            hardpoints: HashMap::new(),
            aim_bones: Vec::new(),
            lift: 0.0,
        }
    }
}

impl AssetDefinition {
    /// [`Self::lift`] in metres, at the def's scale.
    pub fn lift_world(&self) -> f32 {
        self.lift * self.scale
    }

    pub fn def_path(model_path: &str) -> std::path::PathBuf {
        let stem = std::path::Path::new(model_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model");
        std::path::PathBuf::from("assets/defs").join(format!("{stem}.ron"))
    }

    /// Load from `assets/defs/<stem>.ron`. Returns `None` if no file exists.
    pub fn load(model_path: &str) -> Option<Self> {
        let path = Self::def_path(model_path);
        let text = std::fs::read_to_string(&path).ok()?;
        ron::from_str(&text).ok()
    }

    /// Load a def straight from its own `.ron` path (e.g. `"assets/defs/Pistol.ron"`),
    /// as stored in `PlayerRoster::def_paths` and `PlayerProps::weapon`.
    pub fn load_from_def_path(def_path: &str) -> Option<Self> {
        let text = std::fs::read_to_string(def_path).ok()?;
        ron::from_str(&text).ok()
    }

    pub fn save(&self) {
        let path = Self::def_path(&self.model_path);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(path, text);
        }
    }
}

#[cfg(test)]
mod tests {
    /// Every shipped def must parse: a typo in a hand-edited `.ron` otherwise only shows
    /// up as a silently unarmed player or a missing enemy at runtime.
    #[test]
    fn every_def_in_assets_parses() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/defs");
        let mut checked = 0;
        for entry in std::fs::read_dir(root)
            .expect("assets/defs exists")
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            if let Err(e) = ron::from_str::<super::AssetDefinition>(&text) {
                panic!("{} does not parse: {e}", path.display());
            }
            // Unknown fields are skipped on load, so a def still carrying the retired
            // animation fields would parse fine and silently lose its animations.
            for retired in ["clip_tags", "animation_bindings", "animation_mapping"] {
                assert!(
                    !text.contains(&format!("{retired}:")),
                    "{} still has `{retired}`; move it into `animations`",
                    path.display()
                );
            }
            checked += 1;
        }
        assert!(checked > 0, "no defs found under {root}");
    }

    /// `animations` is keyed by the enum, written as bare variant names (`Idle: "..."`).
    #[test]
    fn animations_round_trip_with_enum_keys() {
        use super::{AnimationKey, AssetDefinition};
        let def: AssetDefinition = ron::from_str(
            r#"(model_path: "m.glb", animations: { Idle: "Idle_A", Run: "Sprint" })"#,
        )
        .expect("parses");
        assert_eq!(
            def.animations.get(&AnimationKey::Idle).map(String::as_str),
            Some("Idle_A")
        );
        let text = ron::ser::to_string_pretty(&def, ron::ser::PrettyConfig::default()).unwrap();
        let back: AssetDefinition = ron::from_str(&text).expect("round trips");
        assert_eq!(back.animations, def.animations);
    }
}
