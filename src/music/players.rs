use bevy::asset::Handle;
use bevy_seedling::prelude::AudioSample;
use rusty_music::clock::Beat;
use rusty_music::instrument::{Channels, Instrument, NoteEvent};
use rusty_music::musicians::{Chord, MusicPlayer, Note, Sampler};

/// One `Sampler` per sample, addressed by `NoteEvent::channel`.
fn sample_channels(samples: &[Handle<AudioSample>], volume_db: f32) -> Box<dyn Instrument> {
    Box::new(
        samples
            .iter()
            .fold(Channels::default(), |channels, handle| {
                channels.with(Sampler {
                    handle: handle.clone(),
                    volume: f64::from(volume_db),
                })
            }),
    )
}

/// Plays nothing by itself: an instrument reserved for `Stinger`s.
pub struct StingerOnly;

impl MusicPlayer for StingerOnly {
    fn play(&mut self, _: Beat, _: &mut Vec<NoteEvent>, _: f32, _: &Chord) {}
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
