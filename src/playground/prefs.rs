//! What the playground remembers between sessions, in `playground-prefs.ron` at the
//! project root.
//!
//! Same load/save shape as `GameSettings` and `GamepadBindings`: a missing or unparseable
//! file falls back to defaults, so deleting it always gets you back to a working state.
//!
//! Deliberately not part of `GameSettings` — that file is gameplay and camera config that
//! ships with the project, whereas this is per-developer scratch state. Mixing them would
//! mean every model you tried showed up in the game's diff.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const PLAYGROUND_PREFS_PATH: &str = "playground-prefs.ron";

#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct PlaygroundPrefs {
    /// Def path of the model last worn in the playground, e.g. `"assets/defs/amy.ron"`.
    #[serde(default)]
    pub last_model_def: Option<String>,
}

impl PlaygroundPrefs {
    pub fn load() -> Self {
        let path = std::path::Path::new(PLAYGROUND_PREFS_PATH);
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
            && let Ok(prefs) = ron::from_str::<PlaygroundPrefs>(&text)
        {
            return prefs;
        }
        PlaygroundPrefs::default()
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(PLAYGROUND_PREFS_PATH, text);
        }
    }

    /// The remembered model, but only if its def is still on disk.
    ///
    /// A def can be renamed or deleted between sessions; without this check the playground
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
        let prefs = PlaygroundPrefs::default();
        assert!(prefs.last_model_def.is_none());
    }

    #[test]
    fn prefs_round_trip_through_ron() {
        let prefs = PlaygroundPrefs { last_model_def: Some("assets/defs/amy.ron".into()) };
        let text = ron::ser::to_string_pretty(&prefs, ron::ser::PrettyConfig::default())
            .expect("serializes");
        let back: PlaygroundPrefs = ron::from_str(&text).expect("parses");
        assert_eq!(back.last_model_def.as_deref(), Some("assets/defs/amy.ron"));
    }

    #[test]
    fn an_empty_file_fills_from_defaults() {
        let prefs: PlaygroundPrefs = ron::from_str("()").expect("empty record parses");
        assert!(prefs.last_model_def.is_none());
    }

    /// The failure this guards against is silent: a roster pointing at a deleted def
    /// spawns a player with no model at all.
    #[test]
    fn a_remembered_def_that_no_longer_exists_is_dropped() {
        let prefs = PlaygroundPrefs { last_model_def: Some("assets/defs/gone.ron".into()) };
        assert_eq!(prefs.resolve_last_model(|_| false), None);
        assert_eq!(
            prefs.resolve_last_model(|_| true).as_deref(),
            Some("assets/defs/gone.ron")
        );
    }
}
