//! Model defs (`assets/defs`) and loot tables (`assets/loot`).

use crate::assets::paths::{content_ref, disk_path};
use std::collections::{BTreeMap, HashMap};

use super::{DEFS, LOOT, Report, asset_exists, did_you_mean, files_in, stem};
use crate::assets::asset_definition::{AssetDefinition, ItemKind, ModelType};
use crate::assets::gltf_info;
use crate::loot::{LootEntry, LootTable};

/// Every def that parsed, by its path as content names it (`"defs/Bunny.ron"`).
pub type Defs = BTreeMap<String, AssetDefinition>;
/// Every loot table that parsed, by file stem.
pub type Tables = BTreeMap<String, LootTable>;

/// What a reference to a def expects to find there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Any,
    Enemy,
    Weapon,
    Item,
}

impl Kind {
    const fn name(self) -> &'static str {
        match self {
            Self::Any => "def",
            Self::Enemy => "enemy",
            Self::Weapon => "weapon",
            Self::Item => "item",
        }
    }

    const fn accepts(self, model_type: &ModelType) -> bool {
        matches!(
            (self, model_type),
            (Self::Any, _)
                | (Self::Enemy, ModelType::Enemy(_))
                | (Self::Weapon, ModelType::Weapon(_))
                | (Self::Item, ModelType::Item(_))
        )
    }
}

const fn type_name(model_type: &ModelType) -> &'static str {
    match model_type {
        ModelType::Player(_) => "Player",
        ModelType::Tower(_) => "Tower",
        ModelType::Terrain(_) => "Terrain",
        ModelType::Item(_) => "Item",
        ModelType::Enemy(_) => "Enemy",
        ModelType::Weapon(_) => "Weapon",
    }
}

/// Check that `path` names a def of the right kind; `what` says where it was named
/// ("wave 2's enemy").
pub fn check_ref(report: &mut Report, file: &str, what: &str, path: &str, kind: Kind, defs: &Defs) {
    if let Some(def) = defs.get(&content_ref(path)) {
        if !kind.accepts(&def.model_type) {
            report.error(
                file,
                format!(
                    "{what}: '{path}' is a {} def, not a {} def",
                    type_name(&def.model_type),
                    kind.name()
                ),
            );
        }
    } else if disk_path(path).is_file() {
        // There, but it didn't parse: reported against the def itself.
        report.error(
            file,
            format!("{what}: '{path}' does not parse (see that file)"),
        );
    } else {
        let hint = did_you_mean(path, defs.keys().map(String::as_str));
        report.error(
            file,
            format!("{what}: no {} def at '{path}'{hint}", kind.name()),
        );
    }
}

/// Check a loot table name (a file stem under `assets/loot`).
pub fn check_table(report: &mut Report, file: &str, what: &str, table: &str, tables: &Tables) {
    if !tables.contains_key(table) {
        let hint = did_you_mean(table, tables.keys().map(String::as_str));
        report.error(
            file,
            format!("{what}: no loot table '{table}' in {LOOT}{hint}"),
        );
    }
}

/// Parse every def, then check what each points at. Returns the ones that parsed.
pub fn check_all(report: &mut Report) -> Defs {
    let mut defs = Defs::new();
    let mut paths = files_in(DEFS, "ron");
    paths.extend(files_in(crate::map::neighbourhood::PEOPLE_DIR, "ron"));
    for path in paths {
        report.checked += 1;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                report.error(&path, format!("cannot read: {e}"));
                continue;
            }
        };
        match ron::from_str::<AssetDefinition>(&text) {
            Ok(def) => {
                defs.insert(content_ref(&path), def);
            }
            Err(e) => report.error(&path, format!("does not parse: {e}")),
        }
    }
    for (path, def) in &defs {
        check_def(report, &disk_path(path).to_string_lossy(), def, &defs);
    }
    defs
}

fn check_def(report: &mut Report, file: &str, def: &AssetDefinition, defs: &Defs) {
    if !asset_exists(&def.model_path) {
        let hint = if def.model_path.starts_with("assets/") {
            " (model paths are inside assets/: drop the 'assets/' prefix)"
        } else {
            ""
        };
        report.error(
            file,
            format!("model_path: no file 'assets/{}'{hint}", def.model_path),
        );
    }
    for source in &def.animation_sources {
        if !asset_exists(source) {
            report.error(
                file,
                format!("animation_sources: no file 'assets/{source}'"),
            );
        }
    }
    for attachment in &def.attachments {
        if !asset_exists(&attachment.model_path) {
            report.error(
                file,
                format!(
                    "attachment on '{}': no file 'assets/{}'",
                    attachment.bone, attachment.model_path
                ),
            );
        }
    }
    check_clips(report, file, def);

    match &def.model_type {
        ModelType::Player(props) => {
            for weapon in props.weapon.iter().chain(&props.extra_weapons) {
                check_ref(report, file, "weapon", weapon, Kind::Weapon, defs);
            }
        }
        ModelType::Item(props) => check_item(report, file, "item kind", &props.kind, defs, None),
        ModelType::Enemy(_)
        | ModelType::Terrain(_)
        | ModelType::Tower(_)
        | ModelType::Weapon(_) => {}
    }
}

/// The clips a def maps keys to exist in its model or the named source; hidden nodes exist.
fn check_clips(report: &mut Report, file: &str, def: &AssetDefinition) {
    let Some(model) = gltf_info::inspect(&def.model_path) else {
        return;
    };
    let sources: HashMap<&str, Vec<String>> = def
        .animation_sources
        .iter()
        .filter_map(|s| Some((stem(s), gltf_info::inspect(s)?.animations)))
        .collect();
    for (key, clip) in &def.animations {
        // "Source|Clip" names a clip in an animation source; anything else, including a
        // model's own clip with a '|' in it (Quaternius: "CharacterArmature|Walk"), is
        // looked up in the model.
        let (clips, name, from) = match clip
            .split_once('|')
            .and_then(|(source, name)| Some((sources.get(source)?, name, source)))
        {
            Some((clips, name, source)) => (clips, name, format!("source '{source}'")),
            None => (
                &model.animations,
                clip.as_str(),
                format!("'assets/{}'", def.model_path),
            ),
        };
        if !clips.iter().any(|c| c == name) {
            report.error(
                file,
                format!(
                    "animations: {key:?} -> '{clip}': no clip '{name}' in {from}{}",
                    did_you_mean(name, clips.iter().map(String::as_str))
                ),
            );
        }
    }
    for node in &def.hidden_nodes {
        if !model.mesh_nodes.contains(node) {
            report.warn(
                file,
                format!(
                    "hidden_nodes: no mesh node '{node}' in the model{}",
                    did_you_mean(node, model.mesh_nodes.iter().map(String::as_str))
                ),
            );
        }
    }
}

/// An item kind's references: a weapon pickup's def, a loot bag's table (once tables
/// are known; defs are checked before them).
pub fn check_item(
    report: &mut Report,
    file: &str,
    what: &str,
    kind: &ItemKind,
    defs: &Defs,
    tables: Option<&Tables>,
) {
    match kind {
        ItemKind::WeaponPickup { def } => check_ref(report, file, what, def, Kind::Weapon, defs),
        ItemKind::Loot { table } => {
            if let Some(tables) = tables {
                check_table(report, file, what, table, tables);
            }
        }
        _ => {}
    }
}

/// Parse every loot table and check its entries; also the tables defs name.
pub fn check_loot(report: &mut Report, defs: &Defs) -> Tables {
    let mut tables = Tables::new();
    for path in files_in(LOOT, "ron") {
        report.checked += 1;
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| ron::from_str::<LootTable>(&t).map_err(|e| e.to_string()))
        {
            Ok(table) => {
                tables.insert(stem(&path).to_string(), table);
            }
            Err(e) => report.error(&path, format!("does not parse: {e}")),
        }
    }
    for (name, table) in &tables {
        let file = format!("{LOOT}/{name}.ron");
        for entry in table.always.iter().chain(&table.entries) {
            match entry {
                LootEntry::Table { table, .. } => {
                    check_table(report, &file, "nested table", table, &tables);
                }
                LootEntry::Item { kind, .. } => {
                    check_item(report, &file, "drop", kind, defs, Some(&tables));
                }
                LootEntry::Nothing { .. } => {}
            }
        }
    }

    for (path, def) in defs {
        let path = &disk_path(path).to_string_lossy().into_owned();
        let table = match &def.model_type {
            ModelType::Enemy(props) => props.loot_table.as_ref(),
            ModelType::Terrain(props) => props.loot_table.as_ref(),
            ModelType::Item(props) => {
                check_item(report, path, "item kind", &props.kind, defs, Some(&tables));
                None
            }
            _ => None,
        };
        if let Some(table) = table {
            check_table(report, path, "loot_table", table, &tables);
        }
    }
    tables
}

/// Every story-item id (`ItemKind::Key`) a loot table can drop.
pub fn keys_in_loot(tables: &Tables) -> Vec<String> {
    tables
        .values()
        .flat_map(|t| t.always.iter().chain(&t.entries))
        .filter_map(|entry| match entry {
            LootEntry::Item {
                kind: ItemKind::Key { id },
                ..
            } => Some(id.clone()),
            _ => None,
        })
        .collect()
}
