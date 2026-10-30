//! The campaign, maps (`.ron` and `.map`), houses, particle effects and the model
//! palettes written into the code. Maps and stories are found by `assets::content`.

use std::collections::BTreeSet;
use std::path::Path;

use super::defs::{self, Defs, Kind, Tables};
use super::{Report, asset_exists, did_you_mean, display, files_in, stories};
use crate::assets::asset_definition::{ItemKind, ModelType};
use crate::assets::content::{self, CAMPAIGNS, ContentKind};
use crate::assets::paths::{content_ref, disk_path};
use crate::campaign::{CAMPAIGN_FILE, CampaignFile};
use crate::general::components::map_components::MapFile;
use crate::map::house::{self, HOUSE_DIR, HouseDef};
use crate::map::text_format;

/// The campaign, then every map (the levels' and `assets/maps`), then the story files no
/// map uses and anything in a campaign folder that is none of these.
pub fn check_all(report: &mut Report, defs: &Defs, tables: &Tables) {
    check_campaign(report);

    let maps: Vec<String> = content::map_files().iter().map(|p| display(p)).collect();
    let mut stories_used = BTreeSet::new();
    for path in &maps {
        report.checked += 1;
        let path_buf = Path::new(path);
        if path_buf.extension().is_some_and(|e| e == "ron")
            && path_buf.with_extension("map").is_file()
        {
            report.warn(
                path,
                format!(
                    "shadowed: the game reads '{}' instead; this file is never played (or checked)",
                    Path::new(path).with_extension("map").display()
                ),
            );
            continue;
        }
        match text_format::read_map_file(path) {
            Ok(map) => {
                if !map.stories.is_empty() {
                    stories_used.insert(content_ref(&map.stories));
                }
                check_map(report, path, &map, defs, tables);
            }
            Err(e) => report.error(path, format!("does not parse: {e}")),
        }
    }

    for path in content::story_files().iter().map(|p| display(p)) {
        if !stories_used.contains(&content_ref(&path)) {
            report.checked += 1;
            report.warn(
                &path,
                "no map names this story file (`stories:`), so it never runs",
            );
            stories::check_unused_file(report, &path);
        }
    }

    for path in content::campaign_files(ContentKind::Other) {
        report.warn(
            display(&path),
            format!(
                "not a map, story or crawl; in {CAMPAIGNS} a file says what it is by its name \
                 (<level>.map, <level>_story.ron, <level>_crawl_<name>.ron, campaign.ron)"
            ),
        );
    }
}

fn check_campaign(report: &mut Report) {
    report.checked += 1;
    let file = match std::fs::read_to_string(CAMPAIGN_FILE)
        .map_err(|e| e.to_string())
        .and_then(|t| ron::from_str::<CampaignFile>(&t).map_err(|e| e.to_string()))
    {
        Ok(file) => file,
        Err(e) => {
            report.error(CAMPAIGN_FILE, format!("does not parse: {e}"));
            return;
        }
    };
    if file.levels.is_empty() {
        report.error(CAMPAIGN_FILE, "lists no levels");
    }
    let maps: Vec<String> = content::map_files().iter().map(|p| display(p)).collect();
    for (i, level) in file.levels.iter().enumerate() {
        let what = format!("level {} ('{}')", i + 1, level.title);
        let path = disk_path(&level.map);
        let path = path.as_path();
        if !path.is_file() && !path.with_extension("map").is_file() {
            let known: Vec<String> = maps.iter().map(|m| content_ref(m)).collect();
            let hint = did_you_mean(&level.map, known.iter().map(String::as_str));
            report.error(
                CAMPAIGN_FILE,
                format!("{what}: no map '{}'{hint}", level.map),
            );
        } else if path.extension().is_some_and(|e| e == "ron")
            && path.with_extension("map").is_file()
        {
            report.warn(
                CAMPAIGN_FILE,
                format!(
                    "{what}: names '{}', but the game plays '{}' next to it",
                    level.map,
                    path.with_extension("map").display()
                ),
            );
        }
        if level.title.trim().is_empty() {
            report.warn(CAMPAIGN_FILE, format!("level {} has no title", i + 1));
        }
        if !level.title.is_ascii() || !level.tagline.is_ascii() {
            report.error(
                CAMPAIGN_FILE,
                format!(
                    "{what}: title/tagline must be plain ASCII (the game font has nothing else)"
                ),
            );
        }
    }
}

fn check_map(report: &mut Report, file: &str, map: &MapFile, defs: &Defs, tables: &Tables) {
    let mut keys_placed = Vec::new();
    for placement in &map.placements {
        let what = format!("placement at ({}, {})", placement.x, placement.y);
        defs::check_ref(report, file, &what, &placement.def_path, Kind::Any, defs);
        if let Some(def) = defs.get(&content_ref(&placement.def_path))
            && let ModelType::Item(props) = &def.model_type
            && let ItemKind::Key { id } = &props.kind
        {
            keys_placed.push(id.clone());
        }
    }
    for (i, wave) in map.waves.iter().enumerate() {
        if !wave.enemy_def.is_empty() {
            let what = format!("wave {}'s enemy", i + 1);
            defs::check_ref(report, file, &what, &wave.enemy_def, Kind::Enemy, defs);
        }
        if wave.count == 0 {
            report.warn(file, format!("wave {} spawns nobody (count 0)", i + 1));
        }
    }
    for weapon in &map.weapons {
        defs::check_ref(report, file, "level weapon", weapon, Kind::Weapon, defs);
    }
    if let Some(loot) = &map.loot {
        if !loot.table.is_empty() {
            defs::check_table(report, file, "loot", &loot.table, tables);
        }
        for bag in &loot.bags {
            defs::check_ref(report, file, "loot bag", bag, Kind::Item, defs);
        }
    }
    let houses = house::house_files(HOUSE_DIR);
    for placed in &map.houses {
        let path = house::house_path(&placed.house);
        if !Path::new(&path).is_file() {
            let names: Vec<&str> = houses.iter().map(|h| super::stem(h)).collect();
            report.error(
                file,
                format!(
                    "house at ({}, {}): no house '{}' in {HOUSE_DIR}{}",
                    placed.x,
                    placed.y,
                    placed.house,
                    did_you_mean(&placed.house, names)
                ),
            );
        }
    }
    for decoration in &map.decorations {
        if !asset_exists(&decoration.model) {
            report.error(
                file,
                format!(
                    "decoration at ({}, {}): no file 'assets/{}'",
                    decoration.x, decoration.y, decoration.model
                ),
            );
        }
    }
    if !map.stories.is_empty() {
        keys_placed.extend(defs::keys_in_loot(tables));
        stories::check_level(report, file, &map.stories, &keys_placed);
    }
}

pub fn check_houses(report: &mut Report) {
    for path in house::house_files(HOUSE_DIR) {
        report.checked += 1;
        match HouseDef::load(&path) {
            Ok(house) if house.outline.len() < 3 => {
                report.error(&path, "the outline needs at least 3 corners");
            }
            Ok(_) => {}
            Err(e) => report.error(&path, e),
        }
    }
}

pub fn check_fx(report: &mut Report) {
    for path in files_in("assets/fx", "ron") {
        report.checked += 1;
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| {
                ron::from_str::<bevy_sprinkles::prelude::ParticlesAsset>(&t)
                    .map_err(|e| e.to_string())
            });
        match parsed {
            Ok(fx) => {
                if fx.emitters.iter().all(|e| e.time.one_shot) && !fx.despawn_on_finish {
                    report.error(&path, "a one-shot effect must set despawn_on_finish");
                }
            }
            Err(e) => report.error(&path, format!("does not parse: {e}")),
        }
    }
}

/// The model palettes the map generator and the decoration scatter pick from.
pub fn check_code_palettes(report: &mut Report) {
    let palettes = [
        ("src/map/scatter.rs", crate::map::scatter::PALETTES),
        (
            "src/map/map_generator.rs",
            crate::map::map_generator::ALL_PALETTES,
        ),
    ];
    for (file, palettes) in palettes {
        for &(model, _) in palettes.iter().copied().flatten() {
            if !asset_exists(model) {
                report.error(file, format!("palette model: no file 'assets/{model}'"));
            }
        }
    }
}
