//! What the character editor remembers between sessions, in `editor-prefs.ron` at the
//! project root. A `playground-prefs.ron` from before the rename is read if the new file is
//! missing, so the editor comes back on the model you last wore.
//!
//! Same load/save shape as `GameSettings` and `GamepadBindings`: a missing or unparseable
//! file falls back to defaults, so deleting it always gets you back to a working state.
//!
//! Deliberately not part of `GameSettings` — that file is gameplay and camera config that
//! ships with the project, whereas this is per-developer scratch state. Mixing them would
//! mean every model you tried showed up in the game's diff.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const EDITOR_PREFS_PATH: &str = "editor-prefs.ron";
/// The file's name from when the editor was the playground. Read, never written.
const LEGACY_PREFS_PATH: &str = "playground-prefs.ron";

#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct EditorPrefs {
    /// Def path of the model last worn in the character editor, e.g. `"assets/defs/amy.ron"`.
    #[serde(default)]
    pub last_model_def: Option<String>,
    /// Label of the tab last open, e.g. `"Hardpoints"`.
    #[serde(default)]
    pub last_tab: Option<String>,
    /// Studio (the orbit camera) was on when last left.
    #[serde(default)]
    pub studio: bool,
}

impl EditorPrefs {
    pub fn load() -> Self {
        [EDITOR_PREFS_PATH, LEGACY_PREFS_PATH]
            .into_iter()
            .find_map(|path| {
                let text = std::fs::read_to_string(path).ok()?;
                ron::from_str::<Self>(&text).ok()
            })
            .unwrap_or_default()
    }

    /// Change one thing and write it back, keeping whatever else was remembered.
    pub fn update(change: impl FnOnce(&mut Self)) {
        let mut prefs = Self::load();
        change(&mut prefs);
        prefs.save();
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(EDITOR_PREFS_PATH, text);
        }
    }

    /// The remembered model, but only if its def is still on disk.
    ///
    /// A def can be renamed or deleted between sessions; without this check the character editor
    /// would set a roster pointing at a missing file and spawn a player with no model.
    pub fn resolve_last_model(&self, exists: impl Fn(&str) -> bool) -> Option<String> {
        self.last_model_def.clone().filter(|path| exists(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_gives_empty_prefs_rather_than_failing() {
        let prefs = EditorPrefs::default();
        assert!(prefs.last_model_def.is_none());
    }

    #[test]
    fn prefs_round_trip_through_ron() {
        let prefs = EditorPrefs {
            last_model_def: Some("assets/defs/amy.ron".into()),
            ..Default::default()
        };
        let text = ron::ser::to_string_pretty(&prefs, ron::ser::PrettyConfig::default())
            .expect("serializes");
        let back: EditorPrefs = ron::from_str(&text).expect("parses");
        assert_eq!(back.last_model_def.as_deref(), Some("assets/defs/amy.ron"));
    }

    #[test]
    fn an_empty_file_fills_from_defaults() {
        let prefs: EditorPrefs = ron::from_str("()").expect("empty record parses");
        assert!(prefs.last_model_def.is_none());
    }

    /// The failure this guards against is silent: a roster pointing at a deleted def
    /// spawns a player with no model at all.
    #[test]
    fn a_remembered_def_that_no_longer_exists_is_dropped() {
        let prefs = EditorPrefs {
            last_model_def: Some("assets/defs/gone.ron".into()),
            ..Default::default()
        };
        assert_eq!(prefs.resolve_last_model(|_| false), None);
        assert_eq!(
            prefs.resolve_last_model(|_| true).as_deref(),
            Some("assets/defs/gone.ron")
        );
    }
}
