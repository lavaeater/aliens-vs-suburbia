//! Playing and re-binding animations on the live character.
//!
//! Two jobs. Clicking a key *plays* it on the character standing in the arena, which is
//! the quickest way to find out that a rig's "wave" is actually a T-pose. And each key can
//! be re-bound to a different tag path from the def's `clip_tags`.
//!
//! Re-binding takes effect immediately with no respawn: `build_player_anim_graph` folds
//! `animation_bindings` and `clip_tags` into the signature it compares against
//! `last_sig`, so an edit to the doc (copied into `PlayerAssetDef` by
//! `doc::sync_doc_to_live`) makes it rebuild the graph on the next frame.
//!
//! The key strings are `AnimationKey::default_search()`, which is what
//! `AssetDefinition::resolved_clip` is called with at runtime — so what you bind here is
//! exactly what the game looks up.

use bevy::prelude::*;

use crate::animation::animation_plugin::AnimationKey;
use crate::assets::asset_definition::AssetDefinition;
use crate::assets::gltf_info;

/// Keys offered in the panel. The composite intent keys (`Throwing`, `Building`) are
/// included because they are bindable, even though the state machine usually drives them.
pub const PLAYABLE_KEYS: [AnimationKey; 18] = [
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
    AnimationKey::Reload,
];

#[derive(Resource, Default)]
pub struct AnimationEditor {
    /// Key whose binding is being edited.
    pub selected_key: Option<AnimationKey>,
    /// Clip whose tag is being edited (a key of `clip_tags`: the model's own clip name, or
    /// `"<source stem>|<clip>"`).
    pub selected_clip: Option<String>,
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

/// Every clip the def can bind to, as `clip_tags` keys: the model's own clips by name,
/// then each animation source's as `"<stem>|<clip>"` — the form `resolved_clip` expects.
///
/// Read off the files (`gltf_info`), so it lists clips that have no tag yet, which is the
/// point: tagging is how a clip becomes bindable.
pub fn all_clips(def: &AssetDefinition) -> Vec<String> {
    let mut clips: Vec<String> = gltf_info::inspect(&def.model_path)
        .map_or_default(|info| info.animations);
    for source in &def.animation_sources {
        let stem = crate::character_editor::models::def_stem(source);
        if let Some(info) = gltf_info::inspect(source) {
            clips.extend(info.animations.iter().map(|clip| format!("{stem}|{clip}")));
        }
    }
    // A tag on a clip the files no longer have is still shown, so it can be cleared.
    let mut stale: Vec<String> = def
        .clip_tags
        .keys()
        .filter(|clip| !clips.contains(clip))
        .cloned()
        .collect();
    stale.sort();
    clips.extend(stale);
    clips
}

/// Tidy a typed tag path: trim each `/`-separated segment and drop empty ones.
/// `"  Combat // Ranged /Shoot "` becomes `"Combat/Ranged/Shoot"`.
pub fn normalize_tag_path(raw: &str) -> String {
    raw.split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Tag `clip` with `raw` (normalized); an empty path clears the tag. A path is unique to
/// one clip — a binding resolves a path to exactly one clip — so tagging a clip with a
/// path another clip holds takes it from that clip, as the asset browser always did.
pub fn set_clip_tag(def: &mut AssetDefinition, clip: &str, raw: &str) {
    let path = normalize_tag_path(raw);
    if path.is_empty() {
        def.clip_tags.remove(clip);
        return;
    }
    def.clip_tags.retain(|other, tag| other == clip || *tag != path);
    def.clip_tags.insert(clip.to_string(), path);
}

/// Carry a def written before tags existed onto them: each `animation_mapping` key ->
/// clip becomes a tag on that clip (named after the key, unless it already has one) and a
/// binding to it. Lossless, and only when there are no bindings yet, exactly as the asset
/// browser migrates. Returns whether anything changed.
pub fn migrate_legacy_mapping(def: &mut AssetDefinition) -> bool {
    if !def.animation_bindings.is_empty() || def.animation_mapping.is_empty() {
        return false;
    }
    let mut mapping: Vec<(String, String)> = def
        .animation_mapping
        .iter()
        .filter(|(_, clip)| !clip.is_empty())
        .map(|(key, clip)| (key.clone(), clip.clone()))
        .collect();
    mapping.sort();
    for (key, clip) in mapping {
        let tag = def
            .clip_tags
            .entry(clip)
            .or_insert_with(|| key.clone())
            .clone();
        def.animation_bindings.insert(key, tag);
    }
    true
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
    def.animation_bindings
        .insert(key.default_search().to_string(), tag.to_string());
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
    fn a_typed_tag_path_is_tidied() {
        assert_eq!(normalize_tag_path("  Combat // Ranged /Shoot "), "Combat/Ranged/Shoot");
        assert_eq!(normalize_tag_path(" / "), "");
    }

    /// A path names one clip; tagging a second clip with it moves it.
    #[test]
    fn a_tag_path_moves_to_the_clip_tagged_last() {
        let mut def = def_with_tags(&[("Run", "Locomotion/Run")]);
        set_clip_tag(&mut def, "Sprint", "Locomotion / Run");
        assert_eq!(def.clip_tags.get("Sprint").map(String::as_str), Some("Locomotion/Run"));
        assert!(!def.clip_tags.contains_key("Run"));
    }

    #[test]
    fn an_empty_tag_clears_it() {
        let mut def = def_with_tags(&[("Run", "Locomotion/Run")]);
        set_clip_tag(&mut def, "Run", "  ");
        assert!(def.clip_tags.is_empty());
    }

    #[test]
    fn a_legacy_mapping_becomes_tags_and_bindings() {
        let mut def = AssetDefinition::default();
        def.animation_mapping
            .insert("walk".to_string(), "Armature|Walk".to_string());
        assert!(migrate_legacy_mapping(&mut def));
        assert_eq!(def.clip_tags.get("Armature|Walk").map(String::as_str), Some("walk"));
        assert_eq!(def.resolved_clip("walk").as_deref(), Some("Armature|Walk"));
        assert!(!migrate_legacy_mapping(&mut def), "only once: bindings exist now");
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
        assert!(
            tag_paths(&def).is_empty(),
            "empty tags are not legal targets"
        );
    }

    /// The key string has to be the one the runtime looks up, or a binding made here
    /// would silently do nothing in game.
    #[test]
    fn binding_uses_the_key_the_runtime_resolves_with() {
        let mut def = def_with_tags(&[("Armature|Wave", "Social/Wave")]);
        bind(&mut def, AnimationKey::Wave, "Social/Wave");
        assert_eq!(
            def.resolved_clip(AnimationKey::Wave.default_search())
                .as_deref(),
            Some("Armature|Wave")
        );
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
