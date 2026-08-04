//! The playground's two-pane shell: a tweaking panel on the left, the live game on the
//! right.
//!
//! The right pane is not a separate render — it is the ordinary game camera with its
//! `viewport` clipped to the pane's rectangle, the same trick the asset browser uses in
//! `asset_browser::viewer::sync_viewer_viewport`. That keeps every gameplay system,
//! including the camera follow, working exactly as it does full-screen.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, TextTheme, UIBuilder};

use crate::camera::components::GameCamera;
use crate::game_state::GameState;
use crate::playground::state::PlaygroundSession;
use crate::ui::spawn_ui::StateMarker;

/// The right-hand pane. Its computed rectangle drives the game camera's viewport.
#[derive(Component)]
pub struct PlaygroundViewportPane;

/// Container for the tweak controls, so later stages can refill it without rebuilding
/// the whole screen.
#[derive(Component)]
pub struct PlaygroundPanel;

pub fn spawn_playground_ui(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row();

    let t = theme.text.clone();
    let hint = TextTheme { label_size: 11.0, label_color: Color::srgb(0.45, 0.6, 0.5), ..t.clone() };

    ui.with_child(|left| {
        left.modify_node(|mut n| {
            n.width = Val::Percent(26.0);
            n.min_width = Val::Px(260.0);
            n.max_width = Val::Px(520.0);
            n.height = Val::Percent(100.0);
        })
        .display_flex()
        .flex_column()
        .gap_px(6.0)
        .padding_all_px(10.0)
        .bg_color(Color::srgba(0.04, 0.07, 0.10, 0.97));

        left.with_child(|c| { c.insert_bundle(lava_ui_builder::header("Playground", &t)); });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "WASD / left stick to move, mouse / right stick to aim",
                &hint,
            ));
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "[F3] physics debug   [F7] torso twist",
                &hint,
            ));
        });

        // Filled in by later stages (model list, hardpoints, animation, settings).
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(6.0).insert(PlaygroundPanel)
             .modify_node(|mut n| {
                 n.flex_grow = 1.0;
                 n.align_self = AlignSelf::Stretch;
             });
        });

        left.add_button_observe(
            "Back to Menu",
            |b| { b.size_px(160.0, 40.0).font_size(16.0); },
            |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
                next_state.set(GameState::Menu);
            },
        );
    });

    // Right pane: empty node, exists only to give the game camera a rectangle.
    ui.with_child(|right| {
        right.modify_node(|mut n| {
            n.flex_grow = 1.0;
            n.height = Val::Percent(100.0);
        })
        .insert(PlaygroundViewportPane);
    });

    ui.build();
}

/// Clip the game camera to the right-hand pane. Runs every frame so the viewport tracks
/// window resizes and any future resizing of the panel.
pub fn sync_playground_viewport(
    panes: Query<(&ComputedNode, &UiGlobalTransform), With<PlaygroundViewportPane>>,
    mut cameras: Query<&mut Camera, With<GameCamera>>,
    windows: Query<&Window>,
) {
    let Ok((node, transform)) = panes.single() else { return };
    let Ok(mut camera) = cameras.single_mut() else { return };
    let Ok(window) = windows.single() else { return };

    let size = node.size();
    let top_left = transform.affine().translation - size * 0.5;

    let Some(viewport) = pane_viewport(
        top_left.x,
        top_left.y,
        size.x,
        size.y,
        window.physical_width(),
        window.physical_height(),
    ) else {
        return;
    };
    camera.viewport = Some(viewport);
}

/// Clamp a pane rectangle to the window. Returns `None` while the rectangle is degenerate
/// — a zero-sized viewport makes wgpu complain, and the UI reports zero on the first frame
/// before layout has run.
fn pane_viewport(
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    window_width: u32,
    window_height: u32,
) -> Option<bevy::camera::Viewport> {
    let x = left.max(0.0) as u32;
    let y = top.max(0.0) as u32;
    let w = (width as u32).min(window_width.saturating_sub(x));
    let h = (height as u32).min(window_height.saturating_sub(y));
    if w == 0 || h == 0 {
        return None;
    }
    Some(bevy::camera::Viewport {
        physical_position: UVec2::new(x, y),
        physical_size: UVec2::new(w, h),
        depth: 0.0..1.0,
    })
}

/// Hand the camera back when the session ends, so a following real match is not stuck
/// rendering into the right-hand quarter of the window.
pub fn clear_playground_viewport(mut cameras: Query<&mut Camera, With<GameCamera>>) {
    for mut camera in cameras.iter_mut() {
        camera.viewport = None;
    }
}

/// Leaving `InGame` ends the session; without this the resource would leak into the next
/// real match and quietly suppress the HUD and the map.
pub fn end_playground_session(mut commands: Commands) {
    commands.remove_resource::<PlaygroundSession>();
}

#[cfg(test)]
mod tests {
    use super::pane_viewport;

    #[test]
    fn a_pane_becomes_the_matching_viewport_rect() {
        let v = pane_viewport(400.0, 0.0, 1200.0, 900.0, 1600, 900).expect("valid rect");
        assert_eq!(v.physical_position.to_array(), [400, 0]);
        assert_eq!(v.physical_size.to_array(), [1200, 900]);
    }

    #[test]
    fn a_pane_hanging_off_the_window_is_clamped_to_it() {
        // Mid-resize the UI can report a rectangle wider than the window still is.
        let v = pane_viewport(400.0, 0.0, 1400.0, 900.0, 1600, 900).expect("valid rect");
        assert_eq!(v.physical_size.to_array(), [1200, 900], "clamped to the window edge");
    }

    #[test]
    fn a_zero_sized_pane_is_refused_rather_than_sent_to_the_gpu() {
        assert!(pane_viewport(0.0, 0.0, 0.0, 0.0, 1600, 900).is_none(), "first frame");
        assert!(pane_viewport(1600.0, 0.0, 100.0, 900.0, 1600, 900).is_none(), "fully off-screen");
    }

    #[test]
    fn a_negative_origin_is_pulled_back_to_the_window_corner() {
        let v = pane_viewport(-20.0, -10.0, 500.0, 400.0, 1600, 900).expect("valid rect");
        assert_eq!(v.physical_position.to_array(), [0, 0]);
    }
}
