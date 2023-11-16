use bevy::asset::AssetServer;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, Query, Res, Transform};
use bevy::scene::SceneBundle;
use bevy::time::Time;
use bevy_xpbd_3d::components::{Collider, CollisionLayers, LinearVelocity, Position, RigidBody};
use bevy_xpbd_3d::prelude::Rotation;
use crate::general::components::{Ball, Layer};
use crate::general::components::map_components::CoolDown;
use crate::player::components::general::{Controller, Triggers};

pub fn throwing(
    time_res: Res<Time>,
    mut query: Query<(&Position, &Rotation, &mut Controller)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for (position, rotation, mut controller) in query.iter_mut() {
        if controller.triggers.contains(&Triggers::Throw) && controller.cool_down(time_res.delta_seconds()) {
            let direction = rotation.mul_vec3(Quat::from_axis_angle(Vec3::X, (2.5f32).to_radians()).mul_vec3(Vec3::new(0.0, 0.0, -1.0)));
            let launch_p = position.0 + direction * 0.5 + Vec3::new(0.0, 0.25, 0.0);

            controller.has_thrown = true;
            commands.spawn((
                Ball::default(),
                SceneBundle {
                    scene: asset_server.load("ball_fab.glb#Scene0"),
                    transform: Transform::from_xyz(launch_p.x, launch_p.y, launch_p.z),
                    ..Default::default()
                },
                RigidBody::Dynamic,
                Collider::ball(1.0 / 16.0),
                LinearVelocity(direction * 12.0),
                CollisionLayers::new([Layer::Ball], [Layer::Wall, Layer::Floor, Layer::Alien, Layer::Player, Layer::AlienSpawnPoint, Layer::AlienGoal]),
            ));
        } else {
            controller.fire_cool_down = 0.0;
        }
    }
}