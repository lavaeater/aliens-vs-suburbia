//! `--perf`: measure what a frame really costs (`docs/performance.md`).
//!
//! Logs frame time, entity count, each render pass's CPU and GPU time, and the physics step
//! (`physics/step`, wall time per fixed step, `physics/bodies` and `aliens`) every few seconds.
//! Presents uncapped, so vsync neither hides spare time nor stalls the game when the
//! compositor is not showing the window, and skips story crawls so a match actually runs.

use bevy::diagnostic::{
    Diagnostic, DiagnosticPath, Diagnostics, EntityCountDiagnosticsPlugin,
    FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin, RegisterDiagnostic,
};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::window::PresentMode;
use core::time::Duration;
use bevy::platform::time::Instant;
use lava_ui_builder::crawl::AdvanceCrawl;

use crate::alien::components::general::Alien;
use crate::physics::prelude::{PhysicsStep, RigidBody};

const LOG_EVERY: Duration = Duration::from_secs(5);
const CRAWL_SKIP_SECS: f32 = 1.0;

/// Wall time of one fixed physics step, in ms.
pub const PHYSICS_STEP: DiagnosticPath = DiagnosticPath::const_new("physics/step");
/// Rigid bodies in the world.
pub const PHYSICS_BODIES: DiagnosticPath = DiagnosticPath::const_new("physics/bodies");

/// Aliens alive.
pub const ALIENS: DiagnosticPath = DiagnosticPath::const_new("aliens");

/// When the current physics step started.
#[derive(Resource, Default)]
struct StepClock(Option<Instant>);

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
        .register_diagnostic(Diagnostic::new(PHYSICS_STEP).with_suffix("ms"))
        .register_diagnostic(Diagnostic::new(PHYSICS_BODIES))
        .register_diagnostic(Diagnostic::new(ALIENS))
        .init_resource::<StepClock>()
        .add_systems(
            FixedPostUpdate,
            (
                start_step.before(PhysicsStep),
                end_step.after(PhysicsStep),
            ),
        )
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

fn start_step(mut clock: ResMut<StepClock>) {
    clock.0 = Some(Instant::now());
}

fn end_step(
    mut clock: ResMut<StepClock>,
    mut diagnostics: Diagnostics,
    bodies: Query<(), With<RigidBody>>,
    aliens: Query<(), With<Alien>>,
) {
    if let Some(start) = clock.0.take() {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        diagnostics.add_measurement(&PHYSICS_STEP, || ms);
    }
    let count = bodies.iter().count() as f64;
    diagnostics.add_measurement(&PHYSICS_BODIES, || count);
    let aliens = aliens.iter().count() as f64;
    diagnostics.add_measurement(&ALIENS, || aliens);
}
