//! The F12 panel: the look mode and every setting of it as a `< value >` row ([`Row`]),
//! sections for the retro canvas, the toon shader and PSX shown only in the modes they
//! apply to, then the scene (outline, fog, dread, grass) and the sky. A step, from here or
//! a hotkey, flashes the new value at the top of the screen; the look, its settings,
//! outline and sky are saved to `retro-settings.ron` (fog, dread and grass are for this
//! run). Not in the character editor, where F12 is the bind pose (and the look is off).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, TextStyle, UIBuilder};

use super::look::{ForcedMode, LightSetting, LookSnapshot, RetroFile, RetroLook, Setting, summary};
use super::mode::{LookMode, OutlineLook, PsxLook, PsxSetting, ToonLook, ToonSetting};
use super::rows::{LookChanged, LookParams, Row, SceneSetting};
use super::shots::RetroShots;
use crate::character_editor::state::EditorSession;
use crate::game_state::clear_game_entities_plugin::Persistent;
use crate::general::systems::lights_systems::{LightingPreset, SkyLightSettings};

/// How long the flash stays up, in seconds (the last half second fades).
const FLASH_SECS: f32 = 2.5;
const ROW_HEIGHT: f32 = 22.0;
const FONT: f32 = 14.0;

#[derive(Component, Default)]
struct RetroPanel;

/// The value label of a row.
#[derive(Component)]
struct ValueLabel(Row);

/// A row, or a section heading, shown only in the modes `Row::shown_in` allows.
#[derive(Component)]
struct ShownFor(Row);

#[derive(Component, Default)]
struct RetroFlash {
    left: f32,
}

pub fn plugin(app: &mut App) {
    app.add_message::<LookChanged>()
        .add_systems(Startup, (spawn_panel, spawn_flash))
        .add_systems(
            Update,
            (
                toggle_panel,
                show_sections.run_if(resource_changed::<LookMode>),
                show_values,
                flash,
                save.run_if(
                    resource_changed::<LookMode>
                        .or_else(resource_changed::<RetroLook>)
                        .or_else(resource_changed::<ToonLook>)
                        .or_else(resource_changed::<PsxLook>)
                        .or_else(resource_changed::<OutlineLook>)
                        .or_else(resource_changed::<LightingPreset>)
                        .or_else(resource_changed::<SkyLightSettings>),
                ),
                fade_flash,
            )
                .chain(),
        );
}

fn spawn_panel(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    // Spawned once, so it must outlive the matches.
    ui.component::<RetroPanel>()
        .insert(Persistent)
        .display_none()
        .absolute_position()
        .top(px(8.0))
        .left(px(8.0))
        .flex_column()
        .row_gap_px(2.0)
        .padding_all_px(10.0)
        .min_width_px(360.0)
        .bg_color(Color::srgba(0.09, 0.06, 0.14, 0.92))
        .insert(ZIndex(50));

    let label_color = ui.theme().text.label_color;
    let heading = Color::srgb(0.6, 0.55, 0.75);
    ui.themed_header("Look  [F12]");

    let sections: [(&str, Vec<Row>); 6] = [
        ("", vec![Row::Mode]),
        ("-- Retro canvas --", Setting::ALL.map(Row::Retro).to_vec()),
        (
            "-- Toon shader --",
            ToonSetting::ALL.map(Row::Toon).to_vec(),
        ),
        ("-- PSX --", PsxSetting::ALL.map(Row::Psx).to_vec()),
        ("-- Scene --", SceneSetting::ALL.map(Row::Scene).to_vec()),
        ("-- Light --", LightSetting::ALL.map(Row::Light).to_vec()),
    ];
    for (title, rows) in sections {
        let Some(&first) = rows.first() else { continue };
        if !title.is_empty() {
            ui.label_child(title, 12.0, heading, |c| {
                c.insert(ShownFor(first)).margin_top(px(4.0));
            });
        }
        for row in rows {
            ui.add_row(|r| {
                r.insert(ShownFor(row)).gap_px(4.0).align_items_center();
                r.with_child(|c| {
                    c.with_text(row.name(), Some(TextStyle::size_color(FONT, label_color)))
                        .width_px(80.0);
                });
                for (text, dir) in [("<", -1), (">", 1)] {
                    if dir > 0 {
                        r.with_child(|v| {
                            v.with_text("", Some(TextStyle::size(FONT)))
                                .insert(ValueLabel(row))
                                .min_width_px(200.0);
                        });
                    }
                    r.add_button_observe(
                        text,
                        |b| {
                            b.size_px(ROW_HEIGHT, ROW_HEIGHT);
                        },
                        move |_: On<Activate>,
                              mut p: LookParams,
                              mut changed: MessageWriter<LookChanged>| {
                            row.step(&mut p, dir);
                            changed.write(LookChanged(row));
                        },
                    );
                }
            });
        }
    }
    ui.label(
        "Ctrl+K look  Ctrl+J palette  Ctrl+H dither  Ctrl+U/I pixels\n\
         Ctrl+L sky  Ctrl+O fog  Ctrl+Y grass  (Shift: back)",
        12.0,
        heading,
    );
    ui.build();
}

fn spawn_flash(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Retro Flash"),
            Persistent,
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

fn show_sections(mode: Res<LookMode>, mut rows: Query<(&ShownFor, &mut Node)>) {
    for (shown, mut node) in &mut rows {
        let display = if shown.0.shown_in(*mode) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Every frame the panel is open, so changes made elsewhere (the hotkeys of other
/// plugins, the dread fading in) show too.
fn show_values(
    p: LookParams,
    panels: Query<&Node, With<RetroPanel>>,
    mut labels: Query<(&ValueLabel, &mut Text)>,
) {
    if panels.iter().all(|n| n.display == Display::None) {
        return;
    }
    for (label, mut text) in &mut labels {
        let value = label.0.value(&p);
        if text.0 != value {
            text.0 = value;
        }
    }
}

fn flash(
    mut changed: MessageReader<LookChanged>,
    p: LookParams,
    shots: Option<Res<RetroShots>>,
    mut flashes: Query<(&mut RetroFlash, &mut Text)>,
) {
    let Some(LookChanged(row)) = changed.read().last().copied() else {
        return;
    };
    if shots.is_some() {
        return;
    }
    let line = match row {
        Row::Mode => summary(&snapshot(&p), &p.palettes),
        row => format!("{}   {}", row.name(), row.value(&p)),
    };
    info!("look: {line}");
    for (mut flash, mut text) in &mut flashes {
        flash.left = FLASH_SECS;
        text.0.clone_from(&line);
    }
}

fn snapshot(p: &LookParams) -> LookSnapshot {
    LookSnapshot {
        mode: *p.mode,
        look: p.retro.clone(),
        toon: *p.toon,
        psx: p.psx.clone(),
        outline: *p.outline,
        light: *p.preset,
        sky: *p.sky,
    }
}

/// Save every change after the first frame, but not the character editor's switch to
/// daylight, nor the palette shots run.
fn save(
    p: LookParams,
    editor: Option<Res<EditorSession>>,
    shots: Option<Res<RetroShots>>,
    forced: Option<Res<ForcedMode>>,
) {
    if p.mode.is_added() || p.preset.is_added() || editor.is_some() || shots.is_some() {
        return;
    }
    let mut file = RetroFile::of(&snapshot(&p), &p.palettes);
    // Still in the look a flag forced: the file keeps its own.
    if let Some(forced) = forced
        && *p.mode == forced.forced
    {
        file.mode = Some(forced.saved);
    }
    file.save();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_follow_the_mode() {
        let toon = Row::Toon(ToonSetting::Shadow);
        let retro = Row::Retro(Setting::Palette);
        let psx = Row::Psx(PsxSetting::Snap);
        assert!(toon.shown_in(LookMode::ToonRetro) && retro.shown_in(LookMode::ToonRetro));
        assert!(!toon.shown_in(LookMode::Retro) && !psx.shown_in(LookMode::Toon));
        assert!(psx.shown_in(LookMode::Psx) && !retro.shown_in(LookMode::Psx));
        for mode in LookMode::ALL {
            assert!(Row::Mode.shown_in(mode) && Row::Scene(SceneSetting::Fog).shown_in(mode));
        }
    }

    /// The panel is spawned once, at startup: F12 had nothing to open after a match.
    #[test]
    fn the_panel_and_the_flash_outlive_a_match() {
        let mut world = World::new();
        world.init_resource::<LavaTheme>();
        world.run_system_cached(spawn_panel).unwrap();
        world.run_system_cached(spawn_flash).unwrap();
        let count = |world: &mut World| {
            (
                world.query::<&RetroPanel>().iter(world).count(),
                world.query::<&ValueLabel>().iter(world).count(),
                world.query::<&Text>().iter(world).count(),
                world.query::<&RetroFlash>().iter(world).count(),
            )
        };
        let before = count(&mut world);
        assert!(before.0 == 1 && before.1 > 10 && before.3 == 1, "{before:?}");
        world
            .run_system_cached(crate::game_state::clear_game_entities_plugin::clear_game_entities)
            .unwrap();
        assert_eq!(count(&mut world), before);
    }
}
