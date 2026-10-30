//! `cargo run -- --validate`: read every content file the game reads (campaign, maps,
//! defs, loot tables, stories, crawls, houses, effects) and check what each one points
//! at, without starting the game. A missing def, a typo in a story's fact or a crawl that
//! doesn't exist is a line in the report instead of something that silently does nothing
//! three levels in.
//!
//! The checks go through the game's own loaders, so a file that passes here reads the
//! same way in game. `the_shipped_assets_validate` runs the whole thing as a test.
//!
//! Content names files inside `assets/` (`"defs/Pistol.ron"`, `"packs/guns/Pistol.glb"`;
//! see `assets::paths`). The report names files from the project root, as they are on disk.

mod defs;
mod maps;
mod stories;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

pub const ASSETS: &str = "assets";
pub const DEFS: &str = "assets/defs";
pub const LOOT: &str = "assets/loot";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Broken: something the game reads is missing or won't parse.
    Error,
    /// Suspicious: legal, but probably not what was meant (an unused file, a shadowed map).
    Warning,
}

#[derive(Clone, Debug)]
pub struct Problem {
    pub severity: Severity,
    pub message: String,
}

/// Everything found, by the file it was found in.
#[derive(Default, Debug)]
pub struct Report {
    pub files: BTreeMap<String, Vec<Problem>>,
    /// Files read, problems or not.
    pub checked: usize,
}

impl Report {
    pub fn error(&mut self, file: impl Into<String>, message: impl Into<String>) {
        self.add(file, Severity::Error, message);
    }

    pub fn warn(&mut self, file: impl Into<String>, message: impl Into<String>) {
        self.add(file, Severity::Warning, message);
    }

    fn add(&mut self, file: impl Into<String>, severity: Severity, message: impl Into<String>) {
        let problems = self.files.entry(file.into()).or_default();
        let message = message.into();
        // The same def can be named from many places; say it once per file.
        if !problems.iter().any(|p| p.message == message) {
            problems.push(Problem { severity, message });
        }
    }

    pub fn count(&self, severity: Severity) -> usize {
        self.files
            .values()
            .flatten()
            .filter(|p| p.severity == severity)
            .count()
    }

    /// The report as the CLI prints it: per file, errors first.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (file, problems) in &self.files {
            let _ = writeln!(out, "{file}");
            let mut sorted: Vec<&Problem> = problems.iter().collect();
            sorted.sort_by_key(|p| p.severity);
            for problem in sorted {
                let tag = match problem.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                };
                let _ = writeln!(out, "  {tag}: {}", problem.message);
            }
        }
        let _ = write!(
            out,
            "{} error(s), {} warning(s) in {} file(s) checked",
            self.count(Severity::Error),
            self.count(Severity::Warning),
            self.checked
        );
        out
    }
}

/// Check everything. Run from the project root, like the game.
pub fn validate() -> Report {
    let mut report = Report::default();
    let defs = defs::check_all(&mut report);
    let loot = defs::check_loot(&mut report, &defs);
    maps::check_all(&mut report, &defs, &loot);
    stories::check_crawls(&mut report);
    maps::check_houses(&mut report);
    maps::check_fx(&mut report);
    maps::check_code_palettes(&mut report);
    report
}

/// The `--validate` entry point: print the report, fail if anything is broken.
pub fn run_cli() -> std::process::ExitCode {
    let report = validate();
    println!("{}", report.render());
    if report.count(Severity::Error) > 0 {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

/// The `.ron` (or other `ext`) files directly in `dir`, as `dir/name.ext` strings, sorted.
pub fn files_in(dir: &str, ext: &str) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == ext))
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// A path as the report names it: from the project root, forward slashes.
pub fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A model path as stored in content (inside `assets/`, maybe with a `#Scene0` label).
pub fn asset_exists(path: &str) -> bool {
    let file = path.split('#').next().unwrap_or(path);
    Path::new(ASSETS).join(file).is_file()
}

/// The stem of a path: `"assets/defs/Bunny.ron"` -> `"Bunny"`.
pub fn stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

/// `" (did you mean 'x'?)"` when one of `known` is a near miss for `wanted`, else empty.
///
/// Only the part after a shared prefix is compared, so `ItemFound.fule` finds
/// `ItemFound.fuel` but `CharacterArmature|Idle` doesn't "find" `CharacterArmature|No`.
pub fn did_you_mean<'a>(wanted: &str, known: impl IntoIterator<Item = &'a str>) -> String {
    let wanted_lower = wanted.to_lowercase();
    known
        .into_iter()
        .filter_map(|k| {
            let k_lower = k.to_lowercase();
            let shared = wanted_lower
                .chars()
                .zip(k_lower.chars())
                .take_while(|(a, b)| a == b)
                .count();
            let rest: String = wanted_lower.chars().skip(shared).collect();
            let other: String = k_lower.chars().skip(shared).collect();
            let limit = rest.chars().count().div_ceil(2).clamp(1, 4);
            let distance = edit_distance(&rest, &other);
            (distance <= limit && distance > 0).then_some((distance, k))
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| format!(" (did you mean '{k}'?)"))
        .unwrap_or_default()
}

/// Edit distance by characters, a swap of two neighbours counting as one edit (optimal
/// string alignment).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let width = b.len() + 1;
    let mut d = vec![0usize; (a.len() + 1) * width];
    let at =
        |d: &[usize], i: usize, j: usize| d.get(i * width + j).copied().unwrap_or(usize::MAX / 2);
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            let value = if i == 0 || j == 0 {
                i + j
            } else {
                let (ca, cb) = (a.get(i - 1), b.get(j - 1));
                let cost = usize::from(ca != cb);
                let mut best = (at(&d, i - 1, j) + 1)
                    .min(at(&d, i, j - 1) + 1)
                    .min(at(&d, i - 1, j - 1) + cost);
                if i > 1 && j > 1 && ca == b.get(j - 2) && a.get(i - 2) == cb {
                    best = best.min(at(&d, i - 2, j - 2) + 1);
                }
                best
            };
            if let Some(cell) = d.get_mut(i * width + j) {
                *cell = value;
            }
        }
    }
    at(&d, a.len(), b.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_misses_are_suggested() {
        assert_eq!(
            did_you_mean("ItemFound.fule", ["ItemFound.fuel", "Coins"]),
            " (did you mean 'ItemFound.fuel'?)"
        );
        assert_eq!(did_you_mean("Bunny", ["Stone Golem"]), "");
        assert_eq!(
            did_you_mean("Armature|Idle", ["Armature|No", "Armature|Death"]),
            ""
        );
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("fule", "fuel"), 1, "a swap is one edit");
    }

    #[test]
    fn the_report_lists_errors_before_warnings() {
        let mut report = Report::default();
        report.warn("a.ron", "odd");
        report.error("a.ron", "broken");
        report.error("a.ron", "broken");
        let text = report.render();
        assert!(text.find("error: broken") < text.find("warning: odd"));
        assert_eq!(report.count(Severity::Error), 1, "said once");
    }

    /// The content in `assets/` has nothing broken in it.
    #[test]
    fn the_shipped_assets_validate() {
        let report = validate();
        assert_eq!(report.count(Severity::Error), 0, "\n{}", report.render());
    }
}
