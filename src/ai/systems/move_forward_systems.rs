use bevy::prelude::{Query, With};
use big_brain::prelude::{ActionSpan, Actor, Score};
use big_brain::actions::ActionState;
use bevy::log::debug;
use crate::ai::components::move_forward_components::{MoveForwardAction, MoveForwardScore};
use crate::player::components::general::{ControlDirection, Controller};

pub fn move_forward_action_system(
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<MoveForwardAction>>,
    mut controller_query: Query<&mut Controller>,
) {
    for(actor, mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();
        // Different behavior depending on action state.
        match *action_state {
            // Action was just requested; it hasn't been seen before.
            ActionState::Requested => {
                debug!("Let's move forward!");
                // We don't really need any initialization code here, since the queries are cheap enough.
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                // Look up the actor's position.
                if let Ok(mut controller) = controller_query.get_mut(actor.0) {
                    controller.rotations.clear();
                    controller.speed = controller.max_speed;
                    controller.directions.insert(ControlDirection::Forward);
                }
            }
            ActionState::Cancelled => {
                // Always treat cancellations, or we might keep doing this forever!
                // You don't need to terminate immediately, by the way, this is only a flag that
                // the cancellation has been requested. If the actor is balancing on a tightrope,
                // for instance, you may let them walk off before ending the action.
                // if let Ok(mut controller) = controller_query.get_mut(actor.0) {
                //     controller.rotations.clear();
                //     controller.directions.clear();
                //     // *action_state = ActionState::Success;
                // }
                *action_state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_forward_scorer_system(
    mut query: Query<&mut Score, With<MoveForwardScore>>,
) {
    for mut score in query.iter_mut() {
        score.set(0.9);
        debug!("MoveForwardScore: {}", score.get());
    }
}
