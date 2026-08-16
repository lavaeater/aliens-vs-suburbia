//! `F10` — the procedural walk's tuning panel.
//!
//! Same shape as the camera and model panels in `spawn_ui`: a column of
//! `[ << ][ < ] value [ > ][ >> ]` rows that write a resource and save it.
//!
//! The bottom half is not editable. Gait parameters are stated at human scale and then
//! rescaled to the rig and *clamped to what its legs can reach*, so the number typed into
//! the top half is frequently not the number the feet are walking to. Without somewhere to
//! read the result, tuning is guesswork — the stride can be dragged from 1.6 to 3.0 with no
//! visible effect whatsoever because the clamp was already binding at 1.6.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, TextStyle, TextTheme, UIBuilder};

use crate::ui::spawn_ui::StateMarker;
use crate::player::systems::gait::GaitParams;
use crate::player::systems::leg_ik::{GaitReadout, GaitSettings, LegIkEnabled};
use crate::settings::resources::GameSettings;

#[derive(Component, Default)]
pub struct GaitPanel;

/// Identifies a value label in the panel. One component type covers every row.
#[derive(Component, Clone, Copy)]
pub enum GaitLabel {
    // Editable.
    Stride,
    Stance,
    StepHeight,
    /// Where the footfalls sit fore and aft of the hips, in strides.
    StrideBias,
    /// The straightest the knee may be solved, in degrees.
    KneeStraight,
    /// The most the knee may fold, in degrees.
    KneeBent,
    /// Hip rise and fall over the cycle, as a fraction of leg length.
    HipBob,
    /// Hip height as a fraction of leg length; 0 leaves it to the animation.
    HipTarget,
    Duty,
    Speed,
    LegIk,
    // Read-only: what the rig actually ended up walking with.
    Scale,
    LegLength,
    /// The furthest the ankle can get from the hip, once the knee has its say.
    Furthest,
    HipHeight,
    Reach,
    FittedStride,
    StepsPerSecond,
}

pub fn spawn_gait_panel(commands: Commands, theme: &LavaTheme) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<GaitPanel>()
        .display_none()
        .absolute_position()
        .top(px(8.0))
        .right(px(724.0))
        .flex_column()
        .row_gap_px(6.0)
        .padding_all_px(12.0)
        .min_width_px(350.0)
        .bg_color(Color::srgba(0.13, 0.07, 0.05, 0.92))
        .insert(StateMarker);

    let t = ui.theme().text.clone();
    ui.themed_header("Gait  [F10]");

    // Stated at human scale: a person with a 0.85 m leg. Every rig is scaled from here.
    gait_row(
        &mut ui,
        "Stride",
        &t,
        GaitLabel::Stride,
        |g| g.stride_length = (g.stride_length - 0.2).max(0.1),
        |g| g.stride_length = (g.stride_length - 0.05).max(0.1),
        |g| g.stride_length = (g.stride_length + 0.05).min(4.0),
        |g| g.stride_length = (g.stride_length + 0.2).min(4.0),
    );
    gait_row(
        &mut ui,
        "Stance",
        &t,
        GaitLabel::Stance,
        |g| g.stance_width = (g.stance_width - 0.1).max(0.0),
        |g| g.stance_width = (g.stance_width - 0.02).max(0.0),
        |g| g.stance_width = (g.stance_width + 0.02).min(1.0),
        |g| g.stance_width = (g.stance_width + 0.1).min(1.0),
    );
    gait_row(
        &mut ui,
        "Step up",
        &t,
        GaitLabel::StepHeight,
        |g| g.step_height = (g.step_height - 0.05).max(0.0),
        |g| g.step_height = (g.step_height - 0.01).max(0.0),
        |g| g.step_height = (g.step_height + 0.01).min(0.6),
        |g| g.step_height = (g.step_height + 0.05).min(0.6),
    );
    // Above 0.5 there is always a foot down (a walk); below, a flight phase (a run).
    gait_row(
        &mut ui,
        "Duty",
        &t,
        GaitLabel::Duty,
        |g| g.duty_factor = (g.duty_factor - 0.05).max(0.2),
        |g| g.duty_factor = (g.duty_factor - 0.01).max(0.2),
        |g| g.duty_factor = (g.duty_factor + 0.01).min(0.95),
        |g| g.duty_factor = (g.duty_factor + 0.05).min(0.95),
    );

    // Anatomy, not choreography: how straight the knee may lock and how far it may fold.
    gait_row(
        &mut ui,
        "Knee max",
        &t,
        GaitLabel::KneeStraight,
        |g| g.knee.straightest_deg = (g.knee.straightest_deg - 5.0).max(90.0),
        |g| g.knee.straightest_deg = (g.knee.straightest_deg - 1.0).max(90.0),
        |g| g.knee.straightest_deg = (g.knee.straightest_deg + 1.0).min(180.0),
        |g| g.knee.straightest_deg = (g.knee.straightest_deg + 5.0).min(180.0),
    );
    gait_row(
        &mut ui,
        "Knee min",
        &t,
        GaitLabel::KneeBent,
        |g| g.knee.most_bent_deg = (g.knee.most_bent_deg - 5.0).max(5.0),
        |g| g.knee.most_bent_deg = (g.knee.most_bent_deg - 1.0).max(5.0),
        |g| g.knee.most_bent_deg = (g.knee.most_bent_deg + 1.0).min(170.0),
        |g| g.knee.most_bent_deg = (g.knee.most_bent_deg + 5.0).min(170.0),
    );

    // The hips rise over the planted foot and drop between steps, twice a cycle.
    gait_row(
        &mut ui,
        "Hip bob",
        &t,
        GaitLabel::HipBob,
        |g| g.hip_bob = (g.hip_bob - 0.01).max(0.0),
        |g| g.hip_bob = (g.hip_bob - 0.002).max(0.0),
        |g| g.hip_bob = (g.hip_bob + 0.002).min(0.2),
        |g| g.hip_bob = (g.hip_bob + 0.01).min(0.2),
    );

    // Negative puts the footfalls further back, which is the body riding further forward
    // over them.
    gait_row(
        &mut ui,
        "Fore/aft",
        &t,
        GaitLabel::StrideBias,
        |g| g.stride_bias = (g.stride_bias - 0.05).max(-0.5),
        |g| g.stride_bias = (g.stride_bias - 0.01).max(-0.5),
        |g| g.stride_bias = (g.stride_bias + 0.01).min(0.5),
        |g| g.stride_bias = (g.stride_bias + 0.05).min(0.5),
    );

    // 0 hands the hips back to the animation. The rig's own standing height is around
    // 0.96, and every step of slack below that is stride.
    gait_row(
        &mut ui,
        "Hips",
        &t,
        GaitLabel::HipTarget,
        |g| g.hip_height = (g.hip_height - 0.05).max(0.0),
        |g| g.hip_height = (g.hip_height - 0.01).max(0.0),
        |g| g.hip_height = (g.hip_height + 0.01).min(1.0),
        |g| g.hip_height = (g.hip_height + 0.05).min(1.0),
    );

    setting_row(&mut ui, "Speed", &t, |row| {
        row.add_button_observe("<<", |b| { b.size_px(28.0, 28.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.player_speed_multiplier = (s.player_speed_multiplier - 0.25).max(0.05);
                s.save();
            });
        row.add_button_observe("<", |b| { b.size_px(24.0, 28.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.player_speed_multiplier = (s.player_speed_multiplier - 0.05).max(0.05);
                s.save();
            });
        row.with_child(|v| {
            v.default_text("").insert(GaitLabel::Speed).min_width_px(44.0);
        });
        row.add_button_observe(">", |b| { b.size_px(24.0, 28.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.player_speed_multiplier = (s.player_speed_multiplier + 0.05).min(5.0);
                s.save();
            });
        row.add_button_observe(">>", |b| { b.size_px(28.0, 28.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.player_speed_multiplier = (s.player_speed_multiplier + 0.25).min(5.0);
                s.save();
            });
    });

    setting_row(&mut ui, "Legs", &t, |row| {
        row.add_button_observe("Toggle", |b| { b.size_px(70.0, 28.0); },
            |_: On<Activate>, mut enabled: ResMut<LegIkEnabled>| { enabled.0 = !enabled.0; });
        row.with_child(|v| {
            v.default_text("").insert(GaitLabel::LegIk).min_width_px(44.0);
        });
    });

    ui.label("-- as walked (read-only) --", 12.0, Color::srgb(0.75, 0.5, 0.35));
    readout_row(&mut ui, "Rig", &t, GaitLabel::Scale);
    readout_row(&mut ui, "Leg", &t, GaitLabel::LegLength);
    readout_row(&mut ui, "Extend", &t, GaitLabel::Furthest);
    readout_row(&mut ui, "Hip up", &t, GaitLabel::HipHeight);
    readout_row(&mut ui, "Reach", &t, GaitLabel::Reach);
    readout_row(&mut ui, "Stride", &t, GaitLabel::FittedStride);
    readout_row(&mut ui, "Steps/s", &t, GaitLabel::StepsPerSecond);

    ui.build();
}

/// One editable row: `[ << ][ < ] value [ > ][ >> ]`, writing [`GaitSettings`].
#[allow(clippy::too_many_arguments)]
fn gait_row(
    ui: &mut UIBuilder,
    label: &str,
    t: &TextTheme,
    which: GaitLabel,
    coarse_dec: impl Fn(&mut GaitParams) + Send + Sync + 'static,
    fine_dec: impl Fn(&mut GaitParams) + Send + Sync + 'static,
    fine_inc: impl Fn(&mut GaitParams) + Send + Sync + 'static,
    coarse_inc: impl Fn(&mut GaitParams) + Send + Sync + 'static,
) {
    setting_row(ui, label, t, move |row| {
        row.add_button_observe("<<", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut g: ResMut<GaitSettings>| { coarse_dec(&mut g.0); g.save(); });
        row.add_button_observe("<", |b| { b.size_px(24.0, 28.0); },
            move |_: On<Activate>, mut g: ResMut<GaitSettings>| { fine_dec(&mut g.0); g.save(); });
        row.with_child(|v| {
            v.default_text("").insert(which).min_width_px(44.0);
        });
        row.add_button_observe(">", |b| { b.size_px(24.0, 28.0); },
            move |_: On<Activate>, mut g: ResMut<GaitSettings>| { fine_inc(&mut g.0); g.save(); });
        row.add_button_observe(">>", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut g: ResMut<GaitSettings>| { coarse_inc(&mut g.0); g.save(); });
    });
}

/// A row with no buttons — a measurement, not a setting.
fn readout_row(ui: &mut UIBuilder, label: &str, t: &TextTheme, which: GaitLabel) {
    setting_row(ui, label, t, move |row| {
        row.with_child(|v| {
            v.default_text("").insert(which).min_width_px(44.0);
        });
    });
}

fn setting_row<F: FnOnce(&mut UIBuilder)>(ui: &mut UIBuilder, label: &str, t: &TextTheme, f: F) {
    let color = t.label_color;
    ui.add_row(|row| {
        row.gap_px(4.0).align_items_center().width_px(310.0);
        row.with_child(|c| {
            c.with_text(label, Some(TextStyle::size_color(16.0, color))).width_px(70.0);
        });
        f(row);
    });
}

pub fn toggle_gait_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: Query<&mut Node, With<GaitPanel>>,
) {
    if !keys.just_pressed(KeyCode::F10) {
        return;
    }
    if let Ok(mut node) = panel.single_mut() {
        node.display = match node.display {
            Display::None => Display::Flex,
            _ => Display::None,
        };
    }
}

/// Refresh the labels.
///
/// The readouts change every frame while walking, so this cannot key off `is_changed` the
/// way the camera panel does — but it can do nothing at all while the panel is hidden,
/// which is most of the time.
pub fn update_gait_panel(
    gait: Res<GaitSettings>,
    settings: Res<GameSettings>,
    readout: Res<GaitReadout>,
    enabled: Res<LegIkEnabled>,
    panel: Query<&Node, With<GaitPanel>>,
    mut labels: Query<(&GaitLabel, &mut Text)>,
) {
    if panel.single().is_ok_and(|node| node.display == Display::None) {
        return;
    }
    for (label, mut text) in labels.iter_mut() {
        **text = match label {
            GaitLabel::Stride => format!("{:.2}m", gait.stride_length),
            GaitLabel::Stance => format!("{:.2}m", gait.stance_width),
            GaitLabel::StepHeight => format!("{:.2}m", gait.step_height),
            GaitLabel::StrideBias => format!("{:+.2}", gait.stride_bias),
            GaitLabel::KneeStraight => format!("{:.0}deg", gait.knee.straightest_deg),
            GaitLabel::KneeBent => format!("{:.0}deg", gait.knee.most_bent_deg),
            GaitLabel::HipBob => format!("{:.3}L", gait.hip_bob),
            GaitLabel::HipTarget => if gait.hip_height > 0.0 {
                format!("{:.2}L", gait.hip_height)
            } else {
                "anim".to_string()
            },
            GaitLabel::Duty => format!("{:.2}", gait.duty_factor),
            GaitLabel::Speed => format!("{:.2}x", settings.player_speed_multiplier),
            GaitLabel::LegIk => if enabled.0 { "on" } else { "off" }.to_string(),
            GaitLabel::Scale => format!("{:.2}x", readout.gait_scale),
            GaitLabel::LegLength => format!("{:.3}m", readout.leg_length),
            GaitLabel::Furthest => format!("{:.3}m", readout.furthest),
            GaitLabel::HipHeight => format!("{:.3}m", readout.hip_height),
            GaitLabel::Reach => format!("{:.3}m", readout.reach),
            GaitLabel::FittedStride => format!("{:.3}m", readout.stride),
            GaitLabel::StepsPerSecond => format!("{:.1}", readout.steps_per_second),
        };
    }
}
