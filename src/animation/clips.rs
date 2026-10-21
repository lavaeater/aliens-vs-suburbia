//! Which clip plays for an `AnimationKey`: one pure resolver shared by the graph builder
//! (`model_settings::plugin::build_graph`) and the character editor's Animation tab, so
//! what the editor shows is what the game plays.
//!
//! In order, the first that names a clip the model has:
//!
//! 1. the def's own `animations` entry for the key (exact clip name),
//! 2. the detected rig's preset (see [`Rig`]),
//! 3. the `ModelSettings` override (players only; a name fragment). It is one global
//!    mapping for every player model, written for a quaternius pack, so it must not
//!    shadow a rig preset,
//! 4. a name search on `AnimationKey::default_search` (quaternius/mixamo-era packs),
//! 5. the same chain for `AnimationKey::fallback`, if the key has one.
//!
//! Clip names are the model's own, or `"<source stem>|<clip>"` for clips from an
//! animation source.

use std::collections::BTreeMap;

use crate::animation::animation_plugin::{AnimationKey, clip_matches};
use crate::model_settings::resources::AnimMapping;

/// A Mesh2Motion rig, whose library names its clips the same way on every model built on
/// it. A def for a model on a known rig needs no `animations` at all: the preset fills
/// every key the model has a clip for.
///
/// Names are from the libraries in `mesh2motion-app/static/animations/`; each key lists
/// candidates in order of preference, since an export usually carries a subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rig {
    Human,
    Fox,
    Horse,
    Spider,
    Kaiju,
    Snake,
    Shark,
    Bird,
    Dragon,
}

type Preset = &'static [(AnimationKey, &'static [&'static str])];

const HUMAN: Preset = &[
    (AnimationKey::Idle, &["Idle_A", "Idle_Subtle"]),
    (
        AnimationKey::IdleShoot,
        &["Pistol_Aim_Neutral", "Pistol_Idle"],
    ),
    (AnimationKey::Walk, &["Walk"]),
    (AnimationKey::Run, &["Sprint", "Jog", "Run_Anime"]),
    (AnimationKey::Duck, &["Crouch_Idle"]),
    (AnimationKey::Jump, &["Jump_Start"]),
    (AnimationKey::JumpIdle, &["Jump_air"]),
    (AnimationKey::JumpLand, &["Jump_Land"]),
    (AnimationKey::Punch, &["Punch_Jab", "Punch_Cross"]),
    (AnimationKey::Wave, &["Greeting"]),
    (AnimationKey::Yes, &["Yes", "Head Nod"]),
    (AnimationKey::No, &["Reject"]),
    (
        AnimationKey::Death,
        &["Death_A", "Death_B", "Death_C", "Death_D"],
    ),
    (AnimationKey::HitReact, &["Hit_Chest", "Hit_Head"]),
    (AnimationKey::Throwing, &["OverhandThrow", "Throw Object"]),
    (AnimationKey::Building, &["Interact", "Fixing_Kneeling"]),
    (AnimationKey::Reload, &["Pistol_Reload"]),
];

const FOX: Preset = &[
    (AnimationKey::Idle, &["Idle", "Idle Alert"]),
    (AnimationKey::Walk, &["Walk", "Sneak"]),
    (AnimationKey::Run, &["Run"]),
    (AnimationKey::Jump, &["Jump"]),
    (AnimationKey::Punch, &["Bite"]),
    (AnimationKey::Death, &["Death"]),
];

const HORSE: Preset = &[
    (AnimationKey::Idle, &["Idle"]),
    (AnimationKey::Walk, &["Walk"]),
    (AnimationKey::Run, &["Run", "Trot"]),
    (AnimationKey::Punch, &["Kick", "Head_But"]),
    (AnimationKey::Death, &["Death"]),
];

const SPIDER: Preset = &[
    (AnimationKey::Idle, &["Idle"]),
    (AnimationKey::Walk, &["Walk"]),
    (AnimationKey::Jump, &["Jump"]),
    (AnimationKey::Punch, &["Attack", "Bite"]),
    (AnimationKey::Death, &["Death", "Death 2"]),
    (AnimationKey::HitReact, &["Hit"]),
];

const KAIJU: Preset = &[
    (AnimationKey::Idle, &["Idle"]),
    (AnimationKey::Walk, &["Walk"]),
    (AnimationKey::Punch, &["Attack", "Tail Attack"]),
    (AnimationKey::Death, &["Death"]),
    (AnimationKey::HitReact, &["Hit"]),
];

const SNAKE: Preset = &[
    (AnimationKey::Idle, &["Idle", "Coiled"]),
    (AnimationKey::Walk, &["Side winding"]),
    (AnimationKey::Punch, &["Bite"]),
    (AnimationKey::Death, &["Death"]),
    (AnimationKey::HitReact, &["Hit"]),
];

const SHARK: Preset = &[
    (AnimationKey::Idle, &["Idle"]),
    (AnimationKey::Walk, &["Swim Horizontal"]),
    (AnimationKey::Punch, &["Bite"]),
    (AnimationKey::Death, &["Dead Floating"]),
    (AnimationKey::HitReact, &["Hit"]),
];

const BIRD: Preset = &[
    (AnimationKey::Idle, &["Idle"]),
    (AnimationKey::Walk, &["Walk"]),
    (AnimationKey::Run, &["Flap", "Glide"]),
];

const DRAGON: Preset = &[
    (AnimationKey::Idle, &["Idle"]),
    (AnimationKey::Walk, &["Walk"]),
    (AnimationKey::Run, &["Fly Flap", "Fly Glide"]),
];

impl Rig {
    pub const ALL: [Self; 9] = [
        Self::Human,
        Self::Fox,
        Self::Horse,
        Self::Spider,
        Self::Kaiju,
        Self::Snake,
        Self::Shark,
        Self::Bird,
        Self::Dragon,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Fox => "fox",
            Self::Horse => "horse",
            Self::Spider => "spider",
            Self::Kaiju => "kaiju",
            Self::Snake => "snake",
            Self::Shark => "shark",
            Self::Bird => "bird",
            Self::Dragon => "dragon",
        }
    }

    const fn preset(self) -> Preset {
        match self {
            Self::Human => HUMAN,
            Self::Fox => FOX,
            Self::Horse => HORSE,
            Self::Spider => SPIDER,
            Self::Kaiju => KAIJU,
            Self::Snake => SNAKE,
            Self::Shark => SHARK,
            Self::Bird => BIRD,
            Self::Dragon => DRAGON,
        }
    }

    /// The clip names this rig's preset tries for `key`, best first.
    pub fn candidates(self, key: AnimationKey) -> &'static [&'static str] {
        self.preset()
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(&[], |(_, names)| *names)
    }

    /// The rig whose preset names the most of `clips`; ties go to the earlier rig in
    /// [`Rig::ALL`]. Rigs that tie only share generic names (`Idle`, `Walk`), which their
    /// presets map the same way, so the tie-break does not change what plays.
    fn detect(clips: &[Clip]) -> Option<Self> {
        let mut best: Option<(Self, usize)> = None;
        for rig in Self::ALL {
            let hits = clips
                .iter()
                .filter(|clip| {
                    rig.preset()
                        .iter()
                        .any(|(_, names)| names.contains(&clip.bare.as_str()))
                })
                .count();
            if hits > 0 && best.is_none_or(|(_, most)| hits > most) {
                best = Some((rig, hits));
            }
        }
        best.map(|(rig, _)| rig)
    }
}

/// Where a key's clip came from, for the editor to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Def,
    Settings,
    Preset(Rig),
    NameSearch,
    /// Borrowed from this key's clip (`AnimationKey::fallback`).
    Fallback(AnimationKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub clip: String,
    pub via: Via,
}

struct Clip {
    /// What the def stores and the graph builder looks up: the model's clip name, or
    /// `"<stem>|<clip>"` for a source clip.
    name: String,
    /// The clip's name inside its own file, which is what a rig preset names.
    bare: String,
}

/// Every clip a model can play, with its detected rig.
pub struct ClipSet {
    /// The model's own clips first, each group sorted, so a preset prefers the model's own
    /// clip and the name search does not depend on a `HashMap`'s order.
    clips: Vec<Clip>,
    pub rig: Option<Rig>,
}

impl ClipSet {
    /// `own` are the model's clip names; `sources` are `(source stem, clip name)` pairs.
    pub fn new(
        own: impl IntoIterator<Item = String>,
        sources: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        let mut own: Vec<Clip> = own
            .into_iter()
            .map(|name| Clip {
                bare: name.clone(),
                name,
            })
            .collect();
        let mut sourced: Vec<Clip> = sources
            .into_iter()
            .map(|(stem, bare)| Clip {
                name: format!("{stem}|{bare}"),
                bare,
            })
            .collect();
        for group in [&mut own, &mut sourced] {
            group.sort_by(|a, b| a.name.cmp(&b.name));
            group.dedup_by(|a, b| a.name == b.name);
        }
        own.append(&mut sourced);
        let rig = Rig::detect(&own);
        Self { clips: own, rig }
    }

    /// Every clip name, in the order `resolve` prefers them.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.clips.iter().map(|clip| clip.name.as_str())
    }

    pub fn contains(&self, clip: &str) -> bool {
        self.clips.iter().any(|c| c.name == clip)
    }

    /// The clip `key` plays, or `None` if nothing in the chain (module docs) names one.
    pub fn resolve(
        &self,
        key: AnimationKey,
        animations: Option<&BTreeMap<AnimationKey, String>>,
        settings: Option<&AnimMapping>,
    ) -> Option<Resolved> {
        self.resolve_direct(key, animations, settings).or_else(|| {
            let borrowed = key.fallback()?;
            let clip = self.resolve(borrowed, animations, settings)?.clip;
            Some(Resolved {
                clip,
                via: Via::Fallback(borrowed),
            })
        })
    }

    /// The aim clip to lay over `key`'s upper body. Moving while aiming on a rig with an
    /// aim pose but no clip for the combination (Mesh2Motion has `Pistol_Aim_Neutral`, no
    /// walk-and-aim) plays the borrowed walk on the legs and the aim on the torso
    /// (`build_graph`, `body_mask`). `None` when the key has a clip of its own or the
    /// model has no aim pose (its `IdleShoot` is itself borrowed from `Idle`).
    pub fn upper_body_layer(
        &self,
        key: AnimationKey,
        animations: Option<&BTreeMap<AnimationKey, String>>,
        settings: Option<&AnimMapping>,
    ) -> Option<String> {
        if !matches!(key, AnimationKey::WalkShoot | AnimationKey::RunShoot) {
            return None;
        }
        let own = self.resolve(key, animations, settings)?;
        let aim = self.resolve(AnimationKey::IdleShoot, animations, settings)?;
        let borrowed = |via| matches!(via, Via::Fallback(_));
        (borrowed(own.via) && !borrowed(aim.via)).then_some(aim.clip)
    }

    fn resolve_direct(
        &self,
        key: AnimationKey,
        animations: Option<&BTreeMap<AnimationKey, String>>,
        settings: Option<&AnimMapping>,
    ) -> Option<Resolved> {
        let found = |clip: &String, via| {
            Some(Resolved {
                clip: clip.clone(),
                via,
            })
        };
        if let Some(clip) = animations
            .and_then(|map| map.get(&key))
            .filter(|clip| self.contains(clip))
        {
            return found(clip, Via::Def);
        }
        if let Some(rig) = self.rig {
            for name in rig.candidates(key) {
                if let Some(clip) = self.clips.iter().find(|clip| clip.bare == *name) {
                    return found(&clip.name, Via::Preset(rig));
                }
            }
        }
        if let Some(clip) = settings
            .map(|s| s.get(key))
            .filter(|fragment| !fragment.is_empty())
            .and_then(|fragment| self.search(fragment))
        {
            return found(clip, Via::Settings);
        }
        self.search(key.default_search())
            .and_then(|clip| found(clip, Via::NameSearch))
    }

    /// Exact match on the part after the last `|` or a whole-name suffix first, so
    /// `"walk"` finds `"Armature|Walk"` before `"Armature|Walk_Shoot"`; then a substring.
    fn search(&self, fragment: &str) -> Option<&String> {
        let s = fragment.to_lowercase();
        let names = || self.clips.iter().map(|clip| &clip.name);
        names()
            .find(|name| {
                let lower = name.to_lowercase();
                let last = lower.rsplit('|').next().unwrap_or(&lower);
                last == s || lower.ends_with(&s)
            })
            .or_else(|| names().find(|name| clip_matches(name, fragment)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(clips: &[&str]) -> ClipSet {
        ClipSet::new(clips.iter().map(ToString::to_string), [])
    }

    /// The clips `assets/new-models/male-simple.glb` ships with.
    fn male_simple() -> ClipSet {
        set(&[
            "Death_A",
            "Death_B",
            "Death_C",
            "Death_D",
            "Idle_A",
            "Pistol_Aim_Neutral",
            "Run_Anime",
            "Sprint",
            "Walk",
        ])
    }

    fn clip(set: &ClipSet, key: AnimationKey) -> Option<String> {
        set.resolve(key, None, None).map(|r| r.clip)
    }

    #[test]
    fn a_mesh2motion_human_needs_no_mapping() {
        let s = male_simple();
        assert_eq!(s.rig, Some(Rig::Human));
        assert_eq!(clip(&s, AnimationKey::Idle).as_deref(), Some("Idle_A"));
        assert_eq!(clip(&s, AnimationKey::Run).as_deref(), Some("Sprint"));
        assert_eq!(clip(&s, AnimationKey::Death).as_deref(), Some("Death_A"));
        assert_eq!(
            clip(&s, AnimationKey::IdleShoot).as_deref(),
            Some("Pistol_Aim_Neutral")
        );
    }

    #[test]
    fn walking_while_shooting_without_a_clip_for_it_walks() {
        let r = male_simple()
            .resolve(AnimationKey::WalkShoot, None, None)
            .expect("falls back");
        assert_eq!(r.clip, "Walk");
        assert_eq!(r.via, Via::Fallback(AnimationKey::Walk));
    }

    #[test]
    fn the_def_wins_over_the_preset() {
        let mut map = BTreeMap::new();
        map.insert(AnimationKey::Run, "Run_Anime".to_string());
        let r = male_simple()
            .resolve(AnimationKey::Run, Some(&map), None)
            .expect("mapped");
        assert_eq!((r.clip.as_str(), r.via), ("Run_Anime", Via::Def));
    }

    /// A def naming a clip the model does not have must not shadow the rest of the chain.
    #[test]
    fn a_def_entry_for_a_missing_clip_is_skipped() {
        let mut map = BTreeMap::new();
        map.insert(AnimationKey::Idle, "Gone".to_string());
        assert_eq!(
            male_simple()
                .resolve(AnimationKey::Idle, Some(&map), None)
                .map(|r| r.via),
            Some(Via::Preset(Rig::Human))
        );
    }

    #[test]
    fn source_clips_match_the_preset_by_their_bare_name() {
        let s = ClipSet::new(
            ["Walk".to_string()],
            [("human-base-animations".to_string(), "Idle_A".to_string())],
        );
        assert_eq!(
            clip(&s, AnimationKey::Idle).as_deref(),
            Some("human-base-animations|Idle_A")
        );
    }

    #[test]
    fn the_models_own_clip_beats_a_sources() {
        let s = ClipSet::new(
            ["Idle_A".to_string()],
            [("aaa-library".to_string(), "Idle_A".to_string())],
        );
        assert_eq!(clip(&s, AnimationKey::Idle).as_deref(), Some("Idle_A"));
    }

    #[test]
    fn a_quaternius_pack_still_resolves_by_name() {
        let s = set(&[
            "CharacterArmature|Walk_Shoot",
            "CharacterArmature|Walk",
            "CharacterArmature|Idle",
        ]);
        assert_eq!(s.rig, None);
        assert_eq!(
            clip(&s, AnimationKey::Walk).as_deref(),
            Some("CharacterArmature|Walk")
        );
        assert_eq!(
            clip(&s, AnimationKey::WalkShoot).as_deref(),
            Some("CharacterArmature|Walk_Shoot")
        );
    }

    #[test]
    fn a_throw_borrows_the_punch() {
        let s = set(&["CharacterArmature|Punch", "CharacterArmature|Idle"]);
        assert_eq!(
            s.resolve(AnimationKey::Throwing, None, None),
            Some(Resolved {
                clip: "CharacterArmature|Punch".to_string(),
                via: Via::Fallback(AnimationKey::Punch),
            })
        );
    }

    /// `player-settings.ron` ships `throwing: "Punch"` for a quaternius pack; on a
    /// Mesh2Motion human that substring-matches `Punch_Jab` and would beat the throw.
    #[test]
    fn the_global_settings_override_does_not_beat_a_rig_preset() {
        let s = set(&["Idle_A", "Punch_Jab", "OverhandThrow"]);
        let settings = AnimMapping {
            throwing: "Punch".to_string(),
            ..AnimMapping::default()
        };
        assert_eq!(
            s.resolve(AnimationKey::Throwing, None, Some(&settings))
                .map(|r| r.clip)
                .as_deref(),
            Some("OverhandThrow")
        );
    }

    #[test]
    fn a_distinctive_clip_picks_the_animal_rig() {
        let s = set(&["Idle", "Walk", "Trot", "Kick"]);
        assert_eq!(s.rig, Some(Rig::Horse));
        assert_eq!(clip(&s, AnimationKey::Run).as_deref(), Some("Trot"));
    }

    #[test]
    fn walking_while_aiming_lays_the_aim_over_the_walk() {
        let set = male_simple();
        let layer = |key| set.upper_body_layer(key, None, None);
        assert_eq!(
            layer(AnimationKey::WalkShoot).as_deref(),
            Some("Pistol_Aim_Neutral")
        );
        assert_eq!(
            layer(AnimationKey::RunShoot).as_deref(),
            Some("Pistol_Aim_Neutral")
        );
        assert_eq!(layer(AnimationKey::Walk), None);
    }

    #[test]
    fn no_layer_without_an_aim_pose_or_with_a_walk_and_aim_clip() {
        let no_aim = set(&["Idle_A", "Walk"]);
        assert_eq!(
            no_aim.upper_body_layer(AnimationKey::WalkShoot, None, None),
            None
        );
        let own = set(&["Idle", "Idle_Shoot", "Walk", "Walk_Shoot"]);
        assert_eq!(
            own.upper_body_layer(AnimationKey::WalkShoot, None, None),
            None
        );
    }
}
