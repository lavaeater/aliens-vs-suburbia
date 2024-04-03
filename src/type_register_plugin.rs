use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::camera::camera_components::CameraOffset;
use crate::control::components::{
    CharacterControl, CharacterState, ControllerFlag, DynamicMovement, InputKeyboard,
};
use crate::game_state::score_keeper::Score;
use crate::general::components::map_components::CurrentTile;
use crate::general::components::{CollisionLayer, Health};
use crate::player::components::{AutoAim, Player};
use bevy::app::{App, Plugin};
use bevy_xpbd_3d::components::{CollisionLayers, LockedAxes, Position};
// use space_editor::prelude::{EditorRegistryExt};

pub struct TypeRegisterPlugin;

impl Plugin for TypeRegisterPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CollisionLayer>()
            .register_type::<CameraOffset>()
            .register_type::<ControllerFlag>()
            // .editor_registry::<Player>()
            // .editor_registry::<InputKeyboard>()
            // .editor_registry::<CharacterControl>()
            // .editor_registry::<DynamicMovement>()
            // .editor_registry::<LockedAxes>()
            // .editor_registry::<CollisionLayers>()
            // .editor_registry::<Health>()
            // .editor_registry::<CurrentTile>()
            .register_type::<(usize, usize)>()
            .register_type::<AnimationKey>()
            .register_type::<Vec<AnimationKey>>()
            // .editor_registry::<CurrentAnimationKey>()
            // .editor_registry::<CharacterState>()
            // .editor_registry::<Score>()
            // .editor_registry::<AutoAim>()
            // .editor_registry::<Position>();
        ;
    }
}
