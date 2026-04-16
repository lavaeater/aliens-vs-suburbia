use bevy::prelude::*;
use avian3d::prelude::{Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::math::Vec2;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::general::components::{Attack, CollisionLayer, Health};
use crate::alien::components::general::{Alien, AlienSightShape};
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation};
use crate::player::components::Player;

pub fn can_agent_see_player_system(
    mut approach_player_query: Query<(&mut ApproachAndAttackPlayerData, &AlienSightShape, &Position, &Rotation)>,
    spatial_query: SpatialQuery,
) {
    for (mut alien_brain, sight_shape, position, rotation) in approach_player_query.iter_mut() {
        let direction = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));

        match spatial_query.cast_shape(
            &sight_shape.shape,
            position.0,
            sight_shape.rotation,
            Dir3::new(direction).unwrap_or(Dir3::NEG_Z),
            &avian3d::prelude::ShapeCastConfig {
                max_distance: sight_shape.range,
                ..default()
            },
            &SpatialQueryFilter::from_mask([CollisionLayer::Player]),
        ) {
            None => {
                alien_brain.seen_player = None;
            }
            Some(hit_data) => {
                alien_brain.seen_player = Some(hit_data.entity);
            }
        }
    }
}

pub fn approach_player_system(
    mut alien_query: Query<(&ApproachAndAttackPlayerData, &mut CharacterControl, &Position, &Rotation), With<Alien>>,
    player_query: Query<&Position, With<Player>>,
) {
    for (approach_player_data, mut controller, alien_position, alien_rotation) in alien_query.iter_mut() {
        if let Some(player_entity) = approach_player_data.seen_player {
            let alien_direction_vector3 = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
            let alien_direction_vector2 = Vec2::new(alien_direction_vector3.x, alien_direction_vector3.z);
            let alien_position_vector2 = Vec2::new(alien_position.0.x, alien_position.0.z);
            if let Ok(player_position) = player_query.get(player_entity) {
                let player_position_vector2 = Vec2::new(player_position.0.x, player_position.0.z);
                let alien_to_player_direction = (player_position_vector2 - alien_position_vector2).normalize();
                let angle = alien_direction_vector2.angle_to(alien_to_player_direction).to_degrees();
                controller.rotations.clear();
                if angle.abs() < 15.0 {
                    controller.directions.insert(ControlDirection::Forward);
                } else if angle > 0.0 {
                    controller.rotations.insert(ControlRotation::Right);
                } else {
                    controller.rotations.insert(ControlRotation::Left);
                }
            }
        }
    }
}

pub fn attack_player_system(
    mut alien_query: Query<(&ApproachAndAttackPlayerData, &mut CharacterControl, &Position, &Attack), With<Alien>>,
    mut player_query: Query<(&mut Health, &Position), With<Player>>,
) {
    for (attack_player_data, mut controller, alien_position, alien_attack) in alien_query.iter_mut() {
        if let Some(player_entity) = attack_player_data.seen_player {
            let alien_position_vector2 = Vec2::new(alien_position.0.x, alien_position.0.z);
            if let Ok((mut player_health, player_position)) = player_query.get_mut(player_entity) {
                let player_position_vector2 = Vec2::new(player_position.0.x, player_position.0.z);
                controller.rotations.clear();
                let distance = (player_position_vector2 - alien_position_vector2).length();
                if distance < attack_player_data.attack_distance * 2.0 {
                    player_health.health -= alien_attack.damage_range;
                }
            }
        }
    }
}
