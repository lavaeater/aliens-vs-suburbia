use bevy::math::{EulerRot, Quat, Vec2, vec3, Vec3, Vec3Swizzles};
use bevy::prelude::{Color, Gizmos, GlobalTransform, Query, Res, With};
use bevy::time::Time;
use bevy_xpbd_3d::components::Collider;
use bevy_xpbd_3d::prelude::{SpatialQuery, SpatialQueryFilter};
use crate::control::components::{CharacterControl, ControlCommands};
use crate::general::components::CollisionLayer;
use crate::player::components::{AutoAim, Player};

pub fn auto_aim(
    mut player_query: Query<(&GlobalTransform, &mut AutoAim, &CharacterControl), With<Player>>,
    spatial_query: SpatialQuery,
    alien_query: Query<&GlobalTransform>,
) {
    for (player_transform, mut auto_aim, character_control) in player_query.iter_mut() {
        if character_control.triggers.contains(&ControlCommands::Throw) {
            let aliens =
                spatial_query.shape_hits(
                    &Collider::triangle(vec3(0.0, 0.0, 0.0), vec3(5.0, 0.0, 0.0), vec3(0.0, 0.0, 5.0)),
                    player_transform.translation(),
                    Quat::from_euler(
                        EulerRot::YXZ,
                        135.0f32.to_radians(), 0.0, 0.0),
                    player_transform.forward(),
                    1000.0,
                    10,
                    true,
                    SpatialQueryFilter::default().with_masks([CollisionLayer::AlienGoal, CollisionLayer::Alien, CollisionLayer::AlienSpawnPoint]),
                );
            let closest = aliens.iter().min_by(|a, b| a.time_of_impact.partial_cmp(&b.time_of_impact).unwrap());
            if let Some(closest) = closest {
                if let Ok(alien_transform) = alien_query.get(closest.entity) {
                    auto_aim.0 = (alien_transform.translation() - player_transform.translation()).normalize();
                    auto_aim.0.y = 0.0
                } else {
                    auto_aim.0 = Vec3::NEG_Z;
                }
            }
        }
    }
}

pub fn debug_auto_aim(
    player_query: Query<(&GlobalTransform, &AutoAim), With<Player>>,
    mut gizmos: Gizmos,
    time: Res<Time>,
) {
    for (player_transform, auto_aim) in player_query.iter() {
        let mut rotation = Quat::from_euler(
            EulerRot::YXZ,
            135.0f32.to_radians(), 0.0, 0.0);
        let origin = player_transform.translation();
        let direction_xz = player_transform.forward().zx();
        let desired_angle = Vec2::Y.angle_between(direction_xz).to_degrees();
        let new_rot = Quat::from_axis_angle(Vec3::Y, (desired_angle - 90.0).to_radians());
        let mut t1 = vec3(0.0, 0.0, 0.0) + origin;
        let mut t2 = new_rot * rotation * vec3(5.0, 0.0, 2.0) + origin;
        let mut t3 = new_rot *  rotation * vec3(2.0, 0.0, 5.0) + origin;







        gizmos.line(
            t1,
            t2,
            Color::GREEN,
        );
        gizmos.line(
            t2,
            t3,
            Color::GREEN,
        );
        gizmos.line(
            t3,
            t1,
            Color::GREEN,
        );
    }
}