use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{InteractionPalette, LavaTheme, TextTheme, UIBuilder};
use crate::asset_browser::state::{ANIM_KEY_NAMES, AssetBrowserState, CHARACTER_NODE_PREFIX, ModelType};
use crate::asset_browser::viewer::AssetBrowserViewerPanel;
use crate::game_state::GameState;
use crate::ui::spawn_ui::StateMarker;

// ── Marker components ─────────────────────────────────────────────────────────

#[derive(Component)] pub struct AssetListContainer;
#[derive(Component)] pub struct AssetPathLabel;
#[derive(Component)] pub struct AssetAnimLabel;
#[derive(Component)] pub struct NodeListContainer;
#[derive(Component)] pub struct ClipTagContainer;
/// Kept for a future re-enable of the game-key -> tag-path binding UI. Currently
/// not spawned; bind resolution still works off the persisted `animation_bindings`.
#[allow(dead_code)]
#[derive(Component)] pub struct BindingContainer;
#[derive(Component)] pub struct FolderContainer;
#[derive(Component)] pub struct FolderPathLabel;
#[derive(Component)] pub struct HeightDisplay;
#[derive(Component)] pub struct TypeContainer;
#[derive(Component)] pub struct TypePropsContainer;
#[derive(Component)] pub struct SourcesContainer;

#[derive(Component)] pub struct ListItem(pub usize);

// ── Spawn ─────────────────────────────────────────────────────────────────────

pub fn spawn_asset_browser_ui(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<AssetBrowserState>,
) {
    state.reset_for_enter();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row();

    let t = theme.text.clone();
    let hint = TextTheme { label_size: 11.0, label_color: Color::srgb(0.4, 0.55, 0.4), ..t.clone() };

    ui.with_child(|left| {
        left.modify_node(|mut n| {
            n.width = Val::Percent(24.0);
            n.min_width = Val::Px(240.0);
            n.max_width = Val::Px(500.0);
            n.height = Val::Percent(100.0);
        })
        .display_flex().flex_column().gap_px(4.0).padding_all_px(8.0)
        .bg_color(Color::srgba(0.04, 0.07, 0.10, 0.97));

        left.with_child(|c| { c.insert_bundle(lava_ui_builder::header("Asset Browser", &t)); });
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("[Up/Dn] navigate  [Enter] load  [I] import", &hint)); });
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("[Bksp] up folder  [[ ]] anim  [= / -] height", &hint)); });

        // Current folder path
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 10.0, label_color: Color::srgb(0.5, 0.7, 0.9), ..t.clone()
            })).insert(FolderPathLabel)
            .modify_node(|mut n| n.overflow = Overflow::clip());
        });

        // Folder chips
        left.with_child(|c| {
            c.display_flex().flex_wrap().gap_px(3.0)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch)
             .insert(FolderContainer);
        });

        // Anim label + path label
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 10.0, label_color: Color::srgb(0.55, 0.75, 1.0), ..t.clone()
            })).insert(AssetPathLabel)
            .modify_node(|mut n| n.overflow = Overflow::clip());
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 11.0, label_color: Color::srgb(0.8, 0.65, 1.0), ..t.clone()
            })).insert(AssetAnimLabel);
        });

        // Hidden-node chips
        left.with_child(|c| {
            c.display_flex().flex_wrap().gap_px(3.0)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch)
             .insert(NodeListContainer);
        });

        // Clip tags section — tag each of the model's clips with a free-form path.
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("-- Clip Tags --", &TextTheme {
                label_size: 11.0, label_color: Color::srgb(0.5, 0.8, 0.6), ..t.clone()
            }));
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("click a clip, type a path e.g. Combat/Ranged/Shoot", &hint));
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("scroll wheel over the list to see more", &hint));
        });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .overflow_scroll_y()
             .modify_node(|mut n| { n.align_self = AlignSelf::Stretch; n.max_height = Val::Px(280.0); })
             .insert(ClipTagContainer).insert(ScrollPosition::default());
        });

        // Height section
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("-- Height --", &TextTheme {
                label_size: 11.0, label_color: Color::srgb(0.5, 0.8, 0.6), ..t.clone()
            }));
        });
        left.with_child(|row| {
            row.display_flex().flex_row().gap_px(4.0)
               .modify_node(|mut n| { n.align_items = AlignItems::Center; n.align_self = AlignSelf::Stretch; });

            row.add_button_observe("-", |b| { b.width(px(20.0)).height(px(20.0)).font_size(14.0); },
                |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.height_down(); });

            row.with_child(|c| {
                c.insert_bundle(lava_ui_builder::label("-- m  (x1.0000)", &TextTheme {
                    label_size: 11.0, label_color: Color::srgb(0.9, 0.85, 0.65), ..t.clone()
                })).insert(HeightDisplay)
                .modify_node(|mut n| { n.flex_grow = 1.0; });
            });

            row.add_button_observe("+", |b| { b.width(px(20.0)).height(px(20.0)).font_size(14.0); },
                |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.height_up(); });
        });

        // Model type section
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("-- Model Type --", &TextTheme {
                label_size: 11.0, label_color: Color::srgb(0.5, 0.8, 0.6), ..t.clone()
            }));
        });
        left.with_child(|c| {
            c.display_flex().flex_wrap().gap_px(3.0)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch)
             .insert(TypeContainer);
        });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch)
             .insert(TypePropsContainer);
        });

        // Animation sources section
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("-- Anim Sources --", &TextTheme {
                label_size: 11.0, label_color: Color::srgb(0.5, 0.8, 0.6), ..t.clone()
            }));
        });
        left.with_child(|c| {
            c.display_flex().flex_wrap().gap_px(3.0)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch)
             .insert(SourcesContainer);
        });
        left.add_button_observe("+ Add selected as source", |b| { b.width(percent(100.0)).height(px(22.0)).font_size(11.0); },
            |_: On<Activate>, mut s: ResMut<AssetBrowserState>| {
                if let Some(path) = s.selected_path().map(|p| p.to_string()) {
                    s.add_animation_source(path);
                }
            });

        // File list
        left.with_child(|c| {
            c.with_flex_grow(1.0).width_percent(100.0)
             .overflow_scroll_y().display_flex().flex_column().gap_px(1.0)
             .insert(AssetListContainer).insert(ScrollPosition::default());
        });

        // Import + back buttons
        left.add_button_observe("Clear mapping + nodes", |b| { b.width(percent(100.0)).height(px(26.0)).font_size(12.0); },
            |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.clear_all(); });

        left.add_button_observe("Import definition", |b| { b.width(percent(100.0)).height(px(30.0)).font_size(13.0); },
            |_: On<Activate>, s: ResMut<AssetBrowserState>| { s.export_definition(); });

        left.add_button_observe("<- Back to Menu", |b| { b.width(percent(100.0)).height(px(36.0)).font_size(14.0); },
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| { next.set(GameState::Menu); });
    });

    ui.with_child(|viewer| {
        viewer.with_flex_grow(1.0).height_percent(100.0).insert(AssetBrowserViewerPanel);
    });

    ui.build();
}

// ── Key input ─────────────────────────────────────────────────────────────────

pub fn handle_key_input(
    mut state: ResMut<AssetBrowserState>,
    mut keyboard_reader: MessageReader<KeyboardInput>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in keyboard_reader.read() {
        if event.state != ButtonState::Pressed { continue; }

        // While a tag text-box is open, keystrokes edit the buffer instead of navigating.
        if state.tag_edit_clip.is_some() {
            match &event.logical_key {
                Key::Escape    => state.tag_edit_cancel(),
                Key::Enter     => state.tag_edit_commit(),
                Key::Backspace => state.tag_edit_backspace(),
                Key::Space     => state.tag_edit_push(" "),
                Key::Character(c) => state.tag_edit_push(c.as_str()),
                _ => {}
            }
            continue;
        }

        match &event.logical_key {
            Key::ArrowUp        => state.move_up(),
            Key::ArrowDown      => state.move_down(),
            Key::PageUp         => state.page_up(),
            Key::PageDown       => state.page_down(),
            Key::Enter          => state.load_requested = true,
            Key::Escape         => next_state.set(GameState::Menu),
            Key::Backspace      => state.leave_folder(),
            Key::Character(c) if c == "]" => state.anim_next(),
            Key::Character(c) if c == "[" => state.anim_prev(),
            Key::Character(c) if c == "=" || c == "+" => state.height_up(),
            Key::Character(c) if c == "-" => state.height_down(),
            Key::Character(c) if c == "i" || c == "I" => state.export_definition(),
            _ => {}
        }
    }
}

// ── Rebuild systems ───────────────────────────────────────────────────────────

pub fn rebuild_folder_list(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<FolderContainer>>,
    mut path_label_q: Query<&mut Text, With<FolderPathLabel>>,
) {
    if !state.folder_list_dirty { return; }
    state.folder_list_dirty = false;

    if let Ok(mut t) = path_label_q.single_mut() {
        **t = format!("📁 {}", state.current_folder);
    }

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let folders: Vec<String> = state.folders.clone();
    if folders.is_empty() { return; }

    commands.entity(container).with_children(|parent| {
        for name in folders {
            let name_clone = name.clone();
            let bg = Color::srgba(0.10, 0.22, 0.35, 0.90);
            parent.spawn((
                Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..Default::default() },
                BackgroundColor(bg),
                InteractionPalette { none: bg, hovered: Color::srgba(0.18, 0.35, 0.55, 0.95), pressed: Color::srgba(0.10, 0.22, 0.40, 1.0) },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
            ))
            .with_child((
                Text::new(format!("📁 {name}")),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.75, 0.88, 1.0)),
            ))
            .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| {
                s.enter_folder(&name_clone);
            });
        }
    });
}

pub fn rebuild_list(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<AssetListContainer>>,
    mut path_label: Query<&mut Text, With<AssetPathLabel>>,
) {
    if !state.list_dirty { return; }
    state.list_dirty = false;

    if let Ok(mut t) = path_label.single_mut() {
        **t = state.selected_path().unwrap_or("").to_string();
    }

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let selected = state.selected;
    let files_window: Vec<(usize, String)> = state.visible_files()
        .map(|(i, s)| (i, s.to_string())).collect();
    let total = state.files.len();
    let scroll_offset = state.scroll_offset;

    commands.entity(container).with_children(|parent| {
        let window_end = (scroll_offset + 40).min(total);
        parent.spawn((
            Text::new(format!("{}-{} / {total}", scroll_offset + 1, window_end)),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::srgba(0.5, 0.55, 0.6, 0.6)),
            Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), ..Default::default() },
        ));

        for (idx, path) in files_window {
            let is_selected = idx == selected;
            let bg = if is_selected { Color::srgba(0.15, 0.35, 0.55, 0.95) } else { Color::srgba(0.06, 0.08, 0.12, 0.80) };
            let name_color = if is_selected { Color::srgb(1.0, 1.0, 1.0) } else { Color::srgb(0.72, 0.80, 0.88) };
            let filename = path.split('/').next_back().unwrap_or(&path).to_string();

            parent.spawn((
                Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..Default::default() },
                BackgroundColor(bg),
                InteractionPalette { none: bg, hovered: Color::srgba(0.15, 0.28, 0.45, 0.95), pressed: Color::srgba(0.10, 0.20, 0.36, 1.0) },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
                ListItem(idx),
            ))
            .with_child((Text::new(filename), TextFont::default().with_font_size(13.0), TextColor(name_color)))
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
        let h = child_node.size().y + 1.0;
        if let Some(item) = opt_item
            && item.0 == state.selected
        {
            let cur = scroll.0.y;
            if y < cur { scroll.0.y = y; }
            else if y + h > cur + container_height { scroll.0.y = y + h - container_height; }
            return;
        }
        y += h;
    }
}

/// Mouse-wheel scrolling for the clip-tag list: scrolls only while the cursor is
/// over the list, and clamps to the scrollable range so scroll offset never runs
/// past the content (Bevy clamps the rendered offset but not the component value).
pub fn scroll_clip_tag_list(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window>,
    mut container_q: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform, &mut ScrollPosition), With<ClipTagContainer>>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let mut dy = 0.0f32;
    for ev in wheel.read() {
        dy += match ev.unit {
            MouseScrollUnit::Line => ev.y * 18.0,
            MouseScrollUnit::Pixel => ev.y,
        };
    }
    if dy == 0.0 { return; }

    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let cursor = cursor * window.scale_factor(); // logical -> physical

    let Ok((node, transform, mut scroll)) = container_q.single_mut() else { return };
    let size = node.size();
    let center = transform.affine().translation;
    let min = center - size * 0.5;
    let max = center + size * 0.5;
    if cursor.x < min.x || cursor.x > max.x || cursor.y < min.y || cursor.y > max.y {
        return;
    }

    let max_scroll = (node.content_size().y - size.y).max(0.0);
    scroll.0.y = (scroll.0.y - dy).clamp(0.0, max_scroll);
}

pub fn rebuild_node_list(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<NodeListContainer>>,
) {
    if !state.nodes_ui_dirty { return; }
    state.nodes_ui_dirty = false;

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let nodes: Vec<(String, bool)> = state.mesh_nodes.iter()
        .filter(|n| !n.starts_with(CHARACTER_NODE_PREFIX))
        .map(|n| (n.clone(), state.hidden_nodes.contains(n)))
        .collect();

    if nodes.is_empty() { return; }

    commands.entity(container).with_children(|parent| {
        for (name, hidden) in nodes {
            let bg = if hidden { Color::srgba(0.12, 0.10, 0.18, 0.85) } else { Color::srgba(0.25, 0.18, 0.40, 0.90) };
            let tc = if hidden { Color::srgba(0.45, 0.40, 0.55, 0.70) } else { Color::srgb(0.88, 0.75, 1.0) };
            let name_clone = name.clone();
            parent.spawn((
                Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..Default::default() },
                BackgroundColor(bg),
                InteractionPalette { none: bg, hovered: Color::srgba(0.35, 0.25, 0.55, 0.95), pressed: Color::srgba(0.20, 0.14, 0.35, 1.0) },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
            ))
            .with_child((Text::new(name.clone()), TextFont::default().with_font_size(10.0), TextColor(tc)))
            .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.toggle_node(&name_clone); });
        }
    });
}

/// One list item per clip in the model (plus external-source clips). Click a clip
/// to open a text box and type a free-form hierarchical tag path. While editing,
/// the buffer and any matching autocomplete suggestions are shown inline.
pub fn rebuild_clip_tag_list(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<ClipTagContainer>>,
) {
    if !state.tags_dirty { return; }
    state.tags_dirty = false;

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    // Distinct, non-empty clip names, sorted, matching the browser's clip list.
    let mut clips: Vec<String> = state.anim_names.iter()
        .filter(|n| !n.is_empty())
        .cloned()
        .collect();
    clips.sort();
    clips.dedup();

    let editing = state.tag_edit_clip.clone();
    let buffer = state.tag_edit_buffer.clone();
    let suggestions = state.tag_suggestions();

    commands.entity(container).with_children(|parent| {
        if clips.is_empty() {
            parent.spawn((
                Text::new("no clips in this model"),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgba(0.4, 0.5, 0.4, 0.6)),
            ));
            return;
        }
        for clip in clips {
            let tag = state.clip_tags.get(&clip).cloned().unwrap_or_default();
            let short = clip.rsplit('|').next().unwrap_or(&clip).to_string();
            let is_editing = editing.as_deref() == Some(clip.as_str());

            // Clickable row: "clipname   tag".
            let clip_for_click = clip.clone();
            let bg = if is_editing { Color::srgba(0.14, 0.20, 0.10, 0.95) } else { Color::srgba(0.07, 0.10, 0.14, 0.70) };
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    column_gap: Val::Px(6.0),
                    ..Default::default()
                },
                BackgroundColor(bg),
                InteractionPalette { none: bg, hovered: Color::srgba(0.15, 0.24, 0.14, 0.95), pressed: Color::srgba(0.10, 0.16, 0.10, 1.0) },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(short),
                    TextFont::default().with_font_size(10.0),
                    TextColor(Color::srgb(0.8, 0.75, 0.9)),
                    Node { flex_grow: 1.0, overflow: Overflow::clip(), ..Default::default() },
                ));
                let (tag_text, tag_color) = if tag.is_empty() {
                    ("--".to_string(), Color::srgba(0.5, 0.55, 0.5, 0.6))
                } else {
                    (tag.clone(), Color::srgb(0.55, 0.85, 0.65))
                };
                row.spawn((
                    Text::new(tag_text),
                    TextFont::default().with_font_size(10.0),
                    TextColor(tag_color),
                ));
            })
            .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| {
                s.begin_tag_edit(&clip_for_click);
            });

            // While this clip is being edited, show the text box + suggestions.
            if is_editing {
                let display = format!("> {}_", buffer);
                parent.spawn((
                    Text::new(display),
                    TextFont::default().with_font_size(11.0),
                    TextColor(Color::srgb(0.95, 0.95, 0.7)),
                    Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..Default::default() },
                    BackgroundColor(Color::srgba(0.05, 0.08, 0.05, 0.9)),
                ));
                parent.spawn((
                    Text::new("[Enter] save  [Esc] cancel"),
                    TextFont::default().with_font_size(9.0),
                    TextColor(Color::srgba(0.45, 0.6, 0.45, 0.7)),
                    Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(0.0)), ..Default::default() },
                ));
                if !suggestions.is_empty() {
                    parent.spawn((
                        Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(3.0), row_gap: Val::Px(3.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..Default::default() },
                    ))
                    .with_children(|chips| {
                        for sug in &suggestions {
                            let sug_clone = sug.clone();
                            let cbg = Color::srgba(0.10, 0.22, 0.18, 0.9);
                            chips.spawn((
                                Node { padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..Default::default() },
                                BackgroundColor(cbg),
                                InteractionPalette { none: cbg, hovered: Color::srgba(0.15, 0.35, 0.28, 0.95), pressed: Color::srgba(0.08, 0.18, 0.14, 1.0) },
                                bevy::picking::hover::Hovered::default(),
                                bevy::ui_widgets::Button,
                            ))
                            .with_child((Text::new(sug.clone()), TextFont::default().with_font_size(9.0), TextColor(Color::srgb(0.7, 0.9, 0.8))))
                            .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.tag_edit_accept_suggestion(&sug_clone); });
                        }
                    });
                }
            }
        }
    });
}

/// One row per game animation key, cycling its binding through the in-use tag paths.
/// Currently unregistered — the bindings UI is hidden for now (see BindingContainer).
#[allow(dead_code)]
pub fn rebuild_binding_list(
    state: Res<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<BindingContainer>>,
) {
    if !state.tags_dirty { return; }

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let rows: Vec<(String, String)> = ANIM_KEY_NAMES.iter()
        .map(|&k| (k.to_string(), state.animation_bindings.get(k).cloned().unwrap_or_default()))
        .collect();

    commands.entity(container).with_children(|parent| {
        for (key, tag) in rows {
            let key_prev = key.clone();
            let key_next = key.clone();
            let tag_display = if tag.is_empty() { "--".to_string() } else { tag.clone() };
            let tag_color = if tag.is_empty() { Color::srgb(0.5, 0.55, 0.5) } else { Color::srgb(0.9, 0.85, 0.65) };
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                    column_gap: Val::Px(4.0),
                    ..Default::default()
                },
                BackgroundColor(Color::srgba(0.07, 0.10, 0.14, 0.70)),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(key.clone()),
                    TextFont::default().with_font_size(10.0),
                    TextColor(Color::srgb(0.6, 0.75, 0.6)),
                ));
                row.spawn((
                    Node { width: Val::Px(14.0), ..Default::default() },
                    BackgroundColor(Color::srgba(0.15, 0.15, 0.15, 0.6)),
                    bevy::picking::hover::Hovered::default(),
                    bevy::ui_widgets::Button,
                ))
                .with_child((Text::new("<"), TextFont::default().with_font_size(9.0), TextColor(Color::srgb(0.8, 0.8, 0.8))))
                .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.cycle_binding(&key_prev, -1); });

                row.spawn((
                    Text::new(tag_display),
                    TextFont::default().with_font_size(10.0),
                    TextColor(tag_color),
                    Node { flex_grow: 1.0, overflow: Overflow::clip(), ..Default::default() },
                ));

                row.spawn((
                    Node { width: Val::Px(14.0), ..Default::default() },
                    BackgroundColor(Color::srgba(0.15, 0.15, 0.15, 0.6)),
                    bevy::picking::hover::Hovered::default(),
                    bevy::ui_widgets::Button,
                ))
                .with_child((Text::new(">"), TextFont::default().with_font_size(9.0), TextColor(Color::srgb(0.8, 0.8, 0.8))))
                .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.cycle_binding(&key_next, 1); });
            });
        }
    });
}

pub fn rebuild_type_picker(
    mut state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<TypeContainer>>,
    props_q: Query<Entity, With<TypePropsContainer>>,
    theme: Res<lava_ui_builder::LavaTheme>,
) {
    if !state.type_dirty { return; }
    state.type_dirty = false;

    let current_label = state.model_type.label();

    if let Ok(container) = container_q.single() {
        commands.entity(container).despawn_related::<Children>();
        commands.entity(container).with_children(|parent| {
            for &label in ModelType::all_labels() {
                let is_active = label == current_label;
                let bg = if is_active { Color::srgba(0.20, 0.45, 0.25, 0.95) } else { Color::srgba(0.10, 0.16, 0.12, 0.85) };
                let tc = if is_active { Color::srgb(0.8, 1.0, 0.8) } else { Color::srgb(0.55, 0.70, 0.55) };
                parent.spawn((
                    Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..Default::default() },
                    BackgroundColor(bg),
                    InteractionPalette { none: bg, hovered: Color::srgba(0.25, 0.55, 0.30, 0.95), pressed: Color::srgba(0.15, 0.38, 0.20, 1.0) },
                    bevy::picking::hover::Hovered::default(),
                    bevy::ui_widgets::Button,
                ))
                .with_child((Text::new(label), TextFont::default().with_font_size(10.0), TextColor(tc)))
                .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| { s.set_model_type(label); });
            }
        });
    }

    if let Ok(props_container) = props_q.single() {
        commands.entity(props_container).despawn_related::<Children>();
        let t = theme.text.clone();
        let prop_theme = lava_ui_builder::TextTheme { label_size: 10.0, label_color: Color::srgb(0.75, 0.82, 0.75), ..t };
        commands.entity(props_container).with_children(|parent| {
            match &state.model_type {
                ModelType::Enemy(p) => {
                    parent.spawn(prop_row("HP", p.health, &prop_theme));
                    parent.spawn(prop_row("Speed", p.speed, &prop_theme));
                    parent.spawn(prop_row("Coins", p.coin_drop as f32, &prop_theme));
                }
                ModelType::Tower(p) => {
                    parent.spawn(prop_row("HP", p.health, &prop_theme));
                    parent.spawn(prop_row("Cost", p.cost as f32, &prop_theme));
                    parent.spawn(prop_row("Range", p.range, &prop_theme));
                    parent.spawn(prop_row("Damage", p.damage, &prop_theme));
                    parent.spawn(prop_row("Fire/min", p.fire_rate_per_minute, &prop_theme));
                }
                ModelType::Terrain(p) => {
                    let blocks_e = if p.blocks_enemies { "yes" } else { "no" };
                    let blocks_p = if p.blocks_players { "yes" } else { "no" };
                    let hp_str = p.health.map(|h| format!("{h}")).unwrap_or_else(|| "inf".to_string());
                    parent.spawn((
                        Text::new(format!("blocks enemies: {blocks_e}  players: {blocks_p}  HP: {hp_str}")),
                        TextFont::default().with_font_size(10.0),
                        TextColor(Color::srgb(0.75, 0.82, 0.75)),
                        Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..Default::default() },
                    ));
                }
                ModelType::Item(p) => {
                    let kind = match &p.kind {
                        crate::assets::asset_definition::ItemKind::Decorative => "Decorative".to_string(),
                        crate::assets::asset_definition::ItemKind::HealthPickup { amount } => format!("HealthPickup ({amount} HP)"),
                    };
                    parent.spawn((
                        Text::new(format!("kind: {kind}")),
                        TextFont::default().with_font_size(10.0),
                        TextColor(Color::srgb(0.75, 0.82, 0.75)),
                        Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..Default::default() },
                    ));
                }
                ModelType::Player(_) => {}
            }
        });
    }
}

pub fn rebuild_sources_list(
    state: ResMut<AssetBrowserState>,
    mut commands: Commands,
    container_q: Query<Entity, With<SourcesContainer>>,
) {
    if !state.sources_dirty { return; }
    // Don't clear sources_dirty here — load_extra_animation_sources also reads it.
    // It will be cleared there. We just rebuild the UI whenever it's set.

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let sources: Vec<String> = state.animation_sources.clone();

    if sources.is_empty() {
        commands.entity(container).with_children(|parent| {
            parent.spawn((
                Text::new("none"),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgba(0.4, 0.5, 0.4, 0.6)),
            ));
        });
        return;
    }

    commands.entity(container).with_children(|parent| {
        for (idx, path) in sources.iter().enumerate() {
            let stem = std::path::Path::new(path)
                .file_stem().and_then(|s| s.to_str())
                .unwrap_or(path).to_string();
            let bg = Color::srgba(0.10, 0.22, 0.35, 0.90);
            parent.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    column_gap: Val::Px(4.0),
                    ..Default::default()
                },
                BackgroundColor(bg),
            ))
            .with_children(|chip| {
                chip.spawn((
                    Text::new(stem),
                    TextFont::default().with_font_size(10.0),
                    TextColor(Color::srgb(0.75, 0.88, 1.0)),
                ));
                chip.spawn((
                    Node { width: Val::Px(14.0), height: Val::Px(14.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..Default::default() },
                    BackgroundColor(Color::srgba(0.4, 0.1, 0.1, 0.7)),
                    bevy::picking::hover::Hovered::default(),
                    bevy::ui_widgets::Button,
                    InteractionPalette { none: Color::srgba(0.4, 0.1, 0.1, 0.7), hovered: Color::srgba(0.6, 0.15, 0.15, 0.9), pressed: Color::srgba(0.3, 0.08, 0.08, 1.0) },
                ))
                .with_child((Text::new("x"), TextFont::default().with_font_size(9.0), TextColor(Color::srgb(1.0, 0.7, 0.7))))
                .observe(move |_: On<Activate>, mut s: ResMut<AssetBrowserState>| {
                    s.remove_animation_source(idx);
                });
            });
        }
    });
}

fn prop_row(label: &str, value: f32, theme: &lava_ui_builder::TextTheme) -> impl Bundle {
    (
        Text::new(format!("{label}: {value}")),
        TextFont::default().with_font_size(10.0),
        TextColor(theme.label_color),
        Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..Default::default() },
    )
}
