//! Story crawls: text that rolls up the screen or types into a dialog box, triggered by
//! the facts engine.
//!
//! The crawl itself (layout, scrolling, typing, fades, queueing) is generic and lives in
//! `lava_ui_builder::crawl`. This module is the game's side of it:
//!
//! - **Library.** Every crawl file is a [`CrawlSpec`] whose id is its file stem: a level's
//!   own sit beside its map (`assets/campaigns/main/level_01/level_01_crawl_intro.ron`),
//!   shared ones in `assets/crawls/` (`assets::content`). Re-read on entering `InGame`, so
//!   a crawl can be edited between matches.
//! - **Trigger.** A story consequence `Emit(effect: "crawl:<name>")` shows crawl `<name>`
//!   ([`keys::crawl_effect`]). Anything that can become a fact can therefore start one:
//!   the level being won, a story item picked up (`ItemFound.<id>`), a kill count...
//! - **Follow-up.** When a crawl ends it sets `CrawlFinished.<name>` true
//!   ([`keys::crawl_finished`]), so stories can chain off it; `CrawlShowing` is true while
//!   any crawl is up or queued.
//! - **Holding the game.** A `modal` crawl (the default) puts the match in
//!   [`PauseState::Cutscene`]: `Time<Virtual>` stops (waves, physics, animation) and
//!   player input closes, exactly like the pause menu. The level-end transition also waits
//!   for crawls to finish (`score_keeper::level_state_system`).
//! - **Input.** Fire / Place / Interact / Pause or Enter, from the keyboard or any pad,
//!   advance a modal crawl. A non-modal one plays over the running game, so only
//!   Interact (E / Circle) and Enter advance it - Fire is for shooting.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use lava_ui_builder::crawl::{
    AdvanceCrawl, ClearCrawls, CrawlFinished, CrawlPlugin, CrawlQueue, CrawlSpec,
};

use crate::assets::content;
use crate::control::bindings::{ControlBindings, PlayerAction};
use crate::facts::facts_resource::Facts;
use crate::facts::keys;
use crate::facts::messages::StoryEffect;
use crate::game_state::{GameState, PauseState};

/// Every crawl the stories can ask for, by id (file stem).
#[derive(Resource, Debug, Default)]
pub struct CrawlLibrary(pub HashMap<String, CrawlSpec>);

impl CrawlLibrary {
    /// Reads every crawl file (`assets::content::crawl_files`: `assets/crawls/` and the
    /// levels' `_crawl_` files). A file that does not parse is skipped with a warning; of
    /// two files with the same id, the later one wins, also with a warning.
    pub fn load() -> Self {
        Self::load_files(content::crawl_files())
    }

    pub fn load_files(files: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut crawls = HashMap::new();
        for path in files {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| parse_crawl(stem, &text))
            {
                Ok(spec) => {
                    if crawls.insert(stem.to_string(), spec).is_some() {
                        warn!("crawl '{stem}' is defined twice; using {}", path.display());
                    }
                }
                Err(e) => warn!("crawl {} failed to load: {e}", path.display()),
            }
        }
        Self(crawls)
    }
}

/// Parses one crawl file; the id always comes from the file name.
pub fn parse_crawl(id: &str, text: &str) -> Result<CrawlSpec, String> {
    let mut spec: CrawlSpec = ron::from_str(text).map_err(|e| e.to_string())?;
    id.clone_into(&mut spec.id);
    Ok(spec)
}

pub struct CrawlsPlugin;

impl Plugin for CrawlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CrawlPlugin)
            .insert_resource(CrawlLibrary::load())
            .add_systems(OnEnter(GameState::InGame), reload_library)
            .add_systems(OnExit(GameState::InGame), clear_crawls)
            .add_systems(OnEnter(PauseState::Cutscene), freeze_time)
            .add_systems(OnExit(PauseState::Cutscene), thaw_time)
            .add_systems(
                Update,
                (
                    show_story_crawls,
                    crawl_facts,
                    advance_crawl_input,
                    sync_cutscene,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

fn reload_library(mut library: ResMut<CrawlLibrary>) {
    *library = CrawlLibrary::load();
}

fn clear_crawls(mut clear: MessageWriter<ClearCrawls>) {
    clear.write(ClearCrawls);
}

/// `crawl:<name>` story effects -> queued crawls. Pushed straight into the queue (not via
/// `ShowCrawl`) so `CrawlQueue::is_busy` sees them the same frame.
fn show_story_crawls(
    mut effects: MessageReader<StoryEffect>,
    library: Res<CrawlLibrary>,
    mut queue: ResMut<CrawlQueue>,
) {
    for StoryEffect { effect } in effects.read() {
        let Some(id) = keys::crawl_effect(effect) else {
            continue;
        };
        if let Some(spec) = library.0.get(id) {
            queue.push(spec.clone());
        } else {
            warn!("story asked for crawl '{id}', but there is no crawl file '{id}.ron'");
        }
    }
}

/// Finished crawls and the queue's state, as facts for the stories.
fn crawl_facts(
    mut finished: MessageReader<CrawlFinished>,
    queue: Res<CrawlQueue>,
    mut facts: ResMut<Facts>,
) {
    for done in finished.read() {
        facts.set_bool(&keys::crawl_finished(&done.id), true);
    }
    let showing = queue.is_busy();
    if facts.try_bool(keys::CRAWL_SHOWING) != Some(showing) {
        facts.set_bool(keys::CRAWL_SHOWING, showing);
    }
}

/// Which actions advance a crawl. A modal crawl owns the controls; a non-modal one shares
/// them with play, so it only takes the one that does not shoot or build.
pub const fn advance_actions(modal: bool) -> &'static [PlayerAction] {
    if modal {
        &[
            PlayerAction::Fire,
            PlayerAction::PlaceBuild,
            PlayerAction::Interact,
            PlayerAction::Pause,
        ]
    } else {
        &[PlayerAction::Interact]
    }
}

fn advance_crawl_input(
    queue: Res<CrawlQueue>,
    bindings: Res<ControlBindings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    mut advance: MessageWriter<AdvanceCrawl>,
) {
    let Some(spec) = queue.active() else {
        return;
    };
    let actions = advance_actions(spec.modal);
    let pressed = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter])
        || actions.iter().any(|action| {
            bindings.kbm_just_pressed(*action, &keys, &mouse)
                || gamepads
                    .iter()
                    .any(|pad| bindings.pad_just_pressed(*action, pad))
        });
    if pressed {
        advance.write(AdvanceCrawl);
    }
}

/// Holds the match in `Cutscene` while a modal crawl is up or next in line. Never takes
/// over from the pause menu: a crawl that starts while paused waits for the resume.
fn sync_cutscene(
    queue: Res<CrawlQueue>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
) {
    match (state.get(), queue.is_modal()) {
        (PauseState::Running, true) => next.set(PauseState::Cutscene),
        (PauseState::Cutscene, false) => next.set(PauseState::Running),
        _ => {}
    }
}

fn freeze_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

/// Also runs when the match ends mid-crawl: leaving `InGame` exits the sub-state.
fn thaw_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

#[cfg(test)]
mod tests {
    use super::*;
    use lava_ui_builder::crawl::CrawlStyle;

    #[test]
    fn every_crawl_in_assets_parses() {
        let mut count = 0;
        for path in content::crawl_files() {
            let stem = path.file_stem().unwrap().to_str().unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let spec = parse_crawl(stem, &text)
                .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
            assert!(!spec.lines.is_empty(), "{} has no text", path.display());
            count += 1;
        }
        assert!(count > 0, "no crawls shipped");
    }

    /// Every shipped story file parses, and every crawl a story asks for exists.
    #[test]
    fn every_story_crawl_effect_names_a_shipped_crawl() {
        let library = CrawlLibrary::load();
        for path in content::story_files() {
            let stories = crate::facts::persistence::load_stories(&path)
                .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
            for story in &stories {
                for consequence in &story.consequences {
                    if let crate::facts::consequence::Consequence::Emit { effect } = consequence
                        && let Some(id) = keys::crawl_effect(effect)
                    {
                        assert!(
                            library.0.contains_key(id),
                            "{}: story '{}' asks for missing crawl '{id}'",
                            path.display(),
                            story.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[allow(clippy::float_cmp, reason = "an untouched default compares exactly")]
    fn a_crawl_file_lists_only_what_differs_and_takes_its_id_from_the_name() {
        let spec = parse_crawl(
            "letter",
            r#"(id: "ignored", lines: ["Dear neighbour,"], style: Typewriter, modal: false)"#,
        )
        .unwrap();
        assert_eq!(spec.id, "letter");
        assert_eq!(spec.style, CrawlStyle::Typewriter);
        assert!(!spec.modal);
        assert_eq!(
            spec.speed,
            CrawlSpec::default().speed,
            "defaults fill the rest"
        );
    }

    #[test]
    fn a_non_modal_crawl_never_advances_on_fire() {
        assert!(advance_actions(true).contains(&PlayerAction::Fire));
        assert!(!advance_actions(false).contains(&PlayerAction::Fire));
    }
}
