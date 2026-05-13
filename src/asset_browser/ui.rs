use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{InteractionPalette, LavaTheme, TextTheme, UIBuilder};
use crate::asset_browser::state::AssetBrowserState;
use crate::asset_browser::viewer::AssetBrowserViewerPanel;
use crate::game_state::GameState;
use crate::ui::spawn_ui::StateMarker;

#[derive(Component)]
pub struct AssetListContainer;

#[derive(Component)]
pub struct AssetPathLabel;

#[derive(Component)]
pub struct AssetAnimLabel;

#[derive(Component)]
pub struct NodeListContainer;

#[derive(Component)]
pub struct ListItem(pub usize);

pub fn spawn_asset_browser_ui(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<AssetBrowserState>,
) {
    state.reset_for_enter();

    let file_count = state.files.len();
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row();

    let text_theme = theme.text.clone();

    ui.with_child(|left| {
        left.modify_node(|mut n| {
                n.width = Val::Percent(22.0);
                n.min_width = Val::Px(220.0);
                n.max_width = Val::Px(480.0);
                n.height = Val::Percent(100.0);
            })
            .display_flex()
            .flex_column()
            .gap_px(4.0)
            .padding_all_px(8.0)
            .bg_color(Color::srgba(0.04, 0.07, 0.10, 0.97));

        left.with_child(|t| {
            t.insert_bundle(lava_ui_builder::header("Asset Browser", &text_theme));
        });

        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label(
                &format!("{file_count} models  [↑↓] navigate  [Enter] load"),
                &TextTheme {
                    label_size: 11.0,
                    label_color: Color::srgb(0.5, 0.7, 0.5),
                    ..text_theme.clone()
                },
            ));
        });

        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label(
                "[PgUp/PgDn] jump  [T] toon  [Esc] back",
                &TextTheme {
                    label_size: 11.0,
                    label_color: Color::srgb(0.4, 0.55, 0.4),
                    ..text_theme.clone()
                },
            ));
        });

        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label(
                "[[/]] prev/next anim",
                &TextTheme {
                    label_size: 11.0,
                    label_color: Color::srgb(0.4, 0.55, 0.4),
                    ..text_theme.clone()
                },
            ));
        });

        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 10.0,
                label_color: Color::srgb(0.55, 0.75, 1.0),
                ..text_theme.clone()
            }))
            .insert(AssetPathLabel)
            .modify_node(|mut n| {
                n.overflow = Overflow::clip();
            });
        });

        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 11.0,
                label_color: Color::srgb(0.8, 0.65, 1.0),
                ..text_theme.clone()
            }))
            .insert(AssetAnimLabel);
        });

        left.with_child(|node_list| {
            node_list
                .display_flex()
                .flex_wrap()
                .gap_px(3.0)
                .modify_node(|mut n| n.align_self = AlignSelf::Stretch)
                .insert(NodeListContainer);
        });

        left.with_child(|list_wrap| {
            list_wrap.with_flex_grow(1.0)
                .width_percent(100.0)
                .overflow_scroll_y()
                .display_flex()
                .flex_column()
                .gap_px(1.0)
                .insert(AssetListContainer)
                .insert(ScrollPosition::default());
        });

        left.add_button_observe(
            "← Back to Menu",
            |b| {
                b.width(percent(100.0)).height(px(36.0)).font_size(14.0);
            },
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::Menu);
            },
        );
    });

    ui.with_child(|viewer| {
        viewer.with_flex_grow(1.0)
            .height_percent(100.0)
            .insert(AssetBrowserViewerPanel);
    });

    ui.build();
}

pub fn handle_key_input(
    mut state: ResMut<AssetBrowserState>,
    mut keyboard_reader: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in keyboard_reader.read() {
        if event.state != ButtonState::Pressed { continue; }
        match &event.logical_key {
            Key::ArrowUp => state.move_up(),
            Key::ArrowDown => state.move_down(),
            Key::PageUp => state.page_up(),
            Key::PageDown => state.page_down(),
            Key::Enter => state.load_requested = true,
            Key::Escape => next_state.set(GameState::Menu),
            Key::Character(c) if c.eq_ignore_ascii_case("t") => {
                state.toon_shader = !state.toon_shader;
                state.load_requested = true;
            }
            Key::Character(c) if c == "]" => state.anim_next(),
            Key::Character(c) if c == "[" => state.anim_prev(),
            _ => {}
        }
    }
}

pub fn rebuild_list(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_query: Query<Entity, With<AssetListContainer>>,
    mut path_label: Query<&mut Text, With<AssetPathLabel>>,
) {
    if !state.list_dirty { return; }
    state.list_dirty = false;

    if let Ok(mut t) = path_label.single_mut() {
        **t = state.selected_path().unwrap_or("").to_string();
    }

    let Ok(container) = container_query.single() else { return; };
    commands.entity(container).despawn_related::<Children>();

    let selected = state.selected;
    let files_window: Vec<(usize, String)> = state
        .visible_files()
        .map(|(i, s)| (i, s.to_string()))
        .collect();
    let total = state.files.len();
    let scroll_offset = state.scroll_offset;

    commands.entity(container).with_children(|parent| {
        let window_end = (scroll_offset + 40).min(total);
        parent.spawn((
            Text::new(format!("{}-{} / {total}", scroll_offset + 1, window_end)),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::srgba(0.5, 0.55, 0.6, 0.6)),
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..Default::default()
            },
        ));

        for (idx, path) in files_window {
            let is_selected = idx == selected;
            let bg = if is_selected {
                Color::srgba(0.15, 0.35, 0.55, 0.95)
            } else {
                Color::srgba(0.06, 0.08, 0.12, 0.80)
            };
            let name_color = if is_selected {
                Color::srgb(1.0, 1.0, 1.0)
            } else {
                Color::srgb(0.72, 0.80, 0.88)
            };

            let filename = path.split('/').next_back().unwrap_or(&path).to_string();
            let dir_hint = {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() > 1 {
                    parts[..parts.len() - 1].join("/")
                } else {
                    String::new()
                }
            };

            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    flex_direction: FlexDirection::Column,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..Default::default()
                },
                BackgroundColor(bg),
                InteractionPalette {
                    none: bg,
                    hovered: Color::srgba(0.15, 0.28, 0.45, 0.95),
                    pressed: Color::srgba(0.10, 0.20, 0.36, 1.0),
                },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
                ListItem(idx),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(filename),
                    TextFont::default().with_font_size(13.0),
                    TextColor(name_color),
                ));
                if !dir_hint.is_empty() {
                    row.spawn((
                        Text::new(dir_hint),
                        TextFont::default().with_font_size(10.0),
                        TextColor(Color::srgba(0.50, 0.58, 0.68, 0.65)),
                    ));
                }
            })
            .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| {
                s.selected = idx;
                s.load_requested = true;
                s.list_dirty = true;
            });
        }
    });
}

pub fn scroll_to_selection(
    state: Res<AssetBrowserState>,
    mut container_q: Query<(Entity, &ComputedNode, &mut ScrollPosition), With<AssetListContainer>>,
    child_q: Query<(Option<&ListItem>, &ComputedNode)>,
    children_q: Query<&Children>,
) {
    let Ok((container_entity, container_node, mut scroll)) = container_q.single_mut() else { return };
    let Ok(children) = children_q.get(container_entity) else { return };

    let container_height = container_node.size().y;
    if container_height == 0.0 { return; }

    let mut y = 0.0f32;
    for child in children.iter() {
        let Ok((opt_item, child_node)) = child_q.get(child) else { continue };
        let h = child_node.size().y + 1.0; // +1 matches the gap
        if let Some(item) = opt_item {
            if item.0 == state.selected {
                let cur = scroll.0.y;
                if y < cur {
                    scroll.0.y = y;
                } else if y + h > cur + container_height {
                    scroll.0.y = y + h - container_height;
                }
                return;
            }
        }
        y += h;
    }
}

pub fn rebuild_node_list(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<NodeListContainer>>,
) {
    // Rebuild whenever the mesh node list changes (model just loaded) or nodes_dirty triggered by toggle.
    // We track this via a separate flag so we don't rebuild every frame.
    if !state.nodes_dirty { return; }

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let nodes: Vec<(String, bool)> = state.mesh_nodes.iter()
        .map(|n| (n.clone(), state.hidden_nodes.contains(n)))
        .collect();

    if nodes.is_empty() { return; }

    commands.entity(container).with_children(|parent| {
        for (name, hidden) in nodes {
            let bg = if hidden {
                Color::srgba(0.12, 0.10, 0.18, 0.85)
            } else {
                Color::srgba(0.25, 0.18, 0.40, 0.90)
            };
            let text_color = if hidden {
                Color::srgba(0.45, 0.40, 0.55, 0.70)
            } else {
                Color::srgb(0.88, 0.75, 1.0)
            };
            let name_clone = name.clone();
            parent.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..Default::default()
                },
                BackgroundColor(bg),
                InteractionPalette {
                    none: bg,
                    hovered: Color::srgba(0.35, 0.25, 0.55, 0.95),
                    pressed: Color::srgba(0.20, 0.14, 0.35, 1.0),
                },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
            ))
            .with_child((
                Text::new(name.clone()),
                TextFont::default().with_font_size(10.0),
                TextColor(text_color),
            ))
            .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| {
                s.toggle_node(&name_clone);
            });
        }
    });
}
