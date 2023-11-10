use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::Quat;
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::general::components::HittableTarget;
use crate::player::components::general::{Controller, DynamicMovement, KeyboardController, Player};

pub fn spawn_aliens(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let mut t = Transform::from_xyz(2.0, 0.0, 2.0);
    t.rotate(Quat::from_rotation_z(100.0f32.to_radians()));
    commands.spawn((
        Name::from("Spider"),
        HittableTarget {},
        DynamicMovement {},
        SceneBundle {
            scene: asset_server.load("aliens/animated_spider.glb#Scene0"),
            transform: t,
            ..Default::default()
        },
        Friction::from(0.0),
        AngularDamping(1.0),
        LinearDamping(0.9),
        RigidBody::Dynamic,
        Collider::cuboid(0.5, 0.5, 0.45),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
    ));
}