use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Res};
use bevy::scene::SceneBundle;
use bevy_xpbd_3d::prelude::{Collider, Position, RigidBody};

pub fn spawn_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Name::from("Floor Fab 1"),
        SceneBundle {
            scene: asset_server.load("floor_fab.glb#Scene0"),
            ..Default::default()
        },
        RigidBody::Static,
        Collider::cuboid(0.45, 0.45, 0.3),
        Position::from(Vec3::new(0.0, -10.0, 0.0)),
    ));
}