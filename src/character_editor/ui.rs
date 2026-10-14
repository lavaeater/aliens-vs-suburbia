//! The character editor's two-pane shell: a tweaking panel on the left, the live game on the
//! right.
//!
//! The right pane is not a separate render — it is the ordinary game camera with its
//! `viewport` clipped to the pane's rectangle, the same trick the asset browser uses in
//! `asset_browser::viewer::sync_viewer_viewport`. That keeps every gameplay system,
//! including the camera follow, working exactly as it does full-screen.

use bevy::gizmos::config::GizmoConfigStore;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{LavaTheme, TextTheme, UIBuilder};

use crate::camera::components::GameCamera;
use crate::game_state::GameState;
use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::ecs::system::EntityCommands;
use lava_ui_builder::InteractionPalette;

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::player::components::Player;
use crate::character_editor::animation::{
    AnimationEditor, PLAYABLE_KEYS, bind, resolution_label, tag_paths, unbind,
};
use crate::character_editor::debug::{EditorOverlays, toggle_physics_gizmos};
use crate::character_editor::doc::{
    ConfirmAction, DocBar, EditorDoc, WeaponDoc, perform, request, save_all,
};
use crate::character_editor::hardpoints::{
    COARSE_ROTATION, COARSE_TRANSLATION, FINE_ROTATION, FINE_TRANSLATION, HardpointEditor,
    HardpointSide, ensure_role, nudge_rotation, nudge_translation,
};
use crate::character_editor::models::{
    EditorLibrary, ImportKind, add_animation_source, classify, def_stem,
};
use crate::character_editor::state::EditorSession;
use crate::ui::collapse::{SectionBody, section_header};
use crate::ui::spawn_ui::StateMarker;

/// The right-hand pane. Its computed rectangle drives the game camera's viewport.
#[derive(Component)]
pub struct EditorViewportPane;

/// Container for the tweak controls, so later stages can refill it without rebuilding
/// the whole screen.
#[derive(Component)]
pub struct EditorPanel;

/// Holds one clickable row per imported player def.
#[derive(Component)]
pub struct ModelListContainer;

/// Holds one clickable row per weapon def.
#[derive(Component)]
pub struct WeaponListContainer;

/// Holds the import browser's folder and file rows.
#[derive(Component)]
pub struct ImportBrowserContainer;

#[derive(Component)]
pub struct ImportPathLabel;

#[derive(Component)]
pub struct ImportStatusLabel;

/// Holds the debug-overlay toggle rows.
#[derive(Component)]
pub struct DebugTogglesContainer;

/// The open docs, Save / Revert / Menu, and the unsaved-changes question when there is one.
#[derive(Component)]
pub struct DocBarContainer;

/// Holds the hardpoint role chips, nudge rows and bone picker.
#[derive(Component)]
pub struct HardpointContainer;

/// Holds the animation key list and tag-binding picker.
#[derive(Component)]
pub struct AnimationContainer;

/// How tall a list is allowed to grow before it scrolls inside itself.
///
/// Generous on purpose: the left pane scrolls as a whole, so a long list costs nothing but
/// a scroll, and the old 220px cut the model list to about four rows.
///
/// Every list also gets `flex_shrink = 0`. A scroll container's automatic minimum height
/// is zero, so once the pane overflows, flexbox shrinks the lists instead of letting the
/// pane scroll - and the model list squashes down to nothing you can click.
const LIST_MAX_HEIGHT: f32 = 420.0;

pub fn spawn_editor_ui(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row();

    let t = theme.text.clone();
    let hint = TextTheme {
        label_size: 11.0,
        label_color: Color::srgb(0.45, 0.6, 0.5),
        ..t.clone()
    };
    let section = TextTheme {
        label_size: 12.0,
        label_color: Color::srgb(0.55, 0.8, 0.65),
        ..t.clone()
    };

    ui.with_child(|left| {
        left.modify_node(|mut n| {
            n.width = Val::Percent(26.0);
            n.min_width = Val::Px(260.0);
            n.max_width = Val::Px(520.0);
            n.height = Val::Percent(100.0);
            // The pane scrolls as a whole, so an expanded section can be taller than the
            // window without pushing the sections below it off the bottom for good.
            n.overflow = Overflow::scroll_y();
        })
        .insert(ScrollPosition::default())
        .display_flex()
        .flex_column()
        .gap_px(6.0)
        .padding_all_px(10.0)
        .bg_color(Color::srgba(0.04, 0.07, 0.10, 0.97));

        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::header("Character Editor", &t));
        });
        // First under the title, so the unsaved-changes question can never be scrolled
        // out of sight.
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(DocBarContainer)
                .modify_node(|mut n| {
                    n.align_self = AlignSelf::Stretch;
                    n.flex_shrink = 0.0;
                });
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "WASD / left stick to move, mouse / right stick to aim",
                &hint,
            ));
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "[F3] physics debug   [F7] torso twist",
                &hint,
            ));
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "Orbit camera: MMB orbit, Shift+MMB pan, Ctrl+MMB / wheel zoom, \
                 F frame hardpoint, Home frame character",
                &hint,
            ));
        });

        // ── Debug overlays ───────────────────────────────────────────────
        section_header(left, "DEBUG VIEW", &section);
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(DebugTogglesContainer)
                .insert(SectionBody("DEBUG VIEW"))
                .modify_node(|mut n| n.align_self = AlignSelf::Stretch);
        });

        // ── Imported models ──────────────────────────────────────────────
        section_header(left, "MODEL", &section);
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(ModelListContainer)
                .insert(ScrollPosition::default())
                .insert(SectionBody("MODEL"))
                .modify_node(|mut n| {
                    n.align_self = AlignSelf::Stretch;
                    n.max_height = Val::Px(LIST_MAX_HEIGHT);
                    n.flex_shrink = 0.0;
                    n.overflow = Overflow::scroll_y();
                });
        });

        // ── Weapon ───────────────────────────────────────────────────────
        section_header(left, "WEAPON", &section);
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(WeaponListContainer)
                .insert(ScrollPosition::default())
                .insert(SectionBody("WEAPON"))
                .modify_node(|mut n| {
                    n.align_self = AlignSelf::Stretch;
                    n.max_height = Val::Px(LIST_MAX_HEIGHT);
                    n.flex_shrink = 0.0;
                    n.overflow = Overflow::scroll_y();
                });
        });

        // ── Import browser ───────────────────────────────────────────────
        section_header(left, "IMPORT", &section);
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "",
                &TextTheme {
                    label_size: 10.0,
                    label_color: Color::srgb(0.5, 0.7, 0.9),
                    ..t.clone()
                },
            ))
            .insert(ImportPathLabel)
            .insert(SectionBody("IMPORT"))
            .modify_node(|mut n| n.overflow = Overflow::clip());
        });
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(ImportBrowserContainer)
                .insert(ScrollPosition::default())
                .insert(SectionBody("IMPORT"))
                .modify_node(|mut n| {
                    n.align_self = AlignSelf::Stretch;
                    n.max_height = Val::Px(LIST_MAX_HEIGHT);
                    n.flex_shrink = 0.0;
                    n.min_height = Val::Px(120.0);
                    n.overflow = Overflow::scroll_y();
                });
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("", &hint))
                .insert(ImportStatusLabel)
                .insert(SectionBody("IMPORT"));
        });

        // ── Hardpoints ───────────────────────────────────────────────────
        section_header(left, "HARDPOINTS", &section);
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(HardpointContainer)
                .insert(ScrollPosition::default())
                .insert(SectionBody("HARDPOINTS"))
                .modify_node(|mut n| {
                    n.align_self = AlignSelf::Stretch;
                    n.max_height = Val::Px(LIST_MAX_HEIGHT);
                    n.flex_shrink = 0.0;
                    n.overflow = Overflow::scroll_y();
                });
        });

        // ── Animation ────────────────────────────────────────────────────
        section_header(left, "ANIMATION", &section);
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "click a key to play it   [F1] camera  [F2] model",
                &hint,
            ))
            .insert(SectionBody("ANIMATION"));
        });
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(2.0)
                .insert(AnimationContainer)
                .insert(ScrollPosition::default())
                .insert(SectionBody("ANIMATION"))
                .modify_node(|mut n| {
                    n.align_self = AlignSelf::Stretch;
                    n.max_height = Val::Px(LIST_MAX_HEIGHT);
                    n.flex_shrink = 0.0;
                    n.overflow = Overflow::scroll_y();
                });
        });

        // Spare room for whatever comes next.
        left.with_child(|c| {
            c.display_flex()
                .flex_column()
                .gap_px(6.0)
                .insert(EditorPanel)
                .modify_node(|mut n| n.align_self = AlignSelf::Stretch);
        });
    });

    // Right pane: empty node, exists only to give the game camera a rectangle.
    ui.with_child(|right| {
        right
            .modify_node(|mut n| {
                n.flex_grow = 1.0;
                n.height = Val::Percent(100.0);
            })
            .insert(EditorViewportPane);
    });

    ui.build();
}

/// Clip the game camera to the right-hand pane. Runs every frame so the viewport tracks
/// window resizes and any future resizing of the panel.
pub fn sync_editor_viewport(
    panes: Query<(&ComputedNode, &UiGlobalTransform), With<EditorViewportPane>>,
    mut cameras: Query<&mut Camera, With<GameCamera>>,
    windows: Query<&Window>,
) {
    let Ok((node, transform)) = panes.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else { return };

    let size = node.size();
    let top_left = transform.affine().translation - size * 0.5;

    let Some(viewport) = pane_viewport(
        top_left.x,
        top_left.y,
        size.x,
        size.y,
        window.physical_width(),
        window.physical_height(),
    ) else {
        return;
    };
    camera.viewport = Some(viewport);
}

/// Clamp a pane rectangle to the window. Returns `None` while the rectangle is degenerate
/// — a zero-sized viewport makes wgpu complain, and the UI reports zero on the first frame
/// before layout has run.
fn pane_viewport(
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    window_width: u32,
    window_height: u32,
) -> Option<bevy::camera::Viewport> {
    let x = left.max(0.0) as u32;
    let y = top.max(0.0) as u32;
    let w = (width as u32).min(window_width.saturating_sub(x));
    let h = (height as u32).min(window_height.saturating_sub(y));
    if w == 0 || h == 0 {
        return None;
    }
    Some(bevy::camera::Viewport {
        physical_position: UVec2::new(x, y),
        physical_size: UVec2::new(w, h),
        depth: 0.0..1.0,
    })
}

/// Hand the camera back when the session ends, so a following real match is not stuck
/// rendering into the right-hand quarter of the window.
pub fn clear_editor_viewport(mut cameras: Query<&mut Camera, With<GameCamera>>) {
    for mut camera in cameras.iter_mut() {
        camera.viewport = None;
    }
}

/// Leaving `InGame` ends the session; without this the resource would leak into the next
/// real match and quietly suppress the HUD and the map.
pub fn end_editor_session(mut commands: Commands) {
    commands.remove_resource::<EditorSession>();
}

// ── List rebuilding ───────────────────────────────────────────────────────────
//
// Both lists are mouse-driven rather than keyboard-driven: the keyboard is busy walking
// the character around, so binding Up/Down/Enter here would fight the game.

/// One clickable row. `selected` gives it the highlight colour.
fn row<'a>(
    parent: &'a mut RelatedSpawnerCommands<ChildOf>,
    text: String,
    selected: bool,
    color: Color,
) -> EntityCommands<'a> {
    let bg = if selected {
        Color::srgba(0.12, 0.32, 0.20, 0.95)
    } else {
        Color::srgba(0.09, 0.13, 0.18, 0.90)
    };
    let mut cmds = parent.spawn((
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..Default::default()
        },
        BackgroundColor(bg),
        InteractionPalette {
            none: bg,
            hovered: Color::srgba(0.18, 0.35, 0.45, 0.95),
            pressed: Color::srgba(0.10, 0.25, 0.35, 1.0),
        },
        bevy::picking::hover::Hovered::default(),
        bevy::ui_widgets::Button,
    ));
    cmds.with_child((
        Text::new(text),
        TextFont::default().with_font_size(11.0),
        TextColor(color),
    ));
    cmds
}

/// What the doc bar shows, compared frame to frame so it only rebuilds on a change.
#[derive(PartialEq, Eq, Default)]
pub struct DocBarView {
    character: Option<String>,
    character_dirty: bool,
    weapon: Option<String>,
    weapon_dirty: bool,
    confirm: Option<ConfirmAction>,
    status: String,
}

/// A row of buttons side by side.
fn button_strip<'a>(parent: &'a mut RelatedSpawnerCommands<ChildOf>) -> EntityCommands<'a> {
    parent.spawn(Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(2.0),
        ..Default::default()
    })
}

fn doc_line(name: Option<&str>, kind: &str, dirty: bool) -> (String, Color) {
    match name {
        None => (format!("{kind}: (none)"), Color::srgb(0.55, 0.6, 0.65)),
        Some(name) if dirty => (
            format!("{kind}: {name} *  (unsaved)"),
            Color::srgb(1.0, 0.8, 0.45),
        ),
        Some(name) => (format!("{kind}: {name}"), Color::srgb(0.75, 0.9, 0.8)),
    }
}

/// The doc bar: what is open, whether it has unsaved edits, Save / Revert / Menu, and the
/// question asked before anything would throw edits away.
#[allow(clippy::too_many_lines)]
pub fn rebuild_doc_bar(
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    bar: Res<DocBar>,
    mut commands: Commands,
    container_q: Query<Entity, With<DocBarContainer>>,
    mut last: Local<Option<DocBarView>>,
) {
    if last.is_some() && !doc.is_changed() && !weapon_doc.is_changed() && !bar.is_changed() {
        return;
    }
    let Ok(container) = container_q.single() else {
        return;
    };
    let view = DocBarView {
        character: doc.name().map(str::to_string),
        character_dirty: doc.is_dirty(),
        weapon: weapon_doc.name().map(str::to_string),
        weapon_dirty: weapon_doc.is_dirty(),
        confirm: bar.confirm.clone(),
        status: bar.status.clone(),
    };
    if last.as_ref() == Some(&view) {
        return;
    }

    commands.entity(container).despawn_related::<Children>();
    let any_dirty = view.character_dirty || view.weapon_dirty;
    commands.entity(container).with_children(|parent| {
        for (name, kind, dirty) in [
            (view.character.as_deref(), "character", view.character_dirty),
            (view.weapon.as_deref(), "weapon", view.weapon_dirty),
        ] {
            let (text, color) = doc_line(name, kind, dirty);
            parent.spawn((
                Text::new(text),
                TextFont::default().with_font_size(12.0),
                TextColor(color),
            ));
        }

        button_strip(parent).with_children(|strip| {
            row(
                strip,
                "Save".to_string(),
                any_dirty,
                Color::srgb(0.7, 0.95, 0.75),
            )
            .observe(
                |_: On<Activate>,
                 mut doc: ResMut<EditorDoc>,
                 mut weapon_doc: ResMut<WeaponDoc>,
                 mut bar: ResMut<DocBar>| {
                    bar.status = save_all(&mut doc, &mut weapon_doc);
                },
            );
            row(
                strip,
                "Revert".to_string(),
                false,
                Color::srgb(0.95, 0.8, 0.6),
            )
            .observe(
                |_: On<Activate>,
                 mut doc: ResMut<EditorDoc>,
                 mut weapon_doc: ResMut<WeaponDoc>,
                 mut bar: ResMut<DocBar>| {
                    if !doc.is_dirty() && !weapon_doc.is_dirty() {
                        bar.status = "nothing to revert".to_string();
                        return;
                    }
                    doc.revert();
                    weapon_doc.revert();
                    bar.status = "reverted to the saved files".to_string();
                },
            );
            row(
                strip,
                "Menu".to_string(),
                false,
                Color::srgb(0.75, 0.85, 0.95),
            )
            .observe(
                |_: On<Activate>,
                 mut models: ResMut<EditorLibrary>,
                 mut bar: ResMut<DocBar>,
                 mut doc: ResMut<EditorDoc>,
                 weapon_doc: Res<WeaponDoc>,
                 mut next_state: ResMut<NextState<GameState>>| {
                    request(
                        ConfirmAction::Exit,
                        &mut bar,
                        &mut doc,
                        &weapon_doc,
                        &mut models,
                        &mut next_state,
                    );
                },
            );
        });

        if let Some(action) = view.confirm.clone() {
            parent.spawn((
                Text::new(format!("Unsaved changes. Save them before you {}?", action.describe())),
                TextFont::default().with_font_size(12.0),
                TextColor(Color::srgb(1.0, 0.75, 0.4)),
            ));
            button_strip(parent).with_children(|strip| {
                row(
                    strip,
                    "Save, then continue".to_string(),
                    true,
                    Color::srgb(0.7, 0.95, 0.75),
                )
                .observe(
                    |_: On<Activate>,
                     mut models: ResMut<EditorLibrary>,
                     mut bar: ResMut<DocBar>,
                     mut doc: ResMut<EditorDoc>,
                     mut weapon_doc: ResMut<WeaponDoc>,
                     mut next_state: ResMut<NextState<GameState>>| {
                        let Some(action) = bar.confirm.take() else {
                            return;
                        };
                        bar.status = save_all(&mut doc, &mut weapon_doc);
                        // A save that failed leaves the edits in place; do not walk away
                        // from them.
                        if action.would_lose_edits(&doc, &weapon_doc) {
                            return;
                        }
                        perform(action, &mut bar, &mut doc, &mut models, &mut next_state);
                    },
                );
                row(
                    strip,
                    "Discard".to_string(),
                    false,
                    Color::srgb(0.95, 0.65, 0.55),
                )
                .observe(
                    |_: On<Activate>,
                     mut models: ResMut<EditorLibrary>,
                     mut bar: ResMut<DocBar>,
                     mut doc: ResMut<EditorDoc>,
                     mut weapon_doc: ResMut<WeaponDoc>,
                     mut next_state: ResMut<NextState<GameState>>| {
                        let Some(action) = bar.confirm.take() else {
                            return;
                        };
                        // Only what the action would have lost: equipping replaces the
                        // weapon doc, but the character's edits carry on.
                        weapon_doc.revert();
                        if !matches!(action, ConfirmAction::EquipWeapon(_)) {
                            doc.revert();
                        }
                        bar.status = "discarded unsaved changes".to_string();
                        perform(action, &mut bar, &mut doc, &mut models, &mut next_state);
                    },
                );
                row(
                    strip,
                    "Cancel".to_string(),
                    false,
                    Color::srgb(0.75, 0.85, 0.95),
                )
                .observe(|_: On<Activate>, mut bar: ResMut<DocBar>| {
                    bar.confirm = None;
                });
            });
        }

        if !view.status.is_empty() {
            parent.spawn((
                Text::new(view.status.clone()),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.6, 0.85, 0.6)),
            ));
        }
    });
    *last = Some(view);
}

/// Rebuild the debug toggle rows. Cheap: three rows, only on change.
pub fn rebuild_debug_toggles(
    mut debug: ResMut<EditorOverlays>,
    mut commands: Commands,
    container_q: Query<Entity, With<DebugTogglesContainer>>,
    mut spawned: Local<bool>,
) {
    if !debug.ui_dirty && *spawned {
        return;
    }
    debug.ui_dirty = false;
    *spawned = true;

    let Ok(container) = container_q.single() else {
        // The panel has not been built yet; try again next frame.
        *spawned = false;
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let rows = [
        (debug.physics, "physics colliders  [F3]"),
        (debug.skeleton, "skeleton"),
        (debug.hardpoints, "hardpoint frames"),
        (debug.gait, "gait targets"),
        (debug.orbit_camera, "orbit camera  [F11]"),
        (debug.rest_pose, "rest pose (T-pose)  [F12]"),
    ];
    commands.entity(container).with_children(|parent| {
        for (index, (on, label)) in rows.into_iter().enumerate() {
            let text = format!("[{}] {label}", if on { "x" } else { " " });
            row(parent, text, on, Color::srgb(0.85, 0.9, 0.95)).observe(
                move |_: On<Activate>,
                      mut debug: ResMut<EditorOverlays>,
                      mut store: ResMut<GizmoConfigStore>| {
                    match index {
                        0 => {
                            toggle_physics_gizmos(&mut store);
                            // `sync_physics_toggle` picks the new value up and marks the
                            // panel dirty, so there is one source of truth.
                        }
                        1 => debug.toggle_skeleton(),
                        2 => debug.toggle_hardpoints(),
                        3 => debug.toggle_gait(),
                        4 => debug.toggle_orbit_camera(),
                        _ => debug.toggle_rest_pose(),
                    }
                },
            );
        }
    });
}

/// A left-to-right strip of small buttons sharing one label.
fn nudge_row(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    label: String,
    axis: usize,
    coarse: f32,
    fine: f32,
    rotation: bool,
) {
    parent
        .spawn((Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            ..Default::default()
        },))
        .with_children(|strip| {
            strip.spawn((
                Node {
                    width: Val::Px(86.0),
                    ..Default::default()
                },
                Text::new(label),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
            ));
            for (text, delta) in [("<<", -coarse), ("<", -fine), (">", fine), (">>", coarse)] {
                let bg = Color::srgba(0.09, 0.16, 0.22, 0.95);
                strip
                    .spawn((
                        Node {
                            width: Val::Px(24.0),
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..Default::default()
                        },
                        BackgroundColor(bg),
                        InteractionPalette {
                            none: bg,
                            hovered: Color::srgba(0.18, 0.35, 0.45, 0.95),
                            pressed: Color::srgba(0.10, 0.25, 0.35, 1.0),
                        },
                        bevy::picking::hover::Hovered::default(),
                        bevy::ui_widgets::Button,
                    ))
                    .with_child((
                        Text::new(text),
                        TextFont::default().with_font_size(11.0),
                        TextColor(Color::srgb(0.85, 0.92, 0.95)),
                    ))
                    .observe(
                        move |_: On<Activate>,
                              mut editor: ResMut<HardpointEditor>,
                              mut doc: ResMut<EditorDoc>,
                              mut weapon_doc: ResMut<WeaponDoc>| {
                            let Some(role) = editor.active_role.clone() else {
                                return;
                            };
                            let side = editor.side;
                            // Only reach for the side being edited: `as_mut()` on the other
                            // would mark it changed and re-push an unedited def at the live
                            // weapon every click.
                            let def = match side {
                                HardpointSide::Character => doc.def.as_mut(),
                                HardpointSide::Weapon => weapon_doc.def.as_mut(),
                            };
                            let Some(def) = def else { return };
                            let hardpoint = ensure_role(def, &role);
                            if rotation {
                                nudge_rotation(hardpoint, axis, delta);
                            } else {
                                nudge_translation(hardpoint, axis, delta);
                            }
                            editor.touch();
                        },
                    );
            }
        });
}

/// The character/weapon switch at the top of the hardpoint panel.
///
/// Spawned even when the chosen side has no def, so you can always get back to the one
/// that does.
fn side_chips(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    side: HardpointSide,
    weapon_name: Option<&str>,
) {
    let weapon_label = format!(
        "{}: {}",
        HardpointSide::Weapon.label(),
        weapon_name.unwrap_or("(none)")
    );
    for (which, label) in [
        (
            HardpointSide::Character,
            HardpointSide::Character.label().to_string(),
        ),
        (HardpointSide::Weapon, weapon_label),
    ] {
        let is_active = which == side;
        let marker = if is_active { "*" } else { " " };
        row(
            parent,
            format!("{marker} {label}"),
            is_active,
            Color::srgb(0.9, 0.85, 0.7),
        )
        .observe(
            move |_: On<Activate>, mut editor: ResMut<HardpointEditor>| {
                editor.select_side(which);
            },
        );
    }
}

/// Rebuild the hardpoint editor. Driven by `ui_dirty` rather than every frame because it
/// spawns a row per bone, and a rig has dozens.
#[allow(clippy::too_many_arguments)]
pub fn rebuild_hardpoint_panel(
    mut editor: ResMut<HardpointEditor>,
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    mut commands: Commands,
    container_q: Query<Entity, With<HardpointContainer>>,
    players: Query<Entity, With<Player>>,
    children_q: Query<&Children>,
    skinned_q: Query<&bevy::mesh::skinning::SkinnedMesh>,
    names: Query<&Name>,
    mut last_def: Local<Option<String>>,
) {
    // Opening another model replaces the doc, so the panel has to follow it. The weapon
    // doc is reopened by `sync_weapon_doc` in the same breath, so both sides reset together.
    // Edits to the open doc redraw too: Revert and Save change what the rows should show.
    if *last_def != doc.def_path {
        last_def.clone_from(&doc.def_path);
        editor.active_role = None;
        editor.side = HardpointSide::Character;
        editor.ui_dirty = true;
    }
    if doc.is_changed() || weapon_doc.is_changed() {
        editor.ui_dirty = true;
    }
    if !editor.ui_dirty {
        return;
    }
    editor.ui_dirty = false;

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let side = editor.side;
    let weapon_name = weapon_doc.name().map(str::to_string);
    let def = match side {
        HardpointSide::Character => doc.def.as_ref(),
        HardpointSide::Weapon => weapon_doc.def.as_ref(),
    };

    let Some(def) = def else {
        let message = match side {
            HardpointSide::Character => "no character open",
            HardpointSide::Weapon => "no weapon equipped (set PlayerProps.weapon)",
        };
        commands.entity(container).with_children(|parent| {
            side_chips(parent, side, weapon_name.as_deref());
            parent.spawn((
                Text::new(message),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.6, 0.5, 0.4)),
            ));
        });
        return;
    };

    let active = editor.active_role.clone();
    let present: Vec<String> = def.hardpoints.keys().cloned().collect();
    let hardpoint = active
        .as_ref()
        .and_then(|role| def.hardpoints.get(role))
        .cloned();

    // Bone names of the live rig, so the anchor picker offers what actually exists.
    // Weapon frames are model-origin relative, so the picker is character-side only.
    let bones: Vec<String> = players
        .iter()
        .next()
        .filter(|_| side == HardpointSide::Character)
        .map_or_default(|player| {
            let joints = crate::assets::gizmos::joints_under(player, &children_q, &skinned_q);
            let mut names: Vec<String> = crate::assets::gizmos::bone_map(&joints, &names)
                .into_keys()
                .collect();
            names.sort();
            names
        });

    commands.entity(container).with_children(|parent| {
        side_chips(parent, side, weapon_name.as_deref());

        // Role chips.
        for role in side.roles().iter().copied() {
            let exists = present.iter().any(|r| r == role);
            let is_active = active.as_deref() == Some(role);
            let marker = if is_active {
                "*"
            } else if exists {
                "-"
            } else {
                "+"
            };
            let clicked = role.to_string();
            row(
                parent,
                format!("{marker} {role}"),
                is_active,
                Color::srgb(0.85, 0.92, 0.85),
            )
            .observe(
                move |_: On<Activate>, mut editor: ResMut<HardpointEditor>| {
                    editor.select_role(&clicked);
                },
            );
        }

        let Some(hardpoint) = hardpoint else { return };
        let role = active.clone().unwrap_or_default();

        parent.spawn((
            Text::new(format!(
                "anchor: {}\nT {:.3} {:.3} {:.3}\nR {:.1} {:.1} {:.1}",
                hardpoint.anchor.as_deref().unwrap_or("(model origin)"),
                hardpoint.translation[0],
                hardpoint.translation[1],
                hardpoint.translation[2],
                hardpoint.rotation_euler_deg[0],
                hardpoint.rotation_euler_deg[1],
                hardpoint.rotation_euler_deg[2],
            )),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::srgb(0.6, 0.8, 0.95)),
        ));

        for (axis, name) in ["move X", "move Y", "move Z"].into_iter().enumerate() {
            nudge_row(
                parent,
                name.to_string(),
                axis,
                COARSE_TRANSLATION,
                FINE_TRANSLATION,
                false,
            );
        }
        for (axis, name) in ["turn X", "turn Y", "turn Z"].into_iter().enumerate() {
            nudge_row(
                parent,
                name.to_string(),
                axis,
                COARSE_ROTATION,
                FINE_ROTATION,
                true,
            );
        }

        let removed = role.clone();
        row(
            parent,
            "remove this hardpoint".to_string(),
            false,
            Color::srgb(0.95, 0.7, 0.6),
        )
        .observe(
            move |_: On<Activate>,
                  mut editor: ResMut<HardpointEditor>,
                  mut doc: ResMut<EditorDoc>,
                  mut weapon_doc: ResMut<WeaponDoc>| {
                let def = match editor.side {
                    HardpointSide::Character => doc.def.as_mut(),
                    HardpointSide::Weapon => weapon_doc.def.as_mut(),
                };
                if let Some(def) = def {
                    def.hardpoints.remove(&removed);
                }
                editor.active_role = None;
                editor.touch();
            },
        );

        if side == HardpointSide::Weapon {
            parent.spawn((
                Text::new("weapon frames are relative to the model origin"),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.55, 0.7, 0.8)),
            ));
            // A weapon frame that picked up a bone anchor (defs written against a
            // character carry this) resolves to nothing and draws nothing, which reads as
            // the frame being missing. This is the way back.
            if hardpoint.anchor.is_some() {
                let for_role = role;
                row(
                    parent,
                    "clear anchor (use model origin)".to_string(),
                    false,
                    Color::srgb(0.95, 0.85, 0.6),
                )
                .observe(
                    move |_: On<Activate>,
                          mut editor: ResMut<HardpointEditor>,
                          mut weapon_doc: ResMut<WeaponDoc>| {
                        if let Some(def) = weapon_doc.def.as_mut() {
                            ensure_role(def, &for_role).anchor = None;
                        }
                        editor.touch();
                    },
                );
            }
            return;
        }

        // Anchor picker: the bones of the rig currently on screen.
        parent.spawn((
            Text::new("anchor bone:"),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::srgb(0.55, 0.7, 0.8)),
        ));
        for bone in bones {
            let is_anchor = hardpoint.anchor.as_deref() == Some(bone.as_str());
            let chosen = bone.clone();
            let for_role = role.clone();
            row(parent, bone, is_anchor, Color::srgb(0.8, 0.8, 0.9)).observe(
                move |_: On<Activate>,
                      mut editor: ResMut<HardpointEditor>,
                      mut doc: ResMut<EditorDoc>| {
                    if let Some(def) = doc.def.as_mut() {
                        ensure_role(def, &for_role).anchor = Some(chosen.clone());
                    }
                    editor.touch();
                },
            );
        }
    });
}

/// Rebuild the animation panel: every key, what it resolves to, and — once a key is
/// picked — the tag paths it can be bound to.
pub fn rebuild_animation_panel(
    mut editor: ResMut<AnimationEditor>,
    doc: Res<EditorDoc>,
    mut commands: Commands,
    container_q: Query<Entity, With<AnimationContainer>>,
    mut last_def: Local<Option<String>>,
) {
    // Follow model swaps, and redraw when a binding edit changes what keys resolve to.
    if *last_def != doc.def_path {
        last_def.clone_from(&doc.def_path);
        editor.selected_key = None;
        editor.ui_dirty = true;
    }
    if !editor.ui_dirty {
        return;
    }
    editor.ui_dirty = false;

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let Some(def) = doc.def.as_ref() else {
        commands.entity(container).with_children(|parent| {
            parent.spawn((
                Text::new("no character open"),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.6, 0.5, 0.4)),
            ));
        });
        return;
    };

    let selected = editor.selected_key;
    let labels: Vec<(AnimationKey, String, String)> = PLAYABLE_KEYS
        .iter()
        .map(|&key| {
            (
                key,
                key.default_search().to_string(),
                resolution_label(def, key),
            )
        })
        .collect();
    let tags = tag_paths(def);
    let bound_tag =
        selected.and_then(|key| def.animation_bindings.get(key.default_search()).cloned());
    commands.entity(container).with_children(|parent| {
        for (key, name, resolves_to) in labels {
            let is_selected = selected == Some(key);
            let marker = if is_selected { "*" } else { " " };
            row(
                parent,
                format!("{marker} {name}  ->  {resolves_to}"),
                is_selected,
                Color::srgb(0.85, 0.9, 0.85),
            )
            .observe(
                move |_: On<Activate>,
                      mut editor: ResMut<AnimationEditor>,
                      mut anim_mw: MessageWriter<AnimationEvent>,
                      players: Query<Entity, With<Player>>| {
                    editor.select(key);
                    // Play it on whoever is standing in the arena.
                    for player in players.iter() {
                        anim_mw.write(AnimationEvent(
                            AnimationEventType::GotoAnimState,
                            player,
                            key,
                        ));
                    }
                },
            );
        }

        row(
            parent,
            "back to idle".to_string(),
            false,
            Color::srgb(0.7, 0.8, 0.9),
        )
        .observe(
            |_: On<Activate>,
             editor: Res<AnimationEditor>,
             mut anim_mw: MessageWriter<AnimationEvent>,
             players: Query<Entity, With<Player>>| {
                let Some(key) = editor.selected_key else {
                    return;
                };
                for player in players.iter() {
                    anim_mw.write(AnimationEvent(
                        AnimationEventType::LeaveAnimState,
                        player,
                        key,
                    ));
                }
            },
        );

        let Some(key) = selected else { return };

        parent.spawn((
            Text::new(format!("bind '{}' to:", key.default_search())),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::srgb(0.55, 0.7, 0.8)),
        ));

        if tags.is_empty() {
            parent.spawn((
                Text::new("this def tags no clips - tag them in the asset browser for now"),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.7, 0.6, 0.45)),
            ));
        }

        for tag in tags {
            let is_bound = bound_tag.as_deref() == Some(tag.as_str());
            let chosen = tag.clone();
            row(parent, tag, is_bound, Color::srgb(0.8, 0.85, 0.95)).observe(
                move |_: On<Activate>,
                      mut editor: ResMut<AnimationEditor>,
                      mut doc: ResMut<EditorDoc>| {
                    if let Some(def) = doc.def.as_mut() {
                        bind(def, key, &chosen);
                    }
                    editor.touch();
                },
            );
        }

        row(
            parent,
            "clear binding".to_string(),
            false,
            Color::srgb(0.95, 0.8, 0.6),
        )
        .observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  mut doc: ResMut<EditorDoc>| {
                if let Some(def) = doc.def.as_mut() {
                    unbind(def, key);
                }
                editor.touch();
            },
        );

    });
}

/// A binding edit changes what every key resolves to, so the list has to be redrawn.
pub fn refresh_animation_panel_on_def_change(
    doc: Res<EditorDoc>,
    mut editor: ResMut<AnimationEditor>,
) {
    if doc.is_changed() {
        editor.ui_dirty = true;
    }
}

pub fn rebuild_model_list(
    mut models: ResMut<EditorLibrary>,
    mut commands: Commands,
    container_q: Query<Entity, With<ModelListContainer>>,
) {
    if !models.list_dirty {
        return;
    }
    // Cleared only once there is somewhere to draw, so a frame without the container
    // retries instead of leaving the list empty for the session.
    let Ok(container) = container_q.single() else {
        return;
    };
    models.list_dirty = false;
    commands.entity(container).despawn_related::<Children>();

    let defs = models.defs.clone();
    let mesh_less = models.mesh_less.clone();
    let selected = models.selected.clone();
    commands.entity(container).with_children(|parent| {
        if defs.is_empty() {
            parent.spawn((
                Text::new("no player defs in assets/defs"),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.6, 0.5, 0.4)),
            ));
            return;
        }
        for def_path in defs {
            let is_selected = selected.as_deref() == Some(def_path.as_str());
            let has_no_mesh = mesh_less.contains(&def_path);
            let mark = if is_selected { "*" } else { " " };
            let label = if has_no_mesh {
                format!("{mark} {} (no mesh)", def_stem(&def_path))
            } else {
                format!("{mark} {}", def_stem(&def_path))
            };
            let color = if has_no_mesh {
                Color::srgb(0.75, 0.6, 0.5)
            } else {
                Color::srgb(0.85, 0.92, 0.85)
            };
            let clicked = def_path.clone();
            row(parent, label, is_selected, color).observe(
                move |_: On<Activate>,
                      mut m: ResMut<EditorLibrary>,
                      mut bar: ResMut<DocBar>,
                      mut doc: ResMut<EditorDoc>,
                      weapon_doc: Res<WeaponDoc>,
                      mut next_state: ResMut<NextState<GameState>>| {
                    // Refused rather than swapped to: the swap would succeed and leave you
                    // walking around as nothing, with no clue why.
                    if m.mesh_less.contains(&clicked) {
                        m.status = format!(
                            "{} has no geometry - it is an animation library, not a model",
                            def_stem(&clicked)
                        );
                        m.browser_dirty = true;
                        return;
                    }
                    request(
                        ConfirmAction::OpenModel(clicked.clone()),
                        &mut bar,
                        &mut doc,
                        &weapon_doc,
                        &mut m,
                        &mut next_state,
                    );
                },
            );
        }
    });
}

/// Add a clip library to the open character.
///
/// No respawn needed: `build_player_anim_graph` folds `animation_sources` into the
/// signature it watches and waits for the extra GLTFs to load before committing, so the
/// new clips reach the live character on their own once the file is in. Like any other
/// edit it stays in the doc until you Save — switching models asks first, so the clips
/// cannot quietly vanish.
fn attach_animation_source(
    source_path: &str,
    models: &mut EditorLibrary,
    doc: &mut EditorDoc,
    animation: &mut AnimationEditor,
) {
    let Some(def) = doc.def.as_mut() else {
        models.status = "no character open to add the source to".to_string();
        return;
    };

    let (changed, status) = add_animation_source(def, source_path);
    models.status = status;
    if changed {
        animation.ui_dirty = true;
    }
}

/// Rebuild the weapon list: every weapon def, with the one the worn character carries
/// marked.
///
/// Picking one writes `PlayerProps.weapon` into the doc. That is an edit like any other:
/// `sync_doc_to_live` sees the live character carrying something else and respawns it from
/// the doc, holding the new gun. Nothing is saved until you Save.
pub fn rebuild_weapon_list(
    mut models: ResMut<EditorLibrary>,
    doc: Res<EditorDoc>,
    mut commands: Commands,
    container_q: Query<Entity, With<WeaponListContainer>>,
    mut last: Local<Option<String>>,
) {
    let equipped = doc
        .def
        .as_ref()
        .and_then(crate::character_editor::doc::weapon_def_path);
    // Follows both a model swap (which changes what is equipped) and the list itself.
    let changed = *last != equipped;
    if !models.weapon_list_dirty && !changed {
        return;
    }
    last.clone_from(&equipped);

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let weapons = models.weapon_defs.clone();
    let is_character = doc.def.as_ref().is_some_and(|def| {
        matches!(
            def.model_type,
            crate::assets::asset_definition::ModelType::Player(_)
        )
    });
    models.weapon_list_dirty = false;

    commands.entity(container).with_children(|parent| {
        if !is_character {
            parent.spawn((
                Text::new("no character open"),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.6, 0.5, 0.4)),
            ));
            return;
        }

        let none_selected = equipped.is_none();
        row(
            parent,
            format!("{} (none)", if none_selected { "*" } else { " " }),
            none_selected,
            Color::srgb(0.8, 0.8, 0.85),
        )
        .observe(
            |_: On<Activate>,
             mut models: ResMut<EditorLibrary>,
             mut bar: ResMut<DocBar>,
             mut doc: ResMut<EditorDoc>,
             weapon_doc: Res<WeaponDoc>,
             mut next_state: ResMut<NextState<GameState>>| {
                request(
                    ConfirmAction::EquipWeapon(None),
                    &mut bar,
                    &mut doc,
                    &weapon_doc,
                    &mut models,
                    &mut next_state,
                );
            },
        );

        for weapon_path in weapons {
            let is_equipped = equipped.as_deref() == Some(weapon_path.as_str());
            let label = format!(
                "{} {}",
                if is_equipped { "*" } else { " " },
                def_stem(&weapon_path)
            );
            let chosen = weapon_path.clone();
            row(parent, label, is_equipped, Color::srgb(0.9, 0.88, 0.75)).observe(
                move |_: On<Activate>,
                      mut models: ResMut<EditorLibrary>,
                      mut bar: ResMut<DocBar>,
                      mut doc: ResMut<EditorDoc>,
                      weapon_doc: Res<WeaponDoc>,
                      mut next_state: ResMut<NextState<GameState>>| {
                    request(
                        ConfirmAction::EquipWeapon(Some(chosen.clone())),
                        &mut bar,
                        &mut doc,
                        &weapon_doc,
                        &mut models,
                        &mut next_state,
                    );
                },
            );
        }
    });
}

pub fn rebuild_import_browser(
    mut models: ResMut<EditorLibrary>,
    mut commands: Commands,
    container_q: Query<Entity, With<ImportBrowserContainer>>,
    mut path_label_q: Query<&mut Text, (With<ImportPathLabel>, Without<ImportStatusLabel>)>,
    mut status_label_q: Query<&mut Text, With<ImportStatusLabel>>,
) {
    if !models.browser_dirty {
        return;
    }
    models.browser_dirty = false;

    if let Ok(mut text) = path_label_q.single_mut() {
        **text = format!("assets/{}", models.browse_folder);
    }
    if let Ok(mut text) = status_label_q.single_mut() {
        (**text).clone_from(&models.status);
    }

    let Ok(container) = container_q.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let at_root = models.browse_folder.is_empty();
    let folders = models.folders.clone();
    let files = models.files.clone();
    commands.entity(container).with_children(|parent| {
        if !at_root {
            row(
                parent,
                "[..]".to_string(),
                false,
                Color::srgb(0.75, 0.88, 1.0),
            )
            .observe(|_: On<Activate>, mut m: ResMut<EditorLibrary>| m.leave_folder());
        }
        for name in folders {
            let entered = name.clone();
            row(
                parent,
                format!("[dir] {name}"),
                false,
                Color::srgb(0.75, 0.88, 1.0),
            )
            .observe(move |_: On<Activate>, mut m: ResMut<EditorLibrary>| {
                m.enter_folder(&entered);
            });
        }
        for file in files {
            let imported = file.clone();
            let name = file.rsplit('/').next().unwrap_or(&file).to_string();
            // Say up front which files have no geometry, so the click that follows is an
            // informed one rather than a model that spawns invisible.
            let (label, color) = match classify(&file) {
                ImportKind::Model => (format!("+ {name}"), Color::srgb(0.9, 0.85, 0.7)),
                ImportKind::AnimationLibrary => (
                    format!("~ {name}  (anims only -> add as source)"),
                    Color::srgb(0.7, 0.8, 0.95),
                ),
            };
            row(parent, label, false, color).observe(
                move |_: On<Activate>,
                      mut m: ResMut<EditorLibrary>,
                      mut doc: ResMut<EditorDoc>,
                      mut animation: ResMut<AnimationEditor>| {
                    match classify(&imported) {
                        ImportKind::Model => m.import(&imported),
                        ImportKind::AnimationLibrary => {
                            attach_animation_source(
                                &imported,
                                &mut m,
                                &mut doc,
                                &mut animation,
                            );
                        }
                    }
                    m.browser_dirty = true;
                },
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::pane_viewport;

    #[test]
    fn a_pane_becomes_the_matching_viewport_rect() {
        let v = pane_viewport(400.0, 0.0, 1200.0, 900.0, 1600, 900).expect("valid rect");
        assert_eq!(v.physical_position.to_array(), [400, 0]);
        assert_eq!(v.physical_size.to_array(), [1200, 900]);
    }

    #[test]
    fn a_pane_hanging_off_the_window_is_clamped_to_it() {
        // Mid-resize the UI can report a rectangle wider than the window still is.
        let v = pane_viewport(400.0, 0.0, 1400.0, 900.0, 1600, 900).expect("valid rect");
        assert_eq!(
            v.physical_size.to_array(),
            [1200, 900],
            "clamped to the window edge"
        );
    }

    #[test]
    fn a_zero_sized_pane_is_refused_rather_than_sent_to_the_gpu() {
        assert!(
            pane_viewport(0.0, 0.0, 0.0, 0.0, 1600, 900).is_none(),
            "first frame"
        );
        assert!(
            pane_viewport(1600.0, 0.0, 100.0, 900.0, 1600, 900).is_none(),
            "fully off-screen"
        );
    }

    #[test]
    fn a_negative_origin_is_pulled_back_to_the_window_corner() {
        let v = pane_viewport(-20.0, -10.0, 500.0, 400.0, 1600, 900).expect("valid rect");
        assert_eq!(v.physical_position.to_array(), [0, 0]);
    }
}
