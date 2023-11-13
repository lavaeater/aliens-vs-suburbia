use bevy::math::Vec3;
use bevy::prelude::{Commands, Entity, Query, Res, With};
use bevy::time::Time;
use bevy_xpbd_3d::components::Position;
use bevy_xpbd_3d::components::Rotation;
use bevy_xpbd_3d::math::Vector2;
use bevy_xpbd_3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bonsai_bt::{Event, UpdateArgs};
use crate::enemy::components::bonsai_ai_components::{AlienBehavior, ApproachPlayer, AttackPlayer, BonsaiTree, BonsaiTreeStatus, CanISeePlayer, Loiter, AlienBrain};
use crate::enemy::components::general::AlienSightShape;
use crate::general::components::Layer;
use crate::player::components::general::{Controller, ControlDirection, ControlRotation, Opposite, Player};

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
                        commands.entity(entity).insert(CanISeePlayer {});
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
                        commands.entity(entity).insert(ApproachPlayer {});
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
                        commands.entity(entity).insert(AttackPlayer {});
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

But we're not gonna do that EITHER, are we, mate?
No, we're gonna raycast and if there isn't a wall in front of us, we're
gonna move forward, if there is a wall in front of us, we're gonna turn
90 degrees and THEN check again, and so on and so forth.

*/

pub fn loiter_system(
    mut alien_query: Query<(&mut BonsaiTreeStatus, &mut Controller, &mut AlienBrain, &Position, &Rotation), With<Loiter>>,
    spatial_query: SpatialQuery,
) {
    for (mut status, mut controller, mut loiter_data, position, rotation) in alien_query.iter_mut() {
        let direction = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
        let hits = spatial_query.cast_ray(
            position.0, // Origin
            direction,// Direction
            1.5, // Maximum time of impact (travel distance)
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Wall]), // Query for players
        );
        controller.rotations.clear();
        controller.directions.clear();
        match hits {
            None => {
                // No wall, just keep moving forward
                controller.directions.insert(ControlDirection::Forward);
                loiter_data.loiter_rotation_direction = loiter_data.loiter_rotation_direction.opposite();
                loiter_data.loiter_turns = 0;
            }
            Some(_) => {
                // Wall, start turning
                controller.rotations.insert(loiter_data.loiter_rotation_direction);
                /*
                Sometimes, change directions!
                 */
                loiter_data.loiter_turns += 1;
                print!("Turns: {}", loiter_data.loiter_turns);
                if loiter_data.loiter_turns > loiter_data.loiter_max_turns {
                    loiter_data.loiter_turns = 0;
                    loiter_data.loiter_rotation_direction = loiter_data.loiter_rotation_direction.opposite();
                }
            }
        }
        status.current_action_status = bonsai_bt::Status::Running;
    }
}

pub fn can_i_see_player_system(
    mut alien_query: Query<(&mut BonsaiTreeStatus, &mut AlienBrain, &AlienSightShape, &Position, &Rotation), With<CanISeePlayer>>,
    spatial_query: SpatialQuery, ) {
    for (mut status, mut alien_brain, sight_shape, position, rotation) in alien_query.iter_mut() {
        let direction = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
        let hits = spatial_query.cast_shape(
            &sight_shape.shape, // Shape to cast
            position.0, // Origin
            sight_shape.rotation, // Rotation of shape
            direction,// Direction
            sight_shape.range, // Maximum time of impact (travel distance)
            true,
            SpatialQueryFilter::new().with_masks([Layer::Player]), // Query for players
        );

        /*
        What do we do know?
        We create some kind of "brain" for this alien, this brain will contain facts about
        the world around it, like types of creates it wants to see and of course features
        of the environment like walls etc. Perhaps it can build a mental model of the world in the
        form of a graph? Yes. It can.
         */

        match hits {
            None => {
                alien_brain.seen_player_entity = None;
                status.current_action_status = bonsai_bt::Status::Failure;
            }
            Some(hit_data) => {
                alien_brain.seen_player_entity = Some(hit_data.entity);
                status.current_action_status = bonsai_bt::Status::Success;
            }
        }
    }
}

pub fn approach_player_system(
    mut alien_query: Query<(
        &mut BonsaiTreeStatus,
        &AlienBrain,
        &mut Controller,
        &Position,
        &Rotation), With<ApproachPlayer>>,
    player_query: Query<&Position, With<Player>>,
) {
    for (mut status, alien_brain, mut controller, alien_position, alien_rotation) in alien_query.iter_mut() {
        match alien_brain.seen_player_entity {
            None => {
                status.current_action_status = bonsai_bt::Status::Failure;
            }
            Some(player_entity) => {
                let alien_direction_vector3 = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));

                let alien_direction_vector2 = Vector2::new(alien_direction_vector3.x, alien_direction_vector3.z);
                let alien_position_vector2 = Vector2::new(alien_position.0.x, alien_position.0.z);
                let player_position = player_query.get(player_entity).unwrap();
                let player_position_vector2 = Vector2::new(
                    player_position.0.x,
                    player_position.0.z
                );
                let alien_to_player_direction = (player_position_vector2 - alien_position_vector2).normalize();
                let angle = alien_direction_vector2.angle_between(alien_to_player_direction).to_degrees();
                controller.rotations.clear();
                if angle.abs() < 15.0 {
                    controller.directions.insert(ControlDirection::Forward);
                } else if angle > 0.0 {
                    controller.rotations.insert(ControlRotation::Right);
                } else {
                    controller.rotations.insert(ControlRotation::Left);
                }
                let distance = (player_position_vector2 - alien_position_vector2).length();
                if distance < 0.0 {
                    status.current_action_status = bonsai_bt::Status::Success;
                } else {
                    status.current_action_status = bonsai_bt::Status::Running;
                }
            }
        }
    }
}

pub fn attack_player_system() {

}