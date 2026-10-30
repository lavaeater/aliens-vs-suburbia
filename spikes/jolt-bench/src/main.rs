//! `jolt-bench <avian|jolt> [aliens] [jolt-workers]`
//!
//! Steps the scene of `scene.rs` for `TICKS` fixed steps of 0.05 s in a headless Bevy app and
//! prints the time of each whole `App::update` and of the physics step alone, over the warm
//! ticks (61 on). One engine per process, so neither warms the other's caches.

mod avian_engine;
mod jolt_engine;
mod scene;

use std::time::{Duration, Instant};

use bevy::app::PluginsState;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use scene::{Rng, STEP, Scene, Stats};

const TICKS: usize = 600;
const WARM_FROM: usize = 60;

fn main() {
    let mut args = std::env::args().skip(1);
    let engine = args.next().unwrap_or_else(|| "jolt".into());
    let aliens: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(200);
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get() as u32);
    let workers: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or((cores - 1).max(1));

    let step = Duration::from_secs_f32(STEP);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .insert_resource(Time::<Fixed>::from_duration(step))
        .insert_resource(TimeUpdateStrategy::ManualDuration(step))
        .insert_resource(Scene::new(aliens))
        .insert_resource(Rng(42))
        .init_resource::<Stats>();
    let label = match engine.as_str() {
        "avian" => {
            avian_engine::build(&mut app);
            "avian".to_string()
        }
        "jolt" => {
            jolt_engine::build(&mut app, workers);
            format!("jolt-{workers}w")
        }
        other => panic!("unknown engine {other}: avian or jolt"),
    };
    while app.plugins_state() != PluginsState::Ready {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();

    let mut update = Vec::with_capacity(TICKS);
    let mut physics = Vec::with_capacity(TICKS);
    for _ in 0..TICKS {
        let started = Instant::now();
        app.update();
        update.push(started.elapsed());
        physics.push(app.world().resource::<Stats>().physics_time);
    }

    let bodies = match engine.as_str() {
        "avian" => avian_engine::body_count(app.world_mut()),
        _ => jolt_engine::body_count(app.world_mut()),
    };
    let stats = app.world().resource::<Stats>();
    let walls = app.world().resource::<Scene>().walls.len();
    println!(
        "{label:>10} aliens={aliens:<4} walls={walls} moving_bodies={bodies:<5} ticks={} \
         update[{}] physics[{}] wall_hits={} shot_hits={} arrivals={}",
        stats.tick,
        summary(&update[WARM_FROM..]),
        summary(&physics[WARM_FROM..]),
        stats.wall_hits,
        stats.shot_hits,
        stats.arrivals,
    );
}

fn summary(samples: &[Duration]) -> String {
    let mut ms: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1e3).collect();
    ms.sort_by(f64::total_cmp);
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    let p95 = ms[(ms.len() * 95 / 100).min(ms.len() - 1)];
    format!("mean {mean:.3} p95 {p95:.3} max {:.3} ms", ms[ms.len() - 1])
}
