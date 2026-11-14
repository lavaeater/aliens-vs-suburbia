//! A level's words, as the campaign editor edits them: the intro crawl
//! (`<level>_crawl_intro.ron`, a `CrawlSpec`) and the objective line on the HUD (the
//! `ObjectiveText` fact its story file sets, `("ObjectiveText", Text("..."))`).
//!
//! The crawl file is written whole. The story file is not: only the objective's string is
//! swapped, so the rest of the story keeps its layout and comments.

use std::path::{Path, PathBuf};

use lava_ui_builder::crawl::CrawlSpec;

use crate::assets::paths::disk_path;

const OBJECTIVE: &str = "(\"ObjectiveText\", Text(";

/// A level's intro crawl and objective, with what their files hold.
#[derive(Debug, Clone, PartialEq)]
pub struct IntroDoc {
    pub crawl_path: PathBuf,
    pub crawl: Option<CrawlSpec>,
    crawl_saved: Option<CrawlSpec>,
    pub story_path: PathBuf,
    pub objective: Option<String>,
    objective_saved: Option<String>,
}

/// The words, without where they come from: what undo keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct IntroWords {
    pub crawl: Option<CrawlSpec>,
    pub objective: Option<String>,
}

/// The objective in a story file's text: the first `ObjectiveText` string.
pub fn objective_in(story: &str) -> Option<String> {
    let start = story.find(OBJECTIVE)? + OBJECTIVE.len();
    let rest = story.get(start..)?;
    let end = string_end(rest)?;
    ron::from_str::<String>(rest.get(..end)?).ok()
}

/// The byte length of the RON string literal `text` starts with (quotes included).
fn string_end(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut escaped = false;
    for (i, c) in chars {
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return Some(i + 1),
            _ => escaped = false,
        }
    }
    None
}

/// `story` with its first objective string replaced by `objective`. `None` when the story
/// sets no objective.
pub fn with_objective(story: &str, objective: &str) -> Option<String> {
    let start = story.find(OBJECTIVE)? + OBJECTIVE.len();
    let end = start + string_end(story.get(start..)?)?;
    let literal = ron::to_string(&objective.to_string()).ok()?;
    Some(format!("{}{literal}{}", story.get(..start)?, story.get(end..)?))
}

/// A crawl as a file: a header, then the spec.
pub fn crawl_file(stem: &str, crawl: &CrawlSpec) -> Result<String, String> {
    let body = ron::ser::to_string_pretty(crawl, ron::ser::PrettyConfig::new())
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "// {stem}'s intro crawl, written by the campaign editor. Fields: see\n\
         // assets/campaigns/main/level_01/level_01_crawl_intro.ron. Plain ASCII only.\n{body}\n"
    ))
}

impl IntroDoc {
    /// The intro of the level whose map is `map_path` and whose story is `stories`
    /// (content paths). `None` without a story.
    pub fn load(map_path: &str, stories: &str) -> Option<Self> {
        if stories.is_empty() {
            return None;
        }
        let map = disk_path(map_path);
        let stem = map.file_stem()?.to_string_lossy().into_owned();
        let crawl_path = map.parent()?.join(format!("{stem}_crawl_intro.ron"));
        let crawl = std::fs::read_to_string(&crawl_path)
            .ok()
            .and_then(|t| ron::from_str::<CrawlSpec>(&t).ok());
        let story_path = disk_path(stories);
        let objective = std::fs::read_to_string(&story_path)
            .ok()
            .and_then(|t| objective_in(&t));
        Some(Self {
            crawl_path,
            crawl_saved: crawl.clone(),
            crawl,
            story_path,
            objective_saved: objective.clone(),
            objective,
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.crawl != self.crawl_saved || self.objective != self.objective_saved
    }

    pub fn words(&self) -> IntroWords {
        IntroWords {
            crawl: self.crawl.clone(),
            objective: self.objective.clone(),
        }
    }

    pub fn set_words(&mut self, words: IntroWords) {
        self.crawl = words.crawl;
        self.objective = words.objective;
    }

    /// Write what changed. How many files were written.
    pub fn save(&mut self) -> Result<usize, String> {
        let mut written = 0;
        if self.crawl != self.crawl_saved
            && let Some(crawl) = &self.crawl
        {
            let stem = self
                .crawl_path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            write(&self.crawl_path, &crawl_file(&stem, crawl)?)?;
            self.crawl_saved = self.crawl.clone();
            written += 1;
        }
        if self.objective != self.objective_saved
            && let Some(objective) = &self.objective
        {
            let story = std::fs::read_to_string(&self.story_path)
                .map_err(|e| format!("{}: {e}", self.story_path.display()))?;
            let text = with_objective(&story, objective)
                .ok_or_else(|| format!("{} sets no ObjectiveText", self.story_path.display()))?;
            write(&self.story_path, &text)?;
            self.objective_saved = self.objective.clone();
            written += 1;
        }
        Ok(written)
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORY: &str = r#"// a comment that has to survive
[
    (
        name: "x intro",
        repeat: false,
        init_facts: [
            ("ObjectiveText", Text("Hold the \"hill\"")),
        ],
        rules: [
            (name: "started", criteria: [BoolIs(key: "LevelStarted", expected: true)]),
        ],
        consequences: [Emit(effect: "crawl:x_crawl_intro")],
    ),
]
"#;

    #[test]
    fn the_objective_is_read_and_swapped_in_place() {
        assert_eq!(objective_in(STORY).as_deref(), Some("Hold the \"hill\""));
        let new = with_objective(STORY, "Keep them out").unwrap();
        assert_eq!(objective_in(&new).as_deref(), Some("Keep them out"));
        assert!(new.starts_with("// a comment that has to survive"));
        assert_eq!(new.lines().count(), STORY.lines().count());
        assert!(turbofacts::persistence::stories_from_ron(&new).is_ok());
        assert!(objective_in("[]").is_none());
        assert!(with_objective("[]", "x").is_none());
    }

    #[test]
    fn a_crawl_file_reads_back() {
        let crawl = CrawlSpec {
            title: "THE MALL".into(),
            lines: vec!["one".into(), String::new(), "two".into()],
            ..CrawlSpec::default()
        };
        let text = crawl_file("gis_09_crawl_intro", &crawl).unwrap();
        assert_eq!(ron::from_str::<CrawlSpec>(&text).unwrap(), crawl);
    }

    #[test]
    fn the_shipped_gis_intros_load() {
        let intro = IntroDoc::load(
            "campaigns/gis/gis_02/gis_02.map",
            "campaigns/gis/gis_02/gis_02_story.ron",
        )
        .unwrap();
        assert!(intro.crawl.as_ref().is_some_and(|c| !c.lines.is_empty()));
        assert!(intro.objective.is_some());
        assert!(!intro.is_dirty());
    }
}
