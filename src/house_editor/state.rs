use bevy::prelude::Resource;
use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::map::procgen::{resolve_house, HouseResolveResult, HouseSpec, Polygon};

/// State for the standalone House Editor (`GameState::HouseEditor`) — a focused canvas
/// for iterating on `crate::map::procgen`'s polygon-to-house resolver in isolation, with
/// no grid, no tile array, no map-file plumbing. Points are free-form integer world
/// coordinates (not tile-snapped, not angle-snapped): `docs/procedural-maps.md`'s "we
/// should have options to make perpendicular angles" is a drawing affordance for the
/// full map editor, not a constraint the resolver itself needs — it rasterizes any
/// closed polygon.
#[derive(Resource)]
pub struct HouseEditorState {
    /// Nodes of the polygon currently being drawn, in click order. Cleared on close/cancel.
    pub points: Vec<(i32, i32)>,
    /// The last successfully resolved house, if any (kept alongside `points` after a
    /// close so both the drawn outline and the resolved walls stay visible together).
    pub resolved: Option<HouseResolveResult>,
    pub spec: HouseSpec,
    /// True whenever spec/points/resolved changed and the side-panel label needs a refresh.
    pub info_dirty: bool,
}

impl Default for HouseEditorState {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            resolved: None,
            spec: HouseSpec::new(scan_wall_def().unwrap_or_default()),
            info_dirty: true,
        }
    }
}

impl HouseEditorState {
    /// Adds a click as the next polygon node. Closing happens when a click lands back on
    /// the first node with at least 3 nodes already placed (mirrors the map editor's House
    /// tool), or via `close()`/Enter.
    pub fn add_point(&mut self, x: i32, y: i32) {
        if let Some(&first) = self.points.first() {
            if self.points.len() >= 3 && (x, y) == first {
                self.close();
                return;
            }
        }
        if self.points.last() != Some(&(x, y)) {
            self.points.push((x, y));
        }
        self.info_dirty = true;
    }

    /// Resolves the in-progress polygon (no-op below 3 points). `points` is deliberately
    /// left in place afterward so the drawn outline stays visible next to the result, and
    /// so `+`/`-` tuning re-resolves the same shape without redrawing it.
    pub fn close(&mut self) {
        if self.points.len() >= 3 {
            let polygon = Polygon { points: self.points.clone() };
            self.resolved = Some(resolve_house(&polygon, &self.spec));
        }
        self.info_dirty = true;
    }

    /// Cancels the in-progress polygon without touching any previously resolved result.
    pub fn cancel_points(&mut self) {
        self.points.clear();
        self.info_dirty = true;
    }

    /// Clears everything to start a fresh house.
    pub fn clear_all(&mut self) {
        self.points.clear();
        self.resolved = None;
        self.info_dirty = true;
    }

    /// Re-resolves the current `points` against the current `spec` — used by the
    /// door/window tuning keys so a change is visible immediately on an already-closed house.
    pub fn reresolve_if_closed(&mut self) {
        if self.resolved.is_some() && self.points.len() >= 3 {
            let polygon = Polygon { points: self.points.clone() };
            self.resolved = Some(resolve_house(&polygon, &self.spec));
        }
        self.info_dirty = true;
    }

    pub fn adjust_door_count(&mut self, delta: i32) {
        self.spec.door_count = (self.spec.door_count as i32 + delta).max(0) as u32;
        self.reresolve_if_closed();
    }

    pub fn adjust_window_spacing(&mut self, delta: f32) {
        let rule = &mut self.spec.window_rule;
        rule.max_per_wall_len = (rule.max_per_wall_len + delta).max(1.0);
        self.reresolve_if_closed();
    }
}

/// Same "prefer a def named wall, else first Terrain def" heuristic as
/// `map_editor::state::scan_wall_def`.
fn scan_wall_def() -> Option<String> {
    let dir = std::path::Path::new("assets/defs");
    let entries = std::fs::read_dir(dir).ok()?;
    let mut terrain_defs: Vec<String> = entries.flatten().filter_map(|e| {
        let p = e.path();
        if p.extension()?.to_str()? != "ron" { return None; }
        let text = std::fs::read_to_string(&p).ok()?;
        let def: AssetDefinition = ron::from_str(&text).ok()?;
        if !matches!(def.model_type, ModelType::Terrain(_)) { return None; }
        Some(p.to_string_lossy().replace('\\', "/"))
    }).collect();
    terrain_defs.sort();
    terrain_defs.iter()
        .find(|p| p.to_lowercase().contains("wall"))
        .cloned()
        .or_else(|| terrain_defs.into_iter().next())
}
