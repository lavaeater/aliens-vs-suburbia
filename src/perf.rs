//! `--perf`: measure what a frame really costs (`docs/performance.md`).
//!
//! Logs frame time, entity count and each render pass's CPU and GPU time every few seconds.
//! Presents uncapped, so vsync neither hides spare time nor stalls the game when the
//! compositor is not showing the window, and skips story crawls so a match actually runs.

use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::window::PresentMode;
use core::time::Duration;
use lava_ui_builder::crawl::AdvanceCrawl;

const LOG_EVERY: Duration = Duration::from_secs(5);
const CRAWL_SKIP_SECS: f32 = 1.0;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            RenderDiagnosticsPlugin,
            LogDiagnosticsPlugin {
                wait_duration: LOG_EVERY,
                ..default()
            },
        ))
        .add_systems(Update, (present_uncapped, skip_crawls));
    }
}

fn present_uncapped(mut windows: Query<&mut Window>) {
    for mut window in &mut windows {
        // Reading through `Mut` doesn't mark it changed; only a real change should.
        if window.present_mode != PresentMode::AutoNoVsync {
            window.present_mode = PresentMode::AutoNoVsync;
        }
    }
}

fn skip_crawls(
    mut advance: MessageWriter<AdvanceCrawl>,
    time: Res<Time<Real>>,
    mut next: Local<f32>,
) {
    if time.elapsed_secs() > *next {
        *next = time.elapsed_secs() + CRAWL_SKIP_SECS;
        advance.write(AdvanceCrawl);
    }
}
