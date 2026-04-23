use bevy::prelude::*;
use crate::game_state::GameState;
use crate::general::components::Health;
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

pub fn spawn_ui(mut _commands: Commands) {
    // In-game HUD placeholder — health bars are spawned dynamically via AddHealthBar
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
