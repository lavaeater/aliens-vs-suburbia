//! Barks: ultraviolent one-liners that surface as a fading caption at the bottom of
//! the screen (and play a `bark*.wav` if you've recorded some). They key off the same
//! gore messages as everything else — kills and the player getting hurt.
//!
//! The theme lives here: as the body count climbs (`AtrocityMeter`), the lines drift
//! from gung-ho zeal to something more haunted. Obedience curdling into doubt.
//!
//! Triggers are wired to gore events for now; migrating them to authored `turbofacts`
//! stories (which can weigh wave, health, streaks) is a natural later refinement.

use bevy::prelude::*;

use crate::gore::components::{DamageDealt, EntityDied};
use crate::gore::sfx::{PlaySfx, SfxKind};
use crate::player::components::Player;

/// Running body count — drives the tonal drift. Persists for the session.
#[derive(Resource, Default)]
pub struct AtrocityMeter {
    pub kills: u32,
}

/// Bark pacing + RNG, so lines stay occasional and non-repetitive.
#[derive(Resource)]
pub struct BarkState {
    cooldown: f32,
    seed: u32,
    last: usize,
}

impl Default for BarkState {
    fn default() -> Self {
        Self { cooldown: 0.0, seed: 0x1234_5678, last: usize::MAX }
    }
}

/// Minimum gap between barks so they don't chatter over each other.
const BARK_COOLDOWN: f32 = 3.5;
/// Body count at which the haunted lines fully take over.
const HAUNTED_AT: u32 = 45;

/// The caption entity's fade state.
#[derive(Component)]
pub struct BarkCaption {
    fade: Timer,
}

// ── Line pools ────────────────────────────────────────────────────────────────
// ASCII only (Bevy's default font). `zeal` = early/gung-ho, `haunted` = late/doubtful.

struct Lines {
    zeal: &'static [&'static str],
    haunted: &'static [&'static str],
}

const KILL: Lines = Lines {
    zeal: &[
        "KILL THEM ALL!",
        "For the neighborhood!",
        "Wade through their blood!",
        "That's another one down!",
        "Cleansed!",
    ],
    haunted: &[
        "...was that one of them?",
        "How many is enough?",
        "I can't stop.",
        "They told us to.",
        "It doesn't feel like winning.",
    ],
};

const MULTI_KILL: Lines = Lines {
    zeal: &[
        "MULTIKILL! Glorious!",
        "All of you! At once!",
        "Ultra violence!",
    ],
    haunted: &[
        "So much of it. So fast.",
        "I don't recognize my hands.",
        "We were told they were monsters.",
    ],
};

const HURT: Lines = Lines {
    zeal: &["Is that all?!", "You'll pay for that!", "Come closer!"],
    haunted: &["I'm so tired.", "Why are we still here?", "Make it stop."],
};

const LOW_HEALTH: Lines = Lines {
    zeal: &["Not like this!", "Hold the line!"],
    haunted: &["Maybe I deserve this.", "Let it end."],
};

/// Deterministic-ish pick that avoids repeating the immediately previous index.
fn pick<'a>(state: &mut BarkState, lines: &'a [&'a str]) -> &'a str {
    state.seed = state.seed.wrapping_mul(1664525).wrapping_add(1013904223);
    let mut i = (state.seed >> 16) as usize % lines.len();
    if lines.len() > 1 && i == state.last {
        i = (i + 1) % lines.len();
    }
    state.last = i;
    lines[i]
}

/// Choose from a pool, blending zeal->haunted as atrocity rises.
fn choose(state: &mut BarkState, lines: &Lines, kills: u32) -> &'static str {
    let haunted_frac = (kills as f32 / HAUNTED_AT as f32).clamp(0.0, 1.0);
    state.seed = state.seed.wrapping_mul(22695477).wrapping_add(1);
    let roll = ((state.seed >> 16) & 0xffff) as f32 / 65535.0;
    let pool = if roll < haunted_frac { lines.haunted } else { lines.zeal };
    let pool = if pool.is_empty() { lines.zeal } else { pool };
    pick(state, pool)
}

/// Spawn (or reset) the caption line when entering the game.
pub fn setup_bark_caption(mut commands: Commands, existing: Query<Entity, With<BarkCaption>>) {
    for e in existing.iter() {
        commands.entity(e).despawn();
    }
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Percent(15.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Don't intercept clicks meant for the world/UI beneath.
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                BarkCaption {
                    fade: Timer::from_seconds(1.0, TimerMode::Once),
                },
                Text::new(""),
                TextFont { font_size: FontSize::Px(30.0), ..default() },
                TextColor(Color::srgba(1.0, 0.9, 0.85, 0.0)),
                TextLayout { justify: Justify::Center, ..default() },
            ));
        });
}

/// React to kills and to the player getting hurt; emit an occasional bark.
#[allow(clippy::too_many_arguments)]
pub fn bark_on_events(
    time: Res<Time>,
    mut deaths: MessageReader<EntityDied>,
    mut damage: MessageReader<DamageDealt>,
    players: Query<Entity, With<Player>>,
    player_health: Query<&crate::general::components::Health, With<Player>>,
    mut atrocity: ResMut<AtrocityMeter>,
    mut state: ResMut<BarkState>,
    mut caption: Query<(&mut BarkCaption, &mut Text, &mut TextColor)>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if state.cooldown > 0.0 {
        state.cooldown -= time.delta_secs();
    }

    // Count kills this frame (all EntityDied are deaths of things worth a line).
    let kills_this_frame = deaths.read().count() as u32;
    atrocity.kills += kills_this_frame;

    // Did the player take a hit this frame?
    let player_hit = damage.read().any(|d| players.contains(d.target));

    if state.cooldown > 0.0 {
        return;
    }

    // Priority: player in danger > player hurt > kills.
    let player_low = player_health
        .iter()
        .any(|h| h.max_health > 0 && (h.health as f32 / h.max_health as f32) < 0.25 && h.health > 0);

    let line = if player_low && player_hit {
        Some(choose(&mut state, &LOW_HEALTH, atrocity.kills))
    } else if player_hit {
        // Not every hit talks — roll it.
        state.seed = state.seed.wrapping_mul(214013).wrapping_add(2531011);
        if (state.seed >> 24) & 1 == 0 {
            Some(choose(&mut state, &HURT, atrocity.kills))
        } else {
            None
        }
    } else if kills_this_frame >= 3 {
        Some(choose(&mut state, &MULTI_KILL, atrocity.kills))
    } else if kills_this_frame >= 1 {
        // Roughly half of kills get a shout.
        state.seed = state.seed.wrapping_mul(214013).wrapping_add(2531011);
        if (state.seed >> 23) & 1 == 0 {
            Some(choose(&mut state, &KILL, atrocity.kills))
        } else {
            None
        }
    } else {
        None
    };

    if let Some(line) = line {
        state.cooldown = BARK_COOLDOWN;
        // Caption color drifts from warm white toward sickly red with atrocity.
        let t = (atrocity.kills as f32 / HAUNTED_AT as f32).clamp(0.0, 1.0);
        let color = Color::srgb(1.0, 0.9 - 0.5 * t, 0.85 - 0.6 * t);
        if let Ok((mut cap, mut text, mut text_color)) = caption.single_mut() {
            **text = line.to_string();
            *text_color = TextColor(color.with_alpha(1.0));
            cap.fade = Timer::from_seconds(2.6, TimerMode::Once);
        }
        sfx.write(PlaySfx { kind: SfxKind::Bark, gain_db: -2.0 });
    }
}

/// Fade the caption out over its lifetime.
pub fn tick_bark_caption(
    time: Res<Time>,
    mut caption: Query<(&mut BarkCaption, &mut TextColor)>,
) {
    for (mut cap, mut color) in caption.iter_mut() {
        cap.fade.tick(time.delta());
        let a = 1.0 - cap.fade.fraction();
        let c = color.0.to_srgba();
        *color = TextColor(Color::srgba(c.red, c.green, c.blue, a));
    }
}

/// Reset the body count when a fresh game begins.
pub fn reset_atrocity(mut atrocity: ResMut<AtrocityMeter>) {
    atrocity.kills = 0;
}
