//! RON (de)serialization for facts and stories.
//!
//! - Facts: only scalar facts are saved (bool/int/float/text); list/set facts are
//!   runtime-only, matching the Kotlin `FactPersistence` behavior.
//! - Stories: since [`Story`](super::Story) and its parts all derive serde, story files are
//!   plain RON under `assets/stories/*.ron` — no bespoke loader needed. This matches the
//!   project's RON-everywhere convention (`assets/defs`, `assets/maps`).

use std::collections::BTreeMap;
use std::path::Path;

use super::fact_value::FactValue;
use super::facts_resource::Facts;
use super::story::Story;

/// Serializes the persistable (scalar) facts to a RON string. Keys are sorted for stable,
/// diff-friendly output.
pub fn facts_to_ron(facts: &Facts) -> Result<String, ron::Error> {
    let snapshot: BTreeMap<String, FactValue> = facts
        .iter()
        .filter(|(_, v)| v.is_persistable())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    ron::ser::to_string_pretty(&snapshot, ron::ser::PrettyConfig::default())
        .map_err(ron::Error::from)
}

/// Loads facts from a RON string, applying them silently (no change signalling).
pub fn facts_from_ron(facts: &mut Facts, ron_str: &str) -> Result<(), ron::error::SpannedError> {
    let snapshot: BTreeMap<String, FactValue> = ron::from_str(ron_str)?;
    facts.silent(|f| {
        for (key, value) in snapshot {
            f.apply_value(&key, value);
        }
    });
    Ok(())
}

/// Writes the persistable facts to `path` as RON.
pub fn save_facts(facts: &Facts, path: impl AsRef<Path>) -> std::io::Result<()> {
    let ron_str = facts_to_ron(facts)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(path, ron_str)
}

/// Loads facts from a RON file if it exists. Missing file is a no-op (returns `Ok`).
pub fn load_facts(facts: &mut Facts, path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(());
    }
    let ron_str = std::fs::read_to_string(path)?;
    facts_from_ron(facts, &ron_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

/// Parses a list of stories from a RON string.
pub fn stories_from_ron(ron_str: &str) -> Result<Vec<Story>, ron::error::SpannedError> {
    ron::from_str(ron_str)
}

/// Loads stories from a RON file. Missing file yields an empty list.
pub fn load_stories(path: impl AsRef<Path>) -> std::io::Result<Vec<Story>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let ron_str = std::fs::read_to_string(path)?;
    stories_from_ron(&ron_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}
