use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_scale() -> f32 { 1.0 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyProps {
    pub health: f32,
    pub speed: f32,
    pub coin_drop: u32,
}

impl Default for EnemyProps {
    fn default() -> Self { Self { health: 100.0, speed: 2.0, coin_drop: 5 } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TowerProps {
    pub health: f32,
    pub cost: u32,
    pub range: f32,
    pub damage: f32,
    pub fire_rate_per_minute: f32,
}

impl Default for TowerProps {
    fn default() -> Self { Self { health: 200.0, cost: 50, range: 4.0, damage: 20.0, fire_rate_per_minute: 30.0 } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainProps {
    pub blocks_enemies: bool,
    pub blocks_players: bool,
    /// None = indestructible.
    pub health: Option<f32>,
}

impl Default for TerrainProps {
    fn default() -> Self { Self { blocks_enemies: true, blocks_players: false, health: None } }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ItemKind {
    #[default]
    Decorative,
    HealthPickup { amount: f32 },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemProps {
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum PlayerAbility {
    #[default]
    Bombardment,
    Healing,
    Whirlwind,
    GoldDigger,
    Molotov,
}

fn default_throw_rate() -> f32 { 60.0 }

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
}

impl Default for PlayerProps {
    fn default() -> Self {
        Self {
            ability: PlayerAbility::default(),
            throw_rate_per_minute: default_throw_rate(),
            weapon: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum WeaponHands {
    #[default]
    OneHanded,
    TwoHanded,
}

fn default_weapon_damage() -> i32 { 20 }
fn default_fire_rate() -> f32 { 300.0 }
fn default_weapon_range() -> f32 { 40.0 }
fn default_spread_deg() -> f32 { 1.5 }
fn default_pellets() -> u32 { 1 }

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
    fn default() -> Self { ModelType::Player(PlayerProps::default()) }
}

impl ModelType {
    pub fn label(&self) -> &'static str {
        match self {
            ModelType::Player(_)   => "Player",
            ModelType::Tower(_)    => "Tower",
            ModelType::Terrain(_)  => "Terrain",
            ModelType::Item(_)     => "Item",
            ModelType::Enemy(_)    => "Enemy",
            ModelType::Weapon(_)   => "Weapon",
        }
    }

    pub fn all_labels() -> &'static [&'static str] {
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
            "Tower"   => ModelType::Tower(TowerProps::default()),
            "Terrain" => ModelType::Terrain(TerrainProps::default()),
            "Item"    => ModelType::Item(ItemProps::default()),
            "Enemy"   => ModelType::Enemy(EnemyProps::default()),
            "Weapon"  => ModelType::Weapon(WeaponProps::default()),
            _         => ModelType::Player(PlayerProps::default()),
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
        Self { anchor: None, translation: [0.0; 3], rotation_euler_deg: [0.0; 3] }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// LEGACY. Maps game-state keys (e.g. "idle", "walk", "throwing") directly to
    /// clip name fragments. Superseded by `clip_tags` + `animation_bindings`, but
    /// still read at runtime as a fallback so pre-migration defs keep working.
    #[serde(default)]
    pub animation_mapping: HashMap<String, String>,
    /// Free-form hierarchical tag assigned to each clip. Key = full clip name as it
    /// appears in the browser's clip list (the model's own clip names, or
    /// "SourceStem|ClipName" for clips supplied by an external source). Value = a
    /// "/"-separated category path, e.g. "Combat/Ranged/Shoot". Purely for
    /// organization; the leaf carries no special meaning.
    #[serde(default)]
    pub clip_tags: HashMap<String, String>,
    /// Binds a game-state key (e.g. "idle", "throwing") to a tag path from
    /// `clip_tags`. At runtime the key resolves to whichever clip carries that tag.
    #[serde(default)]
    pub animation_bindings: HashMap<String, String>,
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
            animation_mapping: HashMap::new(),
            clip_tags: HashMap::new(),
            animation_bindings: HashMap::new(),
            animation_sources: Vec::new(),
            attachments: Vec::new(),
            hardpoints: HashMap::new(),
            aim_bones: Vec::new(),
        }
    }
}

impl AssetDefinition {
    /// Resolve a game-state key (e.g. "idle") to the clip name it should play,
    /// using the tag/binding system first and falling back to the legacy
    /// `animation_mapping`. Returns `None` when nothing is configured for the key,
    /// in which case the caller should use its own default search fragment.
    pub fn resolved_clip(&self, key: &str) -> Option<String> {
        if let Some(tag) = self.animation_bindings.get(key).filter(|t| !t.is_empty()) {
            // Find the clip carrying this tag path.
            if let Some((clip, _)) = self.clip_tags.iter().find(|(_, t)| *t == tag) {
                return Some(clip.clone());
            }
            // Binding points at a tag no clip carries (yet) — nothing to play.
            return None;
        }
        self.animation_mapping.get(key).filter(|s| !s.is_empty()).cloned()
    }
}

impl AssetDefinition {
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
    use super::*;

    #[test]
    fn resolved_clip_follows_binding_to_tagged_clip() {
        let mut def = AssetDefinition::default();
        def.clip_tags.insert("CharacterArmature|Run_Shoot".into(), "Combat/Ranged/RunShoot".into());
        def.animation_bindings.insert("run_shoot".into(), "Combat/Ranged/RunShoot".into());

        assert_eq!(def.resolved_clip("run_shoot").as_deref(), Some("CharacterArmature|Run_Shoot"));
    }

    #[test]
    fn resolved_clip_binding_to_missing_tag_yields_none() {
        let mut def = AssetDefinition::default();
        // Binding points at a tag no clip carries.
        def.animation_bindings.insert("idle".into(), "Idle/Neutral".into());

        assert_eq!(def.resolved_clip("idle"), None);
    }

    #[test]
    fn resolved_clip_falls_back_to_legacy_mapping() {
        let mut def = AssetDefinition::default();
        def.animation_mapping.insert("walk".into(), "CharacterArmature|Walk".into());

        assert_eq!(def.resolved_clip("walk").as_deref(), Some("CharacterArmature|Walk"));
    }

    #[test]
    fn resolved_clip_binding_wins_over_legacy_mapping() {
        let mut def = AssetDefinition::default();
        def.animation_mapping.insert("walk".into(), "Old|Walk".into());
        def.clip_tags.insert("New|Stroll".into(), "Locomotion/Walk".into());
        def.animation_bindings.insert("walk".into(), "Locomotion/Walk".into());

        assert_eq!(def.resolved_clip("walk").as_deref(), Some("New|Stroll"));
    }

    #[test]
    fn resolved_clip_unconfigured_key_is_none() {
        let def = AssetDefinition::default();
        assert_eq!(def.resolved_clip("death"), None);
    }
}
