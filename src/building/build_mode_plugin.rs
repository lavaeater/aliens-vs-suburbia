use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs};
use crate::building::systems::{build_tower_system, building_mode, change_build_indicator, enter_build_mode, execute_build, exit_build_mode, init_build_indicator_tint, update_build_indicator_tint};
use crate::game_state::GameState;
use crate::general::systems::map_systems::{add_tile_to_map, remove_tile_from_map};
use crate::player::events::building_events::{AddTile, ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode, RemoveTile};
use crate::towers::events::BuildTower;

pub struct BuildModeEventsPlugin;

impl Plugin for BuildModeEventsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<EnterBuildMode>()
            .add_message::<ExitBuildMode>()
            .add_message::<ExecuteBuild>()
            .add_message::<ChangeBuildIndicator>()
            .add_message::<RemoveTile>()
            .add_message::<AddTile>()
            .add_message::<BuildTower>();
    }
}

pub struct BuildModePlugin;

impl Plugin for BuildModePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(BuildModeEventsPlugin)
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

pub struct StatefulBuildModePlugin;

impl Plugin for StatefulBuildModePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(BuildModeEventsPlugin)
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
                    init_build_indicator_tint,
                    update_build_indicator_tint,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}
