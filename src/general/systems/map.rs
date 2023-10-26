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
    //create a super simple array that is the map.
    let map_array: [[bool; 5];5] = [
        [true, true, false, false, false],
        [false, true, false, false, false],
        [false, true, true, true, true],
        [false, true, true, false, false],
        [false, false, true, false, false]
    ];

    for (y, row) in map_array.iter().enumerate() {
        for (x, column) in row.iter().enumerate() {
            if *column {
                commands.spawn((
                    Name::from(format!("Tile {}:{}", x, y)),
                    SceneBundle {
                        scene: asset_server.load("floor_fab.glb#Scene0"),
                        ..Default::default()
                    },
                    RigidBody::Static,
                    Collider::cuboid(0.45, 0.45, 0.3),
                    Position::from(Vec3::new(0.45 * x as f32, -2.0, 0.45 * y as f32)),
                ));
            }
        }
    }


}