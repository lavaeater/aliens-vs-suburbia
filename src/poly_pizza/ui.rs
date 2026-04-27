use bevy::prelude::*;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{InteractionPalette, LavaTheme, TextTheme, UIBuilder};
use crate::game_state::GameState;
use crate::poly_pizza::async_bridge::{ApiChannels, ApiRequest, ApiResponse};
use crate::poly_pizza::client::SearchFilters;
use crate::poly_pizza::state::PolyPizzaState;
use crate::poly_pizza::viewer::spawn_viewer_model;
use crate::ui::spawn_ui::StateMarker;

// ── Marker components ─────────────────────────────────────────────────────────

#[derive(Component)]
pub struct SearchLabel;

#[derive(Component)]
pub struct UsernameLabel;

#[derive(Component)]
pub struct StatusLabel;

#[derive(Component)]
pub struct AttributionLabel;

#[derive(Component)]
pub struct ResultsContainer;

#[derive(Component)]
pub struct ViewerPanel;

#[derive(Component)]
pub struct AnimatedFilterButton;

#[derive(Component)]
pub struct SaveButton;

#[derive(Component)]
pub struct ResultCard {
    pub index: usize,
}

// ── Spawn the full screen ─────────────────────────────────────────────────────

pub fn spawn_polypizza_screen(
    commands: Commands,
    theme: Res<LavaTheme>,
    mut state: ResMut<PolyPizzaState>,
) {
    state.reset_for_enter();

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    // Full-screen flex row
    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row();

    // ── Left sidebar ──────────────────────────────────────────────────────────
    ui.with_child(|left| {
        left.width_px(280.0)
            .height_percent(100.0)
            .display_flex()
            .flex_column()
            .gap_px(8.0)
            .padding_all_px(12.0)
            .bg_color(Color::srgba(0.05, 0.08, 0.12, 0.95));

        let text_theme = theme.text.clone();

        // Title
        left.with_child(|t| {
            t.insert_bundle(lava_ui_builder::header("Poly Pizza", &text_theme));
        });

        // Search box display — clicking focuses keyword input
        left.with_child(|row| {
            row.display_flex()
                .flex_row()
                .gap_px(4.0)
                .align_items_center()
                .padding_all_px(6.0)
                .bg_color(Color::srgba(0.10, 0.14, 0.20, 1.0))
                .border_all_px(1.0, Color::srgb(0.3, 0.5, 0.7));

            row.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("> _", &TextTheme {
                    label_size: 14.0,
                    label_color: Color::srgb(0.8, 0.9, 1.0),
                    ..text_theme.clone()
                }))
                .insert(SearchLabel);
            });
        })
        .observe(|_: On<Pointer<Click>>, mut s: ResMut<PolyPizzaState>| {
            s.input_focus = crate::poly_pizza::state::InputFocus::Keyword;
        });

        // Search button
        left.add_button_observe(
            "Search",
            |b| { b.size_px(240.0, 36.0).font_size(14.0); },
            |_: On<Activate>, mut state: ResMut<PolyPizzaState>| {
                state.search_requested = true;
            },
        );

        // Category filters
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("Category", &TextTheme {
                label_size: 12.0,
                label_color: Color::srgb(0.5, 0.7, 0.5),
                ..text_theme.clone()
            }));
        });

        left.add_row(|row| {
            row.gap_px(4.0);
            row.add_button_observe("All", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = None; },
            );
            row.add_button_observe("Animals", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(7); },
            );
        });
        left.add_row(|row| {
            row.gap_px(4.0);
            row.add_button_observe("People", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(9); },
            );
            row.add_button_observe("Vehicles", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(3); },
            );
        });
        left.add_row(|row| {
            row.gap_px(4.0);
            row.add_button_observe("Nature", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(6); },
            );
            row.add_button_observe("Objects", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(5); },
            );
        });
        left.add_row(|row| {
            row.gap_px(4.0);
            row.add_button_observe("Weapons", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(2); },
            );
            row.add_button_observe("Buildings", |b| { b.size_px(110.0, 24.0).font_size(11.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| { s.category_filter = Some(8); },
            );
        });

        // Animated filter — button text updated at runtime by update_animated_filter_button
        left.add_button_observe(
            "Animated only: OFF",
            |b| { b.size_px(240.0, 28.0).font_size(12.0).insert(AnimatedFilterButton); },
            |_: On<Activate>, mut s: ResMut<PolyPizzaState>| {
                s.animated_only = !s.animated_only;
            },
        );

        // Pagination
        left.add_row(|row| {
            row.gap_px(8.0).align_items_center();
            row.add_button_observe("< Prev", |b| { b.size_px(100.0, 28.0).font_size(12.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| {
                    if s.page > 0 { s.page -= 1; s.search_requested = true; }
                },
            );
            row.add_button_observe("Next >", |b| { b.size_px(100.0, 28.0).font_size(12.0); },
                |_: On<Activate>, mut s: ResMut<PolyPizzaState>| {
                    s.page += 1; s.search_requested = true;
                },
            );
        });

        // ── User search ───────────────────────────────────────────────────────
        left.with_child(|sep| {
            sep.insert_bundle(lava_ui_builder::label("── By creator ──", &TextTheme {
                label_size: 11.0,
                label_color: Color::srgb(0.4, 0.6, 0.4),
                ..text_theme.clone()
            }));
        });

        // Username input display — clicking switches focus to it
        left.with_child(|row| {
            row.display_flex()
                .flex_row()
                .gap_px(4.0)
                .align_items_center()
                .padding_all_px(6.0)
                .bg_color(Color::srgba(0.10, 0.14, 0.20, 1.0))
                .border_all_px(1.0, Color::srgb(0.3, 0.5, 0.5));

            row.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("user: _", &TextTheme {
                    label_size: 14.0,
                    label_color: Color::srgb(0.7, 0.9, 0.8),
                    ..text_theme.clone()
                }))
                .insert(UsernameLabel);
            });
        })
        .observe(|_: On<Pointer<Click>>, mut s: ResMut<PolyPizzaState>| {
            s.input_focus = crate::poly_pizza::state::InputFocus::Username;
        });

        left.add_button_observe(
            "Search by user",
            |b| { b.size_px(240.0, 32.0).font_size(13.0); },
            |_: On<Activate>, mut s: ResMut<PolyPizzaState>| {
                s.user_search_requested = true;
            },
        );

        // Status label
        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 12.0,
                label_color: Color::srgb(0.6, 0.7, 0.6),
                ..text_theme.clone()
            }))
            .insert(StatusLabel);
        });

        // Toon toggle hint
        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label("[T] toggle toon shader", &TextTheme {
                label_size: 11.0,
                label_color: Color::srgb(0.4, 0.5, 0.4),
                ..text_theme.clone()
            }));
        });

        // Attribution
        left.with_child(|lbl| {
            lbl.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 10.0,
                label_color: Color::srgb(0.5, 0.6, 0.5),
                ..text_theme.clone()
            }))
            .insert(AttributionLabel)
            .modify_node(|mut n| {
                n.overflow = Overflow::clip();
            });
        });

        // Back button at bottom
        left.with_child(|spacer| {
            spacer.with_flex_grow(1.0);
        });
        left.add_button_observe(
            "← Back to Menu",
            |b| { b.size_px(240.0, 40.0).font_size(14.0); },
            |_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::Menu);
            },
        );
    });

    // ── Results column ────────────────────────────────────────────────────────
    ui.with_child(|list| {
        list.width_px(320.0)
            .height_percent(100.0)
            .display_flex()
            .flex_column()
            .overflow_scroll_y()
            .padding_all_px(8.0)
            .gap_px(4.0)
            .bg_color(Color::srgba(0.03, 0.05, 0.08, 0.88))
            .insert(ResultsContainer);
    });

    // ── Viewer column — transparent, Camera3d viewport locked to this rect ────
    ui.with_child(|viewer| {
        viewer.with_flex_grow(1.0)
            .height_percent(100.0)
            .display_flex()
            .flex_column()
            .padding_all_px(6.0)
            .insert(ViewerPanel);

        // Spacer pushes bottom bar down
        viewer.with_child(|spacer| { spacer.with_flex_grow(1.0); });

        // Bottom bar: hint text + save button
        viewer.with_child(|row| {
            row.display_flex().flex_row().gap_px(8.0).align_items_center();

            row.with_child(|hint| {
                hint.insert_bundle(lava_ui_builder::label(
                    "[drag to rotate · T = toon shader]",
                    &lava_ui_builder::TextTheme {
                        label_size: 11.0,
                        label_color: Color::srgba(0.7, 0.8, 0.7, 0.5),
                        ..Default::default()
                    },
                ));
            });

            row.with_child(|sp| { sp.with_flex_grow(1.0); });

            row.add_button_observe(
                "☆ Save",
                |b| { b.size_px(80.0, 28.0).font_size(13.0).insert(SaveButton); },
                |_: On<Activate>,
                 mut state: ResMut<PolyPizzaState>,
                 mut library: ResMut<crate::poly_pizza::library::ModelLibrary>| {
                    if let Some(model) = state.selected_model.clone() {
                        let local_glb = if state.glb_cache_path(&model.id).exists() {
                            Some(state.glb_asset_path(&model.id))
                        } else {
                            None
                        };
                        library.toggle(&model, local_glb);
                        library.save();
                        state.set_changed();
                    }
                },
            );
        });
    });

    ui.build();
}

// ── Key input → search term ───────────────────────────────────────────────────

pub fn handle_key_input(
    mut keyboard_reader: MessageReader<KeyboardInput>,
    mut state: ResMut<PolyPizzaState>,
) {
    use crate::poly_pizza::state::InputFocus;
    for event in keyboard_reader.read() {
        if event.state != ButtonState::Pressed { continue; }
        match &event.logical_key {
            Key::Tab => {
                state.input_focus = match state.input_focus {
                    InputFocus::Keyword => InputFocus::Username,
                    InputFocus::Username => InputFocus::Keyword,
                };
            }
            Key::Character(s) => {
                match state.input_focus {
                    InputFocus::Keyword => state.search_term.push_str(s.as_str()),
                    InputFocus::Username => state.username_term.push_str(s.as_str()),
                }
            }
            Key::Space => {
                match state.input_focus {
                    InputFocus::Keyword => state.search_term.push(' '),
                    InputFocus::Username => state.username_term.push(' '),
                }
            }
            Key::Backspace => {
                match state.input_focus {
                    InputFocus::Keyword => { state.search_term.pop(); }
                    InputFocus::Username => { state.username_term.pop(); }
                }
            }
            Key::Enter => {
                match state.input_focus {
                    InputFocus::Keyword => state.search_requested = true,
                    InputFocus::Username => state.user_search_requested = true,
                }
            }
            _ => {}
        }
    }
}

// ── Submit search when requested ──────────────────────────────────────────────

pub fn handle_search_submit(
    mut state: ResMut<PolyPizzaState>,
    channels: Res<ApiChannels>,
) {
    if !state.search_requested || state.pending { return; }
    state.search_requested = false;
    state.pending = true;
    state.status = "Searching…".to_string();

    let filters = SearchFilters {
        category: state.category_filter,
        license: if state.cc0_only { Some(1) } else { None },
        animated_only: state.animated_only,
        page: state.page,
    };

    let request = if state.search_term.trim().is_empty() {
        ApiRequest::SearchFilters { filters }
    } else {
        ApiRequest::SearchKeyword { keyword: state.search_term.trim().to_string(), filters }
    };

    channels.tx.send(request).ok();
}

// ── Drain API responses ───────────────────────────────────────────────────────

pub fn handle_api_responses(
    mut state: ResMut<PolyPizzaState>,
    channels: Res<ApiChannels>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let rx = channels.rx.lock().unwrap();
    loop {
        match rx.try_recv() {
            Err(_) => break,
            Ok(response) => match response {
                ApiResponse::SearchResults(sr) => {
                    state.total = sr.total;
                    state.results = sr.results;
                    state.pending = false;
                    state.results_dirty = true;
                    state.status = format!("{} results (page {})", state.total, state.page + 1);
                    queue_thumbnail_downloads(&mut state, &channels);
                }
                ApiResponse::ListResults(lr) => {
                    state.results = lr.models;
                    state.total = state.results.len() as u32;
                    state.pending = false;
                    state.results_dirty = true;
                    state.status = format!("{} models in list", state.total);
                    queue_thumbnail_downloads(&mut state, &channels);
                }
                ApiResponse::UserResults(ur) => {
                    state.results = ur.models;
                    state.total = state.results.len() as u32;
                    state.pending = false;
                    state.results_dirty = true;
                    state.status = format!("{} models by user", state.total);
                    queue_thumbnail_downloads(&mut state, &channels);
                }
                ApiResponse::DownloadComplete { id } => {
                    state.viewer_downloading = false;
                    if state.selected_model.as_ref().map(|m| m.id.as_str()) == Some(&id) {
                        let handle = asset_server.load(state.glb_asset_path(&id));
                        let entity = spawn_viewer_model(&mut commands, handle, state.toon_shader);
                        state.viewer_entity = Some(entity);
                        state.status = "Model loaded".to_string();
                    }
                }
                ApiResponse::ThumbnailComplete { id } => {
                    state.downloading_thumbnails.remove(&id);
                    state.results_dirty = true;
                }
                ApiResponse::Error(e) => {
                    state.pending = false;
                    state.viewer_downloading = false;
                    state.status = format!("Error: {e}");
                }
            },
        }
    }
}

fn queue_thumbnail_downloads(state: &mut PolyPizzaState, channels: &ApiChannels) {
    let to_fetch: Vec<_> = state.results.iter()
        .filter(|m| {
            !state.downloading_thumbnails.contains(&m.id)
                && !state.thumb_cache_path(&m.id).exists()
        })
        .map(|m| (m.id.clone(), m.thumbnail_url.clone()))
        .collect();

    for (id, url) in to_fetch {
        let dest = state.thumb_cache_path(&id);
        state.downloading_thumbnails.insert(id.clone());
        channels.tx.send(ApiRequest::DownloadThumbnail { id, url, dest }).ok();
    }
}

// ── Rebuild the results list when results change ──────────────────────────────

pub fn rebuild_results_ui(
    mut state: ResMut<PolyPizzaState>,
    mut commands: Commands,
    container_query: Query<Entity, With<ResultsContainer>>,
    library: Res<crate::poly_pizza::library::ModelLibrary>,
    asset_server: Res<AssetServer>,
) {
    if !state.results_dirty { return; }
    state.results_dirty = false;

    let Ok(container) = container_query.single() else { return; };
    commands.entity(container).despawn_related::<Children>();

    struct CardData {
        index: usize,
        title: String,
        creator: String,
        tri_count: u32,
        animated: bool,
        model_id: String,
        thumb_asset: Option<String>,
        saved: bool,
    }

    let cards: Vec<CardData> = state.results.iter().enumerate().map(|(i, m)| {
        let thumb_path = state.thumb_cache_path(&m.id);
        CardData {
            index: i,
            title: m.title.clone(),
            creator: m.creator.username.clone(),
            tri_count: m.tri_count,
            animated: m.animated.unwrap_or(false),
            saved: library.is_saved(&m.id),
            model_id: m.id.clone(),
            thumb_asset: if thumb_path.exists() {
                Some(state.thumb_asset_path(&m.id))
            } else {
                None
            },
        }
    }).collect();

    commands.entity(container).with_children(|parent| {
        for card in cards {
            let anim_tag = if card.animated { " ★" } else { "" };
            let saved_tag = if card.saved { "♥ " } else { "" };
            let detail = format!("{saved_tag}{}{anim_tag}\n  {} · {}t",
                card.title, card.creator, card.tri_count);

            let card_entity = parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..Default::default()
                },
                BorderColor::all(Color::srgba(0.3, 0.5, 0.7, 0.4)),
                BackgroundColor(Color::srgba(0.08, 0.12, 0.20, 0.9)),
                lava_ui_builder::InteractionPalette {
                    none: Color::srgba(0.08, 0.12, 0.20, 0.9),
                    hovered: Color::srgba(0.15, 0.22, 0.35, 0.95),
                    pressed: Color::srgba(0.05, 0.08, 0.15, 1.0),
                },
                bevy::picking::hover::Hovered::default(),
                bevy::ui_widgets::Button,
                ResultCard { index: card.index },
            )).id();

            parent.commands().entity(card_entity).with_children(|row| {
                // Thumbnail or placeholder
                if let Some(thumb_path) = card.thumb_asset {
                    let handle = asset_server.load(thumb_path);
                    row.spawn((
                        Node {
                            width: Val::Px(56.0),
                            height: Val::Px(56.0),
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            overflow: Overflow::clip(),
                            ..Default::default()
                        },
                        ImageNode::new(handle),
                    ));
                } else {
                    row.spawn((
                        Node {
                            width: Val::Px(56.0),
                            height: Val::Px(56.0),
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..Default::default()
                        },
                        BackgroundColor(Color::srgba(0.12, 0.15, 0.22, 0.8)),
                    ));
                }

                row.spawn((
                    Text::new(detail),
                    TextFont::default().with_font_size(11.0),
                    TextColor(Color::srgb(0.85, 0.90, 0.95)),
                ));
            });

            parent.commands().entity(card_entity).observe(result_card_clicked);
        }
    });
}

fn result_card_clicked(
    trigger: On<Activate>,
    cards: Query<&ResultCard>,
    mut state: ResMut<PolyPizzaState>,
    channels: Res<ApiChannels>,
) {
    let Ok(card) = cards.get(trigger.event().entity) else { return; };
    let index = card.index;
    if index >= state.results.len() { return; }

    let model = state.results[index].clone();
    let id = model.id.clone();
    let download_url = model.download_url.clone();
    state.selected_model = Some(model);
    state.viewer_needs_load = true;

    // If not cached, kick off the download now
    let dest = state.glb_cache_path(&id);
    if !dest.exists() {
        state.viewer_downloading = true;
        state.status = "Downloading model…".to_string();
        channels.tx.send(ApiRequest::DownloadGlb { id, url: download_url, dest }).ok();
    }
}

// ── User search submit ────────────────────────────────────────────────────────

pub fn handle_user_search_submit(
    mut state: ResMut<PolyPizzaState>,
    channels: Res<ApiChannels>,
) {
    if !state.user_search_requested || state.pending { return; }
    let username = state.username_term.trim().to_string();
    if username.is_empty() { state.user_search_requested = false; return; }
    state.user_search_requested = false;
    state.pending = true;
    state.status = format!("Loading models by {username}…");
    channels.tx.send(ApiRequest::GetUser(username)).ok();
}

// ── Update labels ──────────────────────────────────────────────────────────────

pub fn update_search_label(
    state: Res<PolyPizzaState>,
    mut labels: Query<&mut Text, With<SearchLabel>>,
) {
    use crate::poly_pizza::state::InputFocus;
    if !state.is_changed() { return; }
    let cursor = if state.input_focus == InputFocus::Keyword { "█" } else { "_" };
    for mut text in labels.iter_mut() {
        **text = format!("> {}{}", state.search_term, cursor);
    }
}

pub fn update_username_label(
    state: Res<PolyPizzaState>,
    mut labels: Query<&mut Text, With<UsernameLabel>>,
) {
    use crate::poly_pizza::state::InputFocus;
    if !state.is_changed() { return; }
    let cursor = if state.input_focus == InputFocus::Username { "█" } else { "_" };
    for mut text in labels.iter_mut() {
        **text = format!("user: {}{}", state.username_term, cursor);
    }
}

pub fn update_status_label(
    state: Res<PolyPizzaState>,
    mut labels: Query<&mut Text, With<StatusLabel>>,
) {
    if !state.is_changed() { return; }
    for mut text in labels.iter_mut() {
        **text = state.status.clone();
    }
}

pub fn sync_viewer_viewport(
    panels: Query<(&bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform), With<ViewerPanel>>,
    mut cameras: Query<&mut Camera, With<crate::poly_pizza::viewer::ViewerCamera>>,
    windows: Query<&Window>,
) {
    let Ok((node, transform)) = panels.single() else { return };
    let Ok(mut camera) = cameras.single_mut() else { return };
    let Ok(window) = windows.single() else { return };

    // UiGlobalTransform.translation is the physical-pixel CENTER of the node.
    // ComputedNode.size is in physical pixels.
    let phys_size = node.size();
    let center = transform.affine().translation;
    let top_left = center - phys_size * 0.5;

    let win_w = window.physical_width();
    let win_h = window.physical_height();
    let x = top_left.x.max(0.0) as u32;
    let y = top_left.y.max(0.0) as u32;
    let w = (phys_size.x as u32).min(win_w.saturating_sub(x));
    let h = (phys_size.y as u32).min(win_h.saturating_sub(y));

    if w == 0 || h == 0 {
        return;
    }

    camera.viewport = Some(bevy::camera::Viewport {
        physical_position: bevy::math::UVec2::new(x, y),
        physical_size: bevy::math::UVec2::new(w, h),
        depth: 0.0..1.0,
    });
}

pub fn update_animated_filter_button(
    state: Res<PolyPizzaState>,
    buttons: Query<&Children, With<AnimatedFilterButton>>,
    mut texts: Query<&mut Text>,
) {
    if !state.is_changed() { return; }
    let label = if state.animated_only { "Animated only: ON" } else { "Animated only: OFF" };
    for children in buttons.iter() {
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.to_string();
            }
        }
    }
}

pub fn update_save_button_label(
    state: Res<PolyPizzaState>,
    library: Res<crate::poly_pizza::library::ModelLibrary>,
    buttons: Query<&Children, With<SaveButton>>,
    mut texts: Query<&mut Text>,
) {
    if !state.is_changed() && !library.is_changed() { return; }
    let is_saved = state.selected_model.as_ref()
        .map(|m| library.is_saved(&m.id))
        .unwrap_or(false);
    let label = if is_saved { "★ Saved" } else { "☆ Save" };
    for children in buttons.iter() {
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.to_string();
            }
        }
    }
}

pub fn update_attribution_label(
    state: Res<PolyPizzaState>,
    mut labels: Query<&mut Text, With<AttributionLabel>>,
) {
    if !state.is_changed() { return; }
    let attribution = state.selected_model.as_ref()
        .map(|m| m.attribution.clone())
        .unwrap_or_default();
    for mut text in labels.iter_mut() {
        **text = attribution.clone();
    }
}
