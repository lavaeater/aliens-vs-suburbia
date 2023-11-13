use std::cmp::{max, min};
use std::f32::consts::FRAC_PI_2;
use bevy::prelude::*;
use bevy::utils::tracing::{debug, trace};
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::prelude::{RayHitData, SpatialQuery, SpatialQueryFilter};
use big_brain::prelude::*;
use crate::general::components::Layer;

#[derive(Clone, Component, Debug)]
pub struct AvoidWallData {
    pub forward_distance: f32,
    pub left_distance: f32,
    pub right_distance: f32,
    pub max_distance: f32,
}

impl AvoidWallData {
    pub fn new(total_distance: f32, max_distance: f32) -> Self {
        Self { forward_distance: total_distance, left_distance: 0.0, right_distance: 0.0, max_distance }
    }
}

// Scorers are the same as in the thirst example.
#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct AvoidWallScore;

pub fn avoid_walls_scorer_system(
    avoid_wall_data_query: Query<&AvoidWallData>,
    mut query: Query<(&Actor, &mut Score), With<AvoidWallScore>>,
) {
    for (Actor(actor), mut score) in &mut query {
        if let Ok(avoid_wall_data) = avoid_wall_data_query.get(*actor) {
            score.set((avoid_wall_data.forward_distance.min(avoid_wall_data.max_distance) / avoid_wall_data.max_distance).recip());
        }
    }
}

pub fn avoid_wall_data_system(
    time: Res<Time>,
    mut avoid_wall_data_query: Query<(&mut AvoidWallData, &Position, &Rotation)>,
    spatial_query: SpatialQuery,
) {
    for (mut avoid_wall_data, position, rotation) in avoid_wall_data_query.iter_mut() {
        let left_rot = Quat::from_euler(EulerRot::YXZ, 90.0f32.to_radians(), 0.0, 0.0);
        let right_rot = Quat::from_euler(EulerRot::YXZ, -90.0f32.to_radians(), 0.0, 0.0);
        let forward = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0)); //forward
        let left = left_rot.mul_vec3(forward.clone()); //left
        let right = right_rot.mul_vec3(forward.clone()); //right

        avoid_wall_data.forward_distance = 0.0;
        avoid_wall_data.left_distance = 0.0;
        avoid_wall_data.right_distance = 0.0;

        match spatial_query.cast_ray(
            position.0, // Origin
            forward,// Direction
            avoid_wall_data.max_distance, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        ) {
            None => {}
            Some(hit) => {
                avoid_wall_data.forward_distance = hit.time_of_impact;
            }
        };

        match spatial_query.cast_ray(
            position.0, // Origin
            left,// Direction
            avoid_wall_data.max_distance, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        ) {
            None => {}
            Some(hit) => {
                avoid_wall_data.left_distance = hit.time_of_impact;
            }
        };

        match spatial_query.cast_ray(
            position.0, // Origin
            right,// Direction
            avoid_wall_data.max_distance, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        ) {
            None => {}
            Some(hit) => {
                avoid_wall_data.right_distance = hit.time_of_impact;
            }
        };
    }
}

fn move_to_water_source_action_system(
    time: Res<Time>,
    // Find all water sources
    waters: Query<&Position, With<WaterSource>>,
    // We use Without to make disjoint queries.
    mut positions: Query<&mut Position, Without<WaterSource>>,
    // A query on all current MoveToWaterSource actions.
    mut action_query: Query<(&Actor, &mut ActionState, &MoveToWaterSource, &ActionSpan)>,
) {
    // Loop through all actions, just like you'd loop over all entities in any other query.
    for (actor, mut action_state, move_to, span) in &mut action_query {
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
                let mut actor_position = positions.get_mut(actor.0).expect("actor has no position");

                trace!("Actor position: {:?}", actor_position.position);

                // Look up the water source closest to them.
                let closest_water_source = find_closest_water_source(&waters, &actor_position);

                // Find how far we are from it.
                let delta = closest_water_source.position - actor_position.position;

                let distance = delta.length();

                trace!("Distance: {}", distance);

                if distance > MAX_DISTANCE {
                    // We're still too far, take a step toward it!

                    trace!("Stepping closer.");

                    // How far can we travel during this frame?
                    let step_size = time.delta_seconds() * move_to.speed;
                    // Travel towards the water-source position, but make sure to not overstep it.
                    let step = delta.normalize() * step_size.min(distance);

                    // Move the actor.
                    actor_position.position += step;
                } else {
                    // We're within the required distance! We can declare success.

                    debug!("We got there!");

                    // The action will be cleaned up automatically.
                    *action_state = ActionState::Success;
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

/// A simple action: the actor's thirst shall decrease, but only if they are near a water source.
#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct Drink {
    per_second: f32,
}

fn drink_action_system(
    time: Res<Time>,
    mut thirsts: Query<(&Position, &mut Hunger), Without<WaterSource>>,
    waters: Query<&Position, With<WaterSource>>,
    mut query: Query<(&Actor, &mut ActionState, &Drink, &ActionSpan)>,
) {
    // Loop through all actions, just like you'd loop over all entities in any other query.
    for (Actor(actor), mut state, drink, span) in &mut query {
        let _guard = span.span().enter();

        // Look up the actor's position and thirst from the Actor component in the action entity.
        let (actor_position, mut thirst) = thirsts.get_mut(*actor).expect("actor has no thirst");

        match *state {
            ActionState::Requested => {
                // We'll start drinking as soon as we're requested to do so.
                debug!("Drinking the water.");
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                // Look up the closest water source.
                // Note that there is no explicit passing of a selected water source from the GoToWaterSource action,
                // so we look it up again. Note that this decouples the actions from each other,
                // so if the actor is already close to a water source, the GoToWaterSource action
                // will not be necessary (though it will not harm either).
                //
                // Essentially, being close to a water source would be a precondition for the Drink action.
                // How this precondition was fulfilled is not this code's concern.
                let closest_water_source = find_closest_water_source(&waters, actor_position);

                // Find how far we are from it.
                let distance = (closest_water_source.position - actor_position.position).length();

                // Are we close enough?
                if distance < MAX_DISTANCE {
                    trace!("Drinking!");

                    // Start reducing the thirst. Alternatively, you could send out some kind of
                    // DrinkFromSource event that indirectly decreases thirst.
                    thirst.hunger -= drink.per_second * time.delta_seconds();

                    // Once we hit 0 thirst, we stop drinking and report success.
                    if thirst.hunger <= 0.0 {
                        thirst.hunger = 0.0;
                        *state = ActionState::Success;
                    }
                } else {
                    // The actor was told to drink, but they can't drink when they're so far away!
                    // The action doesn't know how to deal with this case, it's the overarching system's
                    // to fulfill the precondition.
                    debug!("We're too far away!");
                    *state = ActionState::Failure;
                }
            }
            // All Actions should make sure to handle cancellations!
            // Drinking is not a complicated action, so we can just interrupt it immediately.
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn init_entities(mut cmd: Commands) {

    // We use the Steps struct to essentially build a "MoveAndDrink" action by composing
    // the MoveToWaterSource and Drink actions.
    //
    // If either of the steps fails, the whole action fails. That is: if the actor somehow fails
    // to move to the water source (which is not possible in our case) they will not attempt to
    // drink either. Getting them un-stuck from that situation is then up to other possible actions.
    //
    // We build up a list of steps that make it so that the actor will...
    let move_and_drink = Steps::build()
        .label("MoveAndDrink")
        // ...move to the water source...
        .step(MoveToWaterSource { speed: 1.0 })
        // ...and then drink.
        .step(Drink { per_second: 10.0 });

    // Build the thinker
    let thinker = Thinker::build()
        .label("ThirstyThinker")
        // We don't do anything unless we're thirsty enough.
        .picker(FirstToScore { threshold: 0.8 })
        .when(AvoidWallScore, move_and_drink);

    cmd.spawn((
        Hunger::new(75.0, 2.0),
        Position {
            position: Vec2::new(0.0, 0.0),
        },
        thinker,
    ));
}
