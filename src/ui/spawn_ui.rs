use crate::alien::components::general::AlienCounter;
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::player::components::IsBuilding;
use crate::settings::resources::{GameSettings, ProjectionMode};
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
        |btn| { btn.size_px(220.0, 52.0); },
        |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
            next_state.set(GameState::InGame);
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

pub fn spawn_ui(commands: Commands, theme: Res<LavaTheme>) {
    let text_theme = theme.text.clone();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
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
            "{proj_name} zoom:{:.0} pitch:{:.0}° yaw:{:.0}°  [P]/[Z,X]/[C,V]/[B,N]",
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
