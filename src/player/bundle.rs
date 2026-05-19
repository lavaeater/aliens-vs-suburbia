use bevy::math::Vec3;
use bevy::prelude::{Bundle, Name};
use avian3d::prelude::{AngularDamping, CollisionLayers, Friction, LayerMask, LinearDamping, LockedAxes, RigidBody};
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::control::components::{CharacterControl, DynamicMovement, InputKeyboard};
use crate::control::components::CharacterState;
use crate::game_state::score_keeper::Score;
use crate::general::components::Health;
use crate::general::systems::coin_system::PickupRange;
use crate::player::systems::abilities::{AbilityCooldown, SpecialAbility};
use crate::general::components::map_components::CurrentTile;
use crate::player::components::{AutoAim, Player};

#[derive(Bundle)]
pub struct PlayerBundle {
    name: Name,
    player: Player,
    input: InputKeyboard,
    character_controller: CharacterControl,
    dynamic_movement: DynamicMovement,
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
    auto_aim: AutoAim,
    pickup_range: PickupRange,
    special_ability: SpecialAbility,
    ability_cooldown: AbilityCooldown,
}

impl PlayerBundle {
    pub fn new(
        name: &str,
        groups: impl Into<LayerMask>,
        masks: impl Into<LayerMask>,
    ) -> Self {
        Self {
            name: Name::new(name.to_string()),
            player: Player {},
            input: InputKeyboard,
            character_controller: CharacterControl::new(3.0, 3.0, 60.0),
            dynamic_movement: DynamicMovement,
            friction: Friction::new(0.0),
            angular_damping: AngularDamping(0.0),
            linear_damping: LinearDamping(0.0),
            rigid_body: RigidBody::Dynamic,
            locked_axes: LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            collision_layers: CollisionLayers::new(groups, masks),
            health: Health {
                health: 100,
                max_health: 100,
            },
            current_tile: CurrentTile {
                tile: (0, 0)
            },
            current_animation_key: CurrentAnimationKey::new("players".into(), AnimationKey::Idle),
            character_state: CharacterState::default(),
            score: Score::new(),
            auto_aim: AutoAim(Vec3::Z),
            pickup_range: PickupRange::default(),
            special_ability: SpecialAbility::Bombardment,
            ability_cooldown: AbilityCooldown::new(SpecialAbility::Bombardment.throws_to_charge()),
        }
    }
}
