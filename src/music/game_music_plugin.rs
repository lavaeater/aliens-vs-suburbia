use std::collections::HashMap;

use bevy::prelude::*;
use bevy_seedling::prelude::AudioSample;
use rusty_music::musicians::arpeggiator::Arpeggiator;
use rusty_music::musicians::bassist::Bassist;
use rusty_music::musicians::drummer::{
    generate_double_time_kick_beat, generate_double_time_snare_beat, generate_half_time_kick_beat,
    generate_half_time_snare_beat, generate_hihat_beat, generate_kick_beat, generate_snare_beat,
    SuperDrummer,
};
use rusty_music::musicians::soloist::Soloist;
use rusty_music::musicians::{Chord, Musician, Muted, Note, Sampler};
use rusty_music::player::Intensity;
use rusty_music::{create_drummer_only, make_conductor, MusicPlugin};

use crate::alien::components::general::AlienCounter;
use crate::alien::wave_manager::WaveManager;
use crate::game_state::score_keeper::{LevelState, LevelTracker};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::music::players::{ChordStabs, PadPlayer, UfoStingers};
use crate::player::components::Player;

/// Which intensity measure gates a musician on/off. The global `Intensity`
/// resource still shapes *how* every unmuted musician plays; the channel
/// decides *whether* it plays at all.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MusicChannel {
    /// Pad + arpeggio: always audible, even in the menu.
    Ambient,
    /// Drums + bass: gated by the combat measure.
    Groove,
    /// Sax lead + chord stabs: gated by a higher combat threshold.
    Combat,
    /// UFO / SID stingers: gated by the danger measure.
    Danger,
}

/// The game's intensity measures. `combat` and `danger` are smoothed values
/// chasing the targets the game-state systems write each frame; both feed the
/// global `Intensity` and independently mute/unmute their channels.
#[derive(Resource, Debug)]
pub struct MusicMoods {
    pub combat: f32,
    pub danger: f32,
    pub target_combat: f32,
    pub target_danger: f32,
    /// Intensity floor, so calm moments still have a gentle pulse.
    pub base: f32,
}

impl Default for MusicMoods {
    fn default() -> Self {
        Self {
            combat: 0.0,
            danger: 0.0,
            target_combat: 0.0,
            target_danger: 0.0,
            base: 0.25,
        }
    }
}

pub struct GameMusicPlugin;

impl Plugin for GameMusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MusicPlugin {
            beats: 4,
            note_type: 4,
            bpm: 100.0,
        })
        .insert_resource(make_conductor(suburbia_chords(), 4.0))
        .init_resource::<MusicMoods>()
        .add_systems(Startup, setup_band)
        .add_systems(
            Update,
            (
                update_game_moods.run_if(in_state(GameState::InGame)),
                update_menu_moods.run_if(not(in_state(GameState::InGame))),
                apply_moods,
            )
                .chain(),
        );
    }
}

// ── Composition ───────────────────────────────────────────────────────────────

/// A-minor surf/spy progression: Am (i) - F (VI) - C (III) - E (V).
/// Semitone offsets are relative to the sample's recorded pitch ("0" = root).
fn suburbia_chords() -> Vec<Chord> {
    // A harmonic-minor flavour (the G# only surfaces over the E chord, which
    // carries it in its chord tones; the shared scale keeps both G and G#
    // available as low-strength colour).
    let scale: Vec<Note> = [
        (-12i32, 0.9),
        (-10, 0.4),
        (-9, 0.5),
        (-7, 0.6),
        (-5, 0.7),
        (-4, 0.5),
        (-1, 0.3),
        (0, 1.0),
        (2, 0.4),
        (3, 0.7),
        (5, 0.5),
        (7, 0.8),
        (8, 0.4),
        (10, 0.4),
        (11, 0.3),
        (12, 0.4),
    ]
    .into_iter()
    .map(|(d, s)| Note::new(d, s))
    .collect();

    vec![
        // i — Am: home, lurking
        Chord::new(
            0.0,
            vec![
                Note::new(-12, 1.0), // A bass
                Note::new(0, 1.0),   // A root
                Note::new(3, 0.9),   // C minor 3rd
                Note::new(7, 0.8),   // E 5th
                Note::new(-5, 0.6),  // E low
                Note::new(12, 0.4),  // A octave
                Note::new(15, 0.3),  // C high colour
            ],
            scale.clone(),
        ),
        // VI — F: wide suburban lawns
        Chord::new(
            1.0,
            vec![
                Note::new(-4, 1.0), // F bass
                Note::new(0, 0.9),  // A 3rd
                Note::new(3, 0.8),  // C 5th
                Note::new(8, 0.6),  // F octave
                Note::new(-9, 0.5), // C low
                Note::new(12, 0.3), // A high
            ],
            scale.clone(),
        ),
        // III — C: bright lift
        Chord::new(
            2.0,
            vec![
                Note::new(-9, 1.0), // C bass
                Note::new(3, 1.0),  // C
                Note::new(7, 0.9),  // E 3rd
                Note::new(10, 0.7), // G 5th
                Note::new(-2, 0.6), // G low
                Note::new(15, 0.3), // C high
            ],
            scale.clone(),
        ),
        // V — E: the spy-movie tension chord, drives back to Am
        Chord::new(
            3.0,
            vec![
                Note::new(-5, 1.0), // E bass
                Note::new(7, 1.0),  // E
                Note::new(11, 0.9), // G# 3rd
                Note::new(2, 0.8),  // B 5th
                Note::new(-1, 0.5), // G# low
                Note::new(14, 0.3), // B high
            ],
            scale.clone(),
        ),
    ]
}

fn shaker_pattern() -> HashMap<(u32, u32), Note> {
    HashMap::from([
        ((0, 0), Note::new(0, 0.9)),
        ((1, 0), Note::new(0, 0.9)),
        ((2, 0), Note::new(0, 0.9)),
        ((3, 0), Note::new(0, 0.9)),
        ((0, 2), Note::new(0, 0.4)),
        ((1, 2), Note::new(0, 0.4)),
        ((2, 2), Note::new(0, 0.4)),
        ((3, 2), Note::new(0, 0.4)),
    ])
}

fn offbeat_hat_pattern() -> HashMap<(u32, u32), Note> {
    HashMap::from([
        ((0, 2), Note::new(0, 0.7)),
        ((1, 2), Note::new(0, 0.7)),
        ((2, 2), Note::new(0, 0.7)),
        ((3, 2), Note::new(0, 0.7)),
    ])
}

fn crash_downbeat_pattern() -> HashMap<(u32, u32), Note> {
    HashMap::from([((0, 0), Note::new(0, 0.5))])
}

// ── Band setup ────────────────────────────────────────────────────────────────

fn setup_band(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |p: &str| -> Handle<AudioSample> { asset_server.load(p.to_string()) };

    // 80PD kit: garage-band suburbia.
    let kick = load("instruments/drums/kick.wav");
    let snare = load("instruments/drums/snare.wav");
    let hihat = load("instruments/drums/hihat.wav");
    let shaker = load("instruments/drums/80PD_KitD-Shaker.wav");
    let ophat = load("instruments/drums/80PD_KitD-OpHat.wav");
    let tom_hi = load("instruments/drums/80PD_KitD-Tom[Hi].wav");
    let tom_mid = load("instruments/drums/80PD_KitD-Tom[Mid].wav");
    let tom_lo = load("instruments/drums/80PD_KitD-Tom[Lo].wav");
    let crash = load("instruments/80PD_KitD-Crash01.wav");

    // Tonal voices.
    let moog = load("instruments/moog.wav");
    let pad = load("instruments/pad.wav");
    let pluck = load("instruments/pluck.wav");
    let sax = load("instruments/sax.wav");
    let stab = load("instruments/stab.wav");
    let hit1 = load("instruments/hit1.wav");
    let hit2 = load("instruments/hit2.wav");
    let ufo = load("instruments/ufo.wav");
    let sid = load("instruments/sid.wav");

    // ── Groove: drum kit with auto time-feel and a tom fill every 4th bar ────
    let mut drums = SuperDrummer::new(vec![
        create_drummer_only(kick.clone(), 0.0, generate_kick_beat()),
        create_drummer_only(snare.clone(), -2.0, generate_snare_beat()),
        create_drummer_only(hihat.clone(), -8.0, generate_hihat_beat()),
    ]);
    drums.auto_time_feel = true;
    drums.half_time_drums = vec![
        create_drummer_only(kick.clone(), 0.0, generate_half_time_kick_beat()),
        create_drummer_only(snare.clone(), -3.0, generate_half_time_snare_beat()),
        create_drummer_only(shaker, -10.0, shaker_pattern()),
    ];
    drums.double_time_drums = vec![
        create_drummer_only(kick.clone(), 0.0, generate_double_time_kick_beat()),
        create_drummer_only(snare.clone(), -2.0, generate_double_time_snare_beat()),
        create_drummer_only(ophat, -9.0, offbeat_hat_pattern()),
        create_drummer_only(crash, -10.0, crash_downbeat_pattern()),
    ];
    let drums = drums.with_fills(
        4,
        vec![
            create_drummer_only(kick, 0.0, generate_kick_beat()),
            create_drummer_only(snare, -2.0, generate_snare_beat()),
            create_drummer_only(
                tom_hi,
                -3.0,
                HashMap::from([((3, 0), Note::new(0, 0.8)), ((3, 1), Note::new(0, 0.6))]),
            ),
            create_drummer_only(tom_mid, -3.0, HashMap::from([((3, 2), Note::new(0, 0.8))])),
            create_drummer_only(tom_lo, -3.0, HashMap::from([((3, 3), Note::new(0, 0.9))])),
        ],
    );
    commands.spawn((Musician::new("Drums".into(), drums), MusicChannel::Groove, Muted));

    // ── Groove: moog bass, riffing in 2-bar loops ────────────────────────────
    let mut bassist = Bassist::new(Sampler { handle: moog, volume: -1.0 });
    bassist.memory_bars = 2;
    bassist.memory_repeats = 2;
    commands.spawn((Musician::new("Bass".into(), bassist), MusicChannel::Groove, Muted));

    // ── Ambient: pad swells + pluck arpeggio ─────────────────────────────────
    commands.spawn((
        Musician::new("Pad".into(), PadPlayer { sampler: Sampler { handle: pad, volume: -4.0 } }),
        MusicChannel::Ambient,
    ));
    let mut arp = Arpeggiator::new(Sampler { handle: pluck, volume: -6.0 });
    arp.use_scale_runs = true;
    commands.spawn((Musician::new("Arp".into(), arp), MusicChannel::Ambient));

    // ── Combat: sax lead (AABA phrases) + surf stabs ─────────────────────────
    commands.spawn((
        Musician::new("Sax".into(), Soloist::new(Sampler { handle: sax, volume: -3.0 }, 4)),
        MusicChannel::Combat,
        Muted,
    ));
    commands.spawn((
        Musician::new("Stabs".into(), ChordStabs { samples: vec![stab, hit1, hit2], volume: -5.0 }),
        MusicChannel::Combat,
        Muted,
    ));

    // ── Danger: alien FX ─────────────────────────────────────────────────────
    commands.spawn((
        Musician::new("Saucer".into(), UfoStingers { ufo, sid, volume: -6.0 }),
        MusicChannel::Danger,
        Muted,
    ));
}

// ── Intensity measures ────────────────────────────────────────────────────────

/// Derives the mood targets from gameplay: alien pressure, wave progression,
/// pre-wave anticipation, hurt players and aliens slipping through to the goal.
fn update_game_moods(
    mut moods: ResMut<MusicMoods>,
    aliens: Res<AlienCounter>,
    waves: Res<WaveManager>,
    tracker: Res<LevelTracker>,
    players: Query<&Health, With<Player>>,
) {
    moods.base = 0.18;

    let alien_pressure = (aliens.count as f32 / 10.0).min(1.0);
    let wave_ramp = if waves.waves.len() > 1 {
        waves.current_wave.min(waves.waves.len() - 1) as f32 / (waves.waves.len() - 1) as f32
    } else {
        0.0
    };

    moods.target_combat = if waves.spawning || aliens.count > 0 {
        (0.35 + 0.45 * alien_pressure + 0.2 * wave_ramp).min(1.0)
    } else if waves.waves_remaining() {
        // Anticipation: tension creeps in over the last 12 seconds before a wave.
        ((12.0 - waves.wave_timer) / 12.0).clamp(0.0, 1.0) * 0.3
    } else {
        0.0
    };

    let goal_pressure = if tracker.aliens_win_cut_off > 0 {
        (tracker.aliens_reached_goal as f32 / tracker.aliens_win_cut_off as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut hurt = 0.0f32;
    let mut player_count = 0u32;
    for health in players.iter() {
        hurt += 1.0 - health.health.max(0) as f32 / health.max_health.max(1) as f32;
        player_count += 1;
    }
    if player_count > 0 {
        hurt /= player_count as f32;
    }
    moods.target_danger = goal_pressure.max(hurt);

    // Level over: let everything wind down to the ambient layer.
    if matches!(tracker.level_state, LevelState::Completed | LevelState::Failed) {
        moods.target_combat = 0.0;
        moods.target_danger = 0.0;
    }
}

/// Menus, editors and showcase screens get a gentle ambient-only soundtrack.
fn update_menu_moods(mut moods: ResMut<MusicMoods>) {
    moods.base = 0.25;
    moods.target_combat = 0.0;
    moods.target_danger = 0.0;
}

/// Smooths the measures toward their targets (rising faster than falling),
/// combines them into the global `Intensity`, and gates each channel's
/// musicians with hysteresis so layers don't flap at the threshold.
fn apply_moods(
    time: Res<Time>,
    mut moods: ResMut<MusicMoods>,
    mut intensity: ResMut<Intensity>,
    mut commands: Commands,
    members: Query<(Entity, &MusicChannel, Has<Muted>)>,
) {
    let dt = time.delta_secs();
    let approach = |current: f32, target: f32| -> f32 {
        let rate = if target > current { 1.2 } else { 0.35 };
        current + (target - current) * (rate * dt).min(1.0)
    };
    moods.combat = approach(moods.combat, moods.target_combat);
    moods.danger = approach(moods.danger, moods.target_danger);

    intensity.0 = moods
        .base
        .max(moods.combat)
        .max(moods.danger * 0.9)
        .clamp(0.0, 1.0);

    for (entity, channel, muted) in members.iter() {
        let (on_at, off_below, value) = match channel {
            MusicChannel::Ambient => continue, // never muted
            MusicChannel::Groove => (0.22, 0.12, moods.combat),
            MusicChannel::Combat => (0.45, 0.30, moods.combat),
            MusicChannel::Danger => (0.30, 0.18, moods.danger),
        };
        if muted && value >= on_at {
            commands.entity(entity).remove::<Muted>();
        } else if !muted && value < off_below {
            commands.entity(entity).insert(Muted);
        }
    }
}
