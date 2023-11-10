use bevy::asset::AssetServer;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Commands, Query, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{Collider, LinearVelocity, Position, RigidBody};
use bevy_xpbd_3d::prelude::Rotation;
use crate::general::components::Ball;
use crate::player::components::general::{Controller, Triggers};

pub fn throwing(
    mut query: Query<(&Position, &Rotation, &mut Controller)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    for (position, rotation, mut controller) in query.iter_mut() {
        if controller.triggers.contains(&Triggers::Throw) {// && !controller.has_thrown {

            let direction = rotation.mul_vec3(Quat::from_axis_angle(Vec3::X, (10.0f32).to_radians()).mul_vec3(Vec3::new(0.0, 0.0, -1.0)));

            let launch_p = position.0 + direction * 0.25 + Vec3::new(0.0, 0.5, 0.0);

            controller.has_thrown = true;
            commands.spawn((
                Ball {},
                SceneBundle {
                    scene: asset_server.load("ball_fab.glb#Scene0"),
                    transform: Transform::from_xyz(launch_p.x, launch_p.y, launch_p.z),
                    ..Default::default()
                },
                RigidBody::Dynamic,
                Collider::ball(1.0 / 16.0),
                LinearVelocity(direction * 8.0),
            ));
        }
        // if controller.has_thrown && !controller.triggers.contains(&Triggers::Throw) {
        //     controller.has_thrown = false;
        // }
    }
}