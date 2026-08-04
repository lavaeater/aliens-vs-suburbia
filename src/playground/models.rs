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
use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, ModelType, PlayerProps};
use crate::building::systems::ToWorldCoordinates;
use crate::general::events::map_events::SpawnPlayer;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::Player;
use crate::player_setup::state::{InputDevice, PlayerRoster};

/// Tile the arena's `PlayerSpawn` sits on, in padded map space — where a swapped-in model
/// appears if there is no live player to inherit a position from.
const SPAWN_TILE: (usize, usize) = (9, 9);

#[derive(Resource, Default)]
pub struct PlaygroundModels {
    /// Def paths of every importedplayer model, sorted.
    pub defs: Vec<String>,
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
        models
    }

    pub fn refresh_defs(&mut self) {
        self.defs = crate::player_setup::state::scan_player_defs();
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

    /// Write a minimal def for `model_path` so the model becomes selectable.
    ///
    /// Refuses to clobber an existing def: re-importing a model you have already tuned
    /// would silently reset its scale, hardpoints and animation bindings.
    pub fn import(&mut self, model_path: &str) {
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
) {
    if let Some(def_path) = models.pending_def.take() {
        // Respawn where the player is standing, so a swap does not teleport you.
        let position = players
            .iter()
            .next()
            .map(|(entity, position)| {
                commands.entity(entity).despawn();
                position.0
            })
            .unwrap_or_else(|| SPAWN_TILE.to_world_coords(&tile_defs) + Vec3::new(0.0, 1.0, 0.0));

        // `spawn_players` reads the roster for slot 0, which is how the def reaches the
        // scene, the animation graph and the equipped weapon.
        commands.insert_resource(PlayerRoster {
            def_paths: vec![def_path.clone()],
            devices: vec![InputDevice::Keyboard],
        });
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

    #[test]
    fn def_stem_is_what_the_list_shows() {
        assert_eq!(def_stem("assets/defs/amy.ron"), "amy");
    }
}
