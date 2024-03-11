use bevy::core::Name;
use bevy::math::Vec3;
use bevy::prelude::{Bundle, Transform};
use bevy_xpbd_3d::components::{AngularVelocity, CollisionLayers, LinearVelocity, LockedAxes};
use bevy_xpbd_3d::prelude::Position;

use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::control::components::{
    CharacterControl, CharacterState, DynamicMovement, InputKeyboard,
};
use crate::game_state::score_keeper::Score;
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::CurrentTile;
use crate::player::components::{AutoAim, Player};

#[derive(Bundle)]
pub struct PlayerPrefab {
    name: Name,
    player: Player,
    input: InputKeyboard,
    character_controller: CharacterControl,
    dynamic_movement: DynamicMovement,
    locked_axes: LockedAxes,
    collision_layers: CollisionLayers,
    health: Health,
    current_tile: CurrentTile,
    current_animation_key: CurrentAnimationKey,
    character_state: CharacterState,
    score: Score,
    auto_aim: AutoAim,
    transform: Transform,
    position: Position,
    linear_velocity: LinearVelocity,
    angular_velocity: AngularVelocity,
}

impl PlayerPrefab {
    pub fn new(name: &str, player_key: &str, transform: Transform) -> Self {
        Self {
            name: Name::new(name.to_string()),
            player: Player {
                key: player_key.into(),
            },
            input: InputKeyboard,
            character_controller: CharacterControl::new(3.0, 3.0, 60.0),
            dynamic_movement: DynamicMovement,
            locked_axes: LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            collision_layers: CollisionLayers::new(
                [CollisionLayer::Player],
                [
                    CollisionLayer::Ball,
                    CollisionLayer::Impassable,
                    CollisionLayer::Floor,
                    CollisionLayer::Alien,
                    CollisionLayer::Player,
                    CollisionLayer::AlienSpawnPoint,
                    CollisionLayer::AlienGoal,
                ],
            ),
            health: Health {
                health: 100,
                max_health: 100,
            },
            current_tile: CurrentTile { tile: (0, 0) },
            current_animation_key: CurrentAnimationKey::new(
                "players".into(),
                AnimationKey::Walking,
            ),
            character_state: CharacterState::default(),
            score: Score::new(),
            auto_aim: AutoAim(Vec3::Z),
            transform,
            position: Position(transform.translation),
            linear_velocity: LinearVelocity::default(),
            angular_velocity: AngularVelocity::default(),
        }
    }
}
