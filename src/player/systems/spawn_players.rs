use bevy::asset::{Assets, AssetServer, LoadState};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::math::Vec3;
use bevy::prelude::{Commands, Mesh, Res, Transform};
use bevy::scene::{SceneBundle};
use bevy_xpbd_3d::prelude::{Collider, RigidBody};
use crate::player::components::general::{FollowCamera, Player};

pub fn spawn_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Player {},
        FollowCamera {
            offset: Vec3::new(5.0, 5.0, 5.0)
        },
        SceneBundle {
            scene: asset_server.load("player.glb#Scene0"),
            transform: Transform::from_xyz(2.0, 0.0, -5.0),

            ..Default::default()
        },
        RigidBody::Dynamic,
        Collider::cuboid(0.5, 0.5, 0.3)
    ));
}