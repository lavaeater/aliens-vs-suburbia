use bevy::core::Name;
use bevy::prelude::{Bundle, SceneBundle};
use bevy_xpbd_3d::components::{AngularDamping, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::animation::animation_plugin::CurrentAnimationKey;
use crate::control::components::{CharacterControl, DynamicMovement, KeyboardInput};
use crate::control::systems::CharacterState;
use crate::game_state::score_keeper::Score;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use crate::player::components::Player;
use crate::player::systems::spawn_players::FixSceneTransform;

#[derive(Bundle)]
struct PlayerBundle {
    name: Name,
    player: Player,
    fix_scene: Option<FixSceneTransform>,
    input: KeyboardInput,
    character_controller: CharacterControl,
    dynamic_movement: DynamicMovement,
    scene_bundle: SceneBundle,
    friction: Friction,
    angular_damping: AngularDamping,
    linear_damping: LinearDamping,
    rigid_body: RigidBody,
    locked_axes: LockedAxes,
    collision_layers: CollisionLayers,
    health: Health,
    current_tile: CurrentTile,
    current_animation_key: CurrentAnimationKey,
    character_state: CharacterState,
    score: Score,
}

impl PlayerBundle {
    pub fn new(
        name: &str,
        player_key: &str,
        fix_scene: Option<FixSceneTransform>,
        input: KeyboardInput,
        character_controller: CharacterControl,
        dynamic_movement: DynamicMovement,
        scene_bundle: SceneBundle,
        friction: Friction,
        angular_damping: AngularDamping,
        linear_damping: LinearDamping,
        rigid_body: RigidBody,
        locked_axes: LockedAxes,
        collision_layers: CollisionLayers,
        health: Health,
        current_tile: CurrentTile,
        current_animation_key: CurrentAnimationKey,
        character_state: CharacterState,
        score: Score,
    ) -> Self {
        Self {
            name,
            player: player_key,
            fix_scene,
            input,
            character_controller,
            dynamic_movement,
            scene_bundle,
            friction,
            angular_damping,
            linear_damping,
            rigid_body,
            locked_axes,
            collision_layers,
            health,
            current_tile,
            current_animation_key,
            character_state,
            score,
        }
    }
}