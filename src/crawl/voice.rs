//! A crawl's voice: a clip per line (`voice: [...]` in the crawl file, inside `assets/`,
//! in the order of `lines`; `""` for a line without one). The crawl reports each line as
//! it comes up (`CrawlLineReached`) and its clip plays; while one is still speaking the
//! crawl holds the next voiced line back (`CrawlQueue::set_voice_busy`), so text and voice
//! stay together at any screen size. A new line's clip cuts the one before (a typewriter
//! page turned early); a crawl skipped, cleared or ended stops it.

use bevy::prelude::*;
use bevy_seedling::prelude::{AudioSample, SamplePlayer, Volume};
use lava_ui_builder::crawl::{CrawlLineReached, CrawlQueue};

/// Voices over the music, in dB.
const VOICE_DB: f32 = 0.0;

/// The clip speaking, and the crawl it belongs to.
#[derive(Resource, Default)]
pub struct CrawlVoice {
    playing: Option<(Entity, String)>,
}

#[derive(Component)]
pub struct CrawlVoiceClip;

/// A reached line's clip starts, cutting any before it.
pub fn play_line_voices(
    mut commands: Commands,
    mut reached: MessageReader<CrawlLineReached>,
    mut voice: ResMut<CrawlVoice>,
    asset_server: Res<AssetServer>,
) {
    for line in reached.read() {
        let Some(path) = &line.voice else {
            continue;
        };
        if let Some((old, _)) = voice.playing.take() {
            commands.entity(old).try_despawn();
        }
        let clip = asset_server.load::<AudioSample>(path.clone());
        let entity = commands
            .spawn((
                Name::new(format!("Crawl voice {} {}", line.id, line.line)),
                CrawlVoiceClip,
                SamplePlayer::new(clip).with_volume(Volume::Decibels(VOICE_DB)),
            ))
            .id();
        voice.playing = Some((entity, line.id.clone()));
    }
}

/// Tells the crawl whether a clip is still speaking, and stops one whose crawl is gone or
/// fading out. A clip despawns itself when it ends; one that fails to load is dropped so
/// the crawl does not wait on it forever.
pub fn track_voice(
    mut commands: Commands,
    mut voice: ResMut<CrawlVoice>,
    mut queue: ResMut<CrawlQueue>,
    clips: Query<&SamplePlayer, With<CrawlVoiceClip>>,
    asset_server: Res<AssetServer>,
) {
    if let Some((entity, crawl)) = &voice.playing {
        let entity = *entity;
        let ours = queue.active().is_some_and(|spec| &spec.id == crawl) && !queue.is_closing();
        let failed = clips.get(entity).is_ok_and(|player| {
            let failed = asset_server.load_state(&player.sample).is_failed();
            if failed {
                warn!("crawl voice {:?} did not load", player.sample.path());
            }
            failed
        });
        // `play_line_voices` runs just before (chained), so a new clip is spawned by now.
        let ended = clips.get(entity).is_err();
        if !ours || failed || ended {
            commands.entity(entity).try_despawn();
            voice.playing = None;
        }
    }
    let busy = voice.playing.is_some();
    queue.set_voice_busy(busy);
}
