//! Every player carries a flashlight: a shadow-casting spot light that points where they
//! aim. The `Flashlight` action (F / L3) switches it; it goes out while the player is down.
//!
//! The light is its own entity rather than a child of the player, because the body faces
//! the walk direction and the aim is somewhere else entirely (`torso_twist`). It is placed
//! after transform propagation, from the player's final position this frame, and writes
//! its own `GlobalTransform` so it does not trail a frame behind.

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::control::bindings::PlayerAction;
use crate::player::components::{AutoAim, Player, PlayerDead};

/// Lumens, against the map lights in `map::map_lights`.
const INTENSITY: f32 = 150_000.0;
/// How far below the horizon the beam points, so its pool lands on the floor a few tiles
/// out instead of skimming across it.
const TILT_DOWN: f32 = 0.32;
/// Above the player's centre, and forward of it along the aim.
const HOLD_HEIGHT: f32 = 0.15;
const HOLD_FORWARD: f32 = 0.2;

#[derive(Component)]
pub struct Flashlight {
    pub owner: Entity,
    pub on: bool,
}

/// On a player who has been given a flashlight.
#[derive(Component)]
pub struct CarriesFlashlight;

pub fn give_flashlights(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Without<CarriesFlashlight>)>,
) {
    for owner in &players {
        commands.spawn((
            Name::from("Flashlight"),
            Flashlight { owner, on: true },
            SpotLight {
                color: Color::srgb(1.0, 0.96, 0.88),
                intensity: INTENSITY,
                range: 10.0,
                outer_angle: 0.42,
                inner_angle: 0.28,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::default(),
        ));
        // `try_`: the editor swaps the player model by despawning it, possibly this frame.
        commands.entity(owner).try_insert(CarriesFlashlight);
    }
}

/// Where the beam starts and which way it points, for a player at `centre` aiming `aim`.
pub fn beam(centre: Vec3, aim: Vec3) -> Transform {
    let flat = Vec3::new(aim.x, 0.0, aim.z).normalize_or(Vec3::NEG_Z);
    let direction = (flat * TILT_DOWN.cos() - Vec3::Y * TILT_DOWN.sin()).normalize();
    let origin = centre + Vec3::Y * HOLD_HEIGHT + flat * HOLD_FORWARD;
    Transform::from_translation(origin).looking_to(direction, Vec3::Y)
}

#[allow(clippy::type_complexity)]
pub fn update_flashlights(
    mut commands: Commands,
    players: Query<
        (
            &GlobalTransform,
            &AutoAim,
            Has<PlayerDead>,
            Option<&ActionState<PlayerAction>>,
        ),
        With<Player>,
    >,
    mut lights: Query<
        (
            Entity,
            &mut Flashlight,
            &mut SpotLight,
            &mut Transform,
            &mut GlobalTransform,
        ),
        Without<Player>,
    >,
) {
    for (entity, mut flashlight, mut spot, mut transform, mut global) in &mut lights {
        let Ok((body, aim, dead, actions)) = players.get(flashlight.owner) else {
            // The player is gone (despawned on death or a model swap); a new one gets a
            // new flashlight.
            commands.entity(entity).despawn();
            continue;
        };
        if actions.is_some_and(|a| a.just_pressed(&PlayerAction::Flashlight)) {
            flashlight.on = !flashlight.on;
        }
        spot.intensity = if flashlight.on && !dead {
            INTENSITY
        } else {
            0.0
        };
        *transform = beam(body.translation(), aim.0);
        *global = GlobalTransform::from(*transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_beam_points_along_the_aim_and_down() {
        let t = beam(Vec3::ZERO, Vec3::new(1.0, 0.5, 0.0));
        let forward = t.forward();
        assert!(forward.x > 0.9, "along the flattened aim: {forward}");
        assert!(forward.y < -0.2, "tilted at the floor: {forward}");
        assert!(forward.z.abs() < 1e-5);
        assert!(t.translation.x > 0.0, "held out in front");
    }

    #[test]
    fn no_aim_still_gives_a_beam() {
        let t = beam(Vec3::ZERO, Vec3::ZERO);
        assert!(t.forward().is_finite());
    }
}
