//! Picking, importing, and live-swapping the model you walk around as.
//!
//! Two lists in the left pane. The upper one is every *imported* model — an
//! `assets/defs/*.ron` whose `model_type` is `Player`. Clicking one respawns the player
//! wearing it, in place, without leaving the session. The lower one is a file browser over
//! `assets/`; "importing" a `.glb` there just writes a minimal def for it, after which it
//! shows up in the upper list forever.
//!
//! The swap goes through `PlayerRoster` rather than poking the player entity, so it takes
//! exactly the path a real match takes: `spawn_players` reads the def for the slot and
//! derives the scene, ability, throw rate, animation graph and equipped weapon from it.

use avian3d::prelude::Position;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, ModelType, PlayerProps};
use crate::building::systems::ToWorldCoordinates;
use crate::general::events::map_events::SpawnPlayer;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::Player;
use crate::player_setup::state::{InputDevice, PlayerRoster};
use crate::playground::gltf_info;
use crate::playground::prefs::PlaygroundPrefs;

/// Tile the arena's `PlayerSpawn` sits on, in padded map space — where a swapped-in model
/// appears if there is no live player to inherit a position from.
const SPAWN_TILE: (usize, usize) = (9, 9);

#[derive(Resource, Default)]
pub struct PlaygroundModels {
    /// Def paths of every imported player model, sorted.
    pub defs: Vec<String>,
    /// Of those, the ones whose model file contains no geometry. Wearing one spawns an
    /// invisible character, which reads as "the model failed to load" — so they are
    /// labelled in the list and refused on click.
    pub mesh_less: HashSet<String>,
    /// The def the player is currently wearing.
    pub selected: Option<String>,
    pub list_dirty: bool,

    /// Folder the import browser is showing, relative to `assets/`.
    pub browse_folder: String,
    pub folders: Vec<String>,
    pub files: Vec<String>,
    pub browser_dirty: bool,

    /// A def picked but not yet applied — see `swap_player_model`.
    pub pending_def: Option<String>,
    /// Where to put the replacement once the old player is gone.
    pub pending_position: Option<Vec3>,

    /// One-line feedback under the import browser.
    pub status: String,
}

impl PlaygroundModels {
    pub fn fresh() -> Self {
        let mut models = Self { browse_folder: "packs".to_string(), ..Default::default() };
        models.refresh_defs();
        models.refresh_browser();
        // Come back wearing whatever you last wore, so the session starts on a rig you
        // were actually working on rather than on whatever `ModelSettings` points at.
        if let Some(def_path) =
            PlaygroundPrefs::load().resolve_last_model(|path| std::path::Path::new(path).exists())
        {
            models.select(&def_path);
        }
        models
    }

    pub fn refresh_defs(&mut self) {
        self.defs = crate::player_setup::state::scan_player_defs();
        self.mesh_less = self
            .defs
            .iter()
            .filter(|def_path| {
                AssetDefinition::load_from_def_path(def_path)
                    .and_then(|def| gltf_info::inspect(&def.model_path))
                    .is_some_and(|info| info.mesh_count == 0)
            })
            .cloned()
            .collect();
        self.list_dirty = true;
    }

    pub fn refresh_browser(&mut self) {
        let (folders, files) = crate::asset_browser::state::scan_folder(&self.browse_folder);
        self.folders = folders;
        self.files = files;
        self.browser_dirty = true;
    }

    pub fn enter_folder(&mut self, name: &str) {
        self.browse_folder = join_folder(&self.browse_folder, name);
        self.refresh_browser();
    }

    pub fn leave_folder(&mut self) {
        self.browse_folder = parent_folder(&self.browse_folder);
        self.refresh_browser();
    }

    /// Queue a model swap. Applied by [`swap_player_model`] over the next couple of frames.
    pub fn select(&mut self, def_path: &str) {
        self.pending_def = Some(def_path.to_string());
        self.list_dirty = true;
    }

    /// Remember this model for next time. Called once the swap is actually applied, so a
    /// def that fails to load is not the one you come back to.
    fn remember(&self, def_path: &str) {
        PlaygroundPrefs { last_model_def: Some(def_path.to_string()) }.save();
    }

    /// Write a minimal def for `model_path` so the model becomes selectable.
    ///
    /// Refuses to clobber an existing def: re-importing a model you have already tuned
    /// would silently reset its scale, hardpoints and animation bindings.
    ///
    /// Refuses geometry-less files outright — see [`classify`].
    pub fn import(&mut self, model_path: &str) {
        if classify(model_path) == ImportKind::AnimationLibrary {
            self.status =
                format!("{} has no meshes - add it as an animation source", def_stem(model_path));
            return;
        }
        let path = AssetDefinition::def_path(model_path);
        if path.exists() {
            self.status = format!("{} already imported", def_stem(&path.to_string_lossy()));
            self.refresh_defs();
            return;
        }
        new_player_def(model_path).save();
        self.status = if path.exists() {
            format!("imported {}", def_stem(&path.to_string_lossy()))
        } else {
            format!("FAILED to write {}", path.display())
        };
        self.refresh_defs();
    }
}

/// What clicking a file in the import browser should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// Has geometry: write a player def for it.
    Model,
    /// Clips but no geometry. Imported as a model it spawns an invisible character, which
    /// is indistinguishable from a load failure — it belongs in another def's
    /// `animation_sources`.
    AnimationLibrary,
}

pub fn classify(model_path: &str) -> ImportKind {
    match gltf_info::inspect(model_path) {
        // Unreadable is not a verdict: a file we cannot parse is still allowed through, so
        // an inspector bug cannot lock a real model out of the list.
        Some(info) if info.is_animation_only() => ImportKind::AnimationLibrary,
        _ => ImportKind::Model,
    }
}

/// Add `source_path` to a def's `animation_sources` and tag every clip it carries.
///
/// The tagging is what makes it useful: the animation panel binds game keys to *tag paths*
/// drawn from `clip_tags`, so a source added without tags contributes 162 clips that
/// nothing can be bound to. Tags are `<stem>/<clip>`; the clip key is the
/// `"<stem>|<clip>"` form `AssetDefinition::resolved_clip` expects for external sources.
///
/// Returns a status line, and `false` if nothing changed.
pub fn add_animation_source(def: &mut AssetDefinition, source_path: &str) -> (bool, String) {
    let stem = def_stem(source_path);
    if def.animation_sources.iter().any(|s| s == source_path) {
        return (false, format!("{stem} is already a source"));
    }
    let Some(info) = gltf_info::inspect(source_path) else {
        return (false, format!("cannot read {source_path}"));
    };

    def.animation_sources.push(source_path.to_string());
    for clip in &info.animations {
        def.clip_tags.insert(format!("{stem}|{clip}"), format!("{stem}/{clip}"));
    }
    (true, format!("added {stem} ({} clips)", info.animations.len()))
}

/// A brand-new def for an imported model: playable, and nothing else assumed.
///
/// Scale stays at the default — working it out needs the mesh AABB, which needs the model
/// loaded. The asset browser computes it from a target height; this is deliberately the
/// bare minimum that makes the model appear in the list.
pub fn new_player_def(model_path: &str) -> AssetDefinition {
    AssetDefinition {
        model_path: model_path.to_string(),
        model_type: ModelType::Player(PlayerProps::default()),
        ..Default::default()
    }
}

/// Append `name` to a browser folder path, treating an empty path as the `assets/` root.
pub fn join_folder(current: &str, name: &str) -> String {
    if current.is_empty() { name.to_string() } else { format!("{current}/{name}") }
}

/// Step one level up. The root is its own parent, so repeated "up" is harmless.
pub fn parent_folder(current: &str) -> String {
    match current.rfind('/') {
        Some(i) => current[..i].to_string(),
        None => String::new(),
    }
}

pub fn def_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// How many frames the swap waits for a player to exist before spawning one itself.
/// Long enough to cover the map's own spawn on session entry, short enough that a swap
/// requested with no player around still happens promptly.
const SWAP_WAIT_FRAMES: u32 = 120;

/// What [`swap_player_model`] should do this frame.
#[derive(Debug, PartialEq)]
pub enum SwapStep {
    /// A player exists: replace it, keeping its position.
    Replace(Vec3),
    /// No player yet — give the map's own spawn a chance to land first.
    Wait,
    /// Waited long enough; spawn one at the arena's spawn tile.
    SpawnFresh,
}

/// Decide the swap step.
///
/// The wait matters on session entry: the remembered model is queued before the map has
/// spawned anyone, and `PlayerRoster` is inserted through `Commands`, so a swap that fired
/// immediately could race the map's spawn and leave the default model on screen with the
/// pending swap already consumed. Waiting for a player means the swap always takes the same
/// path it takes for a click.
pub fn decide_swap(player_position: Option<Vec3>, frames_waited: u32) -> SwapStep {
    match player_position {
        Some(position) => SwapStep::Replace(position),
        None if frames_waited < SWAP_WAIT_FRAMES => SwapStep::Wait,
        None => SwapStep::SpawnFresh,
    }
}

/// Apply a queued swap: despawn the current player, then spawn a replacement from the
/// chosen def once it is actually gone.
///
/// The two halves cannot run in one frame — `spawn_players` skips the event while a player
/// still exists, and the despawn only lands when commands are applied. So this parks the
/// position in `pending_position` and finishes on a later frame.
pub fn swap_player_model(
    mut models: ResMut<PlaygroundModels>,
    mut commands: Commands,
    players: Query<(Entity, &Position), With<Player>>,
    tile_defs: Res<TileDefinitions>,
    mut spawn_player_mw: MessageWriter<SpawnPlayer>,
    mut frames_waited: Local<u32>,
) {
    if let Some(def_path) = models.pending_def.clone() {
        let existing = players.iter().next();
        let position = match decide_swap(existing.map(|(_, p)| p.0), *frames_waited) {
            SwapStep::Wait => {
                *frames_waited += 1;
                return;
            }
            SwapStep::Replace(position) => {
                if let Some((entity, _)) = existing {
                    commands.entity(entity).despawn();
                }
                position
            }
            SwapStep::SpawnFresh => {
                SPAWN_TILE.to_world_coords(&tile_defs) + Vec3::new(0.0, 1.0, 0.0)
            }
        };

        models.pending_def = None;
        *frames_waited = 0;

        // `spawn_players` reads the roster for slot 0, which is how the def reaches the
        // scene, the animation graph and the equipped weapon.
        commands.insert_resource(PlayerRoster {
            def_paths: vec![def_path.clone()],
            devices: vec![InputDevice::Keyboard],
        });
        models.remember(&def_path);
        models.selected = Some(def_path);
        models.pending_position = Some(position);
        return;
    }

    if let Some(position) = models.pending_position
        && players.is_empty()
    {
        spawn_player_mw.write(SpawnPlayer { position });
        models.pending_position = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clips have to be tagged, or the animation panel — which binds keys to tag
    /// paths, not clip names — offers nothing to bind them to.
    #[test]
    fn an_added_source_tags_every_clip_it_brings() {
        let mut def = AssetDefinition::default();
        // Bypass the file read: this is the tagging half, exercised directly.
        def.animation_sources.push("models/male-anims.glb".to_string());
        def.clip_tags.insert("male-anims|Backflip".to_string(), "male-anims/Backflip".to_string());

        let (changed, status) = add_animation_source(&mut def, "models/male-anims.glb");
        assert!(!changed, "a source already listed is not added twice");
        assert!(status.contains("already"), "got {status}");
        assert_eq!(def.animation_sources.len(), 1);
    }

    /// The clip key has to be the `"<stem>|<clip>"` form `resolved_clip` looks for, or a
    /// binding made against the tag resolves to a clip name the runtime never sees.
    #[test]
    fn a_tagged_external_clip_resolves_back_through_its_tag() {
        let mut def = AssetDefinition::default();
        def.clip_tags.insert("male-anims|Backflip".to_string(), "male-anims/Backflip".to_string());
        def.animation_bindings.insert("jump".to_string(), "male-anims/Backflip".to_string());
        assert_eq!(def.resolved_clip("jump").as_deref(), Some("male-anims|Backflip"));
    }

    /// A file we cannot inspect must stay importable — an inspector that guesses wrong
    /// would lock a real model out of the list with no way back.
    #[test]
    fn an_unreadable_file_is_still_treated_as_a_model() {
        assert_eq!(classify("packs/does-not-exist.glb"), ImportKind::Model);
    }

    #[test]
    fn browsing_into_a_folder_builds_a_path_under_the_assets_root() {
        assert_eq!(join_folder("", "packs"), "packs");
        assert_eq!(join_folder("packs", "toon-shooter"), "packs/toon-shooter");
    }

    #[test]
    fn stepping_up_from_the_root_stays_at_the_root() {
        assert_eq!(parent_folder("packs/toon-shooter"), "packs");
        assert_eq!(parent_folder("packs"), "");
        assert_eq!(parent_folder(""), "", "up from the root is a no-op, not a crash");
    }

    #[test]
    fn an_imported_model_is_playable_and_assumes_nothing_else() {
        let def = new_player_def("packs/foo/Bar.glb");
        assert_eq!(def.model_path, "packs/foo/Bar.glb");
        assert!(matches!(def.model_type, ModelType::Player(_)));
        assert!(def.hardpoints.is_empty(), "hardpoints are authored, not guessed");
        assert!(def.aim_bones.is_empty(), "the twist falls back to the default chain");
    }

    /// The def path is what makes an import show up in the list, and what the
    /// clobber-guard checks.
    #[test]
    fn a_model_maps_to_a_def_named_after_its_file_stem() {
        assert_eq!(
            AssetDefinition::def_path("packs/toon-shooter/characters/Soldier.glb"),
            std::path::PathBuf::from("assets/defs/Soldier.ron")
        );
    }

    /// On session entry the remembered model is queued before the map has spawned
    /// anyone. Swapping right then would race the map's own spawn and leave the default
    /// model on screen with the swap already consumed.
    #[test]
    fn a_swap_with_nobody_around_yet_waits_for_the_maps_spawn() {
        assert_eq!(decide_swap(None, 0), SwapStep::Wait);
        assert_eq!(decide_swap(None, 5), SwapStep::Wait);
    }

    /// ...but not forever: a swap requested when no player is coming still has to happen.
    #[test]
    fn a_swap_that_waited_long_enough_spawns_one_itself() {
        assert_eq!(decide_swap(None, SWAP_WAIT_FRAMES), SwapStep::SpawnFresh);
    }

    #[test]
    fn a_swap_with_a_live_player_replaces_it_where_it_stands() {
        let standing = Vec3::new(3.0, 1.0, -2.0);
        assert_eq!(decide_swap(Some(standing), 0), SwapStep::Replace(standing));
        assert_eq!(
            decide_swap(Some(standing), SWAP_WAIT_FRAMES + 99),
            SwapStep::Replace(standing),
            "a live player always wins over the timeout",
        );
    }

    #[test]
    fn def_stem_is_what_the_list_shows() {
        assert_eq!(def_stem("assets/defs/amy.ron"), "amy");
    }
}
