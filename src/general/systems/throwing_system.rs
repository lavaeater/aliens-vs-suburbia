use bevy::asset::AssetServer;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, EventWriter, Query, Res, Transform};
use bevy::scene::SceneBundle;
use bevy::time::Time;
use bevy_xpbd_3d::components::{Collider, CollisionLayers, LinearVelocity, Position, RigidBody};
use bevy_xpbd_3d::prelude::Rotation;
use crate::control::components::{ControlCommands, Controller};
use crate::game_state::score_keeper::{GameEvent, GameTrackingEvent};
use crate::general::components::{Ball, CollisionLayer};
use crate::general::components::map_components::CoolDown;
use crate::player::components::Player;

pub fn throwing(
    time_res: Res<Time>,
    mut query: Query<(&Player, &Position, &Rotation, &mut Controller)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut game_ew: EventWriter<GameTrackingEvent>,
) {
    for (player, position, rotation, mut controller) in query.iter_mut() {
        if controller.triggers.contains(&ControlCommands::Throw)  {
            if controller.cool_down(time_res.delta_seconds()) {
                let direction = rotation.mul_vec3(Quat::from_axis_angle(Vec3::X, (2.5f32).to_radians()).mul_vec3(Vec3::new(0.0, 0.0, -1.0)));
                let launch_p = position.0 + direction * 0.5 + Vec3::new(0.0, 0.25, 0.0);
                game_ew.send(GameTrackingEvent::new(player.key.clone(), GameEvent::ShotFired));
                controller.has_thrown = true;
                commands.spawn((
                    Ball::new(player.key.clone()),
                    SceneBundle {
                        scene: asset_server.load("ball_fab.glb#Scene0"),
                        transform: Transform::from_xyz(launch_p.x, launch_p.y, launch_p.z),
                        ..Default::default()
                    },
                    RigidBody::Dynamic,
                    Collider::ball(1.0 / 16.0),
                    LinearVelocity(direction * 12.0),
                    CollisionLayers::new([CollisionLayer::Ball], [CollisionLayer::Impassable, CollisionLayer::Floor, CollisionLayer::Alien, CollisionLayer::Player, CollisionLayer::AlienSpawnPoint, CollisionLayer::AlienGoal]),
                ));
            }
        } else {
            controller.fire_cool_down = 0.0;
        }
    }
}