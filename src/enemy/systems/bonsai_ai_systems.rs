use bevy::prelude::{Commands, Entity, Query, Res, With};
use bevy::time::Time;
use bonsai_bt::{Event, UpdateArgs};
use crate::enemy::components::bonsai_ai_components::{AlienBehavior, ApproachPlayer, AttackPlayer, BonsaiTree, BonsaiTreeStatus, CanISeePlayer, Loiter};
use crate::player::components::general::Controller;

pub fn update_behavior_tree(
    time: Res<Time>,
    mut bt_query: Query<(&mut BonsaiTree, &BonsaiTreeStatus, Entity)>,
    mut commands: Commands,
    loiter_query: Query<&Loiter>,
    see_player_query: Query<&CanISeePlayer>,
    approach_player_query: Query<&ApproachPlayer>,
    attack_player_query: Query<&AttackPlayer>,
) {
    // proceed to next iteration in event loop

    let dt = time.delta_seconds_f64();
    let e: Event = UpdateArgs { dt }.into();
    for (mut bt, status, entity) in bt_query.iter_mut() {

        bt.tree.state.tick(&e, &mut |args: bonsai_bt::ActionArgs<Event, AlienBehavior>| {
            let status = status.current_action_status;
            match *args.action {
                AlienBehavior::Loiter => {
                    if loiter_query.contains(entity) {

                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<Loiter>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(Loiter {});
                        bonsai_bt::RUNNING
                    }
                }
                AlienBehavior::CanISeePlayer => {
                    if see_player_query.contains(entity) {
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<CanISeePlayer>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(CanISeePlayer { });
                        bonsai_bt::RUNNING
                    }
                }
                AlienBehavior::ApproachPlayer => {
                    if approach_player_query.contains(entity) {
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<ApproachPlayer>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(ApproachPlayer { });
                        bonsai_bt::RUNNING
                    }
                }
                AlienBehavior::AttackPlayer => {
                    if attack_player_query.contains(entity) {
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<AttackPlayer>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(AttackPlayer { });
                        bonsai_bt::RUNNING
                    }
                }
            }
        });
    }
}

/*
The specific component can and should perhaps contain information
that is specific to this behavior?

Or should we contain that is some kind of cool "alien brain" component?

The loitering system could be made much, much more complex, obviously.

We could have soo many things going on here, but we will start with the
basics...

What is loitering? It is a system that makes the alien move around, avoid
walls, and generally look like it is doing something.

To make it really easy we could have a database of "tiles we have visited"
Or in fact, we should have some A* pathfinding of the current map, that's
way easier to get something running quickly.

*/

pub fn loiter_system(
    mut alien_query: Query<(&mut BonsaiTreeStatus, &mut Controller), With<Loiter>>
) {

}

pub fn can_i_see_player_system() {

}

pub fn approach_player_system() {

}

pub fn attack_player_system() {

}