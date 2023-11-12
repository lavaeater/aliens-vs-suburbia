use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::{EulerRot, Quat};
use bevy::prelude::{Commands, Res, Transform};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::components::{AngularDamping, Collider, CollisionLayers, Friction, LinearDamping, LockedAxes, RigidBody};
use crate::enemy::components::general::{Alien, AlienSightShape};
use crate::general::components::{HittableTarget, Layer};
use crate::player::components::general::{DynamicMovement};

pub fn spawn_aliens(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Name::from("Spider"),
        Alien {},
        // We rotat the cone since it is defined as a cone pointing up the y axis. Rotating it -90 degrees around the x axis makes it point forward properly. Maybe.
        AlienSightShape(Collider::cone(5.0, 0.5), Quat::from_euler(EulerRot::YXZ, 0.0, -90.0, 0.0)),
        HittableTarget {},
        DynamicMovement {},
        SceneBundle {
            scene: asset_server.load("player.glb#Scene0"),
            transform: Transform::from_xyz(2.0, 0.0, 2.0),
            ..Default::default()
        },
        Friction::from(0.0),
        AngularDamping(1.0),
        LinearDamping(0.9),
        RigidBody::Dynamic,
        //AsyncCollider(ComputedCollider::ConvexHull),
        Collider::cuboid(0.5, 0.5, 0.45),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
        CollisionLayers::new([Layer::Alien], [Layer::Ball, Layer::Wall, Layer::Floor, Layer::Alien, Layer::Player]),
    ));
}