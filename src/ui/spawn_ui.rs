use crate::alien::components::general::AlienCounter;
use crate::animation::animation_plugin::{AnimationKey, ANIM_KEYS};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::player::components::IsBuilding;
use crate::settings::resources::{GameSettings, ProjectionMode};
use crate::model_settings::resources::{CharacterFolder, DebugAnimSelection, ModelSettings, PlayerAnimClips};
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
        "Create Character",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::CharacterCreator);
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

    ui.add_button_observe(
        "Asset Browser",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::AssetBrowser);
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

    spawn_camera_panel(commands.reborrow(), &theme);
    spawn_model_panel(commands, &theme);
}

pub fn spawn_camera_panel(commands: Commands, theme: &LavaTheme) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<SettingsPanel>()
        .display_none()
        .modify_node(|mut n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(8.0);
            n.right = Val::Px(8.0);
            n.flex_direction = FlexDirection::Column;
            n.row_gap = Val::Px(6.0);
            n.padding = UiRect::all(Val::Px(12.0));
            n.min_width = Val::Px(300.0);
        })
        .bg_color(Color::srgba(0.05, 0.12, 0.07, 0.92))
        .insert(StateMarker);

    let t = ui.theme().text.clone();
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::header("Camera  [F1]", &t)); });

    setting_row(&mut ui, "Projection", &t, |row| {
        row.add_button_observe("Ortho", |b| { b.size_px(70.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| { s.projection = ProjectionMode::Orthographic; s.save(); });
        row.add_button_observe("Persp", |b| { b.size_px(70.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| { s.projection = ProjectionMode::Perspective; s.save(); });
    });
    cam_row(&mut ui, "Zoom",  &t, CameraSetting::Zoom,
        |s: &mut GameSettings| s.zoom = (s.zoom - 1.0).max(1.0),
        |s: &mut GameSettings| s.zoom = (s.zoom + 1.0).min(60.0));
    cam_row(&mut ui, "Pitch", &t, CameraSetting::Pitch,
        |s: &mut GameSettings| s.pitch_degrees = (s.pitch_degrees - 5.0).max(-89.0),
        |s: &mut GameSettings| s.pitch_degrees = (s.pitch_degrees + 5.0).min(-5.0));
    cam_row(&mut ui, "Yaw",   &t, CameraSetting::Yaw,
        |s: &mut GameSettings| s.yaw_degrees = (s.yaw_degrees - 15.0).rem_euclid(360.0),
        |s: &mut GameSettings| s.yaw_degrees = (s.yaw_degrees + 15.0).rem_euclid(360.0));
    cam_row(&mut ui, "Speed", &t, CameraSetting::Speed,
        |s: &mut GameSettings| s.player_speed_multiplier = (s.player_speed_multiplier - 0.25).max(0.25),
        |s: &mut GameSettings| s.player_speed_multiplier = (s.player_speed_multiplier + 0.25).min(5.0));

    let sep = TextTheme { label_size: 12.0, label_color: Color::srgb(0.4, 0.65, 0.5), ..t.clone() };
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Ortho —", &sep)); });
    cam_row(&mut ui, "V.Height", &t, CameraSetting::OrthoVH,
        |s: &mut GameSettings| s.ortho_viewport_height = (s.ortho_viewport_height - 0.25).max(0.25),
        |s: &mut GameSettings| s.ortho_viewport_height += 0.25);
    cam_row(&mut ui, "Near", &t, CameraSetting::OrthoNear,
        |s: &mut GameSettings| s.ortho_near -= 50.0,
        |s: &mut GameSettings| s.ortho_near = (s.ortho_near + 50.0).min(0.0));
    cam_row(&mut ui, "Far", &t, CameraSetting::OrthoFar,
        |s: &mut GameSettings| s.ortho_far = (s.ortho_far - 100.0).max(1.0),
        |s: &mut GameSettings| s.ortho_far += 100.0);

    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Persp —", &sep)); });
    cam_row(&mut ui, "Near", &t, CameraSetting::PerspNear,
        |s: &mut GameSettings| s.persp_near = (s.persp_near - 0.05).max(0.01),
        |s: &mut GameSettings| s.persp_near = (s.persp_near + 0.05).min(s.persp_far - 0.1));
    cam_row(&mut ui, "Far",  &t, CameraSetting::PerspFar,
        |s: &mut GameSettings| s.persp_far = (s.persp_far - 100.0).max(s.persp_near + 1.0),
        |s: &mut GameSettings| s.persp_far += 100.0);

    ui.build();
}

pub fn spawn_model_panel(commands: Commands, theme: &LavaTheme) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<ModelPanel>()
        .display_none()
        .modify_node(|mut n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(8.0);
            n.right = Val::Px(316.0);
            n.flex_direction = FlexDirection::Column;
            n.row_gap = Val::Px(6.0);
            n.padding = UiRect::all(Val::Px(12.0));
            n.min_width = Val::Px(300.0);
        })
        .bg_color(Color::srgba(0.05, 0.08, 0.15, 0.92))
        .insert(StateMarker);

    let t = ui.theme().text.clone();
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::header("Model  [F2]", &t)); });

    // Character selector
    setting_row(&mut ui, "Character", &t, |row| {
        row.add_button_observe("<", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>, folder: Res<CharacterFolder>| {
                let n = folder.files.len();
                if n > 0 { s.character_index = (s.character_index + n - 1) % n; s.save(); }
            });
        row.with_child(|v| {
            v.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 12.0, ..t.clone()
            })).insert(ModelSetting::CharacterName);
        });
        row.add_button_observe(">", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>, folder: Res<CharacterFolder>| {
                let n = folder.files.len();
                if n > 0 { s.character_index = (s.character_index + 1) % n; s.save(); }
            });
    });

    // Transform
    let sep = TextTheme { label_size: 12.0, label_color: Color::srgb(0.4, 0.65, 0.5), ..t.clone() };
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Transform —", &sep)); });
    mdl_row(&mut ui, "Scale",    &t, ModelSetting::Scale,
        |s: &mut ModelSettings| s.scale = (s.scale - 0.1).max(0.1),
        |s: &mut ModelSettings| s.scale = (s.scale + 0.1).min(10.0));
    mdl_row(&mut ui, "Offset Y", &t, ModelSetting::OffsetY,
        |s: &mut ModelSettings| s.translation_y -= 0.05,
        |s: &mut ModelSettings| s.translation_y += 0.05);
    mdl_row(&mut ui, "Rot Y",    &t, ModelSetting::RotY,
        |s: &mut ModelSettings| s.rotation_y_degrees = (s.rotation_y_degrees - 15.0).rem_euclid(360.0),
        |s: &mut ModelSettings| s.rotation_y_degrees = (s.rotation_y_degrees + 15.0).rem_euclid(360.0));

    // Animation mapping
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Animation Mapping —", &sep)); });
    for key in ANIM_KEYS {
        anim_mapping_row(&mut ui, key_label(*key), &t, *key);
    }

    // Debug anim selector
    ui.with_child(|c| { c.insert_bundle(lava_ui_builder::label("— Debug —", &sep)); });
    setting_row(&mut ui, "Play", &t, |row| {
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

fn key_label(key: AnimationKey) -> &'static str {
    match key {
        AnimationKey::Idle     => "Idle",
        AnimationKey::Walking  => "Walking",
        AnimationKey::Throwing => "Throwing",
        AnimationKey::Crawling => "Crawling",
        AnimationKey::Building => "Building",
    }
}

fn anim_mapping_row(ui: &mut UIBuilder, label: &str, t: &TextTheme, key: AnimationKey) {
    setting_row(ui, label, t, move |row| {
        row.add_button_observe("<", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>, clips: Res<PlayerAnimClips>| {
                let names = &clips.names;
                if names.is_empty() { return; }
                let cur = s.anim_mapping.get(key).to_string();
                let idx = names.iter().position(|n| n == &cur)
                    .map(|i| (i + names.len() - 1) % names.len())
                    .unwrap_or(0);
                s.anim_mapping.set(key, names[idx].clone());
                s.save();
            });
        row.with_child(|v| {
            v.insert_bundle(lava_ui_builder::label("—", &TextTheme { label_size: 11.0, ..t.clone() }))
             .insert(AnimMappingLabel(key));
        });
        row.add_button_observe(">", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>, clips: Res<PlayerAnimClips>| {
                let names = &clips.names;
                if names.is_empty() { return; }
                let cur = s.anim_mapping.get(key).to_string();
                let idx = names.iter().position(|n| n == &cur)
                    .map(|i| (i + 1) % names.len())
                    .unwrap_or(0);
                s.anim_mapping.set(key, names[idx].clone());
                s.save();
            });
    });
}

/// Helper: a camera setting row with label, value display, and dec/inc buttons.
fn cam_row(
    ui: &mut UIBuilder,
    label: &str,
    t: &TextTheme,
    setting: CameraSetting,
    dec: impl Fn(&mut GameSettings) + Send + Sync + 'static,
    inc: impl Fn(&mut GameSettings) + Send + Sync + 'static,
) {
    setting_row(ui, label, t, move |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            move |_: On<Activate>, mut s: ResMut<GameSettings>| { dec(&mut s); s.save(); });
        row.with_child(|v| {
            v.insert_bundle(lava_ui_builder::label("", &TextTheme::default()))
             .insert(setting);
        });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            move |_: On<Activate>, mut s: ResMut<GameSettings>| { inc(&mut s); s.save(); });
    });
}

/// Helper: a model setting row with label, value display, and dec/inc buttons.
fn mdl_row(
    ui: &mut UIBuilder,
    label: &str,
    t: &TextTheme,
    setting: ModelSetting,
    dec: impl Fn(&mut ModelSettings) + Send + Sync + 'static,
    inc: impl Fn(&mut ModelSettings) + Send + Sync + 'static,
) {
    setting_row(ui, label, t, move |row| {
        row.add_button_observe("-", |b| { b.size_px(32.0, 32.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>| { dec(&mut s); s.save(); });
        row.with_child(|v| {
            v.insert_bundle(lava_ui_builder::label("", &TextTheme::default()))
             .insert(setting);
        });
        row.add_button_observe("+", |b| { b.size_px(32.0, 32.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>| { inc(&mut s); s.save(); });
    });
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

// ── Panel marker components ───────────────────────────────────────────────────

#[derive(Component, Default)]
pub struct SettingsPanel;

#[derive(Component, Default)]
pub struct ModelPanel;

/// Identifies a camera-setting value label. One component type covers all rows.
#[derive(Component, Clone, Copy)]
pub enum CameraSetting {
    Zoom, Pitch, Yaw, Speed,
    OrthoVH, OrthoNear, OrthoFar,
    PerspNear, PerspFar,
}

/// Identifies a model-setting value label.
#[derive(Component, Clone, Copy)]
pub enum ModelSetting {
    CharacterName, Scale, OffsetY, RotY,
}

/// Identifies an animation-mapping value label; holds the key it represents.
#[derive(Component, Clone, Copy)]
pub struct AnimMappingLabel(pub AnimationKey);

#[derive(Component)]
pub struct AnimSelectorLabel;

// ── Toggle systems ────────────────────────────────────────────────────────────

pub fn toggle_settings_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: Query<&mut Node, With<SettingsPanel>>,
) {
    if !keys.just_pressed(KeyCode::F1) { return; }
    if let Ok(mut node) = panel.single_mut() {
        node.display = match node.display { Display::None => Display::Flex, _ => Display::None };
    }
}

pub fn toggle_model_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: Query<&mut Node, With<ModelPanel>>,
) {
    if !keys.just_pressed(KeyCode::F2) { return; }
    if let Ok(mut node) = panel.single_mut() {
        node.display = match node.display { Display::None => Display::Flex, _ => Display::None };
    }
}

// ── Update systems ────────────────────────────────────────────────────────────

pub fn update_camera_panel(
    settings: Res<GameSettings>,
    mut labels: Query<(&CameraSetting, &mut Text)>,
) {
    if !settings.is_changed() { return; }
    for (setting, mut text) in labels.iter_mut() {
        **text = match setting {
            CameraSetting::Zoom      => format!("{:.0}",  settings.zoom),
            CameraSetting::Pitch     => format!("{:.0}°", settings.pitch_degrees),
            CameraSetting::Yaw       => format!("{:.0}°", settings.yaw_degrees),
            CameraSetting::Speed     => format!("{:.2}×", settings.player_speed_multiplier),
            CameraSetting::OrthoVH   => format!("{:.2}",  settings.ortho_viewport_height),
            CameraSetting::OrthoNear => format!("{:.0}",  settings.ortho_near),
            CameraSetting::OrthoFar  => format!("{:.0}",  settings.ortho_far),
            CameraSetting::PerspNear => format!("{:.2}",  settings.persp_near),
            CameraSetting::PerspFar  => format!("{:.0}",  settings.persp_far),
        };
    }
}

pub fn update_model_labels(
    settings: Res<ModelSettings>,
    folder: Res<CharacterFolder>,
    mut labels: Query<(&ModelSetting, &mut Text)>,
) {
    if !settings.is_changed() { return; }
    for (setting, mut text) in labels.iter_mut() {
        **text = match setting {
            ModelSetting::CharacterName => folder.files
                .get(settings.character_index)
                .map(|f| CharacterFolder::display_name(f).to_string())
                .unwrap_or_else(|| "—".to_string()),
            ModelSetting::Scale   => format!("{:.2}",  settings.scale),
            ModelSetting::OffsetY => format!("{:.2}",  settings.translation_y),
            ModelSetting::RotY    => format!("{:.0}°", settings.rotation_y_degrees),
        };
    }
}

pub fn update_anim_mapping_labels(
    settings: Res<ModelSettings>,
    mut labels: Query<(&AnimMappingLabel, &mut Text)>,
) {
    if !settings.is_changed() { return; }
    for (label, mut text) in labels.iter_mut() {
        let name = settings.anim_mapping.get(label.0);
        **text = if name.is_empty() { "—".to_string() } else { name.to_string() };
    }
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
    if let Some(counter) = alien_counter
        && let Ok(mut t) = alien_text.single_mut() {
            **t = format!("Aliens: {}", counter.count);
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
    #[allow(dead_code)]
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
