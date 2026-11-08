//! What the campaign editor can put in a level: the defs (by kind) and the loot tables,
//! read once when the editor opens.

use bevy::prelude::Resource;

use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::assets::paths::content_ref;
use crate::general::components::map_components::TilePlacement;

/// What a def is, as far as placing it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefKind {
    /// Pickups and set dressing (`Item`).
    Item,
    /// Barrels, crates, things that block and break (`Terrain`).
    Terrain,
    Tower,
    /// Someone living there (a character def in `assets/defs/people`): a neighbour.
    Person,
    Enemy,
    Weapon,
}

impl DefKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Terrain => "terrain",
            Self::Tower => "tower",
            Self::Person => "person",
            Self::Enemy => "enemy",
            Self::Weapon => "weapon",
        }
    }

}

/// A def the editor offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDef {
    /// Content path (`defs/Barrel.ron`).
    pub path: String,
    /// The file stem, with what a pickup does (`Medkit [+25 HP]`).
    pub name: String,
    pub kind: DefKind,
}

/// Everything the editor can place or pick.
#[derive(Resource, Debug, Clone, Default)]
pub struct Catalog {
    pub defs: Vec<CatalogDef>,
    /// Loot table names (`assets/loot/<name>.ron`).
    pub loot_tables: Vec<String>,
}

/// What `def` is, for the editor; `people` = it is in the people folder.
pub fn kind_of(def: &AssetDefinition, people: bool) -> Option<DefKind> {
    Some(match &def.model_type {
        ModelType::Item(_) => DefKind::Item,
        ModelType::Terrain(_) => DefKind::Terrain,
        ModelType::Tower(_) => DefKind::Tower,
        ModelType::Enemy(_) => DefKind::Enemy,
        ModelType::Weapon(_) => DefKind::Weapon,
        ModelType::Player(_) if people => DefKind::Person,
        // A playable character is not placed: players choose it.
        ModelType::Player(_) => return None,
    })
}

fn scan(dir: &str, people: bool, out: &mut Vec<CatalogDef>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let Some(def) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| ron::from_str::<AssetDefinition>(&t).ok())
        else {
            continue;
        };
        let Some(kind) = kind_of(&def, people) else {
            continue;
        };
        let stem = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let name = match &def.model_type {
            ModelType::Item(props) if props.kind.is_pickup() => {
                format!("{stem} [{}]", props.kind.label())
            }
            _ => stem,
        };
        out.push(CatalogDef {
            path: content_ref(&path.to_string_lossy()),
            name,
            kind,
        });
    }
}

impl Catalog {
    /// Read `assets/defs`, `assets/defs/people` and `assets/loot`.
    pub fn load() -> Self {
        let mut defs = Vec::new();
        scan("assets/defs", false, &mut defs);
        scan(crate::map::neighbourhood::PEOPLE_DIR, true, &mut defs);
        defs.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
        let mut loot_tables: Vec<String> = std::fs::read_dir("assets/loot")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                (path.extension()? == "ron")
                    .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .flatten()
            })
            .collect();
        loot_tables.sort();
        Self { defs, loot_tables }
    }

    pub fn of_kind(&self, kind: DefKind) -> impl Iterator<Item = (usize, &CatalogDef)> {
        self.defs.iter().enumerate().filter(move |(_, d)| d.kind == kind)
    }

    /// The def a placement names, if the catalog has it.
    pub fn of_placement(&self, placement: &TilePlacement) -> Option<&CatalogDef> {
        self.defs
            .iter()
            .find(|d| crate::assets::paths::same(&d.path, &placement.def_path))
    }
}

/// The next of `options` after `current` (wrapping; `None` = the first). For cycling a
/// wave's enemy or the loot table with one button.
pub fn next_of<'a>(options: &'a [String], current: &str) -> Option<&'a String> {
    let at = options.iter().position(|o| o == current);
    match at {
        Some(i) => options.get((i + 1) % options.len().max(1)),
        None => options.first(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_catalog_has_something_of_each() {
        let catalog = Catalog::load();
        for kind in [DefKind::Item, DefKind::Terrain, DefKind::Person, DefKind::Enemy, DefKind::Weapon] {
            assert!(catalog.of_kind(kind).next().is_some(), "no {}", kind.label());
        }
        assert!(catalog.loot_tables.contains(&"trash_bag".to_string()));
        assert!(catalog.defs.iter().all(|d| !d.path.starts_with("assets/")));
    }

    #[test]
    fn cycling_wraps_round() {
        let options = vec!["a".to_string(), "b".to_string()];
        assert_eq!(next_of(&options, "a").map(String::as_str), Some("b"));
        assert_eq!(next_of(&options, "b").map(String::as_str), Some("a"));
        assert_eq!(next_of(&options, "zzz").map(String::as_str), Some("a"));
        assert_eq!(next_of(&[], "a"), None);
    }
}
