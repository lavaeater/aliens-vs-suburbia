//! `--shot <seconds> <file.png>`: save one screenshot of whatever is on screen after that
//! many seconds, then quit. For checking a screen without sitting in front of it
//! (`cargo run -- --campaign-editor --shot 6 shots/editor.png`).

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

pub struct ShotPlugin {
    pub after: f32,
    pub path: std::path::PathBuf,
}

#[derive(Resource)]
struct Shot {
    after: f32,
    path: std::path::PathBuf,
    taken_at: Option<f32>,
}

impl Plugin for ShotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Shot {
            after: self.after,
            path: self.path.clone(),
            taken_at: None,
        })
        .add_systems(Update, take_shot);
    }
}

/// `--shot <seconds> <path>` from the arguments, if given.
pub fn from_args(args: &[String]) -> Option<ShotPlugin> {
    let at = args.iter().position(|a| a == "--shot")?;
    let after = args.get(at.saturating_add(1))?.parse().ok()?;
    let path = args.get(at.saturating_add(2))?.into();
    Some(ShotPlugin { after, path })
}

fn take_shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut shot: ResMut<Shot>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    match shot.taken_at {
        None if now >= shot.after => {
            if let Some(dir) = shot.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(shot.path.clone()));
            shot.taken_at = Some(now);
            info!("--shot: {} at {now:.1} s", shot.path.display());
        }
        // Quit once the screenshot is on disk (a slow frame rate takes a while to read it
        // back), or give up after a while.
        Some(taken) if shot.path.is_file() || now - taken > 20.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
