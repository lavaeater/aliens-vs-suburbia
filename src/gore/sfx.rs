//! Sound effects for the carnage. Combat/gore messages ([`DamageDealt`],
//! [`EntityDied`], [`SpawnFire`], gunfire) are translated into [`PlaySfx`] requests,
//! which pick a random sample for that kind and play it once with pitch/gain jitter,
//! capped so a big wave doesn't become white noise.
//!
//! **Asset-free until you add samples.** At startup this scans `assets/sfx/` and loads
//! every `.wav` whose name starts with a kind prefix (`hit*.wav`, `death*.wav`,
//! `gib*.wav`, `fire*.wav`, `shoot*.wav`, `bark*.wav`). No files -> silent, no errors.
//! Drop several variants per kind (e.g. `hit1.wav`, `hit2.wav`) and they'll be chosen at
//! random. Re-encode rejected wavs with `ffmpeg -i in.wav -c:a pcm_s16le out.wav`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_seedling::prelude::{AudioSample, PlaybackSettings, SamplePlayer, Volume};

use crate::game_state::score_keeper::GameTrackingEvent;
use crate::gore::components::{DamageDealt, DamageKind, EntityDied};
use crate::gore::fire::SpawnFire;

/// A category of sound effect. The prefix is matched against filenames in `assets/sfx/`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SfxKind {
    Hit,
    Death,
    Gib,
    Fire,
    Shoot,
    Bark,
    /// The despair heartbeat when a player is bleeding out (see `despair.rs`).
    Heartbeat,
}

impl SfxKind {
    fn prefix(self) -> &'static str {
        match self {
            SfxKind::Hit => "hit",
            SfxKind::Death => "death",
            SfxKind::Gib => "gib",
            SfxKind::Fire => "fire",
            SfxKind::Shoot => "shoot",
            SfxKind::Bark => "bark",
            SfxKind::Heartbeat => "heartbeat",
        }
    }

    const ALL: [SfxKind; 7] = [
        SfxKind::Hit,
        SfxKind::Death,
        SfxKind::Gib,
        SfxKind::Fire,
        SfxKind::Shoot,
        SfxKind::Bark,
        SfxKind::Heartbeat,
    ];
}

/// Request to play a one-shot sound. `gain_db` is the base level (negative = quieter).
#[derive(Message, Clone, Copy)]
pub struct PlaySfx {
    pub kind: SfxKind,
    pub gain_db: f32,
}

/// The loaded samples, grouped by kind. Empty groups are simply silent.
#[derive(Resource, Default)]
pub struct SfxBank {
    samples: HashMap<SfxKind, Vec<Handle<AudioSample>>>,
}

/// Marks a live one-shot voice so we can cap concurrency.
#[derive(Component)]
pub(crate) struct SfxVoice;

/// At most this many gore voices at once.
const MAX_VOICES: usize = 24;

/// Scan `assets/sfx/` and load whatever's there, grouped by filename prefix.
pub fn setup_sfx_bank(asset_server: Res<AssetServer>, mut commands: Commands) {
    let mut bank = SfxBank::default();

    match std::fs::read_dir("assets/sfx") {
        Ok(entries) => {
            let mut loaded = 0usize;
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if !lower.ends_with(".wav") {
                    continue;
                }
                if let Some(kind) = SfxKind::ALL.into_iter().find(|k| lower.starts_with(k.prefix())) {
                    let handle = asset_server.load(format!("sfx/{name}"));
                    bank.samples.entry(kind).or_default().push(handle);
                    loaded += 1;
                }
            }
            if loaded > 0 {
                info!("gore sfx: loaded {loaded} samples from assets/sfx/");
            }
        }
        Err(_) => {
            // No assets/sfx dir yet — that's fine, the game just runs quiet.
        }
    }

    commands.insert_resource(bank);
}

/// Turn combat/gore messages into sound requests. Fire damage is per-tick and noisy, so
/// it's covered once by the `SpawnFire` whoosh rather than by each `DamageDealt(Fire)`.
pub fn emit_combat_sfx(
    mut damage: MessageReader<DamageDealt>,
    mut deaths: MessageReader<EntityDied>,
    mut fires: MessageReader<SpawnFire>,
    mut tracking: MessageReader<GameTrackingEvent>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    for hit in damage.read() {
        if hit.kind == DamageKind::Fire || hit.lethal {
            continue; // fire handled by SpawnFire; kills handled by EntityDied
        }
        sfx.write(PlaySfx { kind: SfxKind::Hit, gain_db: -7.0 });
    }
    for _ in deaths.read() {
        sfx.write(PlaySfx { kind: SfxKind::Death, gain_db: -3.0 });
        sfx.write(PlaySfx { kind: SfxKind::Gib, gain_db: -9.0 });
    }
    for _ in fires.read() {
        sfx.write(PlaySfx { kind: SfxKind::Fire, gain_db: -5.0 });
    }
    for ev in tracking.read() {
        if matches!(ev, GameTrackingEvent::ShotFired(_)) {
            sfx.write(PlaySfx { kind: SfxKind::Shoot, gain_db: -8.0 });
        }
    }
}

/// Play requested sounds: random sample per kind, pitch/gain jitter, concurrency-capped.
pub fn play_sfx(
    mut msgs: MessageReader<PlaySfx>,
    bank: Res<SfxBank>,
    voices: Query<(), With<SfxVoice>>,
    mut commands: Commands,
    mut seed: Local<u32>,
) {
    let mut live = voices.iter().count();

    for msg in msgs.read() {
        if live >= MAX_VOICES {
            break;
        }
        let Some(handles) = bank.samples.get(&msg.kind) else { continue };
        if handles.is_empty() {
            continue;
        }

        *seed = seed.wrapping_add(0x9E3779B9).wrapping_mul(2654435761);
        let pick = (*seed >> 16) as usize % handles.len();
        // +/-8% pitch and +/-2 dB so repeats don't sound identical.
        let pitch = 1.0 + (((*seed >> 8) & 0xff) as f64 / 255.0 - 0.5) * 0.16;
        let gain = msg.gain_db + (((*seed >> 20) & 0xff) as f32 / 255.0 - 0.5) * 4.0;

        commands.spawn((
            SfxVoice,
            SamplePlayer::new(handles[pick].clone()).with_volume(Volume::Decibels(gain)),
            PlaybackSettings::default().with_speed(pitch),
        ));
        live += 1;
    }
}
