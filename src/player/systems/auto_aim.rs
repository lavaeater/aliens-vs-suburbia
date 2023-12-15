use bevy::prelude::{GlobalTransform, Query, With};
use crate::alien::components::general::Alien;
use crate::player::components::{AutoAim, Player};

fn auto_aim(
    mut player_query: Query<(&GlobalTransform, &mut AutoAim), With<Player>>,
    alien_query: Query<&GlobalTransform, With<Alien>>,
) {
    for (player_transform, mut auto_aim) in player_query.iter_mut() {
        let mut closest_alien = None;
        let mut closest_distance = f32::MAX;
        let forward = player_transform.forward();
        for alien_transform in alien_query.iter() {
            let distance = player_transform.translation().distance(alien_transform.translation());
            if distance < closest_distance {
                closest_distance = distance;
                closest_alien = Some(alien_transform.translation());
            }
        }
        if let Some(alien_position) = closest_alien {
            auto_aim.0 = alien_position;
        }
    }
}