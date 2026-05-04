use crate::game_state::GameState;
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};
use bevy::app::{App, Plugin, Update};
use bevy::pbr::{ExtendedMaterial, MeshMaterial3d, StandardMaterial};
use bevy::prelude::{
    Added, Assets, Commands, Component, Entity, IntoScheduleConfigs, Query, Res, ResMut,
    SceneSpawner, Without, in_state,
};
use bevy::scene::SceneInstance;
use bevy_mod_outline::{AutoGenerateOutlineNormalsPlugin, InheritOutline, OutlinePlugin};
use bevy_wind_waker_shader::{WindWakerShader, WindWakerShaderPlugin};

#[derive(Default)]
pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app
              .add_systems(Update, 
                           debug_gizmos.run_if(in_state(GameState::InGame)));
        }
        app.add_plugins((
            OutlinePlugin,
            AutoGenerateOutlineNormalsPlugin::default(),
            WindWakerShaderPlugin::default(),
        ))
        .add_systems(
            Update,
            (
                spawn_players,
                setup_scene_once_loaded,
                fix_scene_transform,
                auto_aim,
                flatten_toon_materials,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

#[derive(Component)]
pub struct OutlineDone;

// The Wind Waker shader clones StandardMaterial verbatim from the GLB, including any metallic /
// roughness values baked in by the exporter. Zero those out so the toon shading isn't buried under
// a specular highlight.
fn flatten_toon_materials(
    new_entities: Query<
        &MeshMaterial3d<ExtendedMaterial<StandardMaterial, WindWakerShader>>,
        Added<MeshMaterial3d<ExtendedMaterial<StandardMaterial, WindWakerShader>>>,
    >,
    mut toon_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, WindWakerShader>>>,
) {
    for handle in new_entities.iter() {
        if let Some(mat) = toon_materials.get_mut(handle) {
            mat.base.metallic = 0.0;
            mat.base.perceptual_roughness = 1.0;
        }
    }
}

fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<(Entity, &SceneInstance), Without<OutlineDone>>,
    scene_manager: Res<SceneSpawner>,
) {
    for (scene_entity, scene) in scene_query.iter() {
        if scene_manager.instance_is_ready(**scene) {
            for entity in scene_manager.iter_instance_entities(**scene) {
                commands.entity(entity).insert(InheritOutline);
            }
            commands.entity(scene_entity).insert(OutlineDone);
        }
    }
}
