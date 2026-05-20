use crate::alien::components::general::AlienCounter;
use crate::alien::wave_manager::WaveManager;
use crate::game_state::score_keeper::LevelTracker;
use crate::animation::animation_plugin::{AnimationKey, ANIM_KEYS};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::player::components::IsBuilding;
use crate::settings::resources::{GameSettings, ProjectionMode};
use crate::model_settings::resources::{CharacterFolder, ModelSettings, PlayerAnimClips};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{
    ButtonTheme, LavaTheme, ProgressBar, TextStyle, TextTheme, UIBuilder, WorldFollower, progress_bar,
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

    ui.themed_header("Aliens vs Suburbia");

    ui.add_button_observe(
        "Start Game",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::PlayerSetup);
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

    ui.add_button_observe(
        "Map Editor",
        |btn| { btn.size_px(220.0, 52.0).font_size(20.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::MapEditor);
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

/// Marker on the pass-through meter progress bar.
#[derive(Component)]
pub struct HudAlienMeter;

/// Marker on the wave info label.
#[derive(Component)]
pub struct HudWaveInfo;

/// Marker on the coin counter label.
#[derive(Component)]
pub struct HudCoins;

/// Marker on the ability cooldown label.
#[derive(Component)]
pub struct HudAbility;

/// Marker on the build cost label.
#[derive(Component)]
pub struct HudBuildCost;

pub fn spawn_ui(mut commands: Commands, theme: Res<LavaTheme>) {
    // ── Top-left HUD labels ──────────────────────────────────────────────────
    {
        let mut ui = UIBuilder::new(commands.reborrow(), Some(theme.clone()));
        ui.insert(StateMarker)
          .absolute_position().top(px(8.0)).left(px(8.0))
          .flex_column().row_gap_px(4.0);

        ui.with_child(|c| { c.with_text("Aliens: 0", Some(TextStyle::size_color(theme.text.label_size, theme.text.label_color))).insert(HudAlienCount); });
        ui.with_child(|c| { c.with_text("Coins: 0",  Some(TextStyle::size_color(14.0, Color::srgb(1.0, 0.85, 0.1)))).insert(HudCoins); });
        ui.with_child(|c| { c.with_text("[Q] Ability — ready", Some(TextStyle::size_color(13.0, Color::srgb(0.5, 0.9, 1.0)))).insert(HudAbility); });
        ui.with_child(|c| { c.with_text("Wave 1 / 3 in 5s",   Some(TextStyle::size_color(13.0, Color::srgb(0.5, 0.8, 1.0)))).insert(HudWaveInfo); });
        ui.with_child(|c| { c.with_text("", Some(TextStyle::size_color(theme.text.label_size, Color::srgb(1.0, 0.8, 0.2)))).insert(HudBuildMode); });
        ui.with_child(|c| { c.with_text("", Some(TextStyle::size_color(13.0, Color::srgb(0.8, 0.8, 0.2)))).insert(HudBuildCost); });
        ui.with_child(|c| { c.with_text("", Some(TextStyle::size_color(14.0, Color::srgb(0.6, 0.6, 0.6)))).insert(HudProjection); });

        ui.build();
    }

    // ── Pass-through meter — fixed at top-centre ─────────────────────────────
    {
        let mut ui = UIBuilder::new(commands.reborrow(), Some(theme.clone()));
        ui.insert(StateMarker)
          .absolute_position().top(px(8.0)).left(percent(50.0))
          .modify_node(|mut n| n.margin.left = Val::Px(-90.0))
          .flex_column().align_items_center().row_gap_px(2.0);

        ui.with_child(|c| { c.with_text("Aliens escaped: 0 / 10", Some(TextStyle::size_color(13.0, Color::srgb(1.0, 0.35, 0.2)))).insert(HudAlienMeter); });
        ui.with_child(|c| {
            c.insert_bundle(progress_bar(0.0, 180.0, 10.0,
                Color::srgb(1.0, 0.25, 0.1),
                Color::srgba(0.0, 0.0, 0.0, 0.5),
            )).insert(HudAlienMeter);
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
        .absolute_position().top(px(8.0)).right(px(8.0))
        .flex_column().row_gap_px(6.0).padding_all_px(12.0)
        .min_width_px(350.0)
        .bg_color(Color::srgba(0.05, 0.12, 0.07, 0.92))
        .insert(StateMarker);

    let t = ui.theme().text.clone();
    ui.themed_header("Camera  [F1]");

    setting_row(&mut ui, "Projection", &t, |row| {
        row.add_button_observe("Ortho", |b| { b.size_px(70.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| { s.projection = ProjectionMode::Orthographic; s.save(); });
        row.add_button_observe("Persp", |b| { b.size_px(70.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<GameSettings>| { s.projection = ProjectionMode::Perspective; s.save(); });
    });
    cam_row(&mut ui, "Zoom",  &t, CameraSetting::Zoom,
        |s| s.zoom = (s.zoom - 1.0).max(1.0),
        |s| s.zoom = (s.zoom - 0.1).max(1.0),
        |s| s.zoom = (s.zoom + 0.1).min(60.0),
        |s| s.zoom = (s.zoom + 1.0).min(60.0));
    cam_row(&mut ui, "Pitch", &t, CameraSetting::Pitch,
        |s| s.pitch_degrees = (s.pitch_degrees - 5.0).max(-89.0),
        |s| s.pitch_degrees = (s.pitch_degrees - 1.0).max(-89.0),
        |s| s.pitch_degrees = (s.pitch_degrees + 1.0).min(-5.0),
        |s| s.pitch_degrees = (s.pitch_degrees + 5.0).min(-5.0));
    cam_row(&mut ui, "Yaw",   &t, CameraSetting::Yaw,
        |s| s.yaw_degrees = (s.yaw_degrees - 15.0).rem_euclid(360.0),
        |s| s.yaw_degrees = (s.yaw_degrees -  1.0).rem_euclid(360.0),
        |s| s.yaw_degrees = (s.yaw_degrees +  1.0).rem_euclid(360.0),
        |s| s.yaw_degrees = (s.yaw_degrees + 15.0).rem_euclid(360.0));
    cam_row(&mut ui, "Speed", &t, CameraSetting::Speed,
        |s| s.player_speed_multiplier = (s.player_speed_multiplier - 0.25).max(0.25),
        |s| s.player_speed_multiplier = (s.player_speed_multiplier - 0.05).max(0.05),
        |s| s.player_speed_multiplier = (s.player_speed_multiplier + 0.05).min(5.0),
        |s| s.player_speed_multiplier = (s.player_speed_multiplier + 0.25).min(5.0));

    ui.label("— Ortho —", 12.0, Color::srgb(0.4, 0.65, 0.5));
    cam_row(&mut ui, "V.Height", &t, CameraSetting::OrthoVH,
        |s| s.ortho_viewport_height = (s.ortho_viewport_height - 0.25).max(0.25),
        |s| s.ortho_viewport_height = (s.ortho_viewport_height - 0.05).max(0.05),
        |s| s.ortho_viewport_height += 0.05,
        |s| s.ortho_viewport_height += 0.25);
    cam_row(&mut ui, "Near", &t, CameraSetting::OrthoNear,
        |s| s.ortho_near -= 50.0,
        |s| s.ortho_near -=  1.0,
        |s| s.ortho_near = (s.ortho_near +  1.0).min(0.0),
        |s| s.ortho_near = (s.ortho_near + 50.0).min(0.0));
    cam_row(&mut ui, "Far", &t, CameraSetting::OrthoFar,
        |s| s.ortho_far = (s.ortho_far - 100.0).max(1.0),
        |s| s.ortho_far = (s.ortho_far -   1.0).max(1.0),
        |s| s.ortho_far += 1.0,
        |s| s.ortho_far += 100.0);

    ui.label("— Persp —", 12.0, Color::srgb(0.4, 0.65, 0.5));
    cam_row(&mut ui, "FOV", &t, CameraSetting::PerspFOV,
        |s| s.persp_fov = (s.persp_fov - 5.0).max(10.0),
        |s| s.persp_fov = (s.persp_fov - 1.0).max(10.0),
        |s| s.persp_fov = (s.persp_fov + 1.0).min(170.0),
        |s| s.persp_fov = (s.persp_fov + 5.0).min(170.0));
    cam_row(&mut ui, "Near", &t, CameraSetting::PerspNear,
        |s| s.persp_near = (s.persp_near - 0.05).max(0.01),
        |s| s.persp_near = (s.persp_near - 0.01).max(0.001),
        |s| s.persp_near = (s.persp_near + 0.01).min(s.persp_far - 0.1),
        |s| s.persp_near = (s.persp_near + 0.05).min(s.persp_far - 0.1));
    cam_row(&mut ui, "Far",  &t, CameraSetting::PerspFar,
        |s| s.persp_far = (s.persp_far - 100.0).max(s.persp_near + 1.0),
        |s| s.persp_far = (s.persp_far -   1.0).max(s.persp_near + 1.0),
        |s| s.persp_far += 1.0,
        |s| s.persp_far += 100.0);

    ui.build();
}

pub fn spawn_model_panel(commands: Commands, theme: &LavaTheme) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<ModelPanel>()
        .display_none()
        .absolute_position().top(px(8.0)).right(px(366.0))
        .flex_column().row_gap_px(6.0).padding_all_px(12.0)
        .min_width_px(350.0)
        .bg_color(Color::srgba(0.05, 0.08, 0.15, 0.92))
        .insert(StateMarker);

    let t = ui.theme().text.clone();
    ui.themed_header("Model  [F2]");

    // Character selector
    setting_row(&mut ui, "Character", &t, |row| {
        row.add_button_observe("<", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>, folder: Res<CharacterFolder>,
             panel: Query<&Node, With<ModelPanel>>| {
                if panel.single().is_ok_and(|n| n.display == Display::None) { return; }
                let n = folder.files.len();
                if n > 0 { s.character_index = (s.character_index + n - 1) % n; s.save(); }
            });
        row.with_child(|v| {
            v.with_text("", Some(TextStyle::size(12.0))).insert(ModelSetting::CharacterName);
        });
        row.add_button_observe(">", |b| { b.size_px(32.0, 32.0); },
            |_: On<Activate>, mut s: ResMut<ModelSettings>, folder: Res<CharacterFolder>,
             panel: Query<&Node, With<ModelPanel>>| {
                if panel.single().is_ok_and(|n| n.display == Display::None) { return; }
                let n = folder.files.len();
                if n > 0 { s.character_index = (s.character_index + 1) % n; s.save(); }
            });
    });

    // Transform
    ui.label("— Transform —", 12.0, Color::srgb(0.4, 0.65, 0.5));
    mdl_row(&mut ui, "Scale",    &t, ModelSetting::Scale,
        |s| s.scale = (s.scale - 0.1).max(0.01),
        |s| s.scale = (s.scale - 0.01).max(0.01),
        |s| s.scale = (s.scale + 0.01).min(20.0),
        |s| s.scale = (s.scale + 0.1).min(20.0));
    mdl_row(&mut ui, "Offset Y", &t, ModelSetting::OffsetY,
        |s| s.translation_y -= 0.1,
        |s| s.translation_y -= 0.01,
        |s| s.translation_y += 0.01,
        |s| s.translation_y += 0.1);
    mdl_row(&mut ui, "Rot Y",    &t, ModelSetting::RotY,
        |s| s.rotation_y_degrees = (s.rotation_y_degrees - 15.0).rem_euclid(360.0),
        |s| s.rotation_y_degrees = (s.rotation_y_degrees -  1.0).rem_euclid(360.0),
        |s| s.rotation_y_degrees = (s.rotation_y_degrees +  1.0).rem_euclid(360.0),
        |s| s.rotation_y_degrees = (s.rotation_y_degrees + 15.0).rem_euclid(360.0));

    // Animation mapping
    ui.label("— Animation Mapping —", 12.0, Color::srgb(0.4, 0.65, 0.5));
    for key in ANIM_KEYS {
        anim_mapping_row(&mut ui, key_label(*key), &t, *key);
    }

    ui.build();
}

fn key_label(key: AnimationKey) -> &'static str {
    match key {
        AnimationKey::Idle      => "Idle",
        AnimationKey::IdleShoot => "Idle Shoot",
        AnimationKey::Walk      => "Walk",
        AnimationKey::WalkShoot => "Walk Shoot",
        AnimationKey::Run       => "Run",
        AnimationKey::RunShoot  => "Run Shoot",
        AnimationKey::RunGun    => "Run Gun",
        AnimationKey::Duck      => "Duck",
        AnimationKey::Jump      => "Jump",
        AnimationKey::JumpIdle  => "Jump Idle",
        AnimationKey::JumpLand  => "Jump Land",
        AnimationKey::Punch     => "Punch",
        AnimationKey::Wave      => "Wave",
        AnimationKey::Yes       => "Yes",
        AnimationKey::No        => "No",
        AnimationKey::Death     => "Death",
        AnimationKey::HitReact  => "Hit React",
        AnimationKey::Throwing  => "Throwing",
        AnimationKey::Building  => "Building",
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
            v.with_text("—", Some(TextStyle::size(11.0))).insert(AnimMappingLabel(key));
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

/// Camera setting row: `[ << ][ < ]  value  [ > ][ >> ]`
/// Outer buttons are the coarse step, inner are the fine step.
#[allow(clippy::too_many_arguments)]
fn cam_row(
    ui: &mut UIBuilder,
    label: &str,
    t: &TextTheme,
    setting: CameraSetting,
    coarse_dec: impl Fn(&mut GameSettings) + Send + Sync + 'static,
    fine_dec:   impl Fn(&mut GameSettings) + Send + Sync + 'static,
    fine_inc:   impl Fn(&mut GameSettings) + Send + Sync + 'static,
    coarse_inc: impl Fn(&mut GameSettings) + Send + Sync + 'static,
) {
    setting_row(ui, label, t, move |row| {
        row.add_button_observe("<<", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<GameSettings>| { coarse_dec(&mut s); s.save(); });
        row.add_button_observe("<",  |b| { b.size_px(24.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<GameSettings>| { fine_dec(&mut s); s.save(); });
        row.with_child(|v| {
            v.default_text("").insert(setting).min_width_px(44.0);
        });
        row.add_button_observe(">",  |b| { b.size_px(24.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<GameSettings>| { fine_inc(&mut s); s.save(); });
        row.add_button_observe(">>", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<GameSettings>| { coarse_inc(&mut s); s.save(); });
    });
}

/// Model setting row: `[ << ][ < ]  value  [ > ][ >> ]`
#[allow(clippy::too_many_arguments)]
fn mdl_row(
    ui: &mut UIBuilder,
    label: &str,
    t: &TextTheme,
    setting: ModelSetting,
    coarse_dec: impl Fn(&mut ModelSettings) + Send + Sync + 'static,
    fine_dec:   impl Fn(&mut ModelSettings) + Send + Sync + 'static,
    fine_inc:   impl Fn(&mut ModelSettings) + Send + Sync + 'static,
    coarse_inc: impl Fn(&mut ModelSettings) + Send + Sync + 'static,
) {
    setting_row(ui, label, t, move |row| {
        row.add_button_observe("<<", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>| { coarse_dec(&mut s); s.save(); });
        row.add_button_observe("<",  |b| { b.size_px(24.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>| { fine_dec(&mut s); s.save(); });
        row.with_child(|v| {
            v.default_text("").insert(setting).min_width_px(44.0);
        });
        row.add_button_observe(">",  |b| { b.size_px(24.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>| { fine_inc(&mut s); s.save(); });
        row.add_button_observe(">>", |b| { b.size_px(28.0, 28.0); },
            move |_: On<Activate>, mut s: ResMut<ModelSettings>| { coarse_inc(&mut s); s.save(); });
    });
}

fn setting_row<F: FnOnce(&mut UIBuilder)>(
    ui: &mut UIBuilder,
    label: &str,
    text_theme: &TextTheme,
    f: F,
) {
    let color = text_theme.label_color;
    ui.add_row(|row| {
        row.gap_px(4.0).align_items_center().width_px(310.0);
        row.with_child(|c| {
            c.with_text(label, Some(TextStyle::size_color(16.0, color))).width_px(70.0);
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
    PerspFOV, PerspNear, PerspFar,
}

/// Identifies a model-setting value label.
#[derive(Component, Clone, Copy)]
pub enum ModelSetting {
    CharacterName, Scale, OffsetY, RotY,
}

/// Identifies an animation-mapping value label; holds the key it represents.
#[derive(Component, Clone, Copy)]
pub struct AnimMappingLabel(pub AnimationKey);


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
            CameraSetting::PerspFOV  => format!("{:.0}°", settings.persp_fov),
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


#[allow(clippy::type_complexity)]
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

pub fn update_build_cost_hud(
    building: Query<&crate::player::components::BuildingIndicator, With<crate::player::components::IsBuilding>>,
    wallet: Option<Res<crate::general::systems::coin_system::TeamWallet>>,
    model_defs: Option<Res<crate::general::components::map_components::MapModelDefinitions>>,
    mut label: Query<(&mut Text, &mut TextColor), With<HudBuildCost>>,
) {
    let Ok((mut text, mut color)) = label.single_mut() else { return };
    let Ok(indicator) = building.single() else {
        **text = String::new();
        return;
    };

    let cost = if let Some(ref defs) = model_defs {
        let key = defs.build_indicators.get(indicator.1 as usize).copied().unwrap_or("");
        match key {
            "tower"      => 50u32,
            "tower_slow" => 75,
            "tower_area" => 100,
            _            => 0,
        }
    } else { 0 };

    let coins = wallet.as_ref().map(|w| w.coins).unwrap_or(0);
    let can_afford = coins >= cost;
    **text = format!("Cost: {} coins  (have {})", cost, coins);
    *color = TextColor(if can_afford {
        Color::srgb(0.8, 0.8, 0.2)
    } else {
        Color::srgb(1.0, 0.2, 0.2)
    });
}

pub fn update_ability_hud(
    players: Query<(&crate::player::systems::abilities::SpecialAbility, &crate::player::systems::abilities::AbilityCooldown), With<crate::player::components::Player>>,
    mut label: Query<&mut Text, With<HudAbility>>,
) {
    let Ok(mut t) = label.single_mut() else { return };
    let Ok((ability, meter)) = players.single() else { return };
    if meter.ready() {
        **t = format!("[Q] {} — READY", ability.label());
    } else {
        let pct = (meter.charge * 100.0) as u32;
        **t = format!("[Q] {} — {}%", ability.label(), pct);
    }
}

pub fn update_coin_hud(
    wallet: Option<Res<crate::general::systems::coin_system::TeamWallet>>,
    mut label: Query<&mut Text, With<HudCoins>>,
) {
    let Some(wallet) = wallet else { return };
    if !wallet.is_changed() { return; }
    if let Ok(mut t) = label.single_mut() {
        **t = format!("Coins: {}", wallet.coins);
    }
}

pub fn update_wave_hud(
    wave_manager: Option<Res<WaveManager>>,
    mut label: Query<&mut Text, With<HudWaveInfo>>,
) {
    let Some(wm) = wave_manager else { return };
    if !wm.is_changed() { return; }
    if let Ok(mut t) = label.single_mut() {
        **t = wm.label();
    }
}

pub fn update_alien_meter(
    tracker: Option<Res<LevelTracker>>,
    mut meter_text: Query<&mut Text, With<HudAlienMeter>>,
    mut meter_bar: Query<&mut ProgressBar, With<HudAlienMeter>>,
) {
    let Some(tracker) = tracker else { return };
    if !tracker.is_changed() { return; }

    let escaped = tracker.aliens_reached_goal;
    let cutoff = tracker.aliens_win_cut_off.max(1);
    let fraction = (escaped as f32 / cutoff as f32).clamp(0.0, 1.0);

    if let Ok(mut text) = meter_text.single_mut() {
        **text = format!("Aliens escaped: {} / {}", escaped, cutoff);
    }
    if let Ok(mut bar) = meter_bar.single_mut() {
        bar.value = fraction;
    }
}
