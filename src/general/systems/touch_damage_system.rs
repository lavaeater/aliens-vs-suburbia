use bevy::prelude::{Query, Res, Time, With, Without};
use avian3d::prelude::CollidingEntities;
use crate::general::components::{Health, TouchDamage};
use crate::player::components::Player;
use crate::player::components::PlayerDead;

pub fn touch_damage_system(
    time: Res<Time>,
    damagers: Query<(&CollidingEntities, &TouchDamage)>,
    mut players: Query<&mut Health, (With<Player>, Without<PlayerDead>)>,
) {
    let dt = time.delta_secs();
    for (colliding, touch) in damagers.iter() {
        for &hit in colliding.iter() {
            if let Ok(mut health) = players.get_mut(hit) {
                health.health -= (touch.dps * dt) as i32;
            }
        }
    }
}
