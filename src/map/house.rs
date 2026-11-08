//! House files: one house as data, `assets/houses/<name>.ron` (the file stem is its name).
//!
//! A [`HouseDef`] is the authored, unresolved house from `docs/house-editor.md`: the
//! outline drawn in the house editor plus the rules it is built by (walls, windows, doors,
//! roof). Nothing is resolved until something uses it: [`HouseDef::spec`] turns the rules
//! the tile resolver understands into a `procgen::HouseSpec` for `procgen::resolve_house`.
//! The rest (material, corners, roof) is for the generated house mesh to come.
//!
//! Distances are metres; a map tile is one metre, so the outline is in tiles too.
//!
//! Every editable property is listed by [`fields`] and changed by [`step`], so an editor
//! draws a row per field without knowing what the fields are.

use serde::{Deserialize, Serialize};

use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::general::components::map_components::{HousePlacement, MapFile};
use crate::map::procgen::{
    HouseResolveResult, HouseSpec, Polygon, WindowRule, apply_house_to_map, resolve_house,
};

/// Where house files live.
pub const HOUSE_DIR: &str = "assets/houses";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HouseDef {
    /// Corners of the footprint, whole metres, in drawing order; closed back to the first.
    pub outline: Vec<(i32, i32)>,
    #[serde(default)]
    pub walls: HouseWalls,
    #[serde(default)]
    pub windows: Windows,
    #[serde(default)]
    pub doors: Doors,
    #[serde(default)]
    pub roof: Roof,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HouseWalls {
    pub height: f32,
    pub material: WallMaterial,
    pub corners: Corners,
}

impl Default for HouseWalls {
    fn default() -> Self {
        Self {
            height: 2.6,
            material: WallMaterial::default(),
            corners: Corners::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WallMaterial {
    Brick,
    Stone,
    #[default]
    Plaster,
    WoodPanel(PanelDirection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PanelDirection {
    #[default]
    Horizontal,
    Vertical,
}

/// Flat corners (a stone or plastered house) or wind-protective planks over them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Corners {
    #[default]
    Flat,
    Planked,
}

/// "1 window per 4 metres, max": a wall gets `floor(length / per_metres)` windows, and
/// none when it is shorter than `min_wall`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Windows {
    pub per_metres: f32,
    pub min_wall: f32,
    pub width: f32,
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            per_metres: 4.0,
            min_wall: 2.0,
            width: 1.0,
        }
    }
}

/// Doors go on the longest wall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doors {
    pub count: u32,
}

impl Default for Doors {
    fn default() -> Self {
        Self { count: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub style: RoofStyle,
    pub material: RoofMaterial,
    /// Slope of a saddle roof, degrees from flat.
    pub pitch_deg: f32,
    pub chimney: bool,
}

impl Default for Roof {
    fn default() -> Self {
        Self {
            style: RoofStyle::default(),
            material: RoofMaterial::default(),
            pitch_deg: 35.0,
            chimney: true,
        }
    }
}

/// Saddle = two slopes meeting at a ridge (Swedish "sadeltak"); flat = flat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RoofStyle {
    #[default]
    Saddle,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RoofMaterial {
    #[default]
    Tiles,
    Metal,
    Felt,
}

impl HouseDef {
    pub fn polygon(&self) -> Polygon {
        Polygon {
            points: self.outline.clone(),
        }
    }

    /// What the tile resolver needs: door count and the window rule, plus the def drawn
    /// for each wall tile.
    pub fn spec(&self, wall_def: &str) -> HouseSpec {
        let mut spec = HouseSpec::new(wall_def);
        spec.door_count = self.doors.count;
        spec.window_rule = WindowRule {
            min_len_for_window: self.windows.min_wall,
            max_per_wall_len: self.windows.per_metres,
        };
        spec
    }

    /// The outline turned `turns` clockwise quarter turns (on the map, where y grows
    /// down) and moved so its bounding box starts at `at`.
    pub fn placed_outline(&self, at: (i32, i32), turns: u8) -> Vec<(i32, i32)> {
        let turned: Vec<(i32, i32)> = self
            .outline
            .iter()
            .map(|&p| (0..turns % 4).fold(p, |(x, y), _| (-y, x)))
            .collect();
        let min_x = turned.iter().map(|p| p.0).min().unwrap_or(0);
        let min_y = turned.iter().map(|p| p.1).min().unwrap_or(0);
        turned
            .into_iter()
            .map(|(x, y)| (x - min_x + at.0, y - min_y + at.1))
            .collect()
    }

    /// The house resolved where `placement` puts it.
    pub fn resolve_placed(&self, placement: &HousePlacement, wall_def: &str) -> HouseResolveResult {
        let points = self.placed_outline((placement.x, placement.y), placement.turns);
        resolve_house(&Polygon { points }, &self.spec(wall_def))
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        ron::from_str(&text).map_err(|e| format!("{path}: {e}"))
    }

    pub fn save(&self, path: &str) -> Result<(), String> {
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())?;
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
    }
}

/// Every house file in `dir`, sorted by path.
pub fn house_files(dir: &str) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();
    files
}

/// The first `house_<n>.ron` in `dir` that does not exist yet.
pub fn free_house_path(dir: &str) -> String {
    // Bounded by the first free name, and by u32::MAX regardless.
    #[allow(clippy::maybe_infinite_iter)]
    let n = (1u32..)
        .find(|n| {
            !std::path::Path::new(dir)
                .join(format!("house_{n}.ron"))
                .exists()
        })
        .unwrap_or(1);
    format!("{dir}/house_{n}.ron")
}

/// The file of the house called `name`.
pub fn house_path(name: &str) -> String {
    format!("{HOUSE_DIR}/{name}.ron")
}

/// Every house in [`HOUSE_DIR`] by name. Files that do not parse are left out.
pub fn house_library() -> std::collections::BTreeMap<String, HouseDef> {
    house_files(HOUSE_DIR)
        .into_iter()
        .filter_map(|path| Some((house_name(&path), HouseDef::load(&path).ok()?)))
        .collect()
}

/// Stamp every house placed in `map` into its tiles and placements, as the game plays
/// it: walls block, doors are floor, each wall tile gets `wall_def`'s model (none for
/// an empty `wall_def`, as `text_format::load_level` bakes them), placements
/// inside the house are kept and those under its walls dropped. Where houses
/// overlap the later one wins. `load` reads a house by name; the ones it cannot read are
/// skipped and returned as errors.
pub fn bake_houses(
    map: &mut MapFile,
    wall_def: &str,
    mut load: impl FnMut(&str) -> Result<HouseDef, String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for placement in std::mem::take(&mut map.houses) {
        match load(&placement.house) {
            Ok(house) => {
                let spec = house.spec(wall_def);
                let resolved = house.resolve_placed(&placement, wall_def);
                // Whatever stands inside (a story item, a barrel) stays; only the walls
                // replace what was under them.
                let inside: Vec<_> = map
                    .placements
                    .iter()
                    .filter(|p| resolved.floor_cells.contains(&(p.x, p.y)))
                    .cloned()
                    .collect();
                apply_house_to_map(&resolved, &spec, map);
                map.placements.extend(inside);
            }
            Err(e) => errors.push(e),
        }
    }
    errors
}

/// The def drawn for each wall tile until houses get their own mesh: a Terrain def named
/// like a wall, else the first Terrain def.
pub fn default_wall_def() -> Option<String> {
    let entries = std::fs::read_dir("assets/defs").ok()?;
    let mut terrain_defs: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? != "ron" {
                return None;
            }
            let text = std::fs::read_to_string(&p).ok()?;
            let def: AssetDefinition = ron::from_str(&text).ok()?;
            if !matches!(def.model_type, ModelType::Terrain(_)) {
                return None;
            }
            Some(p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    terrain_defs.sort();
    terrain_defs
        .iter()
        .find(|p| p.to_lowercase().contains("wall"))
        .cloned()
        .or_else(|| terrain_defs.into_iter().next())
}

/// The file stem: a house's name.
pub fn house_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map_or_else(|| path.to_string(), |s| s.to_string_lossy().into_owned())
}

/// One editable property: an id for [`step`], a label and its value as text.
#[derive(Debug, Clone, PartialEq)]
pub struct HouseField {
    pub id: &'static str,
    pub label: &'static str,
    pub value: String,
}

fn field(id: &'static str, label: &'static str, value: impl Into<String>) -> HouseField {
    HouseField {
        id,
        label,
        value: value.into(),
    }
}

const fn material_name(material: WallMaterial) -> &'static str {
    match material {
        WallMaterial::Brick => "brick",
        WallMaterial::Stone => "stone",
        WallMaterial::Plaster => "plaster",
        WallMaterial::WoodPanel(_) => "wood panel",
    }
}

/// The house's properties, in panel order. Panel direction only shows for wood.
pub fn fields(house: &HouseDef) -> Vec<HouseField> {
    let mut out = vec![
        field(
            "wall_height",
            "wall height",
            format!("{:.1} m", house.walls.height),
        ),
        field("material", "walls", material_name(house.walls.material)),
    ];
    if let WallMaterial::WoodPanel(direction) = house.walls.material {
        let name = match direction {
            PanelDirection::Horizontal => "horizontal",
            PanelDirection::Vertical => "vertical",
        };
        out.push(field("panels", "  panels", name));
    }
    out.extend([
        field(
            "corners",
            "corners",
            match house.walls.corners {
                Corners::Flat => "flat",
                Corners::Planked => "planked",
            },
        ),
        field("doors", "doors", house.doors.count.to_string()),
        field(
            "window_every",
            "a window per",
            format!("{:.1} m", house.windows.per_metres),
        ),
        field(
            "window_min_wall",
            "  none under",
            format!("{:.1} m", house.windows.min_wall),
        ),
        field(
            "window_width",
            "  window width",
            format!("{:.1} m", house.windows.width),
        ),
        field(
            "roof",
            "roof",
            match house.roof.style {
                RoofStyle::Saddle => "saddle",
                RoofStyle::Flat => "flat",
            },
        ),
    ]);
    if house.roof.style == RoofStyle::Saddle {
        out.push(field(
            "pitch",
            "  pitch",
            format!("{:.0} deg", house.roof.pitch_deg),
        ));
    }
    out.extend([
        field(
            "roof_material",
            "  roofing",
            match house.roof.material {
                RoofMaterial::Tiles => "tiles",
                RoofMaterial::Metal => "metal",
                RoofMaterial::Felt => "felt",
            },
        ),
        field(
            "chimney",
            "  chimney",
            if house.roof.chimney { "yes" } else { "no" },
        ),
    ]);
    out
}

/// The next (`forward`) or previous option in a cycle.
fn cycle<T: Copy + PartialEq>(options: &[T], current: T, forward: bool) -> T {
    let Some(i) = options.iter().position(|o| *o == current) else {
        return options.first().copied().unwrap_or(current);
    };
    let n = options.len();
    let next = if forward {
        (i + 1) % n
    } else {
        (i + n - 1) % n
    };
    options.get(next).copied().unwrap_or(current)
}

/// Nudge `value` by `delta`, kept within `min..=max` and rounded to the step.
fn nudge(value: f32, delta: f32, min: f32, max: f32) -> f32 {
    let step = delta.abs();
    (((value + delta) / step).round() * step).clamp(min, max)
}

/// Change one property a step: up / next (`forward`) or down / previous.
pub fn step(house: &mut HouseDef, id: &str, forward: bool) {
    let sign = if forward { 1.0 } else { -1.0 };
    match id {
        "wall_height" => house.walls.height = nudge(house.walls.height, 0.1 * sign, 1.5, 10.0),
        "material" => {
            let wood = match house.walls.material {
                WallMaterial::WoodPanel(d) => d,
                _ => PanelDirection::default(),
            };
            let options = [
                WallMaterial::Plaster,
                WallMaterial::Brick,
                WallMaterial::Stone,
                WallMaterial::WoodPanel(wood),
            ];
            house.walls.material = cycle(&options, house.walls.material, forward);
        }
        "panels" => {
            if let WallMaterial::WoodPanel(direction) = &mut house.walls.material {
                *direction = cycle(
                    &[PanelDirection::Horizontal, PanelDirection::Vertical],
                    *direction,
                    forward,
                );
            }
        }
        "corners" => {
            house.walls.corners = cycle(
                &[Corners::Flat, Corners::Planked],
                house.walls.corners,
                forward,
            );
        }
        "doors" => {
            house.doors.count = if forward {
                house.doors.count.saturating_add(1).min(8)
            } else {
                house.doors.count.saturating_sub(1)
            };
        }
        "window_every" => {
            house.windows.per_metres = nudge(house.windows.per_metres, 0.5 * sign, 1.0, 20.0);
        }
        "window_min_wall" => {
            house.windows.min_wall = nudge(house.windows.min_wall, 0.5 * sign, 0.0, 20.0);
        }
        "window_width" => {
            house.windows.width = nudge(house.windows.width, 0.1 * sign, 0.4, 3.0);
        }
        "roof" => {
            house.roof.style = cycle(
                &[RoofStyle::Saddle, RoofStyle::Flat],
                house.roof.style,
                forward,
            );
        }
        "pitch" => house.roof.pitch_deg = nudge(house.roof.pitch_deg, 5.0 * sign, 10.0, 60.0),
        "roof_material" => {
            house.roof.material = cycle(
                &[RoofMaterial::Tiles, RoofMaterial::Metal, RoofMaterial::Felt],
                house.roof.material,
                forward,
            );
        }
        "chimney" => house.roof.chimney = !house.roof.chimney,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::components::map_components::TilePlacement;
    use crate::map::MapFeatures;
    use crate::map::procgen::WallCellKind;

    #[test]
    fn every_house_in_assets_parses_and_closes() {
        let files = house_files(HOUSE_DIR);
        assert!(
            !files.is_empty(),
            "expected at least one house in {HOUSE_DIR}"
        );
        for path in files {
            let house = HouseDef::load(&path).unwrap_or_else(|e| panic!("{e}"));
            assert!(
                house.outline.len() >= 3,
                "{path}: an outline needs 3 corners"
            );
        }
    }

    #[test]
    fn a_placed_outline_turns_clockwise_and_starts_at_its_tile() {
        let house = HouseDef {
            // 4 wide, 2 deep, with a notch.
            outline: vec![(0, 0), (4, 0), (4, 2), (1, 2), (1, 1), (0, 1)],
            ..HouseDef::default()
        };
        assert_eq!(house.placed_outline((3, 5), 0)[1], (7, 5));
        let turned = house.placed_outline((3, 5), 1);
        let max = |f: fn(&(i32, i32)) -> i32| turned.iter().map(f).max().unwrap();
        let min = |f: fn(&(i32, i32)) -> i32| turned.iter().map(f).min().unwrap();
        assert_eq!((min(|p| p.0), min(|p| p.1)), (3, 5));
        assert_eq!((max(|p| p.0), max(|p| p.1)), (5, 9), "2 wide, 4 deep");
        // The first edge ran east; a clockwise quarter turn sends it south.
        let (a, b) = (turned[0], turned[1]);
        assert_eq!((b.0 - a.0, b.1 - a.1), (0, 4));
        assert_eq!(
            house.placed_outline((0, 0), 4),
            house.placed_outline((0, 0), 0)
        );
    }

    #[test]
    fn baking_stamps_each_house_and_reports_the_missing() {
        let mut map = MapFile {
            map_width: 12,
            map_height: 8,
            tiles: vec![vec![0; 12]; 8],
            houses: vec![
                HousePlacement {
                    house: "box".into(),
                    x: 2,
                    y: 1,
                    turns: 1,
                },
                HousePlacement {
                    house: "gone".into(),
                    ..HousePlacement::default()
                },
            ],
            ..MapFile::default()
        };
        let boxy = HouseDef {
            outline: vec![(0, 0), (6, 0), (6, 4), (0, 4)],
            ..HouseDef::default()
        };
        // A barrel inside the turned house (x 2..6, y 1..7) and one under its wall.
        map.placements = vec![
            TilePlacement {
                x: 3,
                y: 3,
                def_path: "barrel.ron".into(),
                rotation_steps: 0,
            },
            TilePlacement {
                x: 2,
                y: 3,
                def_path: "barrel.ron".into(),
                rotation_steps: 0,
            },
        ];
        let errors = bake_houses(&mut map, "wall.ron", |name| {
            if name == "box" {
                Ok(boxy.clone())
            } else {
                Err(format!("no house {name}"))
            }
        });
        assert_eq!(errors, vec!["no house gone".to_string()]);
        assert!(map.houses.is_empty(), "baked into the tiles");

        let resolved = boxy.resolve_placed(
            &HousePlacement {
                house: "box".into(),
                x: 2,
                y: 1,
                turns: 1,
            },
            "wall.ron",
        );
        // Turned: 4 wide from x 2, 6 deep from y 1.
        assert!(
            resolved
                .wall_cells
                .iter()
                .all(|c| (2..6).contains(&c.x) && (1..7).contains(&c.y))
        );
        let walls = resolved
            .wall_cells
            .iter()
            .filter(|c| c.kind != WallCellKind::Door)
            .count();
        assert_eq!(
            map.placements.len(),
            walls + 1,
            "a model on every wall and window tile, and the barrel inside"
        );
        assert!(map.placements.iter().any(|p| (p.x, p.y, p.def_path.as_str()) == (3, 3, "barrel.ron")));
        let blocked = map
            .tiles
            .iter()
            .flatten()
            .filter(|&&t| t & MapFeatures::ImpassableForPlayers as u64 != 0)
            .count();
        assert_eq!(blocked, walls);
        assert_eq!(map.tiles[0][0], 0, "outside the house is untouched");
    }

    #[test]
    fn a_house_round_trips_through_ron() {
        let mut house = HouseDef {
            outline: vec![(0, 0), (8, 0), (8, 5), (0, 5)],
            ..HouseDef::default()
        };
        house.walls.material = WallMaterial::WoodPanel(PanelDirection::Vertical);
        let text = ron::to_string(&house).unwrap();
        assert_eq!(ron::from_str::<HouseDef>(&text).unwrap(), house);
        // Only the outline is required; the rules fall back to their defaults.
        let bare: HouseDef = ron::from_str("(outline: [(0, 0), (3, 0), (3, 3)])").unwrap();
        assert_eq!(bare.roof, Roof::default());
    }

    #[test]
    fn the_rules_reach_the_resolver() {
        let mut house = HouseDef {
            outline: vec![(0, 0), (8, 0), (8, 5), (0, 5)],
            ..HouseDef::default()
        };
        house.doors.count = 2;
        let resolved = resolve_house(&house.polygon(), &house.spec("wall.ron"));
        assert_eq!(resolved.door_count(), 2);
        step(&mut house, "window_every", false);
        let spec = house.spec("wall.ron");
        assert!((spec.window_rule.max_per_wall_len - 3.5).abs() < 1e-6);
    }

    #[test]
    fn stepping_cycles_and_clamps() {
        let mut house = HouseDef::default();
        step(&mut house, "material", true);
        assert_eq!(house.walls.material, WallMaterial::Brick);
        step(&mut house, "material", false);
        step(&mut house, "material", false);
        assert!(matches!(house.walls.material, WallMaterial::WoodPanel(_)));
        assert!(
            fields(&house).iter().any(|f| f.id == "panels"),
            "wood shows panels"
        );
        step(&mut house, "panels", true);
        assert_eq!(
            house.walls.material,
            WallMaterial::WoodPanel(PanelDirection::Vertical)
        );

        for _ in 0..20 {
            step(&mut house, "doors", false);
        }
        assert_eq!(house.doors.count, 0);
        for _ in 0..200 {
            step(&mut house, "wall_height", true);
        }
        assert!((house.walls.height - 10.0).abs() < 1e-4);
        step(&mut house, "wall_height", false);
        assert!(
            (house.walls.height - 9.9).abs() < 1e-4,
            "no float drift: {}",
            house.walls.height
        );

        step(&mut house, "roof", true);
        assert!(
            !fields(&house).iter().any(|f| f.id == "pitch"),
            "a flat roof has no pitch"
        );
        let chimney = house.roof.chimney;
        step(&mut house, "chimney", false);
        assert_ne!(house.roof.chimney, chimney);
    }

    #[test]
    fn names_and_free_paths() {
        assert_eq!(house_name("assets/houses/bungalow.ron"), "bungalow");
        let free = free_house_path("assets/houses-none-here");
        assert_eq!(free, "assets/houses-none-here/house_1.ron");
    }
}
