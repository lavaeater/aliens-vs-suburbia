use bevy::asset::Handle;
use bevy_seedling::prelude::AudioSample;
use rusty_music::clock::Beat;
use rusty_music::instrument::{Channels, Instrument, NoteEvent};
use rusty_music::musicians::{Chord, MusicPlayer, Note, Sampler, TonalPlayer};

/// One `Sampler` per sample, addressed by `NoteEvent::channel`.
fn sample_channels(samples: &[Handle<AudioSample>], volume_db: f32) -> Box<dyn Instrument> {
    Box::new(samples.iter().fold(Channels::default(), |channels, handle| {
        channels.with(Sampler {
            handle: handle.clone(),
            volume: volume_db as f64,
        })
    }))
}

/// Sustained chord layer. Triggers on quarter-note positions; more positions
/// become active and the volume swells as intensity rises.
pub struct PadPlayer {
    pub sampler: Sampler,
}

impl MusicPlayer for PadPlayer {
    fn take_instrument(&mut self) -> Option<Box<dyn Instrument>> {
        Some(Box::new(Sampler {
            handle: self.sampler.handle.clone(),
            volume: self.sampler.volume,
        }))
    }

    fn play(&mut self, beat: Beat, notes: &mut Vec<NoteEvent>, base_intensity: f32, chord: &Chord) {
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
            notes.push(NoteEvent::from_diff(note.midi_note_diff).gain_db(-2.0 + base_intensity * 4.0));
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
    fn take_instrument(&mut self) -> Option<Box<dyn Instrument>> {
        Some(sample_channels(&self.samples, self.volume))
    }

    fn play(&mut self, beat: Beat, notes: &mut Vec<NoteEvent>, base_intensity: f32, chord: &Chord) {
        if self.samples.is_empty() {
            return;
        }
        let hit = match (beat.beat, beat.sixteenth) {
            (1 | 3, 2) => true,
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
            notes.push(
                NoteEvent::from_diff(note.midi_note_diff)
                    .channel(idx)
                    .gain_db(-(i as f32) * 2.0),
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

impl UfoStingers {
    const UFO: usize = 0;
    const SID: usize = 1;
}

impl MusicPlayer for UfoStingers {
    fn take_instrument(&mut self) -> Option<Box<dyn Instrument>> {
        Some(sample_channels(&[self.ufo.clone(), self.sid.clone()], self.volume))
    }

    fn play(&mut self, beat: Beat, notes: &mut Vec<NoteEvent>, base_intensity: f32, chord: &Chord) {
        if beat.beat == 0
            && beat.sixteenth == 0
            && beat.bar_count.is_multiple_of(2)
            && let Some(note) = TonalPlayer::get_scale_note(chord, 1.0 - base_intensity)
        {
            notes.push(NoteEvent::from_diff(note.midi_note_diff).channel(Self::UFO));
        }
        if base_intensity > 0.5
            && beat.sixteenth == 3
            && beat.beat.is_multiple_of(2)
            && let Some(note) = TonalPlayer::get_chord_note(chord, 0.5)
        {
            notes.push(
                NoteEvent::from_diff(note.midi_note_diff + 12)
                    .channel(Self::SID)
                    .gain_db(-4.0),
            );
        }
    }
}
