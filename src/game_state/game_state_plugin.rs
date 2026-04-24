use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs, OnEnter, Time};
use bevy::state::app::AppExtStates;
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
use crate::general::systems::death_effect_system::{spawn_death_effects, tick_death_effects};
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::throwing_system::throwing;
use crate::inspection::inspector::InspectorPlugin;
use crate::map::map_plugins::StatefulMapPlugin;
use crate::player::player_plugin::PlayerPlugin;
use crate::settings::plugin::SettingsPlugin;
use crate::model_settings::plugin::ModelSettingsPlugin;
use crate::towers::systems::shoot_alien_system;
use crate::ui::ui_plugin::UiPlugin;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(Time::<Fixed>::from_seconds(0.05))
            .init_state::<GameState>()
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
                GamepadPlugin,
                InspectorPlugin,
            ))
            .add_plugins((
                SettingsPlugin,
                ModelSettingsPlugin,
            ))
            .add_systems(
                OnEnter(GameState::InGame),
                spawn_lights,
            )
            .add_systems(
                Update,
                (
                    throwing,
                    collision_handling_system,
                    shoot_alien_system,
                    spawn_death_effects.before(health_monitor_system),
                    health_monitor_system,
                    tick_death_effects,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}
