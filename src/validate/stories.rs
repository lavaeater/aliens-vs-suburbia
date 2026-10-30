//! Stories and crawls: the levels' `_story.ron` / `_crawl_` files and `assets/crawls`
//! (`assets::content`).
//!
//! A story waits on facts; the check is that something can ever set each one: another
//! story (its `init_facts` or a consequence), the game itself (`turbofacts::keys`), a
//! story item placed on the map (`ItemFound.<id>`) or a crawl the stories show
//! (`CrawlFinished.<id>`). A fact nothing sets is almost always a typo, and the story
//! simply never fires.

use std::collections::{BTreeMap, BTreeSet};

use super::{Report, did_you_mean, display};
use crate::assets::content;
use crate::assets::paths::{content_ref, disk_path};
use crate::crawl::parse_crawl;
use crate::facts::keys;
use crate::facts::persistence::load_stories;
use crate::facts::stories::base_stories;
use crate::facts::{Consequence, Criterion, Story};

/// Effects the game acts on, besides `crawl:<id>` (`score_keeper::level_state_system`).
const GAME_EFFECTS: &[&str] = &["level_starting", "level_complete", "level_failed"];

/// Facts the game writes itself, every frame or on an event.
const GAME_FACTS: &[&str] = &[
    keys::LEVEL_COMPLETE,
    keys::LIVING_PLAYER_COUNT,
    keys::ALL_PLAYERS_DEAD,
    keys::ENEMY_COUNT,
    keys::ENEMY_KILL_COUNT,
    keys::ALIENS_TO_SPAWN,
    keys::ALL_ALIENS_DEAD,
    keys::ALIENS_ESCAPED,
    keys::ALIENS_ESCAPED_CUTOFF,
    keys::TOO_MANY_ALIENS_ESCAPED,
    keys::SHOTS_FIRED,
    keys::SHOTS_HIT,
    keys::COINS,
    keys::OBJECTIVES_DONE,
    keys::CURRENT_WAVE,
    keys::WAVE_COUNT,
    keys::ALL_WAVES_DONE,
    keys::CRAWL_SHOWING,
];

/// The crawl ids (file stems).
fn crawl_ids() -> Vec<String> {
    content::crawl_files()
        .iter()
        .map(|p| super::stem(&display(p)).to_string())
        .collect()
}

pub fn check_crawls(report: &mut Report) {
    let mut shown = BTreeSet::new();
    for path in content::story_files() {
        if let Ok(stories) = load_stories(&path) {
            shown.extend(stories.iter().flat_map(crawls_shown));
        }
    }
    let mut first_of: BTreeMap<String, String> = BTreeMap::new();
    for path in content::crawl_files() {
        report.checked += 1;
        let path = display(&path);
        let id = super::stem(&path);
        if let Some(first) = first_of.get(id) {
            report.error(
                &path,
                format!("crawl id '{id}' is taken by {first} too; one of them never shows"),
            );
        } else {
            first_of.insert(id.to_string(), path.clone());
        }
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse_crawl(id, &text))
        {
            Ok(spec) if spec.lines.is_empty() => report.error(&path, "has no text"),
            Ok(_) => {}
            Err(e) => report.error(&path, format!("does not parse: {e}")),
        }
        if !shown.contains(id) {
            report.warn(
                &path,
                format!("no story shows it (Emit(effect: \"crawl:{id}\"))"),
            );
        }
    }
}

fn crawls_shown(story: &Story) -> Vec<String> {
    story
        .consequences
        .iter()
        .filter_map(|c| match c {
            Consequence::Emit { effect } => keys::crawl_effect(effect).map(str::to_string),
            _ => None,
        })
        .collect()
}

/// A level's story file, with the base stories it runs beside and the story items
/// (`keys_placed`) the level can hand out.
pub fn check_level(report: &mut Report, map_file: &str, path: &str, keys_placed: &[String]) {
    let stories = match load_stories(disk_path(path)) {
        Ok(stories) if stories.is_empty() => {
            report.error(map_file, format!("stories: '{path}' is missing or empty"));
            return;
        }
        Ok(stories) => stories,
        Err(e) => {
            let known: Vec<String> = content::story_files()
                .iter()
                .map(|p| content_ref(&display(p)))
                .collect();
            report.error(
                map_file,
                format!(
                    "stories: '{path}': {e}{}",
                    did_you_mean(path, known.iter().map(String::as_str))
                ),
            );
            return;
        }
    };
    let file = disk_path(path).to_string_lossy().into_owned();
    check_stories(report, &file, &stories, keys_placed, Some(map_file));
}

/// A story file no map uses: still check it parses and holds together on its own.
pub fn check_unused_file(report: &mut Report, path: &str) {
    match load_stories(path) {
        Ok(stories) => check_stories(report, path, &stories, &[], None),
        Err(e) => report.error(path, format!("does not parse: {e}")),
    }
}

fn check_stories(
    report: &mut Report,
    path: &str,
    stories: &[Story],
    keys_placed: &[String],
    map_file: Option<&str>,
) {
    let crawls = crawl_ids();
    let base = base_stories();
    let everything: Vec<&Story> = base.iter().chain(stories).collect();

    let mut set: BTreeSet<String> = GAME_FACTS.iter().map(|k| (*k).to_string()).collect();
    for story in &everything {
        set.extend(story.init_facts.iter().map(|(k, _)| k.clone()));
        for consequence in &story.consequences {
            match consequence {
                Consequence::SetFact { key, .. } | Consequence::AddInt { key, .. } => {
                    set.insert(key.clone());
                }
                Consequence::Emit { effect } => {
                    if let Some(id) = keys::crawl_effect(effect) {
                        set.insert(keys::crawl_finished(id));
                    }
                }
            }
        }
    }
    set.extend(keys_placed.iter().map(|id| keys::item_found(id)));

    for story in stories {
        for consequence in &story.consequences {
            let Consequence::Emit { effect } = consequence else {
                continue;
            };
            if let Some(id) = keys::crawl_effect(effect) {
                if !crawls.iter().any(|c| c == id) {
                    report.error(
                        path,
                        format!(
                            "story '{}': shows crawl '{id}', but there is no crawl file '{id}.ron'{}",
                            story.name,
                            did_you_mean(id, crawls.iter().map(String::as_str))
                        ),
                    );
                }
            } else if !GAME_EFFECTS.contains(&effect.as_str()) {
                report.warn(
                    path,
                    format!(
                        "story '{}': effect '{effect}' is not one the game acts on{}",
                        story.name,
                        did_you_mean(effect, GAME_EFFECTS.iter().copied())
                    ),
                );
            }
        }

        for rule in &story.rules {
            for key in rule.criteria.iter().flat_map(read_keys) {
                if set.contains(key) {
                    continue;
                }
                let why = if let Some(id) = key.strip_prefix("ItemFound.") {
                    match map_file {
                        Some(map) => format!(
                            "no story item (ItemKind::Key) with id '{id}' is placed on {map} or dropped by a loot table"
                        ),
                        None => format!("no story item with id '{id}' can be found"),
                    }
                } else if let Some(id) = key.strip_prefix("CrawlFinished.") {
                    format!("no story shows crawl '{id}'")
                } else {
                    "no story sets it and the game doesn't".to_string()
                };
                report.error(
                    path,
                    format!(
                        "story '{}', rule '{}': waits on fact '{key}', which is never set ({why}){}",
                        story.name,
                        rule.name,
                        did_you_mean(key, set.iter().map(String::as_str))
                    ),
                );
            }
        }
    }
}

/// The single fact keys a criterion reads (patterns are left alone).
fn read_keys(criterion: &Criterion) -> Vec<&str> {
    match criterion {
        Criterion::BoolIs { key, .. }
        | Criterion::IntCmp { key, .. }
        | Criterion::FloatCmp { key, .. }
        | Criterion::TextEq { key, .. }
        | Criterion::TextContains { key, .. }
        | Criterion::ListContains { key, .. }
        | Criterion::ListSize { key, .. }
        | Criterion::SetContains { key, .. }
        | Criterion::SetSize { key, .. } => vec![key.as_str()],
        Criterion::IntVsInt { lhs, rhs, .. } => vec![lhs.as_str(), rhs.as_str()],
        Criterion::AnyBool { .. }
        | Criterion::AllBool { .. }
        | Criterion::AnyIntCmp { .. }
        | Criterion::AllIntCmp { .. }
        | Criterion::AnyTextEq { .. }
        | Criterion::AllTextEq { .. } => Vec::new(),
    }
}
