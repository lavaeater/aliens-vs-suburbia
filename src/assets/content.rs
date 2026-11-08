//! Where the level content lives on disk.
//!
//! A campaign is a folder under `assets/campaigns/` with its `campaign.ron`, and one folder
//! per level holding everything that level is made of, named after it:
//!
//! ```text
//! assets/campaigns/main/
//!   campaign.ron
//!   level_01/
//!     level_01.map                  the map (`.map` text, or a `.ron` MapFile)
//!     level_01_story.ron            its stories (`MapFile::stories` names it)
//!     level_01_crawl_intro.ron      a crawl; its id is the file stem
//! ```
//!
//! The kind of a file is in its name: `_story.ron`, `_crawl_` in a `.ron`, `campaign.ron`;
//! any other `.ron` (or `.map`) in a campaign is a map. Crawls not tied to a level go in
//! `assets/crawls/`. Content still names files from `assets/` (`assets::paths`), so a
//! map's `stories:` reads `campaigns/main/level_01/level_01_story.ron`.

use std::path::{Path, PathBuf};

/// One folder per campaign.
pub const CAMPAIGNS: &str = "assets/campaigns";
/// Crawls shared between levels (every `.ron` in it is one).
pub const CRAWL_DIR: &str = "assets/crawls";
/// Maps that are not campaign levels: the editor arena, generated and test maps.
pub const MAP_DIR: &str = "assets/maps";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    Campaign,
    Map,
    Story,
    Crawl,
    /// Anything else in a campaign folder.
    Other,
}

/// What a file in a campaign folder is, by its name.
pub fn kind_of(path: &Path) -> ContentKind {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    match path.extension().and_then(|e| e.to_str()) {
        Some("ron") if stem == "campaign" => ContentKind::Campaign,
        Some("ron") if stem.ends_with("_story") => ContentKind::Story,
        Some("ron") if stem.contains("_crawl_") => ContentKind::Crawl,
        Some("map" | "ron") => ContentKind::Map,
        _ => ContentKind::Other,
    }
}

/// Every file under `dir`, at any depth, sorted. Nothing if `dir` is missing.
pub fn files_under(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                walk(&path, out);
            } else if path.is_file() {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir.as_ref(), &mut out);
    out.sort();
    out
}

/// The campaigns' files of one kind.
pub fn campaign_files(kind: ContentKind) -> Vec<PathBuf> {
    files_under(CAMPAIGNS)
        .into_iter()
        .filter(|p| kind_of(p) == kind)
        .collect()
}

/// Every crawl file: the shared ones in [`CRAWL_DIR`], then the levels' own.
pub fn crawl_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = files_under(CRAWL_DIR)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.extend(campaign_files(ContentKind::Crawl));
    files
}

/// Every story file (the levels' `_story.ron`).
pub fn story_files() -> Vec<PathBuf> {
    campaign_files(ContentKind::Story)
}

/// Every map: the campaigns' levels, then the loose ones in [`MAP_DIR`] (not its `chunks/`).
pub fn map_files() -> Vec<PathBuf> {
    let mut files = campaign_files(ContentKind::Map);
    let loose = std::fs::read_dir(MAP_DIR).into_iter().flatten().flatten();
    let mut loose: Vec<PathBuf> = loose
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "map" || e == "ron"))
        .collect();
    loose.sort();
    files.extend(loose);
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_says_what_it_is_by_its_name() {
        let kind = |p: &str| kind_of(Path::new(p));
        assert_eq!(kind("main/campaign.ron"), ContentKind::Campaign);
        assert_eq!(kind("main/level_01/level_01.map"), ContentKind::Map);
        assert_eq!(kind("main/level_01/level_01.ron"), ContentKind::Map);
        assert_eq!(kind("main/level_01/level_01_story.ron"), ContentKind::Story);
        assert_eq!(
            kind("main/level_01/level_01_crawl_intro.ron"),
            ContentKind::Crawl
        );
        assert_eq!(kind("main/level_01/notes.txt"), ContentKind::Other);
    }

    #[test]
    fn the_shipped_campaign_has_its_levels_stories_and_crawls() {
        assert!(!story_files().is_empty());
        assert!(!crawl_files().is_empty());
        assert!(
            map_files()
                .iter()
                .any(|p| p.starts_with(CAMPAIGNS) && kind_of(p) == ContentKind::Map)
        );
    }
}
