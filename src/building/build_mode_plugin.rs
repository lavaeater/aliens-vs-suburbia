use bevy::app::{App, Plugin, Update};
use crate::building::systems::{build_tower_system, building_mode, change_build_indicator, enter_build_mode, execute_build, exit_build_mode};
use crate::general::systems::map_systems::{add_tile_to_map, remove_tile_from_map};
use crate::player::events::building_events::{AddTile, ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode, RemoveTile};
use crate::towers::events::BuildTower;

pub struct BuildModePlugin;

impl Plugin for BuildModePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EnterBuildMode>()
            .add_event::<ExitBuildMode>()
            .add_event::<ExecuteBuild>()
            .add_event::<ChangeBuildIndicator>()
            .add_event::<RemoveTile>()
            .add_event::<AddTile>()
            .add_event::<BuildTower>()
            .add_systems(
                Update,
                (
                    enter_build_mode,
                    exit_build_mode,
                    building_mode,
                    execute_build,
                    remove_tile_from_map,
                    add_tile_to_map,
                    change_build_indicator,
                    build_tower_system,
                ),
            );
    }
}
