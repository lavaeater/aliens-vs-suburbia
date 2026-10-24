//! The `.map` text format: a hand-editable companion to the RON [`MapFile`].
//!
//! Same data, written so a map can be sketched in any text editor. One letter per tile,
//! sections in square brackets, `key: value` everywhere else. See
//! `docs/map-text-format.md` for the full description; in short:
//!
//! ```text
//! [properties]
//! name: Backyard
//! stories: assets/stories/backyard.ron
//! loot: 1-5 per 20
//!
//! [tiles]
//! wwwwwwww
//! wpfffffw
//! wffvvffe
//! wxffffffw
//!
//! [legend]
//! A: grass + blocks_enemies
//!
//! [decorations]
//! at: 3, 1 | model: packs/nature/Pine.glb | rotation: 90 | scale: 1.2
//!
//! [placements]
//! at: 2, 2 | def: assets/defs/door.ron | rotation: 45
//!
//! [waves]
//! enemy: assets/defs/Alien.ron | count: 5 | rate: 10
//! ```
//!
//! The width and height are never written: they are the longest row and the number of
//! rows. Short rows are padded with void. Whitespace inside a row is ignored, so rows can
//! be spaced out for readability.
//!
//! Converting is lossless both ways (`every_shipped_map_round_trips`): a tile value with
//! no built-in letter gets one in a generated `[legend]`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::general::components::map_components::{
    DecorationItem, HousePlacement, LootSpread, MapFile, TilePlacement, WaveDef,
};
use crate::map::{BitFlags, MapFeatures, house};

/// File extension of the text format.
pub const EXTENSION: &str = "map";

/// The letters every `.map` knows without a `[legend]`: letter, tile value, meaning.
fn builtin_tiles() -> [(char, u64, &'static str); 11] {
    let floor = BitFlags::from(MapFeatures::Floor);
    let wall = floor | MapFeatures::ImpassableForPlayers | MapFeatures::ImpassableForEnemies;
    [
        ('v', 0, "void"),
        ('f', floor.bits(), "floor"),
        ('w', wall.bits(), "wall"),
        ('e', (floor | MapFeatures::EnemySpawn).bits(), "enemy spawn"),
        (
            'p',
            (floor | MapFeatures::PlayerSpawn).bits(),
            "player spawn",
        ),
        (
            'x',
            (floor | MapFeatures::EnemyExit).bits(),
            "goal (enemy exit)",
        ),
        ('g', BitFlags::from(MapFeatures::Grass).bits(), "grass"),
        ('m', BitFlags::from(MapFeatures::Mud).bits(), "mud"),
        ('s', BitFlags::from(MapFeatures::Snow).bits(), "snow"),
        ('r', BitFlags::from(MapFeatures::Rock).bits(), "rock"),
        ('~', BitFlags::from(MapFeatures::Water).bits(), "water"),
    ]
}

/// Flag names for `[legend]` entries, e.g. `A: grass + blocks_enemies`.
const FLAG_NAMES: [(&str, MapFeatures); 12] = [
    ("nothing", MapFeatures::Nothing),
    ("water", MapFeatures::Water),
    ("grass", MapFeatures::Grass),
    ("floor", MapFeatures::Floor),
    ("mud", MapFeatures::Mud),
    ("snow", MapFeatures::Snow),
    ("rock", MapFeatures::Rock),
    ("blocks_players", MapFeatures::ImpassableForPlayers),
    ("blocks_enemies", MapFeatures::ImpassableForEnemies),
    ("enemy_spawn", MapFeatures::EnemySpawn),
    ("player_spawn", MapFeatures::PlayerSpawn),
    ("enemy_exit", MapFeatures::EnemyExit),
];

/// Letters the writer hands out to tile values the built-ins do not cover.
const SPARE_LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdhijklnoqtuyz0123456789";

#[derive(Clone, Copy)]
enum Section {
    Properties,
    Tiles,
    Legend,
    Decorations,
    Placements,
    Houses,
    Waves,
}

impl Section {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "properties" => Self::Properties,
            "tiles" => Self::Tiles,
            "legend" => Self::Legend,
            "decorations" => Self::Decorations,
            "placements" => Self::Placements,
            "houses" => Self::Houses,
            "waves" => Self::Waves,
            _ => return None,
        })
    }
}

fn at_line(line: usize, message: impl std::fmt::Display) -> String {
    format!("line {line}: {message}")
}

/// Parse a `.map` file's text.
pub fn parse(text: &str) -> Result<MapFile, String> {
    let mut map = MapFile::default();
    let mut section = None;
    let mut rows: Vec<(usize, String)> = Vec::new();
    let mut legend: BTreeMap<char, u64> = builtin_tiles()
        .into_iter()
        .map(|(letter, value, _)| (letter, value))
        .collect();
    let mut size = (None, None);

    for (index, raw) in text.lines().enumerate() {
        let line_no = index.saturating_add(1);
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let name = name.trim();
            section = Some(
                Section::from_name(name)
                    .ok_or_else(|| at_line(line_no, format!("unknown section [{name}]")))?,
            );
            continue;
        }
        let result = match section {
            None => Err("expected a [section] first".to_string()),
            Some(Section::Properties) => property(&mut map, &mut size, line),
            Some(Section::Tiles) => {
                rows.push((
                    line_no,
                    line.chars().filter(|c| !c.is_whitespace()).collect(),
                ));
                Ok(())
            }
            Some(Section::Legend) => legend_entry(line).map(|(letter, value)| {
                legend.insert(letter, value);
            }),
            Some(Section::Decorations) => decoration(line).map(|d| map.decorations.push(d)),
            Some(Section::Placements) => placement(line).map(|p| map.placements.push(p)),
            Some(Section::Houses) => house(line).map(|h| map.houses.push(h)),
            Some(Section::Waves) => wave(line).map(|w| map.waves.push(w)),
        };
        result.map_err(|e| at_line(line_no, e))?;
    }

    // Tiles last: the legend may come after them.
    let width = rows
        .iter()
        .map(|(_, row)| row.chars().count())
        .max()
        .unwrap_or(0);
    for (line_no, row) in rows {
        let mut tiles = Vec::with_capacity(width);
        for (col, letter) in row.chars().enumerate() {
            let value = legend.get(&letter).ok_or_else(|| {
                at_line(
                    line_no,
                    format!(
                        "unknown tile '{letter}' in column {} (add it to [legend])",
                        col.saturating_add(1)
                    ),
                )
            })?;
            tiles.push(*value);
        }
        tiles.resize(width, 0);
        map.tiles.push(tiles);
    }

    match size {
        (Some(width), Some(height)) if map.generated => {
            map.map_width = width;
            map.map_height = height;
        }
        (None, None) if !map.generated => {
            map.map_width = width;
            map.map_height = map.tiles.len();
        }
        _ if map.generated => {
            return Err("a generated map needs both width and height in [properties]".into());
        }
        _ => {
            return Err(
                "width and height come from [tiles]; set them only with generated: true".into(),
            );
        }
    }
    Ok(map)
}

fn property(
    map: &mut MapFile,
    size: &mut (Option<usize>, Option<usize>),
    line: &str,
) -> Result<(), String> {
    let (key, value) = line
        .split_once(':')
        .ok_or_else(|| format!("expected `key: value`, got `{line}`"))?;
    let (key, value) = (key.trim(), value.trim());
    match key {
        "name" => map.name = value.to_string(),
        "stories" => map.stories = value.to_string(),
        "loot" => {
            let (min, max, per_tiles) = loot_spread(value)?;
            let spread = map.loot.get_or_insert_with(LootSpread::default);
            (spread.min, spread.max, spread.per_tiles) = (min, max, per_tiles);
        }
        "loot_table" => map.loot.get_or_insert_with(LootSpread::default).table = value.to_string(),
        "loot_bags" => {
            map.loot.get_or_insert_with(LootSpread::default).bags =
                value.split(',').map(|b| b.trim().to_string()).collect();
        }
        "escape_limit" => map.escape_limit = Some(number(key, value)?),
        "weapons" => {
            map.weapons = value
                .split(',')
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty())
                .collect();
        }
        "coins" => map.coins = Some(number(key, value)?),
        "generated" => map.generated = number(key, value)?,
        "seed" => map.seed = number(key, value)?,
        "width" => size.0 = Some(number(key, value)?),
        "height" => size.1 = Some(number(key, value)?),
        _ => return Err(format!("unknown property `{key}`")),
    }
    Ok(())
}

/// `A: grass + blocks_enemies`, or a raw tile value: `A: 261`.
fn legend_entry(line: &str) -> Result<(char, u64), String> {
    let (letter, value) = line
        .split_once(':')
        .ok_or_else(|| format!("expected `<letter>: <flags>`, got `{line}`"))?;
    let mut letters = letter.trim().chars();
    let (Some(letter), None) = (letters.next(), letters.next()) else {
        return Err(format!("`{}` is not a single character", letter.trim()));
    };
    if letter == '#' || letter == '[' {
        return Err(format!("'{letter}' cannot be a tile"));
    }
    let value = value.trim();
    if let Ok(raw) = value.parse::<u64>() {
        return Ok((letter, raw));
    }
    if value == "void" {
        return Ok((letter, 0));
    }
    let mut flags = BitFlags::<MapFeatures>::empty();
    for name in value.split('+').map(str::trim) {
        let (_, flag) = FLAG_NAMES
            .iter()
            .find(|(known, _)| *known == name)
            .ok_or_else(|| {
                let known: Vec<&str> = FLAG_NAMES.iter().map(|(n, _)| *n).collect();
                format!("unknown flag `{name}` (known: {})", known.join(", "))
            })?;
        flags |= *flag;
    }
    Ok((letter, flags.bits()))
}

/// The `key: value | key: value` fields of one record line.
struct Fields<'a>(BTreeMap<&'a str, &'a str>);

impl<'a> Fields<'a> {
    fn parse(line: &'a str) -> Result<Self, String> {
        let mut fields = BTreeMap::new();
        for part in line.split('|') {
            let (key, value) = part
                .split_once(':')
                .ok_or_else(|| format!("expected `key: value`, got `{}`", part.trim()))?;
            if fields.insert(key.trim(), value.trim()).is_some() {
                return Err(format!("`{}` given twice", key.trim()));
            }
        }
        Ok(Self(fields))
    }

    fn take(&mut self, key: &str) -> Option<&'a str> {
        self.0.remove(key)
    }

    fn require(&mut self, key: &str) -> Result<&'a str, String> {
        self.take(key).ok_or_else(|| format!("missing `{key}:`"))
    }

    /// Everything should have been taken by now; a leftover is a typo.
    fn finish(self) -> Result<(), String> {
        match self.0.keys().next() {
            Some(key) => Err(format!("unknown field `{key}`")),
            None => Ok(()),
        }
    }
}

/// `1-5 per 20` (or `3 per 20`): `min`-`max` loot bags for every 20 open tiles.
fn loot_spread(value: &str) -> Result<(u32, u32, u32), String> {
    let (count, per) = value
        .split_once("per")
        .ok_or_else(|| format!("loot: expected `<min>-<max> per <tiles>`, got `{value}`"))?;
    let (min, max) = if let Some((min, max)) = count.split_once('-') {
        (number("loot", min.trim())?, number("loot", max.trim())?)
    } else {
        let n = number("loot", count.trim())?;
        (n, n)
    };
    Ok((min, max, number("loot", per.trim())?))
}

fn number<T: std::str::FromStr>(key: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{key}: `{value}` is not a valid value"))
}

/// `at: 3, 4` -> `(3, 4)`.
fn coordinates(value: &str) -> Result<(i32, i32), String> {
    let (x, y) = value
        .split_once(',')
        .ok_or_else(|| format!("at: expected `x, y`, got `{value}`"))?;
    Ok((number("at", x.trim())?, number("at", y.trim())?))
}

fn decoration(line: &str) -> Result<DecorationItem, String> {
    let mut fields = Fields::parse(line)?;
    let (x, y) = coordinates(fields.require("at")?)?;
    let model = fields.require("model")?.to_string();
    let rotation_y = fields
        .take("rotation")
        .map_or(Ok(0.0), |v| number("rotation", v))?;
    let scale = fields
        .take("scale")
        .map_or(Ok(1.0), |v| number("scale", v))?;
    fields.finish()?;
    Ok(DecorationItem {
        x,
        y,
        model,
        rotation_y,
        scale,
    })
}

fn placement(line: &str) -> Result<TilePlacement, String> {
    let mut fields = Fields::parse(line)?;
    let (x, y) = coordinates(fields.require("at")?)?;
    let def_path = fields.require("def")?.to_string();
    let rotation_steps = match fields.take("rotation") {
        Some(value) => rotation_steps(number("rotation", value)?)?,
        None => 0,
    };
    fields.finish()?;
    Ok(TilePlacement {
        x,
        y,
        def_path,
        rotation_steps,
    })
}

fn house(line: &str) -> Result<HousePlacement, String> {
    let mut fields = Fields::parse(line)?;
    let house = fields.require("house")?.to_string();
    let (x, y) = coordinates(fields.require("at")?)?;
    let turns = match fields.take("rotation") {
        Some(value) => {
            let degrees: i32 = number("rotation", value)?;
            let degrees = degrees.rem_euclid(360);
            if degrees % 90 != 0 {
                return Err(format!(
                    "rotation: houses turn in steps of 90 degrees, got {degrees}"
                ));
            }
            u8::try_from(degrees / 90).map_err(|e| e.to_string())?
        }
        None => 0,
    };
    fields.finish()?;
    Ok(HousePlacement { house, x, y, turns })
}

/// Placements turn in 45 degree steps; the file says degrees.
fn rotation_steps(degrees: i32) -> Result<u8, String> {
    let degrees = degrees.rem_euclid(360);
    if degrees.rem_euclid(45) != 0 {
        return Err(format!(
            "rotation: placements turn in steps of 45 degrees, got {degrees}"
        ));
    }
    u8::try_from(degrees.div_euclid(45)).map_err(|e| e.to_string())
}

fn wave(line: &str) -> Result<WaveDef, String> {
    let mut fields = Fields::parse(line)?;
    let defaults = WaveDef::default();
    let wave = WaveDef {
        enemy_def: fields.take("enemy").unwrap_or_default().to_string(),
        count: fields
            .take("count")
            .map_or(Ok(defaults.count), |v| number("count", v))?,
        spawn_rate_per_minute: fields
            .take("rate")
            .map_or(Ok(defaults.spawn_rate_per_minute), |v| number("rate", v))?,
    };
    fields.finish()?;
    Ok(wave)
}

/// Write a map as `.map` text. Fails only on what the format cannot hold: a path with a
/// `|` in it, or more distinct unusual tile values than there are spare letters.
pub fn to_text(map: &MapFile) -> Result<String, String> {
    let builtins = builtin_tiles();
    let mut letters: BTreeMap<u64, char> = builtins
        .iter()
        .map(|(letter, value, _)| (*value, *letter))
        .collect();
    let mut legend = Vec::new();
    let mut spare = SPARE_LETTERS
        .chars()
        .filter(|c| !builtins.iter().any(|(b, _, _)| b == c));
    for value in map.tiles.iter().flatten() {
        if !letters.contains_key(value) {
            let letter = spare
                .next()
                .ok_or("more distinct tile values than spare letters")?;
            letters.insert(*value, letter);
            legend.push((letter, *value));
        }
    }

    let key: Vec<String> = builtins
        .iter()
        .map(|(letter, _, meaning)| format!("{letter} {meaning}"))
        .collect();
    let mut lines = vec![format!("# Tiles: {}", key.join(", "))];

    lines.extend([String::new(), "[properties]".into()]);
    if !map.name.is_empty() {
        lines.push(format!("name: {}", map.name));
    }
    if !map.stories.is_empty() {
        lines.push(format!("stories: {}", map.stories));
    }
    if let Some(loot) = &map.loot {
        lines.push(format!(
            "loot: {}-{} per {}",
            loot.min, loot.max, loot.per_tiles
        ));
        let default = LootSpread::default();
        if loot.table != default.table {
            lines.push(format!("loot_table: {}", loot.table));
        }
        if loot.bags != default.bags {
            lines.push(format!("loot_bags: {}", loot.bags.join(", ")));
        }
    }
    if let Some(limit) = map.escape_limit {
        lines.push(format!("escape_limit: {limit}"));
    }
    if !map.weapons.is_empty() {
        lines.push(format!("weapons: {}", map.weapons.join(", ")));
    }
    if let Some(coins) = map.coins {
        lines.push(format!("coins: {coins}"));
    }
    if map.generated {
        lines.push("generated: true".into());
        lines.push(format!("width: {}", map.map_width));
        lines.push(format!("height: {}", map.map_height));
    }
    if map.generated || map.seed != 0 {
        lines.push(format!("seed: {}", map.seed));
    }

    lines.extend([String::new(), "[tiles]".into()]);
    for row in &map.tiles {
        lines.push(
            row.iter()
                .map(|value| letters.get(value).copied().unwrap_or('v'))
                .collect(),
        );
    }

    if !legend.is_empty() {
        lines.extend([String::new(), "[legend]".into()]);
        for (letter, value) in legend {
            lines.push(format!("{letter}: {}", flag_names(value)));
        }
    }

    lines.extend([
        String::new(),
        "[decorations]".into(),
        "# at: x, y | model: <path under assets/> | rotation: <degrees> | scale: <s>".into(),
    ]);
    for d in &map.decorations {
        let mut line = format!("at: {}, {} | model: {}", d.x, d.y, field_value(&d.model)?);
        if d.rotation_y.to_bits() != 0.0f32.to_bits() {
            line = format!("{line} | rotation: {}", d.rotation_y);
        }
        if d.scale.to_bits() != 1.0f32.to_bits() {
            line = format!("{line} | scale: {}", d.scale);
        }
        lines.push(line);
    }

    lines.extend([
        String::new(),
        "[placements]".into(),
        "# at: x, y | def: <def .ron path> | rotation: <degrees, steps of 45>".into(),
    ]);
    for p in &map.placements {
        let mut line = format!("at: {}, {} | def: {}", p.x, p.y, field_value(&p.def_path)?);
        if p.rotation_steps != 0 {
            let degrees = u32::from(p.rotation_steps).saturating_mul(45);
            line = format!("{line} | rotation: {degrees}");
        }
        lines.push(line);
    }

    // Only maps with houses get the section, so older maps write back unchanged.
    if !map.houses.is_empty() {
        lines.extend([
            String::new(),
            "[houses]".into(),
            "# house: <name in assets/houses> | at: <top-left tile x, y> | rotation: <degrees clockwise, steps of 90>".into(),
        ]);
    }
    for h in &map.houses {
        let mut line = format!("house: {} | at: {}, {}", field_value(&h.house)?, h.x, h.y);
        if h.turns % 4 != 0 {
            line = format!("{line} | rotation: {}", u32::from(h.turns % 4) * 90);
        }
        lines.push(line);
    }

    lines.extend([
        String::new(),
        "[waves]".into(),
        "# enemy: <enemy def path, omit for the built-in alien> | count: <n> | rate: <per minute>"
            .into(),
    ]);
    for w in &map.waves {
        let rest = format!("count: {} | rate: {}", w.count, w.spawn_rate_per_minute);
        lines.push(if w.enemy_def.is_empty() {
            rest
        } else {
            format!("enemy: {} | {rest}", field_value(&w.enemy_def)?)
        });
    }

    lines.push(String::new());
    Ok(lines.join("\n"))
}

/// `grass + blocks_enemies` when the value is made of known flags, else the raw number.
fn flag_names(value: u64) -> String {
    let Ok(flags) = BitFlags::<MapFeatures>::from_bits(value) else {
        return value.to_string();
    };
    if flags.is_empty() {
        return "void".to_string();
    }
    FLAG_NAMES
        .iter()
        .filter(|(_, flag)| flags.contains(*flag))
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(" + ")
}

/// A value has to survive being split on `|` and trimmed.
fn field_value(value: &str) -> Result<&str, String> {
    if value.contains('|') || value.trim() != value || value.contains('\n') {
        return Err(format!("`{value}` cannot be written to a .map file"));
    }
    Ok(value)
}

/// Whether `path` is a `.map` text file rather than RON.
pub fn is_text_map(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == EXTENSION)
}

/// Read a map in whichever format its extension says.
pub fn read_map_file(path: impl AsRef<Path>) -> Result<MapFile, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let map = if is_text_map(path) {
        parse(&text)
    } else {
        ron::from_str::<MapFile>(&text).map_err(|e| e.to_string())
    };
    map.map_err(|e| format!("cannot parse {}: {e}", path.display()))
}

/// Write a map in whichever format its extension says.
pub fn write_map_file(path: impl AsRef<Path>, map: &MapFile) -> Result<(), String> {
    let path = path.as_ref();
    let text = if is_text_map(path) {
        to_text(map)?
    } else {
        ron::ser::to_string_pretty(map, ron::ser::PrettyConfig::new().depth_limit(4))
            .map_err(|e| e.to_string())?
    };
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Load a level the game asks for by its `.ron` path. A `.map` with the same stem next to
/// it wins, so a level can be reworked as text without touching the code that names it.
/// Returns the file actually read.
///
/// The map comes back as the game plays it: its houses are stamped into the tiles
/// (`house::bake_houses`).
pub fn load_level(path: impl AsRef<Path>) -> Result<(MapFile, PathBuf), String> {
    let path = path.as_ref();
    let text_sibling = path.with_extension(EXTENSION);
    let chosen = if text_sibling.exists() {
        text_sibling
    } else {
        path.to_path_buf()
    };
    let mut map = read_map_file(&chosen)?;
    if !map.houses.is_empty() {
        let wall_def = house::default_wall_def().unwrap_or_default();
        for error in house::bake_houses(&mut map, &wall_def, |name| {
            house::HouseDef::load(&house::house_path(name))
        }) {
            bevy::log::warn!("{}: house skipped: {error}", chosen.display());
        }
    }
    Ok((map, chosen))
}

/// The other format's path for `path`: `foo.ron` <-> `foo.map`.
pub fn converted_path(path: &Path) -> PathBuf {
    if is_text_map(path) {
        path.with_extension("ron")
    } else {
        path.with_extension(EXTENSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "\
# A small test yard.
[properties]
name: Backyard
loot: 1-5 per 20
loot_table: crate

[tiles]
wwwww
wpfew
w v
wxffw

[legend]
A: grass + blocks_enemies

[decorations]
at: 1, 1 | model: packs/nature/Pine.glb | rotation: 90 | scale: 1.5
at: 2, 2 | model: packs/nature/Rock 2.glb

[placements]
at: 3, 1 | def: assets/defs/door.ron | rotation: 90

[waves]
enemy: assets/defs/Alien.ron | count: 5 | rate: 10
count: 3
";

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "the values are parsed from exact literals"
    )]
    fn the_example_parses() {
        let map = parse(EXAMPLE).unwrap();
        assert_eq!(map.name, "Backyard");
        assert_eq!((map.map_width, map.map_height), (5, 4));
        assert_eq!(map.tiles[0], vec![392; 5]);
        assert_eq!(map.tiles[1], vec![392, 1032, 8, 520, 392]);
        // Whitespace inside a row is ignored, and a short row is padded with void.
        assert_eq!(map.tiles[2], vec![392, 0, 0, 0, 0]);
        assert_eq!(map.tiles[3], vec![392, 2056, 8, 8, 392]);

        assert_eq!(map.decorations.len(), 2);
        assert_eq!(map.decorations[0].rotation_y, 90.0);
        assert_eq!(map.decorations[0].scale, 1.5);
        assert_eq!(map.decorations[1].model, "packs/nature/Rock 2.glb");
        assert_eq!(map.decorations[1].scale, 1.0);

        assert_eq!(map.placements[0].rotation_steps, 2);
        assert_eq!(map.waves[0].enemy_def, "assets/defs/Alien.ron");
        assert_eq!(map.waves[0].count, 5);
        // A bare wave takes the built-in alien and the default rate.
        assert_eq!(map.waves[1].enemy_def, "");
        assert_eq!(map.waves[1].count, 3);
        assert_eq!(map.waves[1].spawn_rate_per_minute, 20.0);
    }

    #[test]
    fn legend_letters_mean_what_they_say() {
        let map = parse("[tiles]\nAg\n[legend]\nA: grass + blocks_enemies\ng: 5\n").unwrap();
        assert_eq!(map.tiles[0], vec![4 + 256, 5]);
    }

    #[test]
    fn an_unknown_tile_names_its_line_and_column() {
        let err = parse("[properties]\nname: x\n[tiles]\nwwz\n").unwrap_err();
        assert!(err.contains("line 4") && err.contains("column 3"), "{err}");
    }

    #[test]
    fn a_typo_in_a_field_is_an_error_not_a_silent_default() {
        let err = parse("[waves]\ncount: 5 | rat: 10\n").unwrap_err();
        assert!(err.contains("rat"), "{err}");
    }

    #[test]
    fn placements_only_turn_in_steps_of_45() {
        assert!(parse("[placements]\nat: 0, 0 | def: d.ron | rotation: 30\n").is_err());
        let map = parse("[placements]\nat: 0, 0 | def: d.ron | rotation: -90\n").unwrap();
        assert_eq!(map.placements[0].rotation_steps, 6);
    }

    #[test]
    fn size_is_only_written_for_generated_maps() {
        assert!(parse("[properties]\nwidth: 4\n[tiles]\nff\n").is_err());
        let map = parse("[properties]\ngenerated: true\nseed: 7\nwidth: 30\nheight: 12\n").unwrap();
        assert_eq!((map.map_width, map.map_height, map.seed), (30, 12, 7));
    }

    fn as_ron(map: &MapFile) -> String {
        ron::to_string(map).unwrap()
    }

    /// Every map the game ships converts to text and back unchanged — the format is a
    /// companion, not a lossy export. Width and height of a hand-made map are what its
    /// tiles say, so they are normalised first.
    #[test]
    fn every_shipped_map_round_trips() {
        let mut checked = 0;
        for entry in std::fs::read_dir("assets/maps").unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "ron") {
                continue;
            }
            let mut original = read_map_file(&path).unwrap();
            if !original.generated {
                original.map_width = original.tiles.first().map_or(0, Vec::len);
                original.map_height = original.tiles.len();
            }
            let text = to_text(&original).unwrap();
            let back = parse(&text).unwrap_or_else(|e| panic!("{}: {e}\n{text}", path.display()));
            assert_eq!(as_ron(&back), as_ron(&original), "{}", path.display());
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn loot_is_a_count_per_stretch_of_tiles() {
        let map = parse(EXAMPLE).unwrap();
        let loot = map.loot.unwrap();
        assert_eq!((loot.min, loot.max, loot.per_tiles), (1, 5, 20));
        assert_eq!(loot.table, "crate");
        assert_eq!(loot.bags, LootSpread::default().bags);
        let bags = parse("[properties]\nloot: 1 per 9\nloot_bags: a.ron, b c.ron\n")
            .unwrap()
            .loot
            .unwrap()
            .bags;
        assert_eq!(bags, vec!["a.ron".to_string(), "b c.ron".to_string()]);
        let one = parse("[properties]\nloot: 2 per 30\n")
            .unwrap()
            .loot
            .unwrap();
        assert_eq!((one.min, one.max, one.per_tiles), (2, 2, 30));
        assert!(parse("[properties]\nloot: lots\n").is_err());
        assert!(parse("[properties]\nname: x\n").unwrap().loot.is_none());
    }

    #[test]
    fn level_rules_are_properties() {
        let map = parse(
            "[properties]\nescape_limit: 3\nweapons: assets/defs/Snowball.ron, assets/defs/Pistol.ron\ncoins: 200\n",
        )
        .unwrap();
        assert_eq!(map.escape_limit, Some(3));
        assert_eq!(map.weapons, vec!["assets/defs/Snowball.ron", "assets/defs/Pistol.ron"]);
        assert_eq!(map.coins, Some(200));
        let again = parse(&to_text(&map).unwrap()).unwrap();
        assert_eq!((again.escape_limit, again.weapons, again.coins), (map.escape_limit, map.weapons, map.coins));
    }

    #[test]
    fn houses_are_named_placed_and_turned_in_quarters() {
        let map = parse("[houses]\nhouse: bungalow | at: 3, 4 | rotation: -90\nhouse: shed | at: 0, 0\n").unwrap();
        assert_eq!(
            map.houses,
            vec![
                HousePlacement { house: "bungalow".into(), x: 3, y: 4, turns: 3 },
                HousePlacement { house: "shed".into(), x: 0, y: 0, turns: 0 },
            ]
        );
        assert!(parse("[houses]\nhouse: shed | at: 0, 0 | rotation: 45\n").is_err());
        assert!(parse("[houses]\nat: 0, 0\n").is_err());
        let text = to_text(&map).unwrap();
        assert!(text.contains("house: bungalow | at: 3, 4 | rotation: 270"), "{text}");
        assert_eq!(parse(&text).unwrap().houses, map.houses);
        assert!(!to_text(&MapFile::default()).unwrap().contains("[houses]"));
    }

    #[test]
    fn a_level_loads_with_its_houses_stamped_in() {
        let dir = std::env::temp_dir().join(format!("avs-level-houses-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yard.map");
        let row = "f".repeat(14);
        let tiles = vec![row; 10].join("\n");
        std::fs::write(
            &path,
            format!("[tiles]\n{tiles}\n[houses]\nhouse: bungalow | at: 2, 2\nhouse: no_such_house | at: 0, 0\n"),
        )
        .unwrap();
        let (map, _) = load_level(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(map.houses.is_empty());
        let wall = (MapFeatures::ImpassableForPlayers as u64) | (MapFeatures::ImpassableForEnemies as u64);
        let blocked = map.tiles.iter().flatten().filter(|&&t| t & wall == wall).count();
        assert!(blocked > 10, "the bungalow's walls block: {blocked}");
        assert_eq!(map.placements.len(), blocked, "a wall model on each");
        assert_eq!(map.tiles[0][0] & wall, 0, "the missing house is skipped");
    }

    #[test]
    fn the_example_round_trips() {
        let map = parse(EXAMPLE).unwrap();
        let back = parse(&to_text(&map).unwrap()).unwrap();
        assert_eq!(as_ron(&back), as_ron(&map));
    }
}
