//! The working copy of the def being edited: the one place every panel writes to.
//!
//! Before this, the asset browser and the playground each kept their own copy of a def —
//! the browser as a dozen loose fields rebuilt into a def on export, the playground by
//! editing `PlayerAssetDef` in place with a Save button per panel. Edit in one, save from
//! the other, and the edits were gone. Here there is one [`DefDoc`] per file, the panels
//! edit `doc.def` directly, and Save writes that whole def back to the file it came from.
//!
//! Two docs are open at once: [`EditorDoc`] for the character and [`WeaponDoc`] for the
//! weapon it holds. They are separate files, so they are separate docs — but one Save
//! writes whichever of them has changes.
//!
//! "Unsaved" is computed (`def != saved`), never a flag, so it cannot drift out of step
//! with what is actually on disk.
//!
//! The live character follows the doc through `PlayerAssetDef`, which every gameplay
//! system already reads: see [`sync_doc_to_live`].

use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::character_editor::models::{EditorLibrary, set_weapon};
use crate::game_state::GameState;
use crate::model_settings::plugin::PlayerAssetDef;

/// One def file open for editing.
#[derive(Debug, Clone, Default)]
pub struct DefDoc {
    /// The file this saves to, e.g. `"assets/defs/amy.ron"`. Always the path the def was
    /// opened from, never re-derived from `model_path` — a weapon is reached through the
    /// explicit path in `PlayerProps::weapon`, and the two need not agree.
    pub def_path: Option<String>,
    /// The def being edited. Panels mutate this and nothing else.
    pub def: Option<AssetDefinition>,
    /// What is on disk, as of the last open or save.
    pub saved: Option<AssetDefinition>,
}

impl DefDoc {
    /// Open the def at `def_path`. A file that fails to load gives a doc with a path but
    /// no def, so the panels can say which file is broken.
    pub fn open(def_path: &str) -> Self {
        let def = AssetDefinition::load_from_def_path(def_path);
        Self {
            def_path: Some(def_path.to_string()),
            saved: def.clone(),
            def,
        }
    }

    /// Take over a def that is already loaded, with what is on disk as the baseline.
    pub fn adopt(def_path: &str, def: AssetDefinition) -> Self {
        Self {
            def_path: Some(def_path.to_string()),
            saved: AssetDefinition::load_from_def_path(def_path),
            def: Some(def),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.def.is_some() && self.def != self.saved
    }

    /// File stem of the def, for labels: `"assets/defs/amy.ron"` -> `"amy"`.
    pub fn name(&self) -> Option<&str> {
        self.def_path.as_deref().map(|path| {
            std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
        })
    }

    /// Write the def to its file. Returns a status line either way.
    pub fn save(&mut self) -> Result<String, String> {
        let (Some(def), Some(path)) = (self.def.as_ref(), self.def_path.as_deref()) else {
            return Err("nothing open to save".to_string());
        };
        let text = ron::ser::to_string_pretty(def, ron::ser::PrettyConfig::default())
            .map_err(|error| format!("FAILED to serialize {path}: {error}"))?;
        if let Some(dir) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, text).map_err(|error| format!("FAILED to write {path}: {error}"))?;
        self.saved = self.def.clone();
        Ok(format!("saved {path}"))
    }

    /// Throw the edits away: back to what was last opened or saved.
    pub fn revert(&mut self) {
        self.def.clone_from(&self.saved);
    }
}

/// The character being edited.
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct EditorDoc(pub DefDoc);

/// The weapon the character holds, loaded from its `PlayerProps::weapon`.
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct WeaponDoc(pub DefDoc);

/// The weapon def path a character def asks for, if any.
pub fn weapon_def_path(character: &AssetDefinition) -> Option<String> {
    match &character.model_type {
        ModelType::Player(props) => props.weapon.clone(),
        _ => None,
    }
}

/// Fields that only take effect when the character is spawned: the model itself, its
/// size, what it carries and the twist chain resolved from its skeleton. A change to any
/// of these needs a respawn to show. Everything else — hardpoints, clip tags, bindings,
/// animation sources — the live systems pick up from `PlayerAssetDef` as it changes.
pub fn needs_respawn(live: &AssetDefinition, edited: &AssetDefinition) -> bool {
    live.model_path != edited.model_path
        || live.scale.to_bits() != edited.scale.to_bits()
        || live.model_type != edited.model_type
        || live.hidden_nodes != edited.hidden_nodes
        || live.attachments != edited.attachments
        || live.aim_bones != edited.aim_bones
}

/// Something you asked for that would throw away unsaved edits, held until you say what
/// to do with them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Switch to another model (def path).
    OpenModel(String),
    /// Hand the character a different weapon (weapon def path, `None` = empty-handed).
    /// Only guarded by the *weapon* doc: the character doc just gains an edit.
    EquipWeapon(Option<String>),
    /// Leave for the menu.
    Exit,
}

impl ConfirmAction {
    pub fn describe(&self) -> String {
        match self {
            Self::OpenModel(path) => format!("open {}", crate::character_editor::models::def_stem(path)),
            Self::EquipWeapon(Some(path)) => {
                format!("equip {}", crate::character_editor::models::def_stem(path))
            }
            Self::EquipWeapon(None) => "unequip the weapon".to_string(),
            Self::Exit => "leave the editor".to_string(),
        }
    }

    /// Whether doing this now would lose edits.
    pub fn would_lose_edits(&self, character: &DefDoc, weapon: &DefDoc) -> bool {
        match self {
            Self::OpenModel(_) | Self::Exit => character.is_dirty() || weapon.is_dirty(),
            Self::EquipWeapon(_) => weapon.is_dirty(),
        }
    }
}

/// The doc bar's state: the question it is asking, if any, and the last thing it said.
#[derive(Resource, Debug, Default)]
pub struct DocBar {
    pub confirm: Option<ConfirmAction>,
    pub status: String,
}

/// Ask for `action`: do it now if nothing would be lost, otherwise ask first.
pub fn request(
    action: ConfirmAction,
    bar: &mut DocBar,
    doc: &mut EditorDoc,
    weapon: &WeaponDoc,
    library: &mut EditorLibrary,
    next_state: &mut NextState<GameState>,
) {
    if action.would_lose_edits(doc, weapon) {
        bar.confirm = Some(action);
    } else {
        bar.confirm = None;
        perform(action, bar, doc, library, next_state);
    }
}

/// Carry out an action. The caller has already dealt with unsaved edits.
pub fn perform(
    action: ConfirmAction,
    bar: &mut DocBar,
    doc: &mut EditorDoc,
    library: &mut EditorLibrary,
    next_state: &mut NextState<GameState>,
) {
    match action {
        ConfirmAction::OpenModel(path) => library.select(&path),
        ConfirmAction::EquipWeapon(weapon) => {
            // Just an edit to the character doc. `sync_doc_to_live` sees `PlayerProps`
            // differ from the live character and respawns it holding the new weapon.
            let Some(def) = doc.def.as_mut() else {
                bar.status = "no character open".to_string();
                return;
            };
            if !set_weapon(def, weapon.as_deref()) {
                bar.status = "the open def is not a character".to_string();
                return;
            }
            bar.status = match weapon {
                Some(path) => format!("equipped {}", crate::character_editor::models::def_stem(&path)),
                None => "unequipped".to_string(),
            };
        }
        ConfirmAction::Exit => next_state.set(GameState::Menu),
    }
}

/// Save every doc with changes. Returns the status line.
pub fn save_all(doc: &mut EditorDoc, weapon: &mut WeaponDoc) -> String {
    let mut lines = Vec::new();
    for side in [&mut doc.0, &mut weapon.0] {
        if side.is_dirty() {
            lines.push(side.save().unwrap_or_else(|error| error));
        }
    }
    if lines.is_empty() {
        "nothing to save".to_string()
    } else {
        lines.join("\n")
    }
}

/// Adopt the worn def if the session started without a doc — no prefs yet, so nothing
/// was selected and the map's default model is on screen. Picking a model opens its doc in
/// `swap_player_model` instead.
pub fn adopt_worn_def(
    mut doc: ResMut<EditorDoc>,
    library: Res<EditorLibrary>,
    player_def: Res<PlayerAssetDef>,
) {
    if doc.def_path.is_some() || library.swap_in_progress() {
        return;
    }
    let Some(def) = player_def.0.as_ref() else {
        return;
    };
    let path = AssetDefinition::def_path(&def.model_path);
    if !path.exists() {
        return;
    }
    doc.0 = DefDoc::adopt(&path.to_string_lossy().replace('\\', "/"), def.clone());
}

/// Keep the weapon doc on the weapon the character doc asks for.
///
/// Keyed on the *path*: the weapon doc is only replaced when the character asks for a
/// different gun, never because something else about the character changed — that would
/// throw away weapon edits made since the last save.
pub fn sync_weapon_doc(doc: Res<EditorDoc>, mut weapon: ResMut<WeaponDoc>) {
    let wanted = doc.def.as_ref().and_then(weapon_def_path);
    if wanted == weapon.def_path {
        return;
    }
    weapon.0 = wanted.as_deref().map(DefDoc::open).unwrap_or_default();
}

/// Make the live character match the doc.
///
/// Edits the live systems can follow (hardpoints, bindings, tags, sources) are copied
/// into `PlayerAssetDef`. Edits that only take effect at spawn ([`needs_respawn`]) respawn
/// the character from the doc — through the roster's `def_overrides`, so the respawn
/// shows the unsaved edits rather than the file.
///
/// Waits out a swap in progress: mid-swap the live def is the *old* model and comparing
/// it with the new doc would read as a change to respawn for.
pub fn sync_doc_to_live(
    doc: Res<EditorDoc>,
    mut player_def: ResMut<PlayerAssetDef>,
    mut library: ResMut<EditorLibrary>,
) {
    if library.swap_in_progress() {
        return;
    }
    let (Some(edited), Some(path)) = (doc.def.as_ref(), doc.def_path.as_deref()) else {
        return;
    };
    let Some(live) = player_def.0.as_ref() else {
        return;
    };
    if live == edited {
        return;
    }
    if needs_respawn(live, edited) {
        library.respawn(path);
        return;
    }
    player_def.0 = Some(edited.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::{Hardpoint, PlayerProps};

    fn doc_with(def: AssetDefinition) -> DefDoc {
        DefDoc {
            def_path: Some("assets/defs/test.ron".to_string()),
            saved: Some(def.clone()),
            def: Some(def),
        }
    }

    fn character(weapon: Option<&str>) -> AssetDefinition {
        AssetDefinition {
            model_path: "models/test.glb".to_string(),
            model_type: ModelType::Player(PlayerProps {
                weapon: weapon.map(str::to_string),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn a_freshly_opened_doc_is_clean() {
        assert!(!doc_with(character(None)).is_dirty());
    }

    #[test]
    fn an_edit_makes_it_dirty_and_undoing_it_makes_it_clean_again() {
        let mut doc = doc_with(character(None));
        let def = doc.def.as_mut().unwrap();
        def.hardpoints
            .insert("grip".to_string(), Hardpoint::default());
        assert!(doc.is_dirty());
        // Computed, not a flag: putting it back the way it was is clean, with no save.
        doc.def.as_mut().unwrap().hardpoints.remove("grip");
        assert!(!doc.is_dirty());
    }

    #[test]
    fn revert_restores_what_was_saved() {
        let mut doc = doc_with(character(None));
        doc.def.as_mut().unwrap().scale = 3.0;
        doc.revert();
        assert!(!doc.is_dirty());
        assert_eq!(doc.def.as_ref().unwrap().scale, 1.0);
    }

    #[test]
    fn a_doc_with_no_def_is_never_dirty() {
        let doc = DefDoc {
            def_path: Some("assets/defs/broken.ron".to_string()),
            ..Default::default()
        };
        assert!(!doc.is_dirty(), "a file that failed to load has nothing to save");
    }

    #[test]
    fn the_name_comes_from_the_def_path_not_the_model() {
        let mut doc = doc_with(character(None));
        doc.def_path = Some("assets/defs/Blaster A.ron".to_string());
        assert_eq!(doc.name(), Some("Blaster A"));
        assert_eq!(DefDoc::default().name(), None);
    }

    #[test]
    fn hardpoint_and_binding_edits_apply_live() {
        let live = character(None);
        let mut edited = live.clone();
        edited
            .hardpoints
            .insert("grip".to_string(), Hardpoint::default());
        edited
            .animation_bindings
            .insert("wave".to_string(), "Social/Wave".to_string());
        assert!(!needs_respawn(&live, &edited));
    }

    /// The weapon is resolved by `PendingEquip` at spawn, so a new one needs a respawn.
    #[test]
    fn equipping_another_weapon_needs_a_respawn() {
        let live = character(Some("assets/defs/Pistol.ron"));
        let edited = character(Some("assets/defs/SMG.ron"));
        assert!(needs_respawn(&live, &edited));
    }

    /// Switching models or leaving drops both docs, so either one's edits count.
    #[test]
    fn leaving_asks_first_if_either_doc_has_edits() {
        let clean = doc_with(character(None));
        let mut dirty = doc_with(character(None));
        dirty.def.as_mut().unwrap().scale = 2.0;
        assert!(ConfirmAction::Exit.would_lose_edits(&clean, &dirty));
        assert!(ConfirmAction::Exit.would_lose_edits(&dirty, &clean));
        assert!(!ConfirmAction::Exit.would_lose_edits(&clean, &clean));
        assert!(ConfirmAction::OpenModel("x".into()).would_lose_edits(&dirty, &clean));
    }

    /// Equipping replaces the weapon doc but only edits the character doc, so only
    /// unsaved *weapon* edits are at risk.
    #[test]
    fn equipping_only_asks_about_the_weapon_doc() {
        let clean = doc_with(character(None));
        let mut dirty = doc_with(character(None));
        dirty.def.as_mut().unwrap().scale = 2.0;
        let equip = ConfirmAction::EquipWeapon(Some("assets/defs/SMG.ron".into()));
        assert!(!equip.would_lose_edits(&dirty, &clean));
        assert!(equip.would_lose_edits(&clean, &dirty));
    }
}
