use bevy::prelude::*;
use crate::alien::components::general::{Alien, AlienCounter};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::player::components::IsBuilding;
use crate::settings::resources::{GameSettings, ProjectionMode};
use lava_ui_builder::{progress_bar, ProgressBar, WorldFollower};

#[derive(Message, Clone)]
pub struct GotoState {
    pub state: GameState,
}

pub fn spawn_menu(
    mut commands: Commands,
) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        StateMarker,
    )).with_children(|parent| {
        parent.spawn((
            Button,
            Node {
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.3, 0.3, 0.8)),
        )).with_children(|parent| {
            parent.spawn((
                Text::new("Start Game"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });
}

/// Marker for menu entities so cleanup_menu can remove them.
#[derive(Component)]
pub struct StateMarker;

pub fn goto_state_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut goto_state_mr: MessageReader<GotoState>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
) {
    // Handle button presses
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::InGame);
        }
    }
    // Also handle programmatic state changes
    for goto_state in goto_state_mr.read() {
        next_state.set(goto_state.state.clone());
    }
}

pub fn cleanup_menu(
    mut commands: Commands,
    entities: Query<Entity, With<StateMarker>>,
) {
    for entity in entities.iter() {
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
pub struct HudAlienCount;

#[derive(Component)]
pub struct HudBuildMode;

#[derive(Component)]
pub struct HudProjection;

pub fn spawn_ui(mut commands: Commands) {
    // Top-left HUD overlay
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        StateMarker,
    )).with_children(|p| {
        p.spawn((
            Text::new("Aliens: 0"),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::WHITE),
            HudAlienCount,
        ));
        p.spawn((
            Text::new(""),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::srgb(1.0, 0.8, 0.2)),
            HudBuildMode,
        ));
        p.spawn((
            Text::new(""),
            TextFont { font_size: 14.0, ..default() },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
            HudProjection,
        ));
    });
}

pub fn update_hud(
    alien_counter: Option<Res<AlienCounter>>,
    building_query: Query<(), With<IsBuilding>>,
    settings: Res<GameSettings>,
    mut alien_text: Query<&mut Text, (With<HudAlienCount>, Without<HudBuildMode>, Without<HudProjection>)>,
    mut build_text: Query<&mut Text, (With<HudBuildMode>, Without<HudAlienCount>, Without<HudProjection>)>,
    mut proj_text: Query<&mut Text, (With<HudProjection>, Without<HudAlienCount>, Without<HudBuildMode>)>,
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
        **t = format!("{proj_name} zoom:{:.0} pitch:{:.0}°  [P]/[]/,.", settings.zoom, settings.pitch_degrees);
    }
}

#[derive(Message, Clone)]
pub struct AddHealthBar {
    pub entity: Entity,
    pub name: &'static str,
}

pub fn add_health_bar(
    mut commands: Commands,
    mut add_health_bar_mr: MessageReader<AddHealthBar>,
) {
    for msg in add_health_bar_mr.read() {
        let target = msg.entity;
        let bar = commands.spawn((
            WorldFollower { target, offset: Vec2::new(-30.0, -40.0) },
            progress_bar(1.0, 60.0, 8.0, Color::srgb(0.0, 1.0, 0.0), Color::srgba(0.0, 0.0, 0.0, 0.5)),
        )).id();
        commands.entity(bar).entry::<Node>().and_modify(|mut n| n.position_type = PositionType::Absolute);
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
