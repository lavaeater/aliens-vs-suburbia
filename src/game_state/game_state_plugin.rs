use bevy::app::{App, Plugin, PreUpdate, Update};
use bevy::prelude::{in_state, IntoSystemConfigs, OnEnter, Time};
use bevy::time::Fixed;
use crate::ai::ai_plugin::StatefulAiPlugin;
use crate::alien::alien_plugin::StatefulAlienPlugin;
use crate::animation::animation_plugin::AnimationPlugin;
use crate::assets::assets_plugin::AssetsPlugin;
use crate::building::build_mode_plugin::StatefulBuildModePlugin;
use crate::camera::camera_plugin::StatefulCameraPlugin;
use crate::control::control_plugin::StatefulControlPlugin;
use crate::control::gamepad_input::GamepadPlugin;
use crate::game_state::clear_game_entities_plugin::ClearGameEntitiesPlugin;
use crate::game_state::GameState;
use crate::game_state::score_keeper::ScoreKeeperPlugin;
use crate::general::systems::collision_handling_system::collision_handling_system;
use crate::general::systems::health_monitor_system::health_monitor_system;
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::throwing_system::throwing;
use crate::map::map_plugins::StatefulMapPlugin;
use crate::player::player_plugin::PlayerPlugin;
use crate::towers::systems::{shoot_alien_system, tower_has_alien_in_range_scorer_system};
use crate::ui::spawn_ui::{add_health_bar, AddHealthBar, fellow_system, GotoState};
use crate::ui::ui_plugin::UiPlugin;
use crate::generate_mesh::MeshPlugin;
use crate::inspection::inspector::InspectorPlugin;
use crate::playground::PlaygroundPlugin;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(Time::<Fixed>::from_seconds(0.05))
            .add_state::<GameState>()
            .add_event::<GotoState>()
            .add_event::<AddHealthBar>()
            .add_plugins((
                AssetsPlugin,
                StatefulMapPlugin,
                UiPlugin,
                StatefulAiPlugin,
                StatefulBuildModePlugin,
                StatefulAlienPlugin,
                AnimationPlugin,
                StatefulControlPlugin,
                StatefulCameraPlugin,
                ClearGameEntitiesPlugin,
                PlayerPlugin::default(),
                ScoreKeeperPlugin,
                // VideoGlitchPlugin,
                GamepadPlugin,
                InspectorPlugin,
            ))
            .add_plugins((
                MeshPlugin,
                PlaygroundPlugin
            ))
            .add_systems(
                OnEnter(GameState::InGame),
                (
                    spawn_lights,
                ))
            .add_systems(
                Update,
                (
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