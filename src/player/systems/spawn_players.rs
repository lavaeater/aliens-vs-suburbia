use bevy::asset::{Assets, AssetServer, LoadState};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::math::Vec3;
use bevy::prelude::{Commands, Mesh, Res, Transform};
use bevy::scene::{SceneBundle};
use bevy_third_person_camera::ThirdPersonCameraTarget;
use bevy_xpbd_3d::prelude::{Collider, RigidBody};
use crate::player::components::general::{FollowCamera, Player};

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Player {},
        ThirdPersonCameraTarget {},
        SceneBundle {
            scene: asset_server.load("player.glb#Scene0"),
            transform: Transform::from_xyz(2.0, 0.0, -5.0),

            ..Default::default()
        },
        RigidBody::Kinematic,
        Collider::cuboid(0.5, 0.5, 0.3)
    ));
}