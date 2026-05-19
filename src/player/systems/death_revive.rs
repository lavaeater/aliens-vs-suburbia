use avian3d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;
use lava_ui_builder::{progress_bar, WorldFollower};

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::general::components::Health;
use crate::player::components::{Player, PlayerDead};

const REVIVE_RANGE: f32 = 1.8;
const REVIVE_DURATION: f32 = 3.0;

/// When a player's health reaches 0, mark them as downed instead of letting
/// them be despawned. Zeros their velocity and spawns a revive progress bar.
#[allow(clippy::type_complexity)]
pub fn detect_player_death(
    mut commands: Commands,
    mut query: Query<
        (Entity, &Health, &mut LinearVelocity),
        (With<Player>, Without<PlayerDead>, Changed<Health>),
    >,
    mut anim_ew: MessageWriter<AnimationEvent>,
) {
    for (entity, health, mut vel) in query.iter_mut() {
        if health.health > 0 { continue; }

        vel.0 = Vec3::ZERO;

        let bar = commands.spawn((
            WorldFollower { target: entity, offset: Vec2::new(-30.0, -60.0) },
            progress_bar(0.0, 60.0, 8.0, Color::srgb(0.2, 0.6, 1.0), Color::srgba(0.0, 0.0, 0.0, 0.6)),
            Node { position_type: PositionType::Absolute, ..default() },
        )).id();

        commands.entity(entity).insert(PlayerDead {
            revive_progress: 0.0,
            revive_bar: Some(bar),
        });

        anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Death));
    }
}

/// Hold E near a downed player to revive them over REVIVE_DURATION seconds.
/// Releasing E resets progress.
pub fn player_revive_system(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    living_players: Query<&Position, (With<Player>, Without<PlayerDead>)>,
    mut dead_players: Query<(Entity, &Position, &mut PlayerDead, &mut Health)>,
    mut anim_ew: MessageWriter<AnimationEvent>,
    mut bar_query: Query<&mut lava_ui_builder::ProgressBar>,
) {
    let reviving = keys.pressed(KeyCode::KeyE);

    for (dead_entity, dead_pos, mut dead, mut health) in dead_players.iter_mut() {
        let nearby = living_players.iter()
            .any(|p| p.0.distance(dead_pos.0) <= REVIVE_RANGE);

        if reviving && nearby {
            dead.revive_progress += time.delta_secs() / REVIVE_DURATION;
        } else {
            dead.revive_progress = (dead.revive_progress - time.delta_secs() / REVIVE_DURATION).max(0.0);
        }

        // Sync progress bar.
        if let Some(bar_entity) = dead.revive_bar
            && let Ok(mut bar) = bar_query.get_mut(bar_entity)
        {
            bar.value = dead.revive_progress.clamp(0.0, 1.0);
        }

        if dead.revive_progress >= 1.0 {
            // Revive: restore half health, remove downed state.
            if let Some(bar_entity) = dead.revive_bar {
                commands.entity(bar_entity).despawn();
            }
            health.health = health.max_health / 2;
            commands.entity(dead_entity).remove::<PlayerDead>();
            anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, dead_entity, AnimationKey::Death));
        }
    }
}
