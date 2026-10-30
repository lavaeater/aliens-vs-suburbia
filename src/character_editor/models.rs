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

use crate::physics::prelude::Position;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, ModelType, PlayerProps};
use crate::assets::gltf_info;
use crate::building::systems::ToWorldCoordinates;
use crate::character_editor::doc::{DefDoc, EditorDoc, EditorFocus, expected_live};
use crate::character_editor::prefs::EditorPrefs;
use crate::general::events::map_events::SpawnPlayer;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::Player;
use crate::player_setup::state::{InputDevice, PlayerRoster};

/// Tile the arena's `PlayerSpawn` sits on, in padded map space — where a swapped-in model
/// appears if there is no live player to inherit a position from.
pub const SPAWN_TILE: (usize, usize) = (9, 9);

#[derive(Resource, Default)]
pub struct EditorLibrary {
    /// Def paths of every imported model, of every type, sorted by type then name. The
    /// library lists them all: a character opens to be worn, a weapon to be held, anything
    /// else to stand on its own in Studio.
    pub defs: Vec<(String, DefKind)>,
    /// Of those, the ones whose model file contains no geometry. Wearing one spawns an
    /// invisible character, which reads as "the model failed to load" — so they are
    /// labelled in the list and refused on click.
    pub mesh_less: HashSet<String>,
    /// The def the player is currently wearing.
    pub selected: Option<String>,
    pub list_dirty: bool,
    /// Separate from `list_dirty`: the two lists rebuild in different systems, and with a
    /// shared flag whichever ran first cleared it and the other list stayed empty.
    pub weapon_list_dirty: bool,

    /// Def paths of every weapon, sorted. Any of them can be handed to any character —
    /// that is the whole promise of hardpoints, and this is where it gets tried.
    pub weapon_defs: Vec<String>,

    /// Folder the library is browsing, relative to `assets/`.
    pub browse_folder: String,
    pub folders: Vec<String>,
    pub files: Vec<String>,
    /// Every model file under `assets/`, for searching across folders.
    pub all_files: Vec<String>,
    /// Whether a file has geometry, by model path. Finding out reads the file.
    pub file_kinds: bevy::platform::collections::HashMap<String, ImportKind>,
    /// The search box's text.
    pub query: String,
    /// The type chip picked, `None` = all.
    pub kind_filter: Option<DefKind>,
    /// The list as last drawn, for the keyboard to step through.
    pub rows: Vec<crate::character_editor::library::LibraryRow>,
    /// The row the keyboard is on, an index into `rows`.
    pub highlight: Option<usize>,

    /// A def picked but not yet applied — see `swap_player_model`.
    pub pending_def: Option<String>,
    /// Where to put the replacement once the old player is gone.
    pub pending_position: Option<Vec3>,
    /// `SpawnPlayer` sent, replacement not in the world yet. Until it is, `PlayerAssetDef`
    /// still holds the old def, and comparing the doc against it would ask for another
    /// respawn.
    pub awaiting_spawn: bool,

    /// One-line feedback under the import browser.
    pub status: String,
}

impl EditorLibrary {
    pub fn fresh() -> Self {
        let mut models = Self {
            browse_folder: "packs".to_string(),
            ..Default::default()
        };
        models.refresh_defs();
        models.refresh_browser();
        models.all_files = crate::character_editor::library::scan_all_files();
        // Come back wearing whatever you last wore, so the session starts on a rig you
        // were actually working on rather than on whatever `ModelSettings` points at.
        if let Some(def_path) =
            EditorPrefs::load().resolve_last_model(|path| std::path::Path::new(path).exists())
        {
            models.select(&def_path);
        }
        models
    }

    pub fn refresh_defs(&mut self) {
        self.defs = scan_all_defs();
        self.weapon_defs = scan_weapon_defs();
        self.mesh_less = self
            .defs
            .iter()
            .map(|(def_path, _)| def_path)
            .filter(|def_path| {
                AssetDefinition::load_from_def_path(def_path)
                    .and_then(|def| gltf_info::inspect(&def.model_path))
                    .is_some_and(|info| info.mesh_count == 0)
            })
            .cloned()
            .collect();
        self.list_dirty = true;
        self.weapon_list_dirty = true;
    }

    pub fn refresh_browser(&mut self) {
        let (folders, files) = crate::asset_browser::state::scan_folder(&self.browse_folder);
        self.folders = folders;
        self.files = files;
        self.highlight = None;
        self.list_dirty = true;
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
    ///
    /// Opens the def fresh from disk unless it is the doc already open. Callers deal with
    /// unsaved edits first — see `doc::request`.
    pub fn select(&mut self, def_path: &str) {
        self.pending_def = Some(def_path.to_string());
        self.list_dirty = true;
    }

    /// Respawn the character from the open doc, keeping its edits. Same path as a swap.
    pub fn respawn(&mut self, def_path: &str) {
        self.pending_def = Some(def_path.to_string());
    }

    /// A swap or respawn is somewhere between queued and the new character standing there.
    pub const fn swap_in_progress(&self) -> bool {
        self.pending_def.is_some() || self.pending_position.is_some() || self.awaiting_spawn
    }

    /// Remember this model for next time. Called once the swap is actually applied, so a
    /// def that fails to load is not the one you come back to.
    fn remember(def_path: &str) {
        EditorPrefs::update(|prefs| prefs.last_model_def = Some(def_path.to_string()));
    }
}

/// The library's grouping of defs, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefKind {
    Player,
    Weapon,
    Enemy,
    Tower,
    Terrain,
    Item,
}

impl DefKind {
    pub const ALL: [Self; 6] = [
        Self::Player,
        Self::Weapon,
        Self::Enemy,
        Self::Tower,
        Self::Terrain,
        Self::Item,
    ];

    /// Short name, for chips and "imported as" labels.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Player => "Character",
            Self::Weapon => "Weapon",
            Self::Enemy => "Enemy",
            Self::Tower => "Tower",
            Self::Terrain => "Terrain",
            Self::Item => "Item",
        }
    }

    /// A new def's `model_type` for this kind, every prop at its default.
    pub fn model_type(self) -> ModelType {
        use crate::assets::asset_definition::{
            EnemyProps, ItemProps, TerrainProps, TowerProps, WeaponProps,
        };
        match self {
            Self::Player => ModelType::Player(PlayerProps::default()),
            Self::Weapon => ModelType::Weapon(WeaponProps::default()),
            Self::Enemy => ModelType::Enemy(EnemyProps::default()),
            Self::Tower => ModelType::Tower(TowerProps::default()),
            Self::Terrain => ModelType::Terrain(TerrainProps::default()),
            Self::Item => ModelType::Item(ItemProps::default()),
        }
    }

    pub const fn of(model_type: &ModelType) -> Self {
        match model_type {
            ModelType::Player(_) => Self::Player,
            ModelType::Weapon(_) => Self::Weapon,
            ModelType::Enemy(_) => Self::Enemy,
            ModelType::Tower(_) => Self::Tower,
            ModelType::Terrain(_) => Self::Terrain,
            ModelType::Item(_) => Self::Item,
        }
    }

    /// Heading over the group. ASCII only, like all UI text.
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Player => "CHARACTERS",
            Self::Weapon => "WEAPONS",
            Self::Enemy => "ENEMIES",
            Self::Tower => "TOWERS",
            Self::Terrain => "TERRAIN",
            Self::Item => "ITEMS",
        }
    }
}

/// Every parseable `assets/defs/*.ron`, with its kind, sorted by kind then path.
pub fn scan_all_defs() -> Vec<(String, DefKind)> {
    let Ok(entries) = std::fs::read_dir("assets/defs") else {
        return vec![];
    };
    let mut defs: Vec<(String, DefKind)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.to_str()? != "ron" {
                return None;
            }
            let def = AssetDefinition::load_from_def_path(path.to_str()?)?;
            Some((
                path.to_string_lossy().replace('\\', "/"),
                DefKind::of(&def.model_type),
            ))
        })
        .collect();
    defs.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    defs
}

/// Every `assets/defs/*.ron` whose `model_type` is `Weapon`, sorted.
///
/// The counterpart of `scan_player_defs`, kept here rather than beside it because the
/// player-setup screen has no use for weapons — only the character editor hands them out.
pub fn scan_weapon_defs() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("assets/defs") else {
        return vec![];
    };
    let mut paths: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.to_str()? != "ron" {
                return None;
            }
            let def = AssetDefinition::load_from_def_path(path.to_str()?)?;
            matches!(def.model_type, ModelType::Weapon(_))
                .then(|| path.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    paths.sort();
    paths
}

/// Point a character def at a weapon def.
///
/// Returns `false` for a def that is not a character — `PlayerProps` is where the weapon
/// lives, and there is nowhere to put it on a tower or an alien.
pub fn set_weapon(def: &mut AssetDefinition, weapon_def_path: Option<&str>) -> bool {
    let ModelType::Player(props) = &mut def.model_type else {
        return false;
    };
    props.weapon = weapon_def_path.map(crate::assets::paths::content_ref);
    true
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

/// Add `source_path` to a def's `animation_sources`. Its clips show up in the Animation
/// tab as `"<stem>|<clip>"`, and a Mesh2Motion library on the model's rig fills the
/// preset's keys the model's own export left out.
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
    (
        true,
        format!("added {stem} ({} clips)", info.animations.len()),
    )
}

/// Append `name` to a browser folder path, treating an empty path as the `assets/` root.
pub fn join_folder(current: &str, name: &str) -> String {
    if current.is_empty() {
        name.to_string()
    } else {
        format!("{current}/{name}")
    }
}

/// Step one level up. The root is its own parent, so repeated "up" is harmless.
pub fn parent_folder(current: &str) -> String {
    match current.rfind('/') {
        Some(i) => current.get(..i).unwrap_or_default().to_string(),
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
pub const fn decide_swap(player_position: Option<Vec3>, frames_waited: u32) -> SwapStep {
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
#[allow(clippy::too_many_arguments)]
pub fn swap_player_model(
    mut models: ResMut<EditorLibrary>,
    mut doc: ResMut<EditorDoc>,
    focus: Res<EditorFocus>,
    mut commands: Commands,
    players: Query<(Entity, &Position), With<Player>>,
    tile_defs: Res<TileDefinitions>,
    mut spawn_player_mw: MessageWriter<SpawnPlayer>,
    mut frames_waited: Local<u32>,
) {
    if models.awaiting_spawn && !players.is_empty() {
        models.awaiting_spawn = false;
    }

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

        // A different model opens fresh from disk; the same one is a respawn and keeps
        // its edits.
        if doc.def_path.as_deref() != Some(def_path.as_str()) {
            doc.0 = DefDoc::open(&def_path);
        }

        // `spawn_players` reads the roster for slot 0, which is how the def reaches the
        // scene, the animation graph and the equipped weapon. The doc rides along as an
        // override so the spawn shows its unsaved edits.
        commands.insert_resource(PlayerRoster {
            def_paths: vec![def_path.clone()],
            devices: vec![InputDevice::Keyboard],
            def_overrides: vec![
                doc.def
                    .as_ref()
                    .map(|def| expected_live(def, focus.held_weapon.as_deref())),
            ],
        });
        EditorLibrary::remember(&def_path);
        models.selected = Some(def_path);
        models.pending_position = Some(position);
        return;
    }

    if let Some(position) = models.pending_position
        && players.is_empty()
    {
        spawn_player_mw.write(SpawnPlayer {
            position,
            slot: None,
            lives: None,
        });
        models.pending_position = None;
        models.awaiting_spawn = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_is_not_added_twice() {
        let mut def = AssetDefinition::default();
        def.animation_sources
            .push("models/male-anims.glb".to_string());

        let (changed, status) = add_animation_source(&mut def, "models/male-anims.glb");
        assert!(!changed, "a source already listed is not added twice");
        assert!(status.contains("already"), "got {status}");
        assert_eq!(def.animation_sources.len(), 1);
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
        assert_eq!(
            parent_folder(""),
            "",
            "up from the root is a no-op, not a crash"
        );
    }

    /// Import writes a def of the chosen kind; the chips and the import bar both walk
    /// `DefKind::ALL`, so every kind must be reachable and map back to itself.
    #[test]
    fn every_kind_round_trips_through_its_model_type() {
        for kind in DefKind::ALL {
            assert_eq!(DefKind::of(&kind.model_type()), kind);
            assert!(kind.label().is_ascii() && kind.heading().is_ascii());
        }
    }

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
