//! The working copy of the def being edited: the one place every panel writes to.
//!
//! Before this, the asset browser and the playground each kept their own copy of a def —
//! the browser as a dozen loose fields rebuilt into a def on export, the playground by
//! editing `PlayerAssetDef` in place with a Save button per panel. Edit in one, save from
//! the other, and the edits were gone. Here there is one [`DefDoc`] per file, the panels
//! edit `doc.def` directly, and Save writes that whole def back to the file it came from.
//!
//! Up to three docs are open at once: [`EditorDoc`] for the character, [`WeaponDoc`] for
//! the weapon it holds and [`PropDoc`] for anything else opened on its own (an enemy, a
//! tower). They are separate files, so they are separate docs — but one Save writes
//! whichever of them has changes. [`EditorFocus`] says which one you are working on.
//!
//! "Unsaved" is computed (`def != saved`), never a flag, so it cannot drift out of step
//! with what is actually on disk.
//!
//! The live character follows the doc through `PlayerAssetDef`, which every gameplay
//! system already reads: see [`sync_doc_to_live`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::character_editor::debug::EditorOverlays;
use crate::character_editor::hardpoints::HardpointSide;
use crate::character_editor::models::{EditorLibrary, def_stem, set_weapon};
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

/// The weapon the character holds: the one its `PlayerProps::weapon` names, or the one
/// opened directly from the library (see [`EditorFocus::held_weapon`]).
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct WeaponDoc(pub DefDoc);

/// A model that is neither the character nor its gun: an enemy, a tower, a piece of
/// terrain, an item. Shown on its own in Studio (see `prop.rs`).
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct PropDoc(pub DefDoc);

/// What the editor is working on, which decides what the top bar leads with, which
/// hardpoint side the panel opens on, and what the camera frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Subject {
    /// A player character, walking around in the arena.
    #[default]
    Character,
    /// A weapon, held by the character so it can be checked in the hand.
    Weapon,
    /// Anything else, standing alone in Studio.
    Prop,
}

impl Subject {
    pub const fn of(def: &AssetDefinition) -> Self {
        match def.model_type {
            ModelType::Player(_) => Self::Character,
            ModelType::Weapon(_) => Self::Weapon,
            _ => Self::Prop,
        }
    }

    /// Only a character can be walked around as. A prop has no Live: there is nothing to
    /// drive, so it always shows in Studio.
    pub const fn has_live(self) -> bool {
        !matches!(self, Self::Prop)
    }
}

#[derive(Resource, Debug, Default)]
pub struct EditorFocus {
    pub subject: Subject,
    /// A weapon opened from the library, put in the character's hand *without* editing
    /// the character: its `PlayerProps::weapon` stays what it was, so opening a gun to
    /// fix its muzzle does not leave the character with unsaved changes. Cleared by
    /// opening a character or equipping from the weapon list.
    pub held_weapon: Option<String>,
}

/// The three docs, reached by hardpoint side. Only the side asked for is borrowed
/// mutably: reaching for the others would mark them changed, and a changed weapon doc is
/// pushed at the live weapon again.
#[derive(SystemParam)]
pub struct SideDocs<'w> {
    pub character: ResMut<'w, EditorDoc>,
    pub weapon: ResMut<'w, WeaponDoc>,
    pub prop: ResMut<'w, PropDoc>,
}

impl SideDocs<'_> {
    pub fn def_mut(&mut self, side: HardpointSide) -> Option<&mut AssetDefinition> {
        match side {
            HardpointSide::Character => self.character.def.as_mut(),
            HardpointSide::Weapon => self.weapon.def.as_mut(),
            HardpointSide::Prop => self.prop.def.as_mut(),
        }
    }
}

/// The weapon def path a character def asks for, if any.
pub fn weapon_def_path(character: &AssetDefinition) -> Option<String> {
    match &character.model_type {
        ModelType::Player(props) => props.weapon.clone(),
        _ => None,
    }
}

/// The def the live character should be spawned from: the doc, holding the opened weapon
/// in place of its own if one is open.
pub fn expected_live(character: &AssetDefinition, held_weapon: Option<&str>) -> AssetDefinition {
    let mut live = character.clone();
    if let Some(weapon) = held_weapon {
        set_weapon(&mut live, Some(weapon));
    }
    live
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
    /// Open a def (path) of any type: a character to wear, a weapon to hold, or a prop.
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
            Self::OpenModel(path) => format!("open {}", def_stem(path)),
            Self::EquipWeapon(Some(path)) => format!("equip {}", def_stem(path)),
            Self::EquipWeapon(None) => "unequip the weapon".to_string(),
            Self::Exit => "leave the editor".to_string(),
        }
    }

    /// Whether doing this now would lose edits in any of the open docs.
    pub fn would_lose_edits(&self, character: &DefDoc, weapon: &DefDoc, prop: &DefDoc) -> bool {
        match self {
            Self::OpenModel(_) | Self::Exit => {
                character.is_dirty() || weapon.is_dirty() || prop.is_dirty()
            }
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

/// Everything an action on the docs touches, in one parameter, so a button's observer
/// does not need eight.
#[derive(SystemParam)]
pub struct Docs<'w> {
    pub character: ResMut<'w, EditorDoc>,
    pub weapon: ResMut<'w, WeaponDoc>,
    pub prop: ResMut<'w, PropDoc>,
    pub focus: ResMut<'w, EditorFocus>,
    pub bar: ResMut<'w, DocBar>,
    pub library: ResMut<'w, EditorLibrary>,
    pub overlays: ResMut<'w, EditorOverlays>,
    pub next_state: ResMut<'w, NextState<GameState>>,
}

impl Docs<'_> {
    pub fn would_lose_edits(&self, action: &ConfirmAction) -> bool {
        action.would_lose_edits(&self.character, &self.weapon, &self.prop)
    }

    /// Ask for `action`: do it now if nothing would be lost, otherwise ask first.
    pub fn request(&mut self, action: ConfirmAction) {
        if self.would_lose_edits(&action) {
            self.bar.confirm = Some(action);
        } else {
            self.bar.confirm = None;
            self.perform(action);
        }
    }

    /// Carry out an action. The caller has already dealt with unsaved edits.
    pub fn perform(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::OpenModel(path) => self.open(&path),
            ConfirmAction::EquipWeapon(weapon) => self.equip(weapon),
            ConfirmAction::Exit => self.next_state.set(GameState::Menu),
        }
    }

    /// Open a def of any type, as whatever kind of subject it is.
    fn open(&mut self, path: &str) {
        let Some(def) = AssetDefinition::load_from_def_path(path) else {
            self.bar.status = format!("cannot read {path}");
            return;
        };
        let subject = Subject::of(&def);
        self.focus.subject = subject;
        self.library.list_dirty = true;
        match subject {
            Subject::Character => {
                self.prop.0 = DefDoc::default();
                self.focus.held_weapon = None;
                self.library.select(path);
            }
            Subject::Weapon => {
                // Into the hand of whoever is worn. `sync_weapon_doc` keeps this doc
                // while `held_weapon` names it; `sync_doc_to_live` respawns the character
                // holding it.
                self.prop.0 = DefDoc::default();
                self.focus.held_weapon = Some(path.to_string());
                self.weapon.0 = DefDoc::open(path);
            }
            Subject::Prop => {
                self.prop.0 = DefDoc::open(path);
                // There is nothing to walk around as: a prop is always in Studio.
                if !self.overlays.orbit_camera {
                    self.overlays.toggle_orbit_camera();
                }
            }
        }
    }

    /// Just an edit to the character doc. `sync_doc_to_live` sees `PlayerProps` differ
    /// from the live character and respawns it holding the new weapon.
    fn equip(&mut self, weapon: Option<String>) {
        let Some(def) = self.character.def.as_mut() else {
            self.bar.status = "no character open".to_string();
            return;
        };
        if !set_weapon(def, weapon.as_deref()) {
            self.bar.status = "the open def is not a character".to_string();
            return;
        }
        // The character's own choice now, not a borrowed gun.
        self.focus.held_weapon = None;
        if self.focus.subject == Subject::Weapon {
            self.focus.subject = Subject::Character;
        }
        self.library.list_dirty = true;
        self.bar.status = match weapon {
            Some(path) => format!("equipped {}", def_stem(&path)),
            None => "unequipped".to_string(),
        };
    }

    /// Save every doc with changes. Returns the status line.
    pub fn save_all(&mut self) -> String {
        let mut lines = Vec::new();
        for doc in [&mut self.character.0, &mut self.weapon.0, &mut self.prop.0] {
            if doc.is_dirty() {
                lines.push(doc.save().unwrap_or_else(|error| error));
            }
        }
        if lines.is_empty() {
            "nothing to save".to_string()
        } else {
            lines.join("\n")
        }
    }

    pub fn any_dirty(&self) -> bool {
        self.character.is_dirty() || self.weapon.is_dirty() || self.prop.is_dirty()
    }

    /// Throw away the edits `action` would have lost: equipping replaces only the weapon
    /// doc, so the character's edits carry on.
    pub fn discard_for(&mut self, action: &ConfirmAction) {
        self.weapon.revert();
        if !matches!(action, ConfirmAction::EquipWeapon(_)) {
            self.character.revert();
            self.prop.revert();
        }
    }

    pub fn revert_all(&mut self) {
        self.character.revert();
        self.weapon.revert();
        self.prop.revert();
    }
}

/// Adopt the worn def if the session started without a doc — no prefs yet, so nothing
/// was selected and the map's default model is on screen. Picking a model opens its doc in
/// `swap_player_model` instead.
pub fn adopt_worn_def(
    mut doc: ResMut<EditorDoc>,
    mut library: ResMut<EditorLibrary>,
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
    let def_path = path.to_string_lossy().replace('\\', "/");
    doc.0 = DefDoc::adopt(&def_path, def.clone());
    // Mark it in the model list too, so the list agrees with what is on screen.
    library.selected = Some(def_path);
    library.list_dirty = true;
}

/// Keep the weapon doc on the weapon in the character's hand: the one opened from the
/// library if any, otherwise the one the character doc asks for.
///
/// Keyed on the *path*: the weapon doc is only replaced when the character asks for a
/// different gun, never because something else about the character changed — that would
/// throw away weapon edits made since the last save.
pub fn sync_weapon_doc(
    doc: Res<EditorDoc>,
    focus: Res<EditorFocus>,
    mut weapon: ResMut<WeaponDoc>,
) {
    let wanted = focus
        .held_weapon
        .clone()
        .or_else(|| doc.def.as_ref().and_then(weapon_def_path));
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
    focus: Res<EditorFocus>,
    mut player_def: ResMut<PlayerAssetDef>,
    mut library: ResMut<EditorLibrary>,
) {
    if library.swap_in_progress() {
        return;
    }
    let (Some(edited), Some(path)) = (doc.def.as_ref(), doc.def_path.as_deref()) else {
        return;
    };
    let edited = &expected_live(edited, focus.held_weapon.as_deref());
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
        assert!(
            !doc.is_dirty(),
            "a file that failed to load has nothing to save"
        );
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

    #[test]
    fn a_character_def_reports_the_weapon_it_wants() {
        assert_eq!(
            weapon_def_path(&character(Some("assets/defs/Pistol.ron"))).as_deref(),
            Some("assets/defs/Pistol.ron")
        );
        assert_eq!(weapon_def_path(&character(None)), None);
    }

    /// Switching models or leaving drops both docs, so either one's edits count.
    #[test]
    fn leaving_asks_first_if_either_doc_has_edits() {
        let clean = doc_with(character(None));
        let mut dirty = doc_with(character(None));
        dirty.def.as_mut().unwrap().scale = 2.0;
        assert!(ConfirmAction::Exit.would_lose_edits(&clean, &dirty, &clean));
        assert!(ConfirmAction::Exit.would_lose_edits(&dirty, &clean, &clean));
        assert!(ConfirmAction::Exit.would_lose_edits(&clean, &clean, &dirty));
        assert!(!ConfirmAction::Exit.would_lose_edits(&clean, &clean, &clean));
        assert!(ConfirmAction::OpenModel("x".into()).would_lose_edits(&dirty, &clean, &clean));
    }

    /// Equipping replaces the weapon doc but only edits the character doc, so only
    /// unsaved *weapon* edits are at risk.
    #[test]
    fn equipping_only_asks_about_the_weapon_doc() {
        let clean = doc_with(character(None));
        let mut dirty = doc_with(character(None));
        dirty.def.as_mut().unwrap().scale = 2.0;
        let equip = ConfirmAction::EquipWeapon(Some("assets/defs/SMG.ron".into()));
        assert!(!equip.would_lose_edits(&dirty, &clean, &dirty));
        assert!(equip.would_lose_edits(&clean, &dirty, &clean));
    }

    /// Opening a gun to fix its muzzle must not leave the character with unsaved edits:
    /// the borrowed gun goes into the *live* def only.
    #[test]
    fn a_held_weapon_changes_the_live_def_not_the_doc() {
        let doc = character(Some("assets/defs/Pistol.ron"));
        let live = expected_live(&doc, Some("assets/defs/SMG.ron"));
        assert_eq!(
            weapon_def_path(&live).as_deref(),
            Some("assets/defs/SMG.ron")
        );
        assert_eq!(
            weapon_def_path(&doc).as_deref(),
            Some("assets/defs/Pistol.ron")
        );
        assert_eq!(expected_live(&doc, None), doc);
    }

    #[test]
    fn the_subject_follows_the_model_type() {
        use crate::assets::asset_definition::{EnemyProps, WeaponProps};
        assert_eq!(Subject::of(&character(None)), Subject::Character);
        let weapon = AssetDefinition {
            model_type: ModelType::Weapon(WeaponProps::default()),
            ..Default::default()
        };
        assert_eq!(Subject::of(&weapon), Subject::Weapon);
        let enemy = AssetDefinition {
            model_type: ModelType::Enemy(EnemyProps::default()),
            ..Default::default()
        };
        assert_eq!(Subject::of(&enemy), Subject::Prop);
        assert!(!Subject::Prop.has_live());
    }
}
