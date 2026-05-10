use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, UIBuilder};
use crate::character_creator::config::{CharacterConfig, ComposedSpriteSheet};
use crate::character_creator::lpc_data::{BODY_TYPES, CATEGORIES, CategoryGroup};
use crate::game_state::GameState;
use crate::ui::spawn_ui::StateMarker;

// ── Selection state ───────────────────────────────────────────────────────────

/// One selection slot per entry in `CATEGORIES`.
#[derive(Resource)]
pub struct CreatorSelections {
    pub body_idx: usize,
    /// `selections[i]` = index into `CATEGORIES[i].options`, or None for "none".
    pub selections: Vec<Option<usize>>,
}

impl Default for CreatorSelections {
    fn default() -> Self {
        Self {
            body_idx: 0,
            selections: vec![None; CATEGORIES.len()],
        }
    }
}

impl CreatorSelections {
    pub fn body_type(&self) -> &'static str {
        BODY_TYPES[self.body_idx].0
    }

    pub fn to_config(&self) -> CharacterConfig {
        let body = self.body_type();
        let mut layers = Vec::new();
        for (i, cat) in CATEGORIES.iter().enumerate() {
            if let Some(opt_idx) = self.selections[i] {
                if let Some(path) = cat.options[opt_idx].resolve(body) {
                    layers.push(path);
                }
            }
        }
        CharacterConfig {
            body_type: body.to_string(),
            extra_layers: layers,
        }
    }

    fn cycle(&mut self, cat_idx: usize, delta: i32) {
        let n = CATEGORIES[cat_idx].options.len() as i32 + 1; // +1 for None
        let cur = self.selections[cat_idx].map(|i| i as i32 + 1).unwrap_or(0);
        let next = (cur + delta).rem_euclid(n);
        self.selections[cat_idx] = if next == 0 { None } else { Some((next - 1) as usize) };
    }

    fn current_label(&self, cat_idx: usize) -> String {
        match self.selections[cat_idx] {
            None => "None".into(),
            Some(i) => {
                let n = CATEGORIES[cat_idx].options.len();
                format!("{} ({}/{})", CATEGORIES[cat_idx].options[i].label, i + 1, n)
            }
        }
    }
}

// ── Marker components ─────────────────────────────────────────────────────────

#[derive(Component)]
pub struct PreviewImageNode;

/// Marker to identify a label for a specific CATEGORIES index.
#[derive(Component)]
pub struct CategoryLabel(pub usize);

// ── Spawn ─────────────────────────────────────────────────────────────────────

pub fn spawn_character_creator_ui(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    let text = ui.theme().text.clone();

    // Root
    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_column()
        .align_items_center()
        .gap_px(12.0)
        .bg_color(Color::srgba(0.08, 0.10, 0.14, 0.97))
        .padding_all_px(16.0);

    // Title
    ui.with_child(|h| {
        h.insert_bundle(lava_ui_builder::header("Character Creator", &text));
    });

    // Body: left preview + right scrollable controls
    ui.add_row(|row| {
        row.gap_px(20.0).align_items_start().with_flex_grow(1.0);

        // Left: portrait preview
        row.add_panel(|panel| {
            panel.flex_column().align_items_center().gap_px(8.0).padding_all_px(8.0).width_px(210.0);
            panel.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("Preview", &text));
            });
            // Body type buttons
            panel.add_row(|r| {
                r.gap_px(4.0).align_items_center();
                for (i, (_, label)) in BODY_TYPES.iter().enumerate() {
                    r.add_button_observe(
                        *label,
                        |btn| { btn.size_px(62.0, 30.0).font_size(12.0); },
                        move |_: On<Activate>,
                              mut sel: ResMut<CreatorSelections>,
                              mut config: ResMut<CharacterConfig>| {
                            sel.body_idx = i;
                            *config = sel.to_config();
                        },
                    );
                }
            });
            // Portrait image (192×192, inserted by sync_preview_image)
            panel.with_child(|img| {
                img.size_px(192.0, 192.0).insert(PreviewImageNode);
            });
        });

        // Right: groups in a scrollable column
        row.with_child(|scroll_area| {
            scroll_area
                .display_flex()
                .flex_column()
                .gap_px(6.0)
                .with_flex_grow(1.0)
                .height_px(540.0)
                .overflow_scroll_y()
                .padding_all_px(4.0);

            spawn_group(scroll_area, "Face", CategoryGroup::Face, &text);
            spawn_group(scroll_area, "Hair & Headgear", CategoryGroup::HairHead, &text);
            spawn_group(scroll_area, "Clothes", CategoryGroup::Clothes, &text);
            spawn_group(scroll_area, "Accessories", CategoryGroup::Accessories, &text);
        });
    });

    // Bottom buttons
    ui.add_row(|r| {
        r.gap_px(16.0);
        r.add_button_observe(
            "Play as this Character",
            |btn| { btn.size_px(230.0, 44.0).font_size(16.0); },
            |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
                next_state.set(GameState::InGame);
            },
        );
        r.add_button_observe(
            "Back to Menu",
            |btn| { btn.size_px(160.0, 44.0).font_size(16.0); },
            |_: On<Activate>,
             mut next_state: ResMut<NextState<GameState>>,
             mut sel: ResMut<CreatorSelections>,
             mut config: ResMut<CharacterConfig>| {
                *sel = CreatorSelections::default();
                *config = CharacterConfig::default();
                next_state.set(GameState::Menu);
            },
        );
    });

    ui.build();
}

/// Spawn one collapsible group panel containing all category rows for a group.
fn spawn_group(
    parent: &mut UIBuilder,
    title: &str,
    group: CategoryGroup,
    text_theme: &lava_ui_builder::TextTheme,
) {
    parent.with_collapsible(title, false, |inner| {
        inner.gap_px(4.0).padding_all_px(6.0);
        for (cat_idx, cat) in CATEGORIES.iter().enumerate() {
            if cat.group != group { continue; }
            // One row per category: label, ◀, [name label], ▶
            inner.add_row(|row| {
                row.gap_px(6.0).align_items_center();

                // Category name
                row.with_child(|lbl| {
                    lbl.width_px(90.0)
                        .insert(Text::new(cat.label))
                        .insert(TextFont::default().with_font_size(11.0))
                        .insert(TextColor(Color::srgb(0.7, 0.8, 0.7)));
                });

                row.add_button_observe(
                    "◀",
                    |btn| { btn.size_px(28.0, 28.0).font_size(14.0); },
                    move |_: On<Activate>,
                          mut sel: ResMut<CreatorSelections>,
                          mut config: ResMut<CharacterConfig>| {
                        sel.cycle(cat_idx, -1);
                        *config = sel.to_config();
                    },
                );

                row.with_child(|lbl| {
                    lbl.width_px(180.0)
                        .insert(Text::new("None"))
                        .insert(TextFont::default().with_font_size(11.0))
                        .insert(TextColor(Color::WHITE))
                        .insert(TextLayout::new_with_justify(bevy::text::Justify::Center))
                        .insert(CategoryLabel(cat_idx));
                });

                row.add_button_observe(
                    "▶",
                    |btn| { btn.size_px(28.0, 28.0).font_size(14.0); },
                    move |_: On<Activate>,
                          mut sel: ResMut<CreatorSelections>,
                          mut config: ResMut<CharacterConfig>| {
                        sel.cycle(cat_idx, 1);
                        *config = sel.to_config();
                    },
                );
            });
        }
    });
}

// ── Sync systems ──────────────────────────────────────────────────────────────

/// Update every category label text when selections change.
pub fn sync_labels(
    sel: Res<CreatorSelections>,
    mut label_q: Query<(&mut Text, &CategoryLabel)>,
) {
    if !sel.is_changed() { return; }
    for (mut text, label) in &mut label_q {
        **text = sel.current_label(label.0);
    }
}

/// Insert/update the ImageNode on the preview entity when the portrait is ready.
pub fn sync_preview_image(
    sheet: Res<ComposedSpriteSheet>,
    mut preview_q: Query<Entity, With<PreviewImageNode>>,
    mut commands: Commands,
) {
    if !sheet.is_changed() { return; }
    let Some(handle) = &sheet.portrait_handle else { return };
    for entity in &mut preview_q {
        commands.entity(entity).insert(ImageNode::new(handle.clone()));
    }
}
