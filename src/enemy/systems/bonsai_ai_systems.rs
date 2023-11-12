use bevy::prelude::{Commands, Entity, Query, Res};
use bevy::time::Time;
use bonsai_bt::{Event, UpdateArgs};
use crate::enemy::components::bonsai_ai_components::{AlienBehavior, ApproachPlayer, AttackPlayer, BonsaiTree, CanISeePlayer, Loiter};

pub fn update_behavior_tree(
    time: Res<Time>,
    mut bt_query: Query<(&mut BonsaiTree, Entity)>,
    mut commands: Commands,
    loiter_query: Query<(&Loiter)>,
    see_player_query: Query<(&CanISeePlayer)>,
    approach_player_query: Query<(&ApproachPlayer)>,
    attack_player_query: Query<(&AttackPlayer)>,
) {
    // proceed to next iteration in event loop

    let dt = time.delta_seconds_f64();
    let e: Event = UpdateArgs { dt }.into();
    for (mut bt, entity) in bt_query.iter_mut() {
        #[rustfmt::skip]
        bt.tree.state.tick(&e, &mut |args: bonsai_bt::ActionArgs<Event, AlienBehavior>| {
            match *args.action {
                AlienBehavior::Loiter => {
                    if loiter_query.contains(entity) {
                        let status = loiter_query.get_component::<Loiter>(entity).unwrap().status;
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<Loiter>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(Loiter { status: bonsai_bt::Status::Running });
                        bonsai_bt::RUNNING
                    }
                }
                AlienBehavior::CanISeePlayer => {
                    if see_player_query.contains(entity) {
                        let status = see_player_query.get_component::<CanISeePlayer>(entity).unwrap().status;
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<CanISeePlayer>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(CanISeePlayer { status: bonsai_bt::Status::Running });
                        bonsai_bt::RUNNING
                    }
                }
                AlienBehavior::ApproachPlayer => {
                    if approach_player_query.contains(entity) {
                        let status = approach_player_query.get_component::<ApproachPlayer>(entity).unwrap().status;
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<ApproachPlayer>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(ApproachPlayer { status: bonsai_bt::Status::Running });
                        bonsai_bt::RUNNING
                    }
                }
                AlienBehavior::AttackPlayer => {
                    if attack_player_query.contains(entity) {
                        let status = attack_player_query.get_component::<AttackPlayer>(entity).unwrap().status;
                        if status == bonsai_bt::Status::Success {
                            commands.entity(entity).remove::<AttackPlayer>();
                        }
                        (status, dt)
                    } else {
                        commands.entity(entity).insert(AttackPlayer { status: bonsai_bt::Status::Running });
                        bonsai_bt::RUNNING
                    }
                }
            }
        });
    }
}