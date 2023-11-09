use bevy::prelude::{Query, Transform, With};
use bevy_xpbd_3d::prelude::Position;
use crate::camera::components::camera::{CameraOffset, GameCamera};
use crate::player::components::general::Player;

pub fn camera_follow(
    mut camera_query: Query<(&mut Transform, &CameraOffset), With<GameCamera>>,
    player_position: Query<&Position, With<Player>>,
) {
for (mut camera_transform, offset) in camera_query.iter_mut() {
        for player_position in player_position.iter() {
            camera_transform.translation = player_position.0 + offset.0;
        }
    }
}