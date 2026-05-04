use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, UIBuilder};
use crate::character_creator::config::{CharacterConfig, ComposedSpriteSheet};
use crate::character_creator::lpc_data::{BODY_TYPES, HAIR_OPTIONS, LEGS_OPTIONS, TORSO_OPTIONS};
use crate::game_state::GameState;
use crate::ui::spawn_ui::StateMarker;

// ── Selector state ────────────────────────────────────────────────────────────

/// Tracks UI selections in the creator screen.
#[derive(Resource, Default)]
pub struct CreatorSelections {
    pub body_idx: usize,     // index into BODY_TYPES
    pub hair_idx: Option<usize>,
    pub torso_idx: Option<usize>,
    pub legs_idx: Option<usize>,
}

impl CreatorSelections {
    fn body_type(&self) -> &'static str {
        BODY_TYPES[self.body_idx].0
    }

    pub fn to_config(&self) -> CharacterConfig {
        let body = self.body_type();
        let mut layers = vec![];

        if let Some(i) = self.hair_idx {
            if let Some(path) = HAIR_OPTIONS[i].resolve(body) {
                layers.push(path);
            }
        }
        if let Some(i) = self.torso_idx {
            if let Some(path) = TORSO_OPTIONS[i].resolve(body) {
                layers.push(path);
            }
        }
        if let Some(i) = self.legs_idx {
            if let Some(path) = LEGS_OPTIONS[i].resolve(body) {
                layers.push(path);
            }
        }

        CharacterConfig {
            body_type: body.to_string(),
            extra_layers: layers,
        }
    }
}

// ── Marker components for dynamic UI nodes ────────────────────────────────────

#[derive(Component)]
pub struct PreviewImageNode;

#[derive(Component)]
pub struct HairLabel;

#[derive(Component)]
pub struct TorsoLabel;

#[derive(Component)]
pub struct LegsLabel;

#[derive(Component)]
pub struct BodyTypeRow;

// ── Button actions (observer-friendly closures) ───────────────────────────────

fn cycle_hair(delta: i32, sel: &mut CreatorSelections) {
    let n = HAIR_OPTIONS.len() as i32 + 1; // +1 for "None"
    let cur = sel.hair_idx.map(|i| i as i32 + 1).unwrap_or(0);
    let next = (cur + delta).rem_euclid(n);
    sel.hair_idx = if next == 0 { None } else { Some((next - 1) as usize) };
}

fn cycle_torso(delta: i32, sel: &mut CreatorSelections) {
    let n = TORSO_OPTIONS.len() as i32 + 1;
    let cur = sel.torso_idx.map(|i| i as i32 + 1).unwrap_or(0);
    let next = (cur + delta).rem_euclid(n);
    sel.torso_idx = if next == 0 { None } else { Some((next - 1) as usize) };
}

fn cycle_legs(delta: i32, sel: &mut CreatorSelections) {
    let n = LEGS_OPTIONS.len() as i32 + 1;
    let cur = sel.legs_idx.map(|i| i as i32 + 1).unwrap_or(0);
    let next = (cur + delta).rem_euclid(n);
    sel.legs_idx = if next == 0 { None } else { Some((next - 1) as usize) };
}

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
        .justify_center()
        .gap_px(20.0)
        .bg_color(Color::srgba(0.08, 0.10, 0.14, 0.95));

    // Title
    ui.with_child(|h| {
        h.insert_bundle(lava_ui_builder::header("Character Creator", &text));
    });

    // Main row: preview | controls
    ui.add_row(|row| {
        row.gap_px(24.0).align_items_start();

        // Left: preview panel
        row.add_panel(|panel| {
            panel
                .flex_column()
                .align_items_center()
                .gap_px(8.0)
                .padding_all_px(12.0);

            panel.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("Preview", &text));
            });

            // Portrait image placeholder (updated by sync_preview_image system)
            panel.with_child(|img| {
                img.size_px(192.0, 192.0)
                    .insert(PreviewImageNode);
                // ImageNode is inserted by sync_preview_image once the handle is ready.
            });
        });

        // Right: option controls
        row.add_panel(|panel| {
            panel
                .flex_column()
                .gap_px(14.0)
                .padding_all_px(12.0)
                .width_px(280.0);

            // Body type selector row
            panel.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("Body Type", &text));
            });
            panel.add_row(|r| {
                r.insert(BodyTypeRow).gap_px(6.0);
                for (i, (_, label)) in BODY_TYPES.iter().enumerate() {
                    r.add_button_observe(
                        *label,
                        |btn| { btn.size_px(84.0, 36.0).font_size(14.0); },
                        move |_: On<Activate>,
                              mut sel: ResMut<CreatorSelections>,
                              mut config: ResMut<CharacterConfig>| {
                            sel.body_idx = i;
                            *config = sel.to_config();
                        },
                    );
                }
            });

            // Hair
            panel.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("Hair", &text));
            });
            panel.add_row(|r| {
                r.gap_px(6.0).align_items_center();
                r.add_button_observe(
                    "◀",
                    |btn| { btn.size_px(36.0, 36.0).font_size(16.0); },
                    |_: On<Activate>,
                     mut sel: ResMut<CreatorSelections>,
                     mut config: ResMut<CharacterConfig>| {
                        cycle_hair(-1, &mut sel);
                        *config = sel.to_config();
                    },
                );
                r.with_child(|lbl| {
                    lbl.size_px(140.0, 28.0)
                        .insert(HairLabel)
                        .insert(Text::new("None"))
                        .insert(TextFont::default().with_font_size(13.0))
                        .insert(TextColor(Color::WHITE))
                        .insert(TextLayout::new_with_justify(bevy::text::Justify::Center));
                });
                r.add_button_observe(
                    "▶",
                    |btn| { btn.size_px(36.0, 36.0).font_size(16.0); },
                    |_: On<Activate>,
                     mut sel: ResMut<CreatorSelections>,
                     mut config: ResMut<CharacterConfig>| {
                        cycle_hair(1, &mut sel);
                        *config = sel.to_config();
                    },
                );
            });

            // Torso
            panel.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("Torso", &text));
            });
            panel.add_row(|r| {
                r.gap_px(6.0).align_items_center();
                r.add_button_observe(
                    "◀",
                    |btn| { btn.size_px(36.0, 36.0).font_size(16.0); },
                    |_: On<Activate>,
                     mut sel: ResMut<CreatorSelections>,
                     mut config: ResMut<CharacterConfig>| {
                        cycle_torso(-1, &mut sel);
                        *config = sel.to_config();
                    },
                );
                r.with_child(|lbl| {
                    lbl.size_px(140.0, 28.0)
                        .insert(TorsoLabel)
                        .insert(Text::new("None"))
                        .insert(TextFont::default().with_font_size(13.0))
                        .insert(TextColor(Color::WHITE))
                        .insert(TextLayout::new_with_justify(bevy::text::Justify::Center));
                });
                r.add_button_observe(
                    "▶",
                    |btn| { btn.size_px(36.0, 36.0).font_size(16.0); },
                    |_: On<Activate>,
                     mut sel: ResMut<CreatorSelections>,
                     mut config: ResMut<CharacterConfig>| {
                        cycle_torso(1, &mut sel);
                        *config = sel.to_config();
                    },
                );
            });

            // Legs
            panel.with_child(|lbl| {
                lbl.insert_bundle(lava_ui_builder::label("Legs", &text));
            });
            panel.add_row(|r| {
                r.gap_px(6.0).align_items_center();
                r.add_button_observe(
                    "◀",
                    |btn| { btn.size_px(36.0, 36.0).font_size(16.0); },
                    |_: On<Activate>,
                     mut sel: ResMut<CreatorSelections>,
                     mut config: ResMut<CharacterConfig>| {
                        cycle_legs(-1, &mut sel);
                        *config = sel.to_config();
                    },
                );
                r.with_child(|lbl| {
                    lbl.size_px(140.0, 28.0)
                        .insert(LegsLabel)
                        .insert(Text::new("None"))
                        .insert(TextFont::default().with_font_size(13.0))
                        .insert(TextColor(Color::WHITE))
                        .insert(TextLayout::new_with_justify(bevy::text::Justify::Center));
                });
                r.add_button_observe(
                    "▶",
                    |btn| { btn.size_px(36.0, 36.0).font_size(16.0); },
                    |_: On<Activate>,
                     mut sel: ResMut<CreatorSelections>,
                     mut config: ResMut<CharacterConfig>| {
                        cycle_legs(1, &mut sel);
                        *config = sel.to_config();
                    },
                );
            });
        });
    });

    // Bottom buttons
    ui.add_row(|r| {
        r.gap_px(16.0);
        r.add_button_observe(
            "Play as this Character",
            |btn| { btn.size_px(230.0, 52.0).font_size(18.0); },
            |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
                next_state.set(GameState::InGame);
            },
        );
        r.add_button_observe(
            "Back to Menu",
            |btn| { btn.size_px(160.0, 52.0).font_size(18.0); },
            |_: On<Activate>,
             mut next_state: ResMut<NextState<GameState>>,
             mut sel: ResMut<CreatorSelections>,
             mut config: ResMut<CharacterConfig>| {
                // Reset to defaults
                *sel = CreatorSelections::default();
                *config = CharacterConfig::default();
                next_state.set(GameState::Menu);
            },
        );
    });

    ui.build();
}

// ── Sync systems ──────────────────────────────────────────────────────────────

/// Update label text when selections change.
pub fn sync_labels(
    sel: Res<CreatorSelections>,
    mut hair_q: Query<&mut Text, (With<HairLabel>, Without<TorsoLabel>, Without<LegsLabel>)>,
    mut torso_q: Query<&mut Text, (With<TorsoLabel>, Without<HairLabel>, Without<LegsLabel>)>,
    mut legs_q: Query<&mut Text, (With<LegsLabel>, Without<HairLabel>, Without<TorsoLabel>)>,
) {
    if !sel.is_changed() { return; }

    let hair_name = sel.hair_idx
        .map(|i| HAIR_OPTIONS[i].label)
        .unwrap_or("None");
    let torso_name = sel.torso_idx
        .map(|i| TORSO_OPTIONS[i].label)
        .unwrap_or("None");
    let legs_name = sel.legs_idx
        .map(|i| LEGS_OPTIONS[i].label)
        .unwrap_or("None");

    for mut t in &mut hair_q { **t = hair_name.into(); }
    for mut t in &mut torso_q { **t = torso_name.into(); }
    for mut t in &mut legs_q { **t = legs_name.into(); }
}

/// Once the portrait handle is ready, insert/update the ImageNode on the preview entity.
pub fn sync_preview_image(
    sheet: Res<ComposedSpriteSheet>,
    mut preview_q: Query<(Entity, Option<&ImageNode>), With<PreviewImageNode>>,
    mut commands: Commands,
) {
    if !sheet.is_changed() { return; }
    let Some(handle) = &sheet.portrait_handle else { return };

    for (entity, existing) in &mut preview_q {
        if existing.is_none() {
            commands.entity(entity).insert(ImageNode::new(handle.clone()));
        } else {
            commands.entity(entity).insert(ImageNode::new(handle.clone()));
        }
    }
}
