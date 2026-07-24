use bevy::math::Vec3;
use bevy::prelude::{MessageWriter, Query, Res, Time, With, Without};
use avian3d::prelude::{CollidingEntities, Position};
use crate::general::components::{Health, TouchDamage};
use crate::gore::components::{DamageDealt, DamageKind};
use crate::player::components::Player;
use crate::player::components::PlayerDead;

pub fn touch_damage_system(
    time: Res<Time>,
    damagers: Query<(&CollidingEntities, &TouchDamage, Option<&Position>)>,
    mut players: Query<(&mut Health, Option<&Position>), (With<Player>, Without<PlayerDead>)>,
    mut damage_mw: MessageWriter<DamageDealt>,
) {
    let dt = time.delta_secs();
    for (colliding, touch, damager_pos) in damagers.iter() {
        for &hit in colliding.iter() {
            if let Ok((mut health, player_pos)) = players.get_mut(hit) {
                let amount = (touch.dps * dt) as i32;
                if amount <= 0 {
                    continue;
                }
                health.health -= amount;
                // Blood sprays off the player, away from the thing mauling them.
                let ppos = player_pos.map(|p| p.0).unwrap_or(Vec3::ZERO);
                let apos = damager_pos.map(|p| p.0).unwrap_or(ppos);
                damage_mw.write(DamageDealt {
                    target: hit,
                    position: ppos + Vec3::Y * 0.4,
                    normal: (ppos - apos).normalize_or(Vec3::Y),
                    amount,
                    kind: DamageKind::Blunt,
                    lethal: health.health <= 0,
                });
            }
        }
    }
}
