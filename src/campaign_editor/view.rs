//! The campaign editor's world view: the base map as one texture (a pixel per tile, hills
//! tinted, roads drawn), the levels' regions and markers as gizmos over it, on a
//! `Camera2d` that pans (right or middle drag, arrow keys) and zooms (wheel, +/-).
//!
//! Tile `(x, y)` of the base covers world `x * TILE..(x + 1) * TILE` across and
//! `-y * TILE..-(y + 1) * TILE` down (rows grow downwards, world y up).
//!
//! The mouse, by [`Tool`]: **Select** - click a region to select it, drag inside it to
//! move it, drag its edges to resize it, Shift+drag (or drag on empty ground) to draw a
//! new level. **Player / Spawn / Goal** - click inside the selected level to place one,
//! drag a marker to move it, right click one to remove it.
// Pixel buffers: indices come from their own sizes.
#![allow(clippy::indexing_slicing)]

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;

use super::analysis::Analysis;
use super::doc::{CampaignDoc, spanning};
use super::panel::{TextFocus, panel_px};
use crate::general::components::map_components::{
    ELEVATION_STEP, MapFile, MarkerKind, Region, RoadKind, RoadLine,
};
use crate::ui::spawn_ui::StateMarker;

/// World units per tile.
pub const TILE: f32 = 4.0;

const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 8.0;

#[derive(Component)]
pub struct CampaignCamera;

#[derive(Component)]
pub struct WorldSprite;

/// A level's number and title, at its region's top-left corner.
#[derive(Component)]
pub struct RegionLabel;

/// What the left mouse button does.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Marker(MarkerKind),
}

impl Tool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Marker(kind) => kind.name(),
        }
    }
}

/// Which edges of a region a drag moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Edges {
    const fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// A drag in progress.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub enum Drag {
    #[default]
    None,
    /// Panning, from this cursor position (window pixels).
    Pan(Vec2),
    /// Drawing a new region from this tile.
    Draw((i32, i32)),
    /// Moving the selected level's region, grabbed at this tile, from where it was.
    Move {
        grab: (i32, i32),
        from: Region,
    },
    /// Moving some edges of the selected level's region.
    Resize {
        edges: Edges,
        from: Region,
    },
    /// Moving the selected level's marker on this tile.
    Marker((i32, i32)),
}

/// The tile under the cursor, if the cursor is over the world (not the panel).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct Hover {
    pub tile: Option<(i32, i32)>,
    pub world: Option<Vec2>,
}

pub fn tile_to_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x * TILE, -y * TILE)
}

pub fn world_to_tile(world: Vec2) -> (i32, i32) {
    (
        (world.x / TILE).floor() as i32,
        (-world.y / TILE).floor() as i32,
    )
}

/// A region's world-space centre and size.
fn region_rect(r: Region) -> (Vec2, Vec2) {
    let size = Vec2::new(r.width as f32, r.height as f32) * TILE;
    let corner = tile_to_world(r.x as f32, r.y as f32);
    (corner + Vec2::new(size.x, -size.y) / 2.0, size)
}

fn road_color(kind: RoadKind) -> [u8; 4] {
    match kind {
        RoadKind::Major => [98, 98, 104, 255],
        RoadKind::Street => [84, 84, 90, 255],
        RoadKind::Service => [74, 74, 78, 255],
        RoadKind::Path => [150, 128, 96, 255],
    }
}

fn scale_rgb(color: [u8; 4], by: f32) -> [u8; 4] {
    let scaled = |channel: u8| (f32::from(channel) * by).round().clamp(0.0, 255.0) as u8;
    let [red, green, blue, alpha] = color;
    [scaled(red), scaled(green), scaled(blue), alpha]
}

#[allow(clippy::many_single_char_names)]
/// Paint a road's carriageway onto `data` (`width` x `height` RGBA), segment by segment.
fn paint_road(data: &mut [u8], width: usize, height: usize, road: &RoadLine) {
    let half = (road.width / 2.0).max(0.5);
    let color = road_color(road.kind);
    for pair in road.points.windows(2) {
        let ([ax, ay], [bx, by]) = (pair[0], pair[1]);
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        let lo_x = (ax.min(bx) - half).floor().max(0.0) as usize;
        let hi_x = ((ax.max(bx) + half).ceil().max(0.0) as usize).min(width.saturating_sub(1));
        let lo_y = (ay.min(by) - half).floor().max(0.0) as usize;
        let hi_y = ((ay.max(by) + half).ceil().max(0.0) as usize).min(height.saturating_sub(1));
        for row in lo_y..=hi_y {
            for col in lo_x..=hi_x {
                let (px, py) = (col as f32, row as f32);
                let t = if len2 > 0.0 {
                    (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (cx, cy) = (ax + t * dx - px, ay + t * dy - py);
                if cx * cx + cy * cy <= half * half {
                    let at = (row * width + col) * 4;
                    if data[at + 3] != 0 {
                        data[at..at + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
}

/// The base map as RGBA pixels, one per tile: the minimap's colours, fully opaque, lower
/// ground darker and higher lighter, roads on top. Returns the pixels, width and height.
pub fn world_pixels(base: &MapFile) -> (Vec<u8>, usize, usize) {
    let height = base.tiles.len();
    let width = base.tiles.first().map_or(0, Vec::len);
    let (lowest, highest) = base
        .elevation
        .iter()
        .flatten()
        .fold((u16::MAX, 0u16), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    let span = f32::from(highest.saturating_sub(lowest));
    let mut data = Vec::with_capacity(width * height * 4);
    for (row, tiles) in base.tiles.iter().enumerate() {
        for (col, &tile) in tiles.iter().enumerate() {
            let mut color = crate::ui::minimap::tile_color(tile);
            if color[3] != 0 {
                color[3] = 255;
                if span > 0.0 {
                    let h = base
                        .elevation
                        .get(row / ELEVATION_STEP)
                        .and_then(|r| r.get(col / ELEVATION_STEP))
                        .copied()
                        .unwrap_or(lowest);
                    let at = f32::from(h.saturating_sub(lowest)) / span;
                    color = scale_rgb(color, 0.7 + 0.6 * at);
                }
            }
            data.extend(color);
        }
        data.resize((row + 1) * width * 4, 0);
    }
    for road in base.roads.iter().filter(|r| r.kind != RoadKind::Path) {
        paint_road(&mut data, width, height, road);
    }
    for road in base.roads.iter().filter(|r| r.kind == RoadKind::Path) {
        paint_road(&mut data, width, height, road);
    }
    (data, width, height)
}

pub fn spawn_world(
    mut commands: Commands,
    doc: Res<CampaignDoc>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
) {
    let panel = panel_px(&ui_scale);
    let (w, h) = doc.base_size();
    let window = windows
        .single()
        .map_or(Vec2::new(1280.0, 720.0), |w| Vec2::new(w.width(), w.height()));
    let view = (window - Vec2::new(panel, 0.0)).max(Vec2::splat(100.0));
    let world = Vec2::new(w.max(1) as f32, h.max(1) as f32) * TILE;
    let scale = (world / view).max_element().clamp(ZOOM_MIN, ZOOM_MAX) * 1.05;
    let centre = tile_to_world(w as f32 / 2.0, h as f32 / 2.0);
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.06, 0.07)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scale,
            ..OrthographicProjection::default_2d()
        }),
        CampaignCamera,
        StateMarker,
        // The panel covers the left: shift the view so the map centres in the rest.
        Transform::from_translation((centre - Vec2::new(panel * scale / 2.0, 0.0)).extend(0.0)),
    ));

    let Some(base) = &doc.base else {
        return;
    };
    let (data, width, height) = world_pixels(base);
    if width == 0 || height == 0 {
        return;
    }
    let mut image = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let size = Vec2::new(width as f32, height as f32) * TILE;
    commands.spawn((
        WorldSprite,
        StateMarker,
        Sprite {
            image: images.add(image),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_translation(Vec2::new(size.x / 2.0, -size.y / 2.0).extend(-10.0)),
    ));
}

fn projection_scale(projection: &Projection) -> f32 {
    match projection {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    }
}

/// Where the cursor is on the world, for the tools.
pub fn track_hover(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<CampaignCamera>>,
    ui_scale: Res<UiScale>,
    mut hover: ResMut<Hover>,
) {
    *hover = Hover::default();
    let (Ok(window), Ok((camera, transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x < panel_px(&ui_scale) {
        return;
    }
    if let Ok(world) = camera.viewport_to_world_2d(transform, cursor) {
        hover.world = Some(world);
        hover.tile = Some(world_to_tile(world));
    }
}

/// Wheel / +- zoom about the cursor, right or middle drag and the arrow keys pan.
#[allow(clippy::too_many_arguments)]
pub fn pan_and_zoom(
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    focus: Res<TextFocus>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<CampaignCamera>>,
    hover: Res<Hover>,
    mut drag: ResMut<Drag>,
) {
    let Ok((mut transform, mut projection)) = cameras.single_mut() else {
        return;
    };
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    let mut steps: f32 = 0.0;
    for event in wheel.read() {
        if hover.world.is_none() {
            continue;
        }
        steps += match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 40.0,
        };
    }
    if focus.0.is_none() {
        if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
            steps += 1.0;
        }
        if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
            steps -= 1.0;
        }
    }
    if steps != 0.0 {
        let new_scale = (ortho.scale * 0.85_f32.powf(steps)).clamp(ZOOM_MIN, ZOOM_MAX);
        // Keep the point under the cursor where it is.
        if let Some(world) = hover.world {
            let offset = world - transform.translation.truncate();
            let moved = offset * (new_scale / ortho.scale);
            transform.translation += (offset - moved).extend(0.0);
        }
        ortho.scale = new_scale;
    }

    if focus.0.is_none() {
        let mut dir = Vec2::ZERO;
        for (key, d) in [
            (KeyCode::ArrowLeft, Vec2::NEG_X),
            (KeyCode::ArrowRight, Vec2::X),
            (KeyCode::ArrowUp, Vec2::Y),
            (KeyCode::ArrowDown, Vec2::NEG_Y),
        ] {
            if keys.pressed(key) {
                dir += d;
            }
        }
        transform.translation += (dir * 600.0 * ortho.scale * time.delta_secs()).extend(0.0);
    }

    let Some(cursor) = windows.single().ok().and_then(Window::cursor_position) else {
        return;
    };
    let panning = mouse.any_pressed([MouseButton::Right, MouseButton::Middle]);
    match *drag {
        Drag::Pan(last) if panning => {
            let delta = cursor - last;
            transform.translation += Vec3::new(-delta.x, delta.y, 0.0) * ortho.scale;
            *drag = Drag::Pan(cursor);
        }
        Drag::Pan(_) => *drag = Drag::None,
        Drag::None
            if mouse.any_just_pressed([MouseButton::Right, MouseButton::Middle])
                && hover.world.is_some() =>
        {
            *drag = Drag::Pan(cursor);
        }
        _ => {}
    }
}

/// Which edges of `r` the world point `at` is on, within `tolerance` world units.
fn edges_at(r: Region, at: Vec2, tolerance: f32) -> Edges {
    let left = r.x as f32 * TILE;
    let right = (r.x + r.width) as f32 * TILE;
    let top = -(r.y as f32) * TILE;
    let bottom = -((r.y + r.height) as f32) * TILE;
    let inside_x = at.x > left - tolerance && at.x < right + tolerance;
    let inside_y = at.y < top + tolerance && at.y > bottom - tolerance;
    Edges {
        left: inside_y && (at.x - left).abs() <= tolerance,
        right: inside_y && (at.x - right).abs() <= tolerance,
        top: inside_x && (at.y - top).abs() <= tolerance,
        bottom: inside_x && (at.y - bottom).abs() <= tolerance,
    }
}

/// The level whose region holds `tile`: the selected one if it does, else the smallest.
fn level_at(doc: &CampaignDoc, tile: (i32, i32)) -> Option<usize> {
    if let Some(i) = doc.selected
        && doc.region_of(i).is_some_and(|r| r.contains(tile.0, tile.1))
    {
        return Some(i);
    }
    (0..doc.levels.len())
        .filter_map(|i| doc.region_of(i).map(|r| (i, r)))
        .filter(|(_, r)| r.contains(tile.0, tile.1))
        .min_by_key(|(_, r)| r.width * r.height)
        .map(|(i, _)| i)
}

/// The left mouse button, by tool.
#[allow(clippy::too_many_arguments)]
pub fn use_tool(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    hover: Res<Hover>,
    tool: Res<Tool>,
    cameras: Query<&Projection, With<CampaignCamera>>,
    mut doc: ResMut<CampaignDoc>,
    mut drag: ResMut<Drag>,
    mut focus: ResMut<TextFocus>,
) {
    let scale = cameras.single().map_or(1.0, projection_scale);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    if mouse.just_pressed(MouseButton::Right)
        && let (Tool::Marker(_), Some(i), Some((x, y))) = (*tool, doc.selected, hover.tile)
    {
        doc.remove_marker(i, x, y);
    }

    if mouse.just_pressed(MouseButton::Left)
        && let (Some(tile), Some(world)) = (hover.tile, hover.world)
    {
        focus.0 = None;
        let marker_here = doc.selected_level().is_some_and(|l| {
            l.map.markers.iter().any(|m| (m.x, m.y) == tile)
        });
        *drag = match *tool {
            Tool::Marker(_) if marker_here => Drag::Marker(tile),
            Tool::Marker(kind) => {
                if let Some(i) = doc.selected {
                    doc.place_marker(i, kind, tile.0, tile.1);
                }
                Drag::None
            }
            Tool::Select if shift => Drag::Draw(tile),
            Tool::Select => {
                let selected = doc.selected.and_then(|i| doc.region_of(i).map(|r| (i, r)));
                let edges = selected.map(|(_, r)| edges_at(r, world, (6.0 * scale).max(TILE * 0.6)));
                match (selected, edges) {
                    (Some((_, r)), Some(edges)) if edges.any() => Drag::Resize { edges, from: r },
                    _ => match level_at(&doc, tile) {
                        Some(i) => {
                            doc.select(Some(i));
                            match doc.region_of(i) {
                                Some(from) => Drag::Move { grab: tile, from },
                                None => Drag::None,
                            }
                        }
                        None => Drag::Draw(tile),
                    },
                }
            }
        };
    }

    let held = mouse.pressed(MouseButton::Left);
    let Some(tile) = hover.tile else {
        if !held && !matches!(*drag, Drag::Pan(_)) {
            *drag = Drag::None;
        }
        return;
    };
    match *drag {
        Drag::Move { grab, from } => {
            if let Some(i) = doc.selected {
                let (dx, dy) = (tile.0 - grab.0, tile.1 - grab.1);
                doc.set_region(i, Region::new(from.x + dx, from.y + dy, from.width, from.height));
            }
        }
        Drag::Resize { edges, from } => {
            if let Some(i) = doc.selected {
                let (mut x0, mut y0) = (from.x, from.y);
                let (mut x1, mut y1) = (from.x + from.width - 1, from.y + from.height - 1);
                if edges.left {
                    x0 = tile.0.min(x1);
                }
                if edges.right {
                    x1 = tile.0.max(x0);
                }
                if edges.top {
                    y0 = tile.1.min(y1);
                }
                if edges.bottom {
                    y1 = tile.1.max(y0);
                }
                doc.set_region(i, spanning((x0, y0), (x1, y1)));
            }
        }
        Drag::Marker(from) if from != tile => {
            if let Some(i) = doc.selected {
                doc.move_marker(i, from, tile);
                *drag = Drag::Marker(tile);
            }
        }
        Drag::Draw(start) if !held => {
            let region = spanning(start, tile);
            if region.width >= 3 && region.height >= 3 {
                doc.add_level(region);
            } else {
                doc.select(None);
            }
        }
        _ => {}
    }
    if !held && !matches!(*drag, Drag::Pan(_)) {
        *drag = Drag::None;
    }
}

const REGION: Color = Color::srgba(1.0, 0.85, 0.3, 0.55);
const SELECTED: Color = Color::srgb(1.0, 0.95, 0.4);
const OFF_LEVEL: Color = Color::srgba(1.0, 1.0, 1.0, 0.25);
const DRAWING: Color = Color::srgb(0.5, 1.0, 0.6);
const HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.6);
const BAD: Color = Color::srgb(1.0, 0.15, 0.15);

pub fn marker_color(kind: MarkerKind, slot: usize) -> Color {
    match kind {
        MarkerKind::Player => crate::player::systems::player_ring::slot_color(slot),
        MarkerKind::Spawn => Color::srgb(0.95, 0.25, 0.85),
        MarkerKind::Goal => Color::srgb(0.2, 0.95, 1.0),
    }
}

/// Regions, the selected level's markers and path check, and the drag preview.
pub fn draw(
    mut gizmos: Gizmos,
    doc: Res<CampaignDoc>,
    analysis: Res<Analysis>,
    hover: Res<Hover>,
    drag: Res<Drag>,
    tool: Res<Tool>,
    cameras: Query<&Projection, With<CampaignCamera>>,
) {
    let scale = cameras.single().map_or(1.0, projection_scale);
    let (w, h) = doc.base_size();
    let (centre, size) = region_rect(Region::new(0, 0, w, h));
    gizmos.rect_2d(Isometry2d::from_translation(centre), size, OFF_LEVEL);

    for i in 0..doc.levels.len() {
        let Some(r) = doc.region_of(i) else {
            continue;
        };
        let (centre, size) = region_rect(r);
        if doc.selected == Some(i) {
            for grow in [0.0, 1.5 * scale] {
                gizmos.rect_2d(Isometry2d::from_translation(centre), size + grow, SELECTED);
            }
        } else {
            gizmos.rect_2d(Isometry2d::from_translation(centre), size, REGION);
        }
    }

    if let Some(level) = doc.selected_level() {
        let mut players = 0;
        for m in &level.map.markers {
            let at = tile_to_world(m.x as f32 + 0.5, m.y as f32 + 0.5);
            let slot = if m.kind == MarkerKind::Player {
                players += 1;
                players - 1
            } else {
                0
            };
            let color = marker_color(m.kind, slot);
            let r = (TILE * 0.9).max(5.0 * scale);
            match m.kind {
                MarkerKind::Goal => {
                    gizmos.rect_2d(Isometry2d::from_translation(at), Vec2::splat(r * 2.0), color);
                    gizmos.circle_2d(at, r * 0.6, color);
                }
                MarkerKind::Spawn => {
                    gizmos.circle_2d(at, r, color);
                    gizmos.line_2d(at - Vec2::splat(r * 0.7), at + Vec2::splat(r * 0.7), color);
                    gizmos.line_2d(
                        at + Vec2::new(-r * 0.7, r * 0.7),
                        at + Vec2::new(r * 0.7, -r * 0.7),
                        color,
                    );
                }
                MarkerKind::Player => {
                    gizmos.circle_2d(at, r, color);
                    gizmos.circle_2d(at, r * 0.5, color);
                }
            }
            if analysis.bad.contains(&(m.x, m.y)) {
                gizmos.circle_2d(at, r * 1.6, BAD);
                gizmos.circle_2d(at, r * 1.8, BAD);
            }
        }
        // The goal's path to each spawn that reaches it would be busy; a line from the
        // goal to every spawn shows which ones the check is about.
        if let Some(goal) = level.map.markers.iter().find(|m| m.kind == MarkerKind::Goal) {
            let g = tile_to_world(goal.x as f32 + 0.5, goal.y as f32 + 0.5);
            for s in level.map.markers.iter().filter(|m| m.kind == MarkerKind::Spawn) {
                let at = tile_to_world(s.x as f32 + 0.5, s.y as f32 + 0.5);
                let color = if analysis.bad.contains(&(s.x, s.y)) {
                    BAD
                } else {
                    Color::srgba(0.95, 0.25, 0.85, 0.25)
                };
                gizmos.line_2d(g, at, color);
            }
        }
    }

    if let (Drag::Draw(start), Some(tile)) = (*drag, hover.tile) {
        let (centre, size) = region_rect(spanning(start, tile));
        gizmos.rect_2d(Isometry2d::from_translation(centre), size, DRAWING);
    }
    if let Some((x, y)) = hover.tile {
        let color = match *tool {
            Tool::Marker(kind) => marker_color(kind, 0),
            Tool::Select => HOVER,
        };
        gizmos.rect_2d(
            Isometry2d::from_translation(tile_to_world(x as f32 + 0.5, y as f32 + 0.5)),
            Vec2::splat(TILE),
            color,
        );
    }
}

/// Every level's number and title over its region's corner, kept readable at any zoom.
pub fn draw_labels(
    mut commands: Commands,
    doc: Res<CampaignDoc>,
    labels: Query<Entity, With<RegionLabel>>,
    cameras: Query<&Projection, With<CampaignCamera>>,
    mut sized: Query<&mut Transform, With<RegionLabel>>,
    mut built: Local<u64>,
) {
    let scale = cameras.single().map_or(1.0, projection_scale);
    if *built != doc.revision {
        *built = doc.revision;
        for e in &labels {
            commands.entity(e).despawn();
        }
        for i in 0..doc.levels.len() {
            let Some(r) = doc.region_of(i) else {
                continue;
            };
            let title = doc.file.levels.get(i).map_or("", |l| l.title.as_str());
            let color = if doc.selected == Some(i) {
                SELECTED
            } else {
                REGION.with_alpha(0.9)
            };
            commands.spawn((
                RegionLabel,
                StateMarker,
                Text2d::new(format!("{}. {title}", i + 1)),
                TextFont::default().with_font_size(14.0),
                TextColor(color),
                Anchor::BOTTOM_LEFT,
                Transform::from_translation(tile_to_world(r.x as f32, r.y as f32).extend(5.0))
                    .with_scale(Vec3::splat(scale)),
            ));
        }
        return;
    }
    for mut t in &mut sized {
        t.scale = Vec3::splat(scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{BitFlags, MapFeatures};

    #[test]
    fn a_tile_round_trips_through_the_world() {
        for tile in [(0, 0), (5, 9), (-3, 2)] {
            let world = tile_to_world(tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
            assert_eq!(world_to_tile(world), tile);
        }
    }

    #[test]
    fn the_world_image_has_a_pixel_per_tile_and_its_roads() {
        let grass = BitFlags::from(MapFeatures::Grass).bits();
        let base = MapFile {
            tiles: vec![vec![grass; 12]; 8],
            roads: vec![RoadLine {
                kind: RoadKind::Street,
                width: 2.0,
                points: vec![[0.0, 4.0], [11.0, 4.0]],
            }],
            ..MapFile::default()
        };
        let (data, w, h) = world_pixels(&base);
        assert_eq!((w, h, data.len()), (12, 8, 12 * 8 * 4));
        let pixel = |x: usize, y: usize| &data[(y * w + x) * 4..(y * w + x) * 4 + 4];
        assert_eq!(pixel(6, 4), road_color(RoadKind::Street));
        assert_ne!(pixel(6, 0), road_color(RoadKind::Street));
        assert_eq!(pixel(6, 0)[3], 255);
    }

    #[test]
    fn edges_are_found_near_the_border_only() {
        let r = Region::new(10, 10, 10, 10);
        let left = tile_to_world(10.0, 15.0);
        assert!(edges_at(r, left, 1.0).left);
        assert!(!edges_at(r, left, 1.0).right);
        let middle = tile_to_world(15.0, 15.0);
        assert!(!edges_at(r, middle, 1.0).any());
        let corner = tile_to_world(20.0, 20.0);
        let e = edges_at(r, corner, 1.0);
        assert!(e.right && e.bottom);
    }
}
