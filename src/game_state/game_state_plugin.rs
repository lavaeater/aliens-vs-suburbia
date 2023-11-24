use bevy::app::{App, Plugin, PreUpdate, Update};
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::prelude::{Commands, Entity, in_state, IntoSystemConfigs, OnEnter, OnExit, Query, Without};
use bevy::window::Window;
use crate::ai::ai_plugin::StatefulAiPlugin;
use crate::alien::alien_plugin::StatefulAlienPlugin;
use crate::building::build_mode_plugin::StatefulBuildModePlugin;
use crate::camera::camera_plugin::StatefulCameraPlugin;
use crate::control::control_plugin::StatefulControlPlugin;
use crate::game_state::GameState;
use crate::general::systems::collision_handling_system::collision_handling_system;
use crate::general::systems::health_monitor_system::health_monitor_system;
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::throwing_system::throwing;
use crate::map::map_plugins::StatefulMapPlugin;
use crate::player::systems::spawn_players::spawn_players;
use crate::towers::systems::{shoot_alien_system, tower_has_alien_in_range_scorer_system};
use crate::ui::spawn_ui::{add_health_bar, fellow_system, GotoState};
use crate::ui::ui_plugin::UiPlugin;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_state::<GameState>()
            .add_event::<GotoState>()
            .add_plugins((
                StatefulMapPlugin,
                UiPlugin,
                StatefulAiPlugin,
                StatefulBuildModePlugin,
                StatefulAlienPlugin,
                StatefulControlPlugin,
                StatefulCameraPlugin,
                ClearGameEntitiesPlugin
            ))
            .add_systems(
                OnEnter(GameState::InGame),
                (
                    spawn_lights,
                ))
            .add_systems(
                Update,
                (
                    spawn_players,
                    throwing,
                    collision_handling_system,
                ).run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    shoot_alien_system,
                    health_monitor_system,
                    add_health_bar,
                    fellow_system,
                ).run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PreUpdate,
                (
                    tower_has_alien_in_range_scorer_system,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}

pub struct ClearGameEntitiesPlugin;

impl Plugin for ClearGameEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnExit(GameState::InGame), clear_game_entities);
    }
}

pub fn clear_game_entities(
    mut commands: Commands,
    query: Query<Entity, Without<Window>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}