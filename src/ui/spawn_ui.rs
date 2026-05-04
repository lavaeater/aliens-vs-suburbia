use crate::alien::components::general::AlienCounter;
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::player::components::IsBuilding;
use crate::settings::resources::{GameSettings, ProjectionMode};
use crate::model_settings::resources::{ANIM_KEYS, DebugAnimSelection, ModelSettings};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{
    ButtonTheme, LavaTheme, ProgressBar, TextTheme, UIBuilder, WorldFollower, progress_bar,
};

// ── Theme ────────────────────────────────────────────────────────────────────

pub fn game_theme() -> LavaTheme {
    LavaTheme {
        button: ButtonTheme {
            bg: Color::srgb(0.15, 0.35, 0.20),
            bg_hovered: Color::srgb(0.22, 0.55, 0.30),
            bg_pressed: Color::srgb(0.10, 0.25, 0.15),
            text_color: Color::srgb(0.85, 1.0, 0.88),
            font_size: 28.0,
            border_radius: BorderRadius::all(Val::Px(4.0)),
            border_color: Color::srgb(0.30, 0.70, 0.40),
            height: Val::Px(52.0),
            width: Val::Px(220.0),
            ..ButtonTheme::default()
        },
        text: TextTheme {
            header_size: 36.0,
            label_size: 20.0,
            header_color: Color::srgb(0.85, 1.0, 0.88),
            label_color: Color::srgb(0.60, 0.85, 0.65),
            ..TextTheme::default()
        },
        bg_color: Color::srgba(0.05, 0.12, 0.07, 0.85),
        border_color: Color::srgba(0.30, 0.70, 0.40, 0.50),
        ..LavaTheme::default()
    }
}

// ── State messaging ───────────────────────────────────────────────────────────

#[derive(Message, Clone)]
pub struct GotoState {
    pub state: GameState,
}

// ── Menu ─────────────────────────────────────────────────────────────────────

/// Marker for all entities spawned by menu/hud so cleanup_state can remove them.
#[derive(Component, Default)]
pub struct StateMarker;

pub fn spawn_menu(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_column()
        .align_items_center()
        .justify_center()
        .gap_px(24.0);

    let text_theme = ui.theme().text.clone();
    ui.with_child(|h| {
        h.insert_bundle(lava_ui_builder::header("Aliens vs Suburbia", &text_theme));
    });

    ui.add_button_observe(
        "Start Game",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::InGame);
        },
    );

    ui.add_button_observe(
        "Model Showcase",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::ModelShowcase);
        },
    );

    ui.add_button_observe(
        "Browse Models",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::PolyPizza);
        },
    );

    ui.build();
}

pub fn spawn_showcase_ui(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_column()
        .align_items_start()
        .justify_start()
        .padding_all_px(16.0);

    ui.add_button_observe(
        "Back to Menu",
        |btn| { btn.size_px(160.0, 44.0).font_size(18.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::Menu);
        },
    );

    ui.build();
}

pub fn goto_state_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut goto_state_mr: MessageReader<GotoState>,
) {
    for msg in goto_state_mr.read() {
        next_state.set(msg.state.clone());
    }
}

pub fn cleanup_state(mut commands: Commands, entities: Query<Entity, With<StateMarker>>) {
    for entity in entities.iter() {
        commands.entity(entity).despawn();
    }
}

// ── HUD ──────────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct HudAlienCount;

#[derive(Component)]
pub struct HudBuildMode;

#[derive(Component)]
pub struct HudProjection;

pub fn spawn_ui(mut commands: Commands, theme: Res<LavaTheme>) {
    let text_theme = theme.text.clone();

    {
        let mut ui = UIBuilder::new(commands.reborrow(), Some(theme.clone()));
        ui.insert(StateMarker).modify_node(|mut n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(8.0);
            n.left = Val::Px(8.0);
            n.flex_direction = FlexDirection::Column;
            n.row_gap = Val::Px(4.0);
        });

        ui.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("Aliens: 0", &text_theme))
                .insert(HudAlienCount);
        });

        ui.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "",
                &TextTheme {
                    label_color: Color::srgb(1.0, 0.8, 0.2),
                    ..text_theme.clone()
                },
            ))
            .insert(HudBuildMode);
        });

        ui.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "",
                &TextTheme {
                    label_size: 14.0,
                    label_color: Color::srgb(0.6, 0.6, 0.6),
                    ..text_theme.clone()
                },
            ))
            .insert(HudProjection);
        });

        ui.build();
    }

    spawn_settings_panel(commands, theme);
}

fn spawn_settings_panel(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<SettingsPanel>()
        .display_none() // hidden until F1
        .modify_node(|mut n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(8.0);
            n.right = Val::Px(8.0);
            n.flex_direction = FlexDirection::Column;
            n.row_gap = Val::Px(6.0);
            n.padding = UiRect::all(Val::Px(12.0));
        })
        .bg_color(Color::srgba(0.05, 0.12, 0.07, 0.92))
        .insert(StateMarker);

    let text_theme = ui.theme().text.clone();

    // Title
    ui.with_child(|c| {
        c.insert_bundle(lava_ui_builder::header("Settings  [F1]", &text_theme));
    });

    // Projection
    setting_row(&mut ui, "Projection", &text_theme, |row| {
        row.add_button_observe("Ortho", |b| { b.size_px(70.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.projection = ProjectionMode::Orthographic; s.save();
            });
        row.add_button_observe("Persp", |b| { b.size_px(70.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.projection = ProjectionMode::Perspective; s.save();
            });
    });

    // Zoom
    setting_row(&mut ui, "Zoom", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.zoom = (s.zoom - 1.0).max(1.0); s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(SettingValueZoom); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.zoom = (s.zoom + 1.0).min(60.0); s.save();
            });
    });

    // Pitch
    setting_row(&mut ui, "Pitch", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.pitch_degrees = (s.pitch_degrees - 5.0).max(-89.0); s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(SettingValuePitch); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.pitch_degrees = (s.pitch_degrees + 5.0).min(-5.0); s.save();
            });
    });

    // Yaw
    setting_row(&mut ui, "Yaw", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.yaw_degrees = (s.yaw_degrees - 15.0).rem_euclid(360.0); s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(SettingValueYaw); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.yaw_degrees = (s.yaw_degrees + 15.0).rem_euclid(360.0); s.save();
            });
    });

    // Speed multiplier
    setting_row(&mut ui, "Speed", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.player_speed_multiplier = (s.player_speed_multiplier - 0.25).max(0.25); s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(SettingValueSpeed); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| {
                s.player_speed_multiplier = (s.player_speed_multiplier + 0.25).min(5.0); s.save();
            });
    });

    // ── Model settings ────────────────────────────────────────────────────────
    let sep_theme = TextTheme { label_size: 13.0, label_color: Color::srgb(0.4, 0.7, 0.5), ..text_theme.clone() };
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Model —", &sep_theme)); });

    // Scale
    setting_row(&mut ui, "Scale", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>| {
                s.scale = (s.scale - 0.1).max(0.1); s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(ModelSettingValueScale); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>| {
                s.scale = (s.scale + 0.1).min(10.0); s.save();
            });
    });

    // Offset Y
    setting_row(&mut ui, "Offset Y", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>| {
                s.translation_y -= 0.05; s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(ModelSettingValueOffsetY); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>| {
                s.translation_y += 0.05; s.save();
            });
    });

    // Rotation Y
    setting_row(&mut ui, "Rot Y", &text_theme, |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>| {
                s.rotation_y_degrees = (s.rotation_y_degrees - 15.0).rem_euclid(360.0); s.save();
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(ModelSettingValueRotY); });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>| {
                s.rotation_y_degrees = (s.rotation_y_degrees + 15.0).rem_euclid(360.0); s.save();
            });
    });

    // ── Animation selector ────────────────────────────────────────────────────
    let sep2_theme = TextTheme { label_size: 13.0, label_color: Color::srgb(0.4, 0.7, 0.5), ..text_theme.clone() };
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Animation —", &sep2_theme)); });

    setting_row(&mut ui, "Anim", &text_theme, |row| {
        row.add_button_observe("<", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut sel: ResMut<DebugAnimSelection>| {
                sel.index = (sel.index + ANIM_KEYS.len() - 1) % ANIM_KEYS.len();
                sel.dirty = true;
            });
        row.with_child(|v| { v.insert_bundle(lava_ui_builder::label("", &TextTheme::default())).insert(AnimSelectorLabel); });
        row.add_button_observe(">", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut sel: ResMut<DebugAnimSelection>| {
                sel.index = (sel.index + 1) % ANIM_KEYS.len();
                sel.dirty = true;
            });
    });

    ui.build();
}

fn setting_row<F: FnOnce(&mut UIBuilder)>(
    ui: &mut UIBuilder,
    label: &str,
    text_theme: &TextTheme,
    f: F,
) {
    let label_theme = TextTheme { label_size: 16.0, ..text_theme.clone() };
    ui.add_row(|row| {
        row.gap_px(6.0).align_items_center().width_px(260.0);
        row.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(label, &label_theme));
            c.modify_node(|mut n| n.width = Val::Px(70.0));
        });
        f(row);
    });
}

#[derive(Component, Default)]
pub struct SettingsPanel;

#[derive(Component)] pub struct SettingValueZoom;
#[derive(Component)] pub struct SettingValuePitch;
#[derive(Component)] pub struct SettingValueYaw;
#[derive(Component)] pub struct SettingValueSpeed;
#[derive(Component)] pub struct ModelSettingValueScale;
#[derive(Component)] pub struct ModelSettingValueOffsetY;
#[derive(Component)] pub struct ModelSettingValueRotY;
#[derive(Component)] pub struct AnimSelectorLabel;

pub fn toggle_settings_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: Query<&mut Node, With<SettingsPanel>>,
) {
    if !keys.just_pressed(KeyCode::F1) { return; }
    if let Ok(mut node) = panel.single_mut() {
        node.display = match node.display {
            Display::None => Display::Flex,
            _ => Display::None,
        };
    }
}

pub fn update_settings_panel(
    settings: Res<GameSettings>,
    model_settings: Res<ModelSettings>,
    mut zoom: Query<&mut Text, (With<SettingValueZoom>, Without<SettingValuePitch>, Without<SettingValueYaw>, Without<SettingValueSpeed>, Without<ModelSettingValueScale>, Without<ModelSettingValueOffsetY>, Without<ModelSettingValueRotY>)>,
    mut pitch: Query<&mut Text, (With<SettingValuePitch>, Without<SettingValueZoom>, Without<SettingValueYaw>, Without<SettingValueSpeed>, Without<ModelSettingValueScale>, Without<ModelSettingValueOffsetY>, Without<ModelSettingValueRotY>)>,
    mut yaw: Query<&mut Text, (With<SettingValueYaw>, Without<SettingValueZoom>, Without<SettingValuePitch>, Without<SettingValueSpeed>, Without<ModelSettingValueScale>, Without<ModelSettingValueOffsetY>, Without<ModelSettingValueRotY>)>,
    mut speed: Query<&mut Text, (With<SettingValueSpeed>, Without<SettingValueZoom>, Without<SettingValuePitch>, Without<SettingValueYaw>, Without<ModelSettingValueScale>, Without<ModelSettingValueOffsetY>, Without<ModelSettingValueRotY>)>,
    mut model_scale: Query<&mut Text, (With<ModelSettingValueScale>, Without<SettingValueZoom>, Without<SettingValuePitch>, Without<SettingValueYaw>, Without<SettingValueSpeed>, Without<ModelSettingValueOffsetY>, Without<ModelSettingValueRotY>)>,
    mut model_offset_y: Query<&mut Text, (With<ModelSettingValueOffsetY>, Without<SettingValueZoom>, Without<SettingValuePitch>, Without<SettingValueYaw>, Without<SettingValueSpeed>, Without<ModelSettingValueScale>, Without<ModelSettingValueRotY>)>,
    mut model_rot_y: Query<&mut Text, (With<ModelSettingValueRotY>, Without<SettingValueZoom>, Without<SettingValuePitch>, Without<SettingValueYaw>, Without<SettingValueSpeed>, Without<ModelSettingValueScale>, Without<ModelSettingValueOffsetY>)>,
) {
    if let Ok(mut t) = zoom.single_mut() { **t = format!("{:.0}", settings.zoom); }
    if let Ok(mut t) = pitch.single_mut() { **t = format!("{:.0}°", settings.pitch_degrees); }
    if let Ok(mut t) = yaw.single_mut() { **t = format!("{:.0}°", settings.yaw_degrees); }
    if let Ok(mut t) = speed.single_mut() { **t = format!("{:.2}×", settings.player_speed_multiplier); }
    if let Ok(mut t) = model_scale.single_mut() { **t = format!("{:.2}", model_settings.scale); }
    if let Ok(mut t) = model_offset_y.single_mut() { **t = format!("{:.2}", model_settings.translation_y); }
    if let Ok(mut t) = model_rot_y.single_mut() { **t = format!("{:.0}°", model_settings.rotation_y_degrees); }
}

pub fn update_anim_selector_label(
    anim_sel: Res<DebugAnimSelection>,
    mut label: Query<&mut Text, With<AnimSelectorLabel>>,
) {
    if !anim_sel.is_changed() { return; }
    if let Ok(mut t) = label.single_mut() {
        **t = format!("{:?}", ANIM_KEYS[anim_sel.index % ANIM_KEYS.len()]);
    }
}

pub fn update_hud(
    alien_counter: Option<Res<AlienCounter>>,
    building_query: Query<(), With<IsBuilding>>,
    settings: Res<GameSettings>,
    mut alien_text: Query<
        &mut Text,
        (
            With<HudAlienCount>,
            Without<HudBuildMode>,
            Without<HudProjection>,
        ),
    >,
    mut build_text: Query<
        &mut Text,
        (
            With<HudBuildMode>,
            Without<HudAlienCount>,
            Without<HudProjection>,
        ),
    >,
    mut proj_text: Query<
        &mut Text,
        (
            With<HudProjection>,
            Without<HudAlienCount>,
            Without<HudBuildMode>,
        ),
    >,
) {
    if let Some(counter) = alien_counter {
        if let Ok(mut t) = alien_text.single_mut() {
            **t = format!("Aliens: {}", counter.count);
        }
    }

    if let Ok(mut t) = build_text.single_mut() {
        **t = if building_query.iter().next().is_some() {
            "[BUILD MODE]".to_string()
        } else {
            String::new()
        };
    }

    if let Ok(mut t) = proj_text.single_mut() {
        let proj_name = match settings.projection {
            ProjectionMode::Orthographic => "Ortho",
            ProjectionMode::Perspective => "Persp",
        };
        **t = format!(
            "{proj_name} zoom:{:.0} pitch:{:.0}° yaw:{:.0}°  [P]/[Z,X]/[C,V]/[N,M]",
            settings.zoom, settings.pitch_degrees, settings.yaw_degrees
        );
    }
}

// ── Health bars ───────────────────────────────────────────────────────────────

#[derive(Message, Clone)]
pub struct AddHealthBar {
    pub entity: Entity,
    pub name: &'static str,
}

pub fn add_health_bar(mut commands: Commands, mut add_health_bar_mr: MessageReader<AddHealthBar>) {
    for msg in add_health_bar_mr.read() {
        let target = msg.entity;
        let bar = commands
            .spawn((
                WorldFollower {
                    target,
                    offset: Vec2::new(-30.0, -40.0),
                },
                progress_bar(
                    1.0,
                    60.0,
                    8.0,
                    Color::srgb(0.2, 0.85, 0.3),
                    Color::srgba(0.0, 0.0, 0.0, 0.5),
                ),
            ))
            .id();
        commands
            .entity(bar)
            .entry::<Node>()
            .and_modify(|mut n| n.position_type = PositionType::Absolute);
    }
}

pub fn sync_health_bars(
    mut bars: Query<(&WorldFollower, &mut ProgressBar)>,
    health_query: Query<&Health>,
) {
    for (follower, mut bar) in bars.iter_mut() {
        if let Ok(health) = health_query.get(follower.target) {
            bar.value = (health.health as f32 / health.max_health as f32).clamp(0.0, 1.0);
        }
    }
}
