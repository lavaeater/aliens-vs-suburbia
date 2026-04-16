use bevy::prelude::*;
use crate::camera::components::GameCamera;
use crate::game_state::GameState;
use crate::general::components::Health;

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
    mut state: ResMut<NextState<GameState>>,
    mut goto_state_mr: MessageReader<GotoState>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // Handle button presses
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::InGame);
        }
    }
    // Also handle programmatic state changes
    for goto_state in goto_state_mr.read() {
        state.set(goto_state.state.clone());
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
    for add_health_bar in add_health_bar_mr.read() {
        let target = add_health_bar.entity;
        commands.spawn((
            Fellow { target },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(60.0),
                height: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        )).with_children(|parent| {
            parent.spawn((
                HealthBarFill { target },
                Node {
                    position_type: PositionType::Relative,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.0, 1.0, 0.0)),
            ));
        });
    }
}

#[derive(Component)]
pub struct Fellow {
    pub target: Entity,
}

#[derive(Component)]
pub struct HealthBarFill {
    pub target: Entity,
}

pub fn fellow_system(
    mut fellows: Query<(Entity, &Fellow, &mut Node)>,
    mut health_fills: Query<(&HealthBarFill, &mut Node), Without<Fellow>>,
    transforms: Query<&GlobalTransform>,
    health_query: Query<&Health>,
    mut commands: Commands,
    camera_q: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
) {
    let Ok((camera, camera_global_transform)) = camera_q.single() else {
        return;
    };

    for (entity, fellow, mut node) in fellows.iter_mut() {
        let Ok(tr) = transforms.get(fellow.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        if let Ok(pos) = camera.world_to_viewport(camera_global_transform, tr.translation()) {
            node.left = Val::Px((pos.x - 30.0).round());
            node.top = Val::Px((pos.y - 40.0).round());
        }
    }

    for (fill, mut node) in health_fills.iter_mut() {
        if let Ok(health) = health_query.get(fill.target) {
            let pct = (health.health as f32 / health.max_health as f32 * 100.0).clamp(0.0, 100.0);
            node.width = Val::Percent(pct);
        }
    }
}
