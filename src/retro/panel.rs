//! The F12 panel: every [`Setting`] of the retro look as a `< value >` row, like the F1
//! camera panel. Any change, from here or a hotkey, flashes a one-line summary at the top
//! of the screen and is saved to `retro-settings.ron`. Not in the character editor, where
//! F12 is the bind pose (and the look is off).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, TextStyle, UIBuilder};

use super::look::{LightSetting, RetroFile, Setting, summary};
use super::shots::RetroShots;
use super::{Palettes, RetroLook};
use crate::character_editor::state::EditorSession;
use crate::general::systems::lights_systems::{LightingPreset, SkyLightSettings};

/// How long the summary stays up, in seconds (the last half second fades).
const FLASH_SECS: f32 = 2.5;

#[derive(Component, Default)]
struct RetroPanel;

/// The value label of a row.
#[derive(Component)]
struct ValueLabel(Setting);

/// The value label of a lighting row.
#[derive(Component)]
struct LightLabel(LightSetting);

#[derive(Component, Default)]
struct RetroFlash {
    left: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (spawn_panel, spawn_flash))
        .add_systems(
            Update,
            (
                toggle_panel,
                (show_values, flash_and_save).run_if(resource_changed::<RetroLook>),
                (show_light_values, save_light).run_if(
                    resource_changed::<LightingPreset>.or_else(resource_changed::<SkyLightSettings>),
                ),
                fade_flash,
            ),
        );
}

fn spawn_panel(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<RetroPanel>()
        .display_none()
        .absolute_position()
        .top(px(8.0))
        .left(px(8.0))
        .flex_column()
        .row_gap_px(6.0)
        .padding_all_px(12.0)
        .min_width_px(380.0)
        .bg_color(Color::srgba(0.09, 0.06, 0.14, 0.92))
        .insert(ZIndex(50));

    let label_color = ui.theme().text.label_color;
    ui.themed_header("Retro look  [F12]");
    for setting in Setting::ALL {
        ui.add_row(|row| {
            row.gap_px(4.0).align_items_center();
            row.with_child(|c| {
                c.with_text(setting.name(), Some(TextStyle::size_color(16.0, label_color)))
                    .width_px(80.0);
            });
            for (text, dir) in [("<", -1), (">", 1)] {
                if dir > 0 {
                    row.with_child(|v| {
                        v.default_text("")
                            .insert(ValueLabel(setting))
                            .min_width_px(200.0);
                    });
                }
                row.add_button_observe(
                    text,
                    |b| {
                        b.size_px(28.0, 28.0);
                    },
                    move |_: On<Activate>, mut look: ResMut<RetroLook>, palettes: Res<Palettes>| {
                        setting.step(&mut look, &palettes, dir);
                    },
                );
            }
        });
    }
    ui.label("-- Light --", 12.0, Color::srgb(0.6, 0.55, 0.75));
    for setting in LightSetting::ALL {
        ui.add_row(|row| {
            row.gap_px(4.0).align_items_center();
            row.with_child(|c| {
                c.with_text(setting.name(), Some(TextStyle::size_color(16.0, label_color)))
                    .width_px(80.0);
            });
            for (text, dir) in [("<", -1), (">", 1)] {
                if dir > 0 {
                    row.with_child(|v| {
                        v.default_text("")
                            .insert(LightLabel(setting))
                            .min_width_px(200.0);
                    });
                }
                row.add_button_observe(
                    text,
                    |b| {
                        b.size_px(28.0, 28.0);
                    },
                    move |_: On<Activate>,
                          mut preset: ResMut<LightingPreset>,
                          mut sky: ResMut<SkyLightSettings>| {
                        setting.step(&mut preset, &mut sky, dir);
                    },
                );
            }
        });
    }
    ui.label(
        "Ctrl+K on/off  Ctrl+J palette  Ctrl+H dither  Ctrl+U/I pixels",
        12.0,
        Color::srgb(0.6, 0.55, 0.75),
    );
    ui.build();
}

fn spawn_flash(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Retro Flash"),
            Node {
                position_type: PositionType::Absolute,
                top: px(64.0),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            ZIndex(60),
        ))
        .with_child((
            RetroFlash::default(),
            Text::new(""),
            TextFont::from_font_size(18.0),
            TextColor(Color::NONE),
            TextShadow::default(),
            Pickable::IGNORE,
        ));
}

fn toggle_panel(
    keys: Res<ButtonInput<KeyCode>>,
    editor: Option<Res<EditorSession>>,
    mut panels: Query<&mut Node, With<RetroPanel>>,
) {
    let open = keys.just_pressed(KeyCode::F12) && editor.is_none();
    for mut node in &mut panels {
        if open {
            node.display = match node.display {
                Display::None => Display::Flex,
                _ => Display::None,
            };
        } else if editor.is_some() {
            node.display = Display::None;
        }
    }
}

fn show_values(
    look: Res<RetroLook>,
    palettes: Res<Palettes>,
    mut labels: Query<(&ValueLabel, &mut Text)>,
) {
    for (label, mut text) in &mut labels {
        let value = label.0.value(&look, &palettes);
        if text.0 != value {
            text.0 = value;
        }
    }
}

fn show_light_values(
    preset: Res<LightingPreset>,
    sky: Res<SkyLightSettings>,
    mut labels: Query<(&LightLabel, &mut Text)>,
) {
    for (label, mut text) in &mut labels {
        let value = label.0.value(*preset, &sky);
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Save lighting changes, but not the character editor's switch to daylight.
fn save_light(
    look: Res<RetroLook>,
    palettes: Res<Palettes>,
    preset: Res<LightingPreset>,
    sky: Res<SkyLightSettings>,
    editor: Option<Res<EditorSession>>,
    shots: Option<Res<RetroShots>>,
    mut flashes: Query<(&mut RetroFlash, &mut Text)>,
) {
    if preset.is_added() || sky.is_added() || editor.is_some() || shots.is_some() {
        return;
    }
    let line = format!(
        "Sky {}   height {}   turn {}   {}   ambient {}   (F12)",
        LightSetting::Preset.value(*preset, &sky),
        LightSetting::Height.value(*preset, &sky),
        LightSetting::Turn.value(*preset, &sky),
        LightSetting::Strength.value(*preset, &sky),
        LightSetting::Ambient.value(*preset, &sky),
    );
    for (mut flash, mut text) in &mut flashes {
        flash.left = FLASH_SECS;
        text.0.clone_from(&line);
    }
    RetroFile::of_all(&look, &palettes, *preset, *sky).save();
}

/// Flash and save every change after the first frame; the shots run changes palettes
/// on its own and leaves the file alone.
fn flash_and_save(
    look: Res<RetroLook>,
    palettes: Res<Palettes>,
    preset: Res<LightingPreset>,
    sky: Res<SkyLightSettings>,
    shots: Option<Res<RetroShots>>,
    mut flashes: Query<(&mut RetroFlash, &mut Text)>,
) {
    if look.is_added() || shots.is_some() {
        return;
    }
    let line = summary(&look, &palettes);
    info!("retro look: {line}");
    for (mut flash, mut text) in &mut flashes {
        flash.left = FLASH_SECS;
        text.0.clone_from(&line);
    }
    RetroFile::of_all(&look, &palettes, *preset, *sky).save();
}

fn fade_flash(time: Res<Time<Real>>, mut flashes: Query<(&mut RetroFlash, &mut TextColor)>) {
    for (mut flash, mut colour) in &mut flashes {
        if flash.left <= 0.0 {
            continue;
        }
        flash.left = (flash.left - time.delta_secs()).max(0.0);
        colour.0 = Color::srgba(1.0, 0.95, 0.85, (flash.left / 0.5).min(1.0));
    }
}
