use bevy::math::{Vec3Swizzles};
use bevy::prelude::{Color, Gizmos, GlobalTransform, Query, Res, With};
use bevy::time::Time;
use crate::alien::components::general::Alien;
use crate::control::components::{CharacterControl, ControlCommands};
use crate::player::components::{AutoAim, Player};

pub fn auto_aim(
    mut player_query: Query<(&GlobalTransform, &mut AutoAim, &CharacterControl), With<Player>>,
    alien_query: Query<&GlobalTransform, With<Alien>>,
) {
    for (player_transform, mut auto_aim, character_control) in player_query.iter_mut() {
        if character_control.triggers.contains(&ControlCommands::Throw) {
            let closest =
                alien_query
                    .iter()
                    .filter(|t|
                        player_transform
                            .forward()
                            .xz()
                            .dot(
                                (t.translation().xz() - player_transform.translation().xz()).normalize()) > 0.8)
                    .min_by(|a, b|
                        player_transform
                            .translation()
                            .distance(
                                a.translation()
                            )
                            .total_cmp(
                                &player_transform
                                    .translation()
                                    .distance(
                                        b.translation()
                                    )
                            )
                    );
            if let Some(closest) = closest {
                auto_aim.0 = (closest.translation() - player_transform.translation()).normalize();
                auto_aim.0.y = 0.0
            } else {
                auto_aim.0 = player_transform.forward();
            }
        }
    }
}

pub fn debug_gizmos(
    player_query: Query<(&GlobalTransform, &AutoAim), With<Player>>,
    mut gizmos: Gizmos,
) {
    for (player_transform, auto_aim) in player_query.iter() {
        gizmos.line(
            player_transform.translation(),
            player_transform.translation() + player_transform.forward() * 10.0,
            Color::GREEN,
        );
        gizmos.line(
            player_transform.translation(),
            player_transform.translation() + auto_aim.0 * 10.0,
            Color::RED,
        );
    }
}