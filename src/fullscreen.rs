//! `--fullscreen`: the game in 1920x1080 fullscreen. Exclusive fullscreen in that video
//! mode (the fastest refresh it offers) when the monitor has one, borderless otherwise. The
//! monitor is only known once winit has put the window on one (`OnMonitor`), so the switch
//! waits for that. Wayland has no exclusive fullscreen (winit ignores it): there it is always
//! borderless, at the monitor's own size.

use bevy::prelude::*;
use bevy::window::{
    Monitor, MonitorSelection, OnMonitor, PrimaryWindow, VideoMode, VideoModeSelection,
    WindowMode, WindowResolution,
};

pub const SIZE: UVec2 = UVec2::new(1920, 1080);

/// winit picks Wayland over X11 whenever it can.
pub fn on_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// The window the game opens with under `--fullscreen`: already 1920x1080, so nothing
/// resizes when the mode switches.
pub fn window() -> Window {
    Window {
        title: "Aliens vs Suburbia".into(),
        resolution: WindowResolution::new(SIZE.x, SIZE.y),
        ..default()
    }
}

/// 1920x1080 at the highest refresh rate (then bit depth) on offer, else (or without
/// `exclusive`) borderless.
pub fn mode_for(modes: &[VideoMode], exclusive: bool) -> WindowMode {
    modes
        .iter()
        .filter(|mode| exclusive && mode.physical_size == SIZE)
        .max_by_key(|mode| (mode.refresh_rate_millihertz, mode.bit_depth))
        .map_or(
            WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            |mode| WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Specific(*mode)),
        )
}

pub struct FullscreenPlugin;

impl Plugin for FullscreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, go_fullscreen);
    }
}

fn go_fullscreen(
    mut done: Local<bool>,
    monitors: Query<&Monitor>,
    mut windows: Query<(&mut Window, &OnMonitor), With<PrimaryWindow>>,
) {
    if *done {
        return;
    }
    let Ok((mut window, on)) = windows.single_mut() else {
        return;
    };
    let Ok(monitor) = monitors.get(on.0) else {
        return;
    };
    window.mode = mode_for(&monitor.video_modes, !on_wayland());
    info!("fullscreen: {:?}", window.mode);
    *done = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn mode(x: u32, y: u32, hz: u32) -> VideoMode {
        VideoMode {
            physical_size: UVec2::new(x, y),
            bit_depth: 32,
            refresh_rate_millihertz: hz * 1000,
        }
    }

    #[test]
    fn the_fastest_1080p_mode_wins_and_without_one_it_is_borderless() {
        let modes = [mode(2560, 1440, 144), mode(1920, 1080, 60), mode(1920, 1080, 120)];
        assert_eq!(
            mode_for(&modes, true),
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Specific(modes[2]))
        );
        assert_eq!(
            mode_for(&[mode(2560, 1440, 144)], true),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
        assert_eq!(
            mode_for(&modes, false),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            "Wayland"
        );
    }
}
