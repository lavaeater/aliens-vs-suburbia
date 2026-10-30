//! The minimap, top right in a match: the map's tiles drawn one pixel each (walls dark,
//! streets grey, lawns green), with a dot for everyone on it - players green, neighbours
//! blue, monsters red, spawners a bigger dark red. It is turned with the camera
//! (`GameSettings::yaw_degrees`), so up on the minimap is up on the screen.
//!
//! Rebuilt on every `LoadMap`; the dots are a pool of nodes, moved every frame and
//! hidden when not needed.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::alien::components::general::Alien;
use crate::character_editor::state::in_normal_game;
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::general::events::map_events::LoadMap;
use crate::infestation::{Neighbour, Spawner};
use crate::map::{BitFlags, MapFeatures};
use crate::player::components::Player;
use crate::settings::resources::GameSettings;
use crate::ui::spawn_ui::StateMarker;

/// The longer side of the minimap, in pixels.
const SIZE: f32 = 220.0;
const DOT: f32 = 6.0;
const SPAWNER_DOT: f32 = 10.0;

const PLAYER: Color = Color::srgb(0.2, 0.95, 0.3);
const NEIGHBOUR: Color = Color::srgb(0.3, 0.55, 1.0);
const MONSTER: Color = Color::srgb(1.0, 0.2, 0.2);
const SPAWNER: Color = Color::srgb(0.6, 0.0, 0.1);

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (build_minimap, place_dots)
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(in_normal_game),
        );
    }
}

/// The minimap's frame; turned with the camera.
#[derive(Component)]
pub struct Minimap {
    /// The padded grid's size in tiles (columns, rows).
    tiles: Vec2,
    /// The drawn map's size in pixels.
    pixels: Vec2,
}

/// One dot of the pool.
#[derive(Component)]
pub struct MinimapDot;

/// The colour a tile is drawn in (RGBA).
pub fn tile_color(tile: u64) -> [u8; 4] {
    if tile == 0 {
        return [0, 0, 0, 0];
    }
    let flags = BitFlags::<MapFeatures>::from_bits_truncate(tile);
    if flags.contains(MapFeatures::ImpassableForPlayers)
        || flags.contains(MapFeatures::ImpassableForEnemies)
    {
        return [30, 28, 26, 235];
    }
    if flags.contains(MapFeatures::Grass) {
        [52, 96, 52, 200]
    } else if flags.contains(MapFeatures::Rock) {
        [78, 78, 84, 210]
    } else if flags.contains(MapFeatures::Water) {
        [40, 80, 150, 210]
    } else {
        [120, 116, 108, 200]
    }
}

/// Where a world position falls on the drawn map, in pixels from its top-left (tile
/// `(x, y)` of the padded grid is centred on world `(x, y)`).
pub fn map_pixel(world: Vec3, tiles: Vec2, pixels: Vec2) -> Vec2 {
    (Vec2::new(world.x, world.z) + 0.5) / tiles * pixels
}

/// The minimap's clockwise turn for a camera yaw, so screen-up is up on it: the camera's
/// screen-up is world -Z turned by the yaw (`gamepad_input::stick_to_world`).
pub fn turn_for(yaw_degrees: f32) -> Rot2 {
    Rot2::radians(yaw_degrees.to_radians())
}

fn build_minimap(
    mut loads: MessageReader<LoadMap>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    old: Query<Entity, With<Minimap>>,
) {
    let Some(LoadMap { map }) = loads.read().last() else {
        return;
    };
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let tiles = crate::map::walls::padded_tiles(map);
    let rows = tiles.len();
    let cols = tiles.first().map_or(0, Vec::len);
    if rows == 0 || cols == 0 {
        return;
    }
    let data: Vec<u8> = tiles
        .iter()
        .flatten()
        .flat_map(|&t| tile_color(t))
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: u32::try_from(cols).unwrap_or(1),
            height: u32::try_from(rows).unwrap_or(1),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);

    let scale = SIZE / cols.max(rows) as f32;
    let pixels = Vec2::new(cols as f32 * scale, rows as f32 * scale);
    commands.spawn((
        Name::new("Minimap"),
        StateMarker,
        Minimap {
            tiles: Vec2::new(cols as f32, rows as f32),
            pixels,
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(70.0),
            right: px(16.0),
            width: px(pixels.x),
            height: px(pixels.y),
            border: UiRect::all(px(2.0)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ImageNode::new(handle),
        UiTransform::IDENTITY,
    ));
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn place_dots(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut minimap: Query<(Entity, &Minimap, &mut UiTransform)>,
    mut dots: Query<(&mut Node, &mut BackgroundColor), With<MinimapDot>>,
    players: Query<&GlobalTransform, With<Player>>,
    neighbours: Query<(&GlobalTransform, &Health), With<Neighbour>>,
    monsters: Query<&GlobalTransform, With<Alien>>,
    spawners: Query<&GlobalTransform, With<Spawner>>,
) {
    let Ok((frame, map, mut turn)) = minimap.single_mut() else {
        return;
    };
    turn.rotation = turn_for(settings.yaw_degrees);

    // Drawn back to front: spawners under everything, players on top.
    let mut marks: Vec<(Vec3, Color, f32)> = Vec::new();
    marks.extend(
        spawners
            .iter()
            .map(|g| (g.translation(), SPAWNER, SPAWNER_DOT)),
    );
    marks.extend(monsters.iter().map(|g| (g.translation(), MONSTER, DOT)));
    marks.extend(
        neighbours
            .iter()
            .filter(|(_, h)| h.health > 0)
            .map(|(g, _)| (g.translation(), NEIGHBOUR, DOT)),
    );
    marks.extend(players.iter().map(|g| (g.translation(), PLAYER, DOT + 1.0)));

    let mut pool = dots.iter_mut();
    for &(at, color, size) in &marks {
        let spot = map_pixel(at, map.tiles, map.pixels) - size / 2.0;
        if let Some((mut node, mut background)) = pool.next() {
            node.display = Display::Flex;
            node.left = px(spot.x);
            node.top = px(spot.y);
            node.width = px(size);
            node.height = px(size);
            background.0 = color;
        } else {
            // Shows from the next frame on.
            commands.spawn((
                MinimapDot,
                ChildOf(frame),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(spot.x),
                    top: px(spot.y),
                    width: px(size),
                    height: px(size),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(color),
            ));
        }
    }
    for (mut node, _) in pool {
        node.display = Display::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_centre_is_the_middle_of_its_pixel() {
        let tiles = Vec2::new(10.0, 5.0);
        let pixels = Vec2::new(100.0, 50.0);
        assert_eq!(
            map_pixel(Vec3::new(0.0, 0.0, 0.0), tiles, pixels),
            Vec2::new(5.0, 5.0)
        );
        assert_eq!(
            map_pixel(Vec3::new(9.0, 3.0, 4.0), tiles, pixels),
            Vec2::new(95.0, 45.0)
        );
    }

    /// Screen-up in the world (from the yaw) lands at the top of the turned minimap. In
    /// the UI's y-down space a clockwise turn by `a` takes (x, y) to
    /// (x cos a - y sin a, x sin a + y cos a).
    #[test]
    fn the_minimap_turns_screen_up_to_the_top() {
        for yaw in [0.0f32, 30.0, 90.0, 200.0, 315.0] {
            let up = Quat::from_rotation_y(yaw.to_radians()) * Vec3::NEG_Z;
            let on_map = Vec2::new(up.x, up.z);
            let a = turn_for(yaw).as_radians();
            let turned = Vec2::new(
                on_map.x * a.cos() - on_map.y * a.sin(),
                on_map.x * a.sin() + on_map.y * a.cos(),
            );
            assert!(
                turned.distance(Vec2::new(0.0, -1.0)) < 1e-4,
                "{yaw}: {turned}"
            );
        }
    }

    #[test]
    fn walls_are_dark_and_void_is_clear() {
        assert_eq!(tile_color(0)[3], 0);
        let wall = MapFeatures::Floor as u64 | MapFeatures::ImpassableForPlayers as u64;
        assert!(tile_color(wall)[0] < 50);
    }
}
