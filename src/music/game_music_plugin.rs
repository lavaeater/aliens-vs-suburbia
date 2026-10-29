use bevy::prelude::*;
use bevy_seedling::prelude::AudioSample;
use rusty_music::MusicPlugin;
use rusty_music::harmony::Harmony;
use rusty_music::improv::{ChimeArp, Drone, GrooveDrummer, MotifSoloist, VoiceLedPad, WalkingBass};
use rusty_music::mix::{Mix, MixPlugin, MixScenes, Mixer, Section, Sidechain};
use rusty_music::musicians::{Musician, Muted};
use rusty_music::situation::{
    Dim, Gesture, Layer, MusicState, Quantize, SituationPlugin, Stinger, TempoFollows,
};
use rusty_music::synth::{
    GlicolSynth, MusicListener, Proximity, Responds, SynthPatch, SynthPlugin, glicol_musician,
    spawn_synth_kit_with, synth, synth_musician,
};

use crate::alien::components::general::AlienCounter;
use crate::alien::wave_manager::WaveManager;
use crate::camera::components::CameraFocus;
use crate::game_state::GameState;
use crate::game_state::clear_game_entities_plugin::Persistent;
use crate::game_state::score_keeper::{GameTrackingEvent, LevelState, LevelTracker};
use crate::general::components::Health;
use crate::general::components::map_components::AlienSpawnPoint;
use crate::items::ItemPickedUp;
use crate::music::players::{ChordStabs, StingerOnly};
use crate::player::components::{Player, PlayerDead};

/// The game's danger measure, smoothed (rising faster than falling). The music
/// reads the raw targets through `MusicState`; this copy is for the visuals
/// (`gore::despair`) so image and sound swell together.
#[derive(Resource, Debug, Default)]
pub struct MusicMoods {
    pub danger: f32,
    pub target_danger: f32,
}

/// Musicians the game plays stingers on.
#[derive(Resource)]
struct Band {
    chime: Entity,
    alarm: Entity,
    fanfare: Entity,
}

/// Where the music is heard from: follows the camera's focus, so the saucer
/// hum of a spawn point swells as the players close in on it.
#[derive(Component)]
struct MusicEar;

pub struct GameMusicPlugin;

impl Plugin for GameMusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MusicPlugin {
                beats: 4,
                note_type: 4,
                bpm: 100.0,
            },
            SituationPlugin,
            MixPlugin,
            SynthPlugin,
        ))
        .insert_resource(MusicState::default().with_response(1.5))
        // A minor, i - VI - III - VII; the mode brightens and darkens with valence.
        .insert_resource(Harmony {
            key: -3,
            progression: vec![0, 5, 2, 6],
            ..default()
        })
        .insert_resource(TempoFollows {
            dim: Dim::Energy,
            slow: 92.0,
            fast: 128.0,
        })
        .insert_resource(mix_scenes())
        .init_resource::<MusicMoods>()
        .add_systems(Startup, setup_band)
        .add_systems(
            Update,
            (
                (describe_game, game_stingers).run_if(in_state(GameState::InGame)),
                describe_menu.run_if(not(in_state(GameState::InGame))),
                smooth_danger,
                shape_mix,
                follow_camera_focus,
                hum_spawn_points,
            )
                .chain(),
        );
    }
}

// ── Band setup ────────────────────────────────────────────────────────────────

fn setup_band(mut commands: Commands, mixer: Res<Mixer>, asset_server: Res<AssetServer>) {
    let load = |p: &str| -> Handle<AudioSample> { asset_server.load(p.to_string()) };

    commands.spawn((
        Persistent,
        Name::new("Music ear"),
        MusicEar,
        MusicListener,
        Transform::default(),
    ));

    // ── Core, always on: acid bass walking the changes, voice-led pads ───────
    let (node, config) = synth(SynthPatch::acid_bass(), -12.0);
    commands.spawn((
        Persistent,
        synth_musician("Bass", WalkingBass::new(1), -24.0, node, config),
        Section::Bass,
        Responds::default()
            .cutoff(Dim::Tension, 300.0, 1_800.0)
            .volume_db(Dim::Intensity, -18.0, -10.0),
    ));
    let (node, config) = synth(SynthPatch::pad(), -8.0);
    commands.spawn((
        Persistent,
        synth_musician("Pad", VoiceLedPad::new(2).voices(3), -12.0, node, config),
        Section::Harmony,
        Responds::default()
            .cutoff(Dim::Valence, 400.0, 2_500.0)
            .volume_db(Dim::Tension, -8.0, -14.0),
    ));

    // ── Layers, entering at bar lines as the siege heats up ─────────────────
    // 8-bit chime arpeggios: the suburban calm, even in the menu.
    let (node, config) = synth(SynthPatch::chip(), -22.0);
    commands.spawn((
        Persistent,
        synth_musician("Arp", ChimeArp::new(3), 0.0, node, config),
        Section::Harmony,
        Responds::default().cutoff(Dim::Tension, 600.0, 4_000.0),
        Layer::above(Dim::Intensity, 0.15),
        Muted,
    ));

    // Synth kit; the kick (channel 0) pumps the pads.
    let kit = vec![
        (SynthPatch::kick(), 36.0, -8.0),
        (SynthPatch::snare(), 55.0, -12.0),
        (SynthPatch::hat(), 60.0, -19.0),
        (SynthPatch::kick().named("tom"), 48.0, -13.0),
    ];
    let pump = |kit| {
        Box::new(
            Sidechain::new(kit)
                .channel(0)
                .ducks(&mixer, Section::Harmony, 6.0, 0.22),
        ) as Box<_>
    };
    let drums = spawn_synth_kit_with(
        &mut commands,
        "Drums",
        GrooveDrummer::new(5).with_toms(),
        kit,
        pump,
    );
    commands.entity(drums).insert((
        Persistent,
        Section::Drums,
        Layer::above(Dim::Intensity, 0.3),
        Muted,
    ));

    // Surf-spy sample stabs on the backbeat: the suburbia in the sci-fi.
    commands.spawn((
        Persistent,
        Musician::new(
            "Stabs".into(),
            ChordStabs {
                samples: vec![
                    load("instruments/stab.wav"),
                    load("instruments/hit1.wav"),
                    load("instruments/hit2.wav"),
                ],
                volume: -9.0,
            },
        ),
        Section::Harmony,
        Layer::above(Dim::Intensity, 0.5),
        Muted,
    ));

    let (node, config) = synth(SynthPatch::glide_lead(), -16.0);
    commands.spawn((
        Persistent,
        synth_musician("Lead", MotifSoloist::new(4), 0.0, node, config),
        Section::Lead,
        Responds::default().cutoff(Dim::Tension, 800.0, 4_500.0),
        Layer::above(Dim::Intensity, 0.6),
        Muted,
    ));

    // The aliens' own voice: a theremin that wails as the danger rises, its
    // vibrato getting faster and wider (knobs 0 = rate, 1 = wobble).
    commands.spawn((
        Persistent,
        glicol_musician(
            "Theremin",
            MotifSoloist::new(7),
            12.0,
            GlicolSynth::new(asset_server.load("synths/theremin.glicol"))
                .volume_db(-14.0)
                .voices(1),
        ),
        Section::Lead,
        Responds::default()
            .knob(Dim::Tension, 0, 4.0, 9.0)
            .knob(Dim::Tension, 1, 0.006, 0.03),
        Layer::above(Dim::Tension, 0.35),
        Muted,
    ));

    // ── Stinger-only instruments: silent until the game fires one ────────────
    let mut stinger = |name: &str, patch: SynthPatch, transpose: f32, db: f32| {
        let (node, config) = synth(patch, db);
        commands
            .spawn((
                Persistent,
                synth_musician(name, StingerOnly, transpose, node, config),
                Section::Fx,
                Muted,
            ))
            .id()
    };
    let band = Band {
        chime: stinger("Chime", SynthPatch::bell(), 24.0, -15.0),
        alarm: stinger("Alarm", SynthPatch::saw_lead().named("alarm"), 0.0, -14.0),
        fanfare: stinger("Fanfare", SynthPatch::e_piano(), 12.0, -10.0),
    };
    commands.insert_resource(band);
}

/// Every alien spawn point hums the chord root an octave down: you hear the
/// portal before you see it.
fn hum_spawn_points(mut commands: Commands, spawn_points: Query<Entity, Added<AlienSpawnPoint>>) {
    for entity in &spawn_points {
        let (node, config) = synth(SynthPatch::saw_lead().named("hum"), -60.0);
        // A child, not on the spawn point itself: the sweep between matches spares audio
        // nodes, and the child goes with its parent.
        commands.spawn((
            ChildOf(entity),
            Transform::default(),
            synth_musician(
                "Portal hum",
                Drone { interval: -12 },
                0.0,
                node,
                config.voices(2),
            ),
            Section::Fx,
            Proximity::new(2.0, 14.0)
                .volume(-14.0, -60.0)
                .cutoff(250.0, 1_600.0),
        ));
    }
}

fn follow_camera_focus(focus: Res<CameraFocus>, mut ear: Single<&mut Transform, With<MusicEar>>) {
    if ear.translation != focus.center {
        ear.translation = focus.center;
    }
}

// ── Describing the situation ─────────────────────────────────────────────────

/// The game's side of the music: alien pressure, wave progression, pre-wave
/// anticipation, hurt players and aliens slipping through to the goal,
/// described as a mood. `MusicState` does the smoothing.
fn describe_game(
    mut state: ResMut<MusicState>,
    mut moods: ResMut<MusicMoods>,
    aliens: Res<AlienCounter>,
    waves: Res<WaveManager>,
    tracker: Res<LevelTracker>,
    players: Query<&Health, With<Player>>,
) {
    let alien_pressure = (aliens.count as f32 / 10.0).min(1.0);
    let wave_ramp = if waves.waves.len() > 1 {
        waves.current_wave.min(waves.waves.len() - 1) as f32 / (waves.waves.len() - 1) as f32
    } else {
        0.0
    };
    let fighting = waves.spawning || aliens.count > 0;
    // Tension creeps in over the last 12 seconds before a wave.
    let anticipation = if !fighting && waves.waves_remaining() {
        ((12.0 - waves.wave_timer) / 12.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let combat = if fighting {
        (0.35 + 0.45 * alien_pressure + 0.2 * wave_ramp).min(1.0)
    } else {
        0.3 * anticipation
    };

    let goal_pressure = if tracker.aliens_win_cut_off > 0 {
        (tracker.aliens_reached_goal as f32 / tracker.aliens_win_cut_off as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (hurt, count) = players.iter().fold((0.0f32, 0u32), |(hurt, n), health| {
        let lost = 1.0 - health.health.max(0) as f32 / health.max_health.max(1) as f32;
        (hurt + lost, n + 1)
    });
    let hurt = if count > 0 { hurt / count as f32 } else { 0.0 };
    let danger = goal_pressure.max(hurt);
    moods.target_danger = danger;

    let mood = &mut state.target;
    match tracker.level_state {
        // Level over: wind down, bright and triumphant or dark and defeated.
        LevelState::Completed => {
            moods.target_danger = 0.0;
            mood.intensity = 0.35;
            mood.tension = 0.0;
            mood.valence = 0.95;
            mood.energy = 0.3;
        }
        LevelState::Failed => {
            moods.target_danger = 0.0;
            mood.intensity = 0.1;
            mood.tension = 0.3;
            mood.valence = 0.0;
            mood.energy = 0.0;
        }
        _ => {
            mood.intensity = 0.18f32.max(combat).max(danger * 0.9);
            mood.tension = danger.max(0.4 * anticipation).max(0.3 * alien_pressure);
            mood.valence = (0.55 - 0.45 * danger - 0.15 * alien_pressure).clamp(0.0, 1.0);
            mood.energy = (0.2 + 0.6 * combat + 0.2 * anticipation).min(1.0);
        }
    }
}

/// Menus, editors and showcase screens: gentle, slow and fairly bright.
fn describe_menu(mut state: ResMut<MusicState>, mut moods: ResMut<MusicMoods>) {
    moods.target_danger = 0.0;
    let mood = &mut state.target;
    mood.intensity = 0.2;
    mood.tension = 0.1;
    mood.valence = 0.65;
    mood.energy = 0.1;
}

fn smooth_danger(time: Res<Time>, mut moods: ResMut<MusicMoods>) {
    let rate = if moods.target_danger > moods.danger {
        1.2
    } else {
        0.35
    };
    moods.danger += (moods.target_danger - moods.danger) * (rate * time.delta_secs()).min(1.0);
}

/// In-key, quantised stingers on game events.
#[allow(clippy::too_many_arguments)]
fn game_stingers(
    time: Res<Time>,
    band: Res<Band>,
    waves: Res<WaveManager>,
    tracker: Res<LevelTracker>,
    mut state: ResMut<MusicState>,
    mut tracking: MessageReader<GameTrackingEvent>,
    mut pickups: MessageReader<ItemPickedUp>,
    downed: Query<(), Added<PlayerDead>>,
    mut stingers: MessageWriter<Stinger>,
    mut last_kill: Local<f32>,
    mut was_spawning: Local<bool>,
    mut last_level_state: Local<Option<LevelState>>,
) {
    let mut sting = |musician: Entity, gesture: Gesture, quantize: Quantize| {
        stingers.write(Stinger {
            musician,
            gesture,
            quantize,
        });
    };

    let now = time.elapsed_secs();
    for event in tracking.read() {
        match event {
            // Towers can kill in bursts: at most one sparkle per eighth note or so.
            GameTrackingEvent::AlienKilled(_) if now - *last_kill > 0.25 => {
                *last_kill = now;
                sting(
                    band.chime,
                    Gesture::arpeggio_up(2, 1.0, 0.5),
                    Quantize::Step,
                );
            }
            GameTrackingEvent::AlienReachedGoal => {
                sting(band.alarm, Gesture::dissonant_hit(2.0), Quantize::Step);
                state.jolt(Dim::Tension, 0.8);
            }
            _ => {}
        }
    }
    if pickups.read().count() > 0 {
        sting(
            band.chime,
            Gesture::arpeggio_up(4, 1.0, 0.8),
            Quantize::Step,
        );
    }
    if !downed.is_empty() {
        sting(band.alarm, Gesture::dissonant_hit(3.0), Quantize::Step);
        state.jolt(Dim::Tension, 1.0);
        state.jolt(Dim::Valence, 0.0);
    }
    // A new wave announces itself on the next downbeat.
    if waves.spawning && !*was_spawning {
        sting(band.fanfare, Gesture::chord_stab(8.0, 0.9), Quantize::Bar);
    }
    *was_spawning = waves.spawning;

    let level_state = tracker.level_state;
    if *last_level_state != Some(level_state) {
        match level_state {
            LevelState::Completed => {
                sting(band.fanfare, Gesture::chord_stab(14.0, 1.0), Quantize::Bar);
            }
            LevelState::Failed => sting(band.alarm, Gesture::dissonant_hit(8.0), Quantize::Bar),
            _ => {}
        }
        *last_level_state = Some(level_state);
    }
}

// ── Mixing ───────────────────────────────────────────────────────────────────

/// Three places with their own sound: the calm lawns, the siege, and the dread
/// of a losing fight (a huge dark room, echoing lead).
fn mix_scenes() -> MixScenes {
    let mut calm = Mix::default();
    calm.reverb.room_size = 0.8;
    calm.section(Section::Harmony).reverb_db = -9.0;
    calm.section(Section::Lead).reverb_db = -12.0;

    let mut siege = Mix::default();
    siege.reverb.room_size = 0.65;
    siege.section(Section::Harmony).reverb_db = -16.0;
    siege.section(Section::Lead).reverb_db = -16.0;
    siege.section(Section::Lead).echo_db = -14.0;

    let mut dread = calm.clone();
    dread.reverb.room_size = 0.93;
    dread.reverb.damping = 0.8;
    dread.section(Section::Lead).reverb_db = -6.0;
    dread.section(Section::Lead).echo_db = -9.0;
    dread.section(Section::Fx).reverb_db = -6.0;

    MixScenes {
        glide: 2.0,
        ..default()
    }
    .with("calm", calm)
    .with("siege", siege)
    .with("dread", dread)
}

fn shape_mix(state: Res<MusicState>, mut scenes: ResMut<MixScenes>) {
    let mood = state.current();
    scenes.activate(if mood.tension > 0.6 {
        "dread"
    } else if mood.intensity > 0.45 {
        "siege"
    } else {
        "calm"
    });
}

#[cfg(test)]
mod tests {
    use rusty_music::synth::SynthPatch;

    /// The theremin parses, and its knobs are in the order `setup_band`'s
    /// `Responds::knob` indices assume.
    #[test]
    fn theremin_patch_parses_with_rate_then_wobble() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/synths/theremin.glicol"
        ))
        .expect("theremin.glicol exists");
        let patch = SynthPatch::from_glicol("theremin", &source).expect("theremin parses");
        let program = patch.glicol.expect("a glicol patch");
        let knobs: Vec<&str> = program
            .params
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(knobs, ["rate", "wobble"]);
        assert_eq!(program.cutoff, Some(2_400.0));
    }
}
