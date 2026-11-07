//! `--retro-shots <dir>`: a contact sheet of the palettes. Once a match has been running a
//! while, skip the crawls, turn the retro look on and save one screenshot per palette
//! (full colour first) to `<dir>/<nn>-<palette>.png`, then quit.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use lava_ui_builder::crawl::ClearCrawls;

use super::{Palettes, RetroLook};
use crate::game_state::GameState;

/// Seconds into the match before the first shot (the map streams in, the crawl goes).
const SETTLE: f32 = 8.0;
/// Frames to draw after a palette change before capturing, and before quitting.
const FRAMES: u32 = 4;

#[derive(Resource)]
pub struct RetroShots {
    pub dir: std::path::PathBuf,
    /// The palette index to shoot next.
    pub next: usize,
    pub timer: f32,
    /// Frames drawn since `next` was set.
    pub frames: Option<u32>,
}

pub fn take_retro_shots(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut shots: ResMut<RetroShots>,
    mut look: ResMut<RetroLook>,
    palettes: Res<Palettes>,
    mut clear: MessageWriter<ClearCrawls>,
    mut exit: MessageWriter<AppExit>,
) {
    shots.timer += time.delta_secs();
    if shots.timer < SETTLE {
        clear.write(ClearCrawls);
        look.enabled = true;
        return;
    }
    let Some(frames) = shots.frames else {
        look.palette = shots.next;
        shots.frames = Some(0);
        return;
    };
    shots.frames = Some(frames + 1);
    if frames < FRAMES {
        return;
    }
    if shots.next > palettes.0.len() {
        exit.write(AppExit::Success);
        return;
    }
    let path = shots.dir.join(format!(
        "{:02}-{}.png",
        shots.next,
        palettes.name(shots.next).replace(' ', "-")
    ));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    shots.next += 1;
    // Past the last palette, keep counting frames so the last shot is saved before quitting.
    if shots.next <= palettes.0.len() {
        shots.frames = None;
    } else {
        shots.frames = Some(0);
    }
}

pub fn plugin(app: &mut App, dir: std::path::PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    app.insert_resource(RetroShots {
        dir,
        next: 0,
        timer: 0.0,
        frames: None,
    })
    .add_systems(Update, take_retro_shots.run_if(in_state(GameState::InGame)));
}
