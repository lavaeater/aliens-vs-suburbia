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

        // Animated filter
        left.add_button_observe(
            "Animated only: OFF",
            |b| { b.size_px(240.0, 28.0).font_size(12.0); },
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

    // ── Right area: results (top) + transparent viewer gap (bottom) ──────────
    ui.with_child(|right| {
        right.with_flex_grow(1.0)
            .height_percent(100.0)
            .display_flex()
            .flex_column();

        // Results list — opaque, scrollable, upper portion
        right.with_child(|list| {
            list.height_percent(55.0)
                .width_percent(100.0)
                .display_flex()
                .flex_column()
                .overflow_scroll_y()
                .padding_all_px(8.0)
                .gap_px(4.0)
                .bg_color(Color::srgba(0.03, 0.05, 0.08, 0.88))
                .insert(ResultsContainer);
        });

        // Viewer area — fully transparent so the Camera3d shows through.
        // A small label strip at the top of this region gives context.
        right.with_child(|viewer_strip| {
            viewer_strip.with_flex_grow(1.0)
                .width_percent(100.0)
                .display_flex()
                .flex_column()
                .padding_all_px(6.0);

            viewer_strip.with_child(|hint| {
                hint.insert_bundle(lava_ui_builder::label(
                    "[drag to rotate · T = toon shader]",
                    &lava_ui_builder::TextTheme {
                        label_size: 11.0,
                        label_color: Color::srgba(0.7, 0.8, 0.7, 0.6),
                        ..Default::default()
                    },
                ));
            });
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
                }
                ApiResponse::ListResults(lr) => {
                    state.results = lr.models;
                    state.total = state.results.len() as u32;
                    state.pending = false;
                    state.results_dirty = true;
                    state.status = format!("{} models in list", state.total);
                }
                ApiResponse::UserResults(ur) => {
                    state.results = ur.models;
                    state.total = state.results.len() as u32;
                    state.pending = false;
                    state.results_dirty = true;
                    state.status = format!("{} models by user", state.total);
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
                ApiResponse::Error(e) => {
                    state.pending = false;
                    state.viewer_downloading = false;
                    state.status = format!("Error: {e}");
                }
            },
        }
    }
}

// ── Rebuild the results list when results change ──────────────────────────────

pub fn rebuild_results_ui(
    mut state: ResMut<PolyPizzaState>,
    mut commands: Commands,
    container_query: Query<Entity, With<ResultsContainer>>,
    theme: Res<LavaTheme>,
) {
    if !state.results_dirty { return; }
    state.results_dirty = false;

    let Ok(container) = container_query.single() else { return; };

    // Clear old children
    commands.entity(container).despawn_related::<Children>();

    let results: Vec<_> = state.results.iter().enumerate().map(|(i, m)| {
        (i, m.title.clone(), m.creator.username.clone(), m.tri_count, m.animated.unwrap_or(false))
    }).collect();

    let theme_clone = theme.clone();
    commands.entity(container).with_children(|parent| {
        for (index, title, creator, tri_count, animated) in results {
            let anim_tag = if animated { " ★" } else { "" };
            let label = format!("{title}{anim_tag}\n  by {creator} · {tri_count} tris");

            let card_entity = parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
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
                ResultCard { index },
            )).id();

            // Text child — spawn directly under card_entity, not under the container
            parent.commands().entity(card_entity).with_children(|card| {
                card.spawn((
                    Text::new(label),
                    TextFont::default().with_font_size(12.0),
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
