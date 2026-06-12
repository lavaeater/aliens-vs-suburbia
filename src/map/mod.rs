pub use enumflags2::BitFlags;
use enumflags2::*;
#[cfg(feature = "map-editor")]
use crossterm::event::KeyCode;

pub mod map_plugins;
pub mod map_generator;

#[bitflags(default = Nothing)]
#[repr(u64)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MapFeatures {
  Nothing, // Clear flags
  Water, // Terrain type indicator
  Grass, // Terrain type indicator
  Floor, // Terrain Type indicator
  Mud, // Terrain type indicator
  Snow, // Terrain type indicator
  Rock, // Terrain type indicator
  ImpassableForPlayers, // Tile addor - not removed for terrain types
  ImpassableForEnemies, // Tile addor - 
  EnemySpawn, // Tile Addor - removes impassableforEnemies
  PlayerSpawn, // Tile Addor - removes impassableForPlayers
  EnemyExit, // Tile Addor - removes impassableFor Enemies
}

#[cfg(feature = "map-editor")]
fn terrain_bits() -> BitFlags<MapFeatures> {
    MapFeatures::Water | MapFeatures::Grass | MapFeatures::Floor
    | MapFeatures::Mud | MapFeatures::Snow | MapFeatures::Rock
}

#[cfg(feature = "map-editor")]
fn set_terrain(current: BitFlags<MapFeatures>, terrain: MapFeatures) -> BitFlags<MapFeatures> {
    (current & !terrain_bits()) | BitFlags::from_flag(terrain)
}

#[cfg(feature = "map-editor")]
fn toggle(current: BitFlags<MapFeatures>, flag: MapFeatures) -> BitFlags<MapFeatures> {
    if current.contains(flag) { current & !BitFlags::from_flag(flag) } else { current | flag }
}

#[cfg(feature = "map-editor")]
pub fn key_to_feature(key_code: &KeyCode, current: BitFlags<MapFeatures>) -> BitFlags<MapFeatures> {
    match key_code {
        // Terrain keys — replace terrain bits, keep addors, apply implied passability
        KeyCode::Char('w') => set_terrain(current, MapFeatures::Water)
            | MapFeatures::ImpassableForPlayers,
        KeyCode::Char('g') => set_terrain(current, MapFeatures::Grass),
        KeyCode::Char('f') => set_terrain(current, MapFeatures::Floor),
        KeyCode::Char('m') => set_terrain(current, MapFeatures::Mud),
        KeyCode::Char('s') => set_terrain(current, MapFeatures::Snow),
        KeyCode::Char('r') => set_terrain(current, MapFeatures::Rock)
            | MapFeatures::ImpassableForPlayers
            | MapFeatures::ImpassableForEnemies,

        // Addor keys — toggle the single bit; some clear conflicting passability
        KeyCode::Char('i') => toggle(current, MapFeatures::ImpassableForPlayers),
        KeyCode::Char('e') => toggle(current, MapFeatures::ImpassableForEnemies),
        KeyCode::Char('p') => toggle(current, MapFeatures::PlayerSpawn)
            & !(BitFlags::from_flag(MapFeatures::ImpassableForPlayers)),
        KeyCode::Char('z') => toggle(current, MapFeatures::EnemySpawn)
            & !(BitFlags::from_flag(MapFeatures::ImpassableForEnemies)),
        KeyCode::Char('x') => toggle(current, MapFeatures::EnemyExit)
            & !(BitFlags::from_flag(MapFeatures::ImpassableForEnemies)),

        // Clear
        KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('0') => BitFlags::default(),

        // Unknown — leave unchanged
        _ => current,
    }
}

