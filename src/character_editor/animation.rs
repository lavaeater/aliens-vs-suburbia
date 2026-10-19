//! Previewing and re-mapping animations on the live character.
//!
//! Two jobs. Clicking a key *previews* it on the character standing in the arena
//! (`PreviewAnimation`, which plays the clip outright rather than going through
//! `CharacterState`), the quickest way to find out that a rig's "wave" is actually a
//! T-pose. And each key can be mapped to any clip the model or its sources carry.
//!
//! A def only lists what differs from the default: a key it leaves out resolves through
//! the rig preset and the name search (`animation::clips::ClipSet::resolve`), which is
//! also what the panel shows, so a Mesh2Motion model arrives fully mapped.
//!
//! Re-mapping takes effect immediately with no respawn: `build_player_anim_graph` folds
//! `animations` into the signature it compares against `last_sig`, so an edit to the doc
//! (copied into `PlayerAssetDef` by `doc::sync_doc_to_live`) rebuilds the graph on the
//! next frame, and the rebuild replays the previewed key.

use bevy::prelude::*;

use crate::animation::animation_plugin::AnimationKey;
use crate::animation::clips::{ClipSet, Via};
use crate::assets::asset_definition::AssetDefinition;
use crate::assets::gltf_info;
use crate::model_settings::resources::AnimMapping;

#[derive(Resource, Default)]
pub struct AnimationEditor {
    /// Key whose clip is being picked (and, on a character, previewed).
    pub selected_key: Option<AnimationKey>,
    pub ui_dirty: bool,
}

impl AnimationEditor {
    pub const fn select(&mut self, key: AnimationKey) {
        self.selected_key = Some(key);
        self.ui_dirty = true;
    }

    /// Redraw the panel after an edit.
    pub const fn touch(&mut self) {
        self.ui_dirty = true;
    }
}

/// Every clip the def can map a key to, read off the files (`gltf_info`): the model's own
/// clips, and each animation source's as `"<stem>|<clip>"`.
pub fn clip_set(def: &AssetDefinition) -> ClipSet {
    let own = gltf_info::inspect(&def.model_path).map_or_default(|info| info.animations);
    let mut sourced = Vec::new();
    for source in &def.animation_sources {
        let stem = crate::character_editor::models::def_stem(source);
        if let Some(info) = gltf_info::inspect(source) {
            sourced.extend(info.animations.into_iter().map(|clip| (stem.clone(), clip)));
        }
    }
    ClipSet::new(own, sourced)
}

pub fn map(def: &mut AssetDefinition, key: AnimationKey, clip: &str) {
    def.animations.insert(key, clip.to_string());
}

/// Drop the def's entry, handing the key back to the preset / name search.
pub fn unmap(def: &mut AssetDefinition, key: AnimationKey) {
    def.animations.remove(&key);
}

/// What `key` plays, for display: the clip and, unless the def names it, where it came
/// from. A def entry naming a clip the model lacks is called out — it looks mapped but
/// the game skips it.
pub fn resolution_label(
    clips: &ClipSet,
    def: &AssetDefinition,
    settings: Option<&AnimMapping>,
    key: AnimationKey,
) -> String {
    let resolved = clips.resolve(key, Some(&def.animations), settings);
    let missing = def
        .animations
        .get(&key)
        .filter(|clip| !clips.contains(clip));
    let shown = match resolved {
        None => "(no clip)".to_string(),
        Some(r) => match r.via {
            Via::Def => r.clip,
            Via::Preset(rig) => format!("{}  ({} preset)", r.clip, rig.label()),
            Via::Settings => format!("{}  (player-settings)", r.clip),
            Via::NameSearch => format!("{}  (by name)", r.clip),
            Via::Fallback(from) => format!("{}  (as {})", r.clip, from.default_search()),
        },
    };
    match missing {
        Some(clip) => format!("'{clip}' is not in the model - {shown}"),
        None => shown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clips(names: &[&str]) -> ClipSet {
        ClipSet::new(names.iter().map(ToString::to_string), [])
    }

    #[test]
    fn an_unmapped_key_shows_where_its_clip_comes_from() {
        let def = AssetDefinition::default();
        let set = clips(&["Idle_A", "Walk", "Sprint"]);
        assert_eq!(
            resolution_label(&set, &def, None, AnimationKey::Run),
            "Sprint  (human preset)"
        );
        assert_eq!(
            resolution_label(&set, &def, None, AnimationKey::WalkShoot),
            "Walk  (as walk)"
        );
        assert_eq!(resolution_label(&set, &def, None, AnimationKey::Wave), "(no clip)");
    }

    #[test]
    fn a_mapped_key_shows_just_the_clip_and_unmapping_restores_the_preset() {
        let mut def = AssetDefinition::default();
        let set = clips(&["Idle_A", "Run_Anime", "Sprint"]);
        map(&mut def, AnimationKey::Run, "Run_Anime");
        assert_eq!(resolution_label(&set, &def, None, AnimationKey::Run), "Run_Anime");
        unmap(&mut def, AnimationKey::Run);
        assert_eq!(
            resolution_label(&set, &def, None, AnimationKey::Run),
            "Sprint  (human preset)"
        );
    }

    /// The failure worth surfacing: it looks mapped but plays something else.
    #[test]
    fn a_mapping_to_a_missing_clip_says_so() {
        let mut def = AssetDefinition::default();
        map(&mut def, AnimationKey::Idle, "Renamed");
        let label = resolution_label(&clips(&["Idle_A"]), &def, None, AnimationKey::Idle);
        assert!(label.starts_with("'Renamed' is not in the model"), "got {label}");
    }
}
