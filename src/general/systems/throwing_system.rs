use bevy::math::{Vec3};
use bevy::prelude::{Commands, Entity, EventWriter, Query, Res, Transform};
use bevy::scene::SceneBundle;
use bevy::time::Time;
use bevy_xpbd_3d::components::{Collider, CollisionLayers, LinearVelocity, Position, RigidBody};
use crate::assets::assets_plugin::GameAssets;
use crate::control::components::{ControlCommands, CharacterControl};
use crate::game_state::score_keeper::{GameTrackingEvent};
use crate::general::components::{Ball, CollisionLayer};
use crate::general::components::map_components::CoolDown;
use crate::player::components::{AutoAim, Player};

pub fn throwing(
    time_res: Res<Time>,
    mut query: Query<(Entity, &Player, &Position, &AutoAim, &mut CharacterControl)>,
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut game_ew: EventWriter<GameTrackingEvent>,
) {
    for (entity, _player, position, auto_aim, mut controller) in query.iter_mut() {
        if controller.triggers.contains(&ControlCommands::Throw) {
            if controller.cool_down(time_res.delta_seconds()) {
                let launch_p = position.0 + auto_aim.0 * 0.5 + Vec3::new(0.0, 0.25, 0.0);
                game_ew.send(GameTrackingEvent::ShotFired(entity));
                controller.has_thrown = true;
                commands.spawn((
                    Ball::new(entity),
                    SceneBundle {
                        scene: game_assets.ball_scene.clone(),
                        transform: Transform::from_xyz(launch_p.x, launch_p.y, launch_p.z),
                        ..Default::default()
                    },
                    RigidBody::Dynamic,
                    Collider::ball(1.0 / 16.0),
                    LinearVelocity(auto_aim.0 * 12.0),
                    CollisionLayers::new([CollisionLayer::Ball],
                                         [
                                             CollisionLayer::Impassable,
                                             CollisionLayer::Floor,
                                             CollisionLayer::Alien,
                                             CollisionLayer::Player,
                                             CollisionLayer::AlienSpawnPoint,
                                             CollisionLayer::AlienGoal
                                         ]),
                ));
            }
        } else {
            controller.fire_cool_down = 0.0;
        }
    }
}