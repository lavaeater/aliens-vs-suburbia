//! Playing and re-binding animations on the live character.
//!
//! Two jobs. Clicking a key *plays* it on the character standing in the arena, which is
//! the quickest way to find out that a rig's "wave" is actually a T-pose. And each key can
//! be re-bound to a different tag path from the def's `clip_tags`.
//!
//! Re-binding takes effect immediately with no respawn: `build_player_anim_graph` folds
//! `animation_bindings` and `clip_tags` into the signature it compares against
//! `last_sig`, so mutating `PlayerAssetDef` makes it rebuild the graph on the next frame.
//!
//! The key strings are `AnimationKey::default_search()`, which is what
//! `AssetDefinition::resolved_clip` is called with at runtime — so what you bind here is
//! exactly what the game looks up.

use bevy::prelude::*;

use crate::animation::animation_plugin::AnimationKey;
use crate::assets::asset_definition::AssetDefinition;

/// Keys offered in the panel. The composite intent keys (`Throwing`, `Building`) are
/// included because they are bindable, even though the state machine usually drives them.
pub const PLAYABLE_KEYS: [AnimationKey; 17] = [
    AnimationKey::Idle,
    AnimationKey::IdleShoot,
    AnimationKey::Walk,
    AnimationKey::WalkShoot,
    AnimationKey::Run,
    AnimationKey::RunShoot,
    AnimationKey::RunGun,
    AnimationKey::Duck,
    AnimationKey::Jump,
    AnimationKey::JumpIdle,
    AnimationKey::JumpLand,
    AnimationKey::Punch,
    AnimationKey::Wave,
    AnimationKey::Death,
    AnimationKey::HitReact,
    AnimationKey::Throwing,
    AnimationKey::Building,
];

#[derive(Resource, Default)]
pub struct AnimationEditor {
    /// Key whose binding is being edited.
    pub selected_key: Option<AnimationKey>,
    pub dirty: bool,
    pub ui_dirty: bool,
    pub status: String,
}

impl AnimationEditor {
    pub fn select(&mut self, key: AnimationKey) {
        self.selected_key = Some(key);
        self.ui_dirty = true;
    }

    pub fn touch(&mut self) {
        self.dirty = true;
        self.ui_dirty = true;
    }
}

/// Every distinct tag path the def assigns to a clip, sorted.
///
/// Bindings point at tag paths rather than clip names, so this is the set of legal
/// right-hand sides. Several clips can share a tag; the first match wins at runtime.
pub fn tag_paths(def: &AssetDefinition) -> Vec<String> {
    let mut paths: Vec<String> = def
        .clip_tags
        .values()
        .filter(|tag| !tag.is_empty())
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

pub fn bind(def: &mut AssetDefinition, key: AnimationKey, tag: &str) {
    def.animation_bindings.insert(key.default_search().to_string(), tag.to_string());
}

pub fn unbind(def: &mut AssetDefinition, key: AnimationKey) {
    def.animation_bindings.remove(key.default_search());
}

/// What this key currently resolves to, for display: the bound clip if the binding
/// resolves, otherwise a note about why not.
pub fn resolution_label(def: &AssetDefinition, key: AnimationKey) -> String {
    let name = key.default_search();
    match def.resolved_clip(name) {
        Some(clip) => clip,
        None => match def.animation_bindings.get(name) {
            // A binding that resolves to nothing is worth calling out: it means the tag
            // was renamed or the clip carrying it went away.
            Some(tag) if !tag.is_empty() => format!("(tag '{tag}' matches no clip)"),
            _ => "(unbound - falls back to name search)".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_with_tags(tags: &[(&str, &str)]) -> AssetDefinition {
        let mut def = AssetDefinition::default();
        for (clip, tag) in tags {
            def.clip_tags.insert(clip.to_string(), tag.to_string());
        }
        def
    }

    #[test]
    fn tag_paths_are_deduplicated_and_sorted() {
        let def = def_with_tags(&[
            ("Armature|Run", "Locomotion/Run"),
            ("Armature|Run2", "Locomotion/Run"),
            ("Armature|Idle", "Locomotion/Idle"),
        ]);
        assert_eq!(tag_paths(&def), vec!["Locomotion/Idle", "Locomotion/Run"]);
    }

    #[test]
    fn an_untagged_clip_offers_no_binding_target() {
        let def = def_with_tags(&[("Armature|Idle", "")]);
        assert!(tag_paths(&def).is_empty(), "empty tags are not legal targets");
    }

    /// The key string has to be the one the runtime looks up, or a binding made here
    /// would silently do nothing in game.
    #[test]
    fn binding_uses_the_key_the_runtime_resolves_with() {
        let mut def = def_with_tags(&[("Armature|Wave", "Social/Wave")]);
        bind(&mut def, AnimationKey::Wave, "Social/Wave");
        assert_eq!(def.resolved_clip(AnimationKey::Wave.default_search()).as_deref(), Some("Armature|Wave"));
    }

    #[test]
    fn unbinding_puts_the_key_back_to_the_name_search_fallback() {
        let mut def = def_with_tags(&[("Armature|Wave", "Social/Wave")]);
        bind(&mut def, AnimationKey::Wave, "Social/Wave");
        unbind(&mut def, AnimationKey::Wave);
        assert!(!def.animation_bindings.contains_key("wave"));
        assert!(resolution_label(&def, AnimationKey::Wave).contains("unbound"));
    }

    /// A tag that no clip carries is the failure worth surfacing — it looks bound but
    /// plays nothing.
    #[test]
    fn a_binding_pointing_at_a_missing_tag_says_so() {
        let mut def = def_with_tags(&[("Armature|Wave", "Social/Wave")]);
        bind(&mut def, AnimationKey::Wave, "Social/Renamed");
        assert!(
            resolution_label(&def, AnimationKey::Wave).contains("matches no clip"),
            "got {}",
            resolution_label(&def, AnimationKey::Wave)
        );
    }
}
