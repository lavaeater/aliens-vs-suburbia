use bevy::asset::AssetServer;
use bevy::prelude::{Commands, Query, Res};
use bevy_xpbd_3d::components::LinearVelocity;
use bevy_xpbd_3d::prelude::Rotation;
use crate::player::components::general::{Controller, Triggers};

pub fn throwing(
    query: Query<(&LinearVelocity, &Rotation, &Controller)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    for (linear_velocity, rotation, controller) in query.iter() {
        if controller.triggers.contains(&Triggers::Throw) {
            commands.spawn((
                Name::from("Ball"),
                Ball {},
                SceneBundle {
                    scene: asset_server.load("ball.glb#Scene0"),
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    ..Default::default()
                },
                Friction::from(0.0),
                AngularDamping(1.0),
                LinearDamping(0.9),
                RigidBody::Dynamic,
                Collider::cuboid(0.5, 0.5, 0.45),
            ));
        }
    }
}