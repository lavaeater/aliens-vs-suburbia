use bevy::prelude::{Query, Res};
use avian3d::prelude::{Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::time::Time;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::control::components::{CharacterControl, ControlRotation};
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::CoolDown;

pub fn avoid_walls_data_system(
    mut avoid_wall_data_query: Query<(&mut AvoidWallsData, &Position, &Rotation)>,
    spatial_query: SpatialQuery,
) {
    for (mut avoid_wall_data, position, rotation) in avoid_wall_data_query.iter_mut() {
        let left_rot = Quat::from_euler(EulerRot::YXZ, 90.0f32.to_radians(), 0.0, 0.0);
        let right_rot = Quat::from_euler(EulerRot::YXZ, -90.0f32.to_radians(), 0.0, 0.0);
        let forward = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
        let left = left_rot.mul_vec3(forward);
        let right = right_rot.mul_vec3(forward);

        avoid_wall_data.forward_distance = avoid_wall_data.max_forward_distance;
        avoid_wall_data.left_distance = avoid_wall_data.max_left_distance;
        avoid_wall_data.right_distance = avoid_wall_data.max_right_distance;

        let filter = SpatialQueryFilter::from_mask([CollisionLayer::Impassable]);

        if let Ok(forward_dir) = bevy::math::Dir3::new(forward) {
            if let Some(hit) = spatial_query.cast_ray(
                position.0,
                forward_dir,
                avoid_wall_data.max_forward_distance,
                true,
                &filter,
            ) {
                avoid_wall_data.forward_distance = hit.distance;
            }
        }

        if let Ok(left_dir) = bevy::math::Dir3::new(left) {
            if let Some(hit) = spatial_query.cast_ray(
                position.0,
                left_dir,
                avoid_wall_data.max_left_distance,
                true,
                &filter,
            ) {
                avoid_wall_data.left_distance = hit.distance;
            }
        }

        if let Ok(right_dir) = bevy::math::Dir3::new(right) {
            if let Some(hit) = spatial_query.cast_ray(
                position.0,
                right_dir,
                avoid_wall_data.max_right_distance,
                true,
                &filter,
            ) {
                avoid_wall_data.right_distance = hit.distance;
            }
        }
    }
}

pub fn avoid_walls_action_system(
    res: Res<Time>,
    mut actor_query: Query<(&mut CharacterControl, &mut AvoidWallsData)>,
) {
    for (mut controller, mut avoid_walls_data) in actor_query.iter_mut() {
        if avoid_walls_data.forward_distance >= avoid_walls_data.max_forward_distance {
            continue; // Not blocking, skip
        }
        if avoid_walls_data.left_distance < avoid_walls_data.max_left_distance {
            avoid_walls_data.rotation_direction = ControlRotation::Right;
        } else if avoid_walls_data.right_distance < avoid_walls_data.max_right_distance {
            avoid_walls_data.rotation_direction = ControlRotation::Left;
        } else {
            avoid_walls_data.cool_down(res.delta_secs());
        }

        controller.rotations.clear();
        controller.rotations.insert(avoid_walls_data.rotation_direction);
        let speed_factor = (avoid_walls_data.forward_distance / avoid_walls_data.max_forward_distance) * 2.0;
        controller.speed = controller.max_speed * speed_factor;
        controller.turn_speed = controller.max_turn_speed;
    }
}
