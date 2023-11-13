use bevy::prelude::{Query, With};
use big_brain::prelude::{ActionSpan, Actor, Score};
use bevy::log::debug;
use big_brain::actions::ActionState;
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::math::{EulerRot, Quat, Vec3};
use crate::ai::components::avoid_wall_components::{AvoidWallsAction, AvoidWallScore, AvoidWallsData};
use crate::general::components::Layer;
use crate::player::components::general::{Controller, ControlRotation};

pub fn avoid_walls_scorer_system(
    avoid_wall_data_query: Query<&AvoidWallsData>,
    mut query: Query<(&Actor, &mut Score), With<AvoidWallScore>>,
) {
    for (Actor(actor), mut score) in &mut query {
        if let Ok(avoid_wall_data) = avoid_wall_data_query.get(*actor) {
            if avoid_wall_data.forward_distance < avoid_wall_data.max_distance {
                score.set(0.9);
            } else {
                score.set(0.0);
            }
            let s =(avoid_wall_data.forward_distance.min(avoid_wall_data.max_distance) / avoid_wall_data.max_distance).recip().clamp(0.0, 1.0);
            debug!("AvoidWallScore: {}", s);
        }
    }
}

pub fn avoid_walls_data_system(
    mut avoid_wall_data_query: Query<(&mut AvoidWallsData, &Position, &Rotation)>,
    spatial_query: SpatialQuery,
) {
    for (mut avoid_wall_data, position, rotation) in avoid_wall_data_query.iter_mut() {
        let left_rot = Quat::from_euler(EulerRot::YXZ, 90.0f32.to_radians(), 0.0, 0.0);
        let right_rot = Quat::from_euler(EulerRot::YXZ, -90.0f32.to_radians(), 0.0, 0.0);
        let forward = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0)); //forward
        let left = left_rot.mul_vec3(forward.clone()); //left
        let right = right_rot.mul_vec3(forward.clone()); //right

        avoid_wall_data.forward_distance = avoid_wall_data.max_distance;
        avoid_wall_data.left_distance = avoid_wall_data.max_distance;
        avoid_wall_data.right_distance = avoid_wall_data.max_distance;

        if let Some(hit) = spatial_query.cast_ray(
            position.0, // Origin
            forward,// Direction
            avoid_wall_data.max_distance, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        ) {
            avoid_wall_data.forward_distance = hit.time_of_impact;
        };

        if let Some(hit) = spatial_query.cast_ray(
            position.0, // Origin
            left,// Direction
            avoid_wall_data.max_distance, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        ) {
            avoid_wall_data.left_distance = hit.time_of_impact;
        };

        if let Some(hit) = spatial_query.cast_ray(
            position.0, // Origin
            right,// Direction
            avoid_wall_data.max_distance, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        ) {
            avoid_wall_data.right_distance = hit.time_of_impact;
        };
    }
}

pub fn avoid_walls_action_system(
    // A query on all current MoveToWaterSource actions.
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<AvoidWallsAction>>,
    mut actor_query: Query<(&mut Controller, &AvoidWallsData)>,
) {
    // Loop through all actions, just like you'd loop over all entities in any other query.
    for (actor, mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();

        // Different behavior depending on action state.
        match *action_state {
            // Action was just requested; it hasn't been seen before.
            ActionState::Requested => {
                debug!("Let's go find some water!");
                // We don't really need any initialization code here, since the queries are cheap enough.
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                // Look up the actor's position.
                if let Ok((mut controller, avoid_walls_data)) = actor_query.get_mut(actor.0) {
                    // let rotation = if avoid_walls_data.right_distance < avoid_walls_data.left_distance
                    // {
                    //     ControlRotation::Right
                    // } else {
                    //     ControlRotation::Left
                    // };

                    controller.rotations.clear();
                    controller.rotations.insert(ControlRotation::Left);
                    controller.directions.clear();
                   // *action_state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                // Always treat cancellations, or we might keep doing this forever!
                // You don't need to terminate immediately, by the way, this is only a flag that
                // the cancellation has been requested. If the actor is balancing on a tightrope,
                // for instance, you may let them walk off before ending the action.
                *action_state = ActionState::Failure;
            }
            _ => {}
        }
    }
}
