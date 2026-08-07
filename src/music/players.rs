use bevy::asset::Handle;
use bevy::prelude::Commands;
use bevy_seedling::prelude::{AudioSample, PlaybackSettings, SamplePlayer, Volume};
use rusty_music::clock::Beat;
use rusty_music::musicians::{midi_diff_to_pitch, Chord, MusicPlayer, Note, Sampler, TonalPlayer};

fn spawn_note(
    commands: &mut Commands,
    handle: &Handle<AudioSample>,
    volume_db: f32,
    midi_note_diff: i32,
) {
    commands.spawn((
        SamplePlayer::new(handle.clone()).with_volume(Volume::Decibels(volume_db)),
        PlaybackSettings::default().with_speed(midi_diff_to_pitch(midi_note_diff)),
    ));
}

/// Sustained chord layer. Triggers on quarter-note positions; more positions
/// become active and the volume swells as intensity rises.
pub struct PadPlayer {
    pub sampler: Sampler,
}

impl MusicPlayer for PadPlayer {
    fn play(&mut self, beat: Beat, commands: &mut Commands, base_intensity: f32, chord: &Chord) {
        if beat.sixteenth != 0 {
            return;
        }
        let active = match beat.beat {
            0 => true,
            2 => base_intensity > 0.3,
            1 | 3 => base_intensity > 0.65,
            _ => false,
        };
        if !active {
            return;
        }
        if let Some(note) = TonalPlayer::get_chord_note(chord, 1.0 - base_intensity) {
            let vol = self.sampler.volume as f32 - 2.0 + base_intensity * 4.0;
            spawn_note(commands, &self.sampler.handle, vol, note.midi_note_diff);
        }
    }
}

/// Surf-spy chord stabs on the "and" of 2 and 4. Near the climax an extra
/// syncopated stab appears and a second voice thickens the hit. Cycles
/// through its samples for timbral variety.
pub struct ChordStabs {
    pub samples: Vec<Handle<AudioSample>>,
    pub volume: f32,
}

impl MusicPlayer for ChordStabs {
    fn play(&mut self, beat: Beat, commands: &mut Commands, base_intensity: f32, chord: &Chord) {
        if self.samples.is_empty() {
            return;
        }
        let hit = match (beat.beat, beat.sixteenth) {
            (1, 2) | (3, 2) => true,
            (2, 2) => base_intensity > 0.8,
            _ => false,
        };
        if !hit {
            return;
        }
        let voices: usize = if base_intensity > 0.75 { 2 } else { 1 };
        let min_strength = 1.0 - base_intensity;
        let strong: Vec<&Note> = chord
            .chord_notes
            .iter()
            .filter(|n| n.strength >= min_strength)
            .take(voices)
            .collect();
        for (i, note) in strong.iter().enumerate() {
            let idx = (beat.bar_count as usize + beat.beat as usize + i) % self.samples.len();
            spawn_note(
                commands,
                &self.samples[idx],
                self.volume - i as f32 * 2.0,
                note.midi_note_diff,
            );
        }
    }
}

/// Alien-danger FX layer: eerie saucer swells on alternating bar downbeats,
/// with SID-chip blips creeping onto offbeats as the danger rises.
pub struct UfoStingers {
    pub ufo: Handle<AudioSample>,
    pub sid: Handle<AudioSample>,
    pub volume: f32,
}

impl MusicPlayer for UfoStingers {
    fn play(&mut self, beat: Beat, commands: &mut Commands, base_intensity: f32, chord: &Chord) {
        if beat.beat == 0 && beat.sixteenth == 0 && beat.bar_count.is_multiple_of(2) {
            if let Some(note) = TonalPlayer::get_scale_note(chord, 1.0 - base_intensity) {
                spawn_note(commands, &self.ufo, self.volume, note.midi_note_diff);
            }
        }
        if base_intensity > 0.5 && beat.sixteenth == 3 && beat.beat.is_multiple_of(2) {
            if let Some(note) = TonalPlayer::get_chord_note(chord, 0.5) {
                spawn_note(commands, &self.sid, self.volume - 4.0, note.midi_note_diff + 12);
            }
        }
    }
}
