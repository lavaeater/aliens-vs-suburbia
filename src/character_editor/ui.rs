//! The character editor's panels: what fills the containers `shell.rs` lays out, plus the
//! viewport clipping.
//!
//! The game pane is not a separate render — it is the ordinary game camera with its
//! `viewport` clipped to the pane's rectangle, the same trick the asset browser uses in
//! `asset_browser::viewer::sync_viewer_viewport`. That keeps every gameplay system,
//! including the camera follow, working exactly as it does full-screen.

use bevy::gizmos::config::GizmoConfigStore;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::camera::components::GameCamera;
use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::ecs::system::EntityCommands;
use lava_ui_builder::InteractionPalette;

use crate::character_editor::debug::{EditorOverlays, toggle_physics_gizmos};
use crate::character_editor::doc::{
    ConfirmAction, DocBar, Docs, EditorDoc, EditorFocus, PropDoc, SideDocs, Subject, WeaponDoc,
};
use crate::character_editor::hardpoints::{
    ActiveFrame, COARSE_ROTATION, COARSE_TRANSLATION, FINE_ROTATION, FINE_TRANSLATION,
    HardpointEditor, HardpointSide, NudgeSpace, add_role, ensure_role, guess_hand_bone,
    nudge_rotation, nudge_rotation_in_model, nudge_translation, nudge_translation_in_model,
};
use crate::character_editor::models::{EditorLibrary, def_stem};
use crate::character_editor::prop::EditorProp;
use crate::character_editor::state::EditorSession;
use crate::character_editor::suggest_hardpoints::ModelShape;
use crate::player::components::Player;

/// The right-hand pane. Its computed rectangle drives the game camera's viewport.
#[derive(Component)]
pub struct EditorViewportPane;

/// Holds one clickable row per weapon def.
#[derive(Component)]
pub struct WeaponListContainer;

/// Holds the debug-overlay toggle rows.
#[derive(Component)]
pub struct DebugTogglesContainer;

/// The open docs, Save / Revert / Menu, and the unsaved-changes question when there is one.
#[derive(Component)]
pub struct DocBarContainer;

/// The "suggest" rows in the hardpoint panel.
const SUGGEST_COLOR: Color = Color::srgb(0.7, 0.85, 0.95);

/// Holds the hardpoint role chips, nudge rows and bone picker.
#[derive(Component)]
pub struct HardpointContainer;

/// Holds the animation key list and tag-binding picker.
#[derive(Component)]
pub struct AnimationContainer;

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
    // The block only exists while the editor does; a real match never sees it.
    commands.remove_resource::<crate::control::components::GameInputBlock>();
}

// ── List rebuilding ───────────────────────────────────────────────────────────
//
// Both lists are mouse-driven rather than keyboard-driven: the keyboard is busy walking
// the character around, so binding Up/Down/Enter here would fight the game.

/// One clickable row. `selected` gives it the highlight colour.
pub fn row<'a>(
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
    subject: Subject,
    studio: bool,
    character: Option<String>,
    character_dirty: bool,
    weapon: Option<String>,
    weapon_dirty: bool,
    prop: Option<String>,
    prop_dirty: bool,
    confirm: Option<ConfirmAction>,
    status: String,
}

/// A row of buttons side by side.
fn button_strip<'a>(parent: &'a mut RelatedSpawnerCommands<ChildOf>) -> EntityCommands<'a> {
    parent.spawn(Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(2.0),
        ..Default::default()
    })
}

fn doc_line(name: Option<&str>, kind: &str, dirty: bool, lead: bool) -> (String, Color) {
    let marker = if lead { ">" } else { " " };
    match name {
        None => (
            format!("{marker}{kind}: (none)"),
            Color::srgb(0.55, 0.6, 0.65),
        ),
        Some(name) if dirty => (
            format!("{marker}{kind}: {name} *  (unsaved)"),
            Color::srgb(1.0, 0.8, 0.45),
        ),
        Some(name) if lead => (
            format!("{marker}{kind}: {name}"),
            Color::srgb(0.85, 1.0, 0.9),
        ),
        Some(name) => (
            format!("{marker}{kind}: {name}"),
            Color::srgb(0.6, 0.75, 0.7),
        ),
    }
}

/// The doc bar: what is open (the subject marked `>`), whether it has unsaved edits, the
/// Studio / Live switch, Save / Revert / Menu, and the question asked before anything
/// would throw edits away.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn rebuild_doc_bar(
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    overlays: Res<EditorOverlays>,
    bar: Res<DocBar>,
    mut commands: Commands,
    container_q: Query<Entity, With<DocBarContainer>>,
    mut last: Local<Option<DocBarView>>,
) {
    let changed = doc.is_changed()
        || weapon_doc.is_changed()
        || prop_doc.is_changed()
        || focus.is_changed()
        || overlays.is_changed()
        || bar.is_changed();
    if last.is_some() && !changed {
        return;
    }
    let Ok(container) = container_q.single() else {
        return;
    };
    let view = DocBarView {
        subject: focus.subject,
        studio: overlays.orbit_camera,
        character: doc.name().map(str::to_string),
        character_dirty: doc.is_dirty(),
        weapon: weapon_doc.name().map(str::to_string),
        weapon_dirty: weapon_doc.is_dirty(),
        prop: prop_doc.name().map(str::to_string),
        prop_dirty: prop_doc.is_dirty(),
        confirm: bar.confirm.clone(),
        status: bar.status.clone(),
    };
    if last.as_ref() == Some(&view) {
        return;
    }

    commands.entity(container).despawn_related::<Children>();
    let any_dirty = view.character_dirty || view.weapon_dirty || view.prop_dirty;
    commands.entity(container).with_children(|parent| {
        button_strip(parent).with_children(|strip| {
            let mut lines = vec![
                (
                    view.character.as_deref(),
                    "character",
                    view.character_dirty,
                    view.subject == Subject::Character,
                ),
                (
                    view.weapon.as_deref(),
                    "weapon",
                    view.weapon_dirty,
                    view.subject == Subject::Weapon,
                ),
            ];
            if view.subject == Subject::Prop || view.prop_dirty {
                lines.push((
                    view.prop.as_deref(),
                    "model",
                    view.prop_dirty,
                    view.subject == Subject::Prop,
                ));
            }
            for (name, kind, dirty, lead) in lines {
                let (text, color) = doc_line(name, kind, dirty, lead);
                strip.spawn((
                    Text::new(text),
                    TextFont::default().with_font_size(13.0),
                    TextColor(color),
                    Node {
                        margin: UiRect::right(Val::Px(12.0)),
                        ..Default::default()
                    },
                ));
            }

            // Studio / Live. A prop has no Live: there is nothing to drive.
            for (studio, label) in [(true, "Studio"), (false, "Live")] {
                let is_on = view.studio == studio;
                let available = studio || view.subject.has_live();
                let color = if available {
                    Color::srgb(0.85, 0.92, 0.95)
                } else {
                    Color::srgb(0.4, 0.45, 0.5)
                };
                row(strip, label.to_string(), is_on, color).observe(
                    move |_: On<Activate>,
                          mut overlays: ResMut<EditorOverlays>,
                          focus: Res<EditorFocus>| {
                        if !studio && !focus.subject.has_live() {
                            return;
                        }
                        if overlays.orbit_camera != studio {
                            overlays.toggle_orbit_camera();
                        }
                    },
                );
            }

            row(
                strip,
                "Save".to_string(),
                any_dirty,
                Color::srgb(0.7, 0.95, 0.75),
            )
            .observe(|_: On<Activate>, mut docs: Docs| {
                docs.bar.status = docs.save_all();
            });
            row(
                strip,
                "Revert".to_string(),
                false,
                Color::srgb(0.95, 0.8, 0.6),
            )
            .observe(|_: On<Activate>, mut docs: Docs| {
                if !docs.any_dirty() {
                    docs.bar.status = "nothing to revert".to_string();
                    return;
                }
                docs.revert_all();
                docs.bar.status = "reverted to the saved files".to_string();
            });
            row(
                strip,
                "Menu".to_string(),
                false,
                Color::srgb(0.75, 0.85, 0.95),
            )
            .observe(|_: On<Activate>, mut docs: Docs| {
                docs.request(ConfirmAction::Exit);
            });
        });

        if let Some(action) = view.confirm.clone() {
            parent.spawn((
                Text::new(format!(
                    "Unsaved changes. Save them before you {}?",
                    action.describe()
                )),
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
                .observe(|_: On<Activate>, mut docs: Docs| {
                    let Some(action) = docs.bar.confirm.take() else {
                        return;
                    };
                    docs.bar.status = docs.save_all();
                    // A save that failed leaves the edits in place; do not walk away
                    // from them.
                    if docs.would_lose_edits(&action) {
                        return;
                    }
                    docs.perform(action);
                });
                row(
                    strip,
                    "Discard".to_string(),
                    false,
                    Color::srgb(0.95, 0.65, 0.55),
                )
                .observe(|_: On<Activate>, mut docs: Docs| {
                    let Some(action) = docs.bar.confirm.take() else {
                        return;
                    };
                    docs.discard_for(&action);
                    docs.bar.status = "discarded unsaved changes".to_string();
                    docs.perform(action);
                });
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
        (debug.orbit_camera, "studio: orbit camera  [F11]"),
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
                    width: Val::Px(104.0),
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
                              frame: Res<ActiveFrame>,
                              mut docs: SideDocs| {
                            let Some(role) = editor.active_role.clone() else {
                                return;
                            };
                            // Model axes need the live anchor; without one (nothing
                            // spawned yet) the stored axes are all there is.
                            let in_model = frame
                                .anchor_in_model
                                .filter(|_| editor.space == NudgeSpace::Model);
                            let Some(def) = docs.def_mut(editor.side) else {
                                return;
                            };
                            let hardpoint = ensure_role(def, &role);
                            match (in_model, rotation) {
                                (Some((turn, _)), true) => {
                                    nudge_rotation_in_model(hardpoint, turn, axis, delta);
                                }
                                (Some(anchor), false) => {
                                    nudge_translation_in_model(hardpoint, anchor, axis, delta);
                                }
                                (None, true) => nudge_rotation(hardpoint, axis, delta),
                                (None, false) => nudge_translation(hardpoint, axis, delta),
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
    subject: Subject,
    weapon_name: Option<&str>,
) {
    for which in HardpointSide::for_subject(subject).iter().copied() {
        let label = match which {
            HardpointSide::Weapon => {
                format!("{}: {}", which.label(), weapon_name.unwrap_or("(none)"))
            }
            _ => which.label().to_string(),
        };
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
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    mut commands: Commands,
    container_q: Query<Entity, With<HardpointContainer>>,
    players: Query<Entity, With<Player>>,
    props: Query<Entity, With<EditorProp>>,
    children_q: Query<&Children>,
    skinned_q: Query<&bevy::mesh::skinning::SkinnedMesh>,
    names: Query<&Name>,
    new_skins: Query<(), Added<bevy::mesh::skinning::SkinnedMesh>>,
    mut last_subject: Local<Option<(Subject, Option<String>)>>,
) {
    // Opening another model replaces a doc, so the panel has to follow it, opening on the
    // side that fits what was opened: a weapon opens on the weapon's frames. Edits to the
    // open docs redraw too: Revert and Save change what the rows should show.
    let subject_path = match focus.subject {
        Subject::Character => doc.def_path.clone(),
        Subject::Weapon => weapon_doc.def_path.clone(),
        Subject::Prop => prop_doc.def_path.clone(),
    };
    let subject = Some((focus.subject, subject_path));
    if *last_subject != subject {
        *last_subject = subject;
        editor.active_role = None;
        editor.side = HardpointSide::default_for(focus.subject);
        editor.ui_dirty = true;
    }
    // A rig spawns a few frames after its model is opened; the bone list follows it.
    if doc.is_changed() || weapon_doc.is_changed() || prop_doc.is_changed() || !new_skins.is_empty()
    {
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
        HardpointSide::Prop => prop_doc.def.as_ref(),
    };

    let Some(def) = def else {
        let message = match side {
            HardpointSide::Character => "no character open",
            HardpointSide::Weapon => "no weapon - equip one above, or open one in the Library",
            HardpointSide::Prop => "no model open",
        };
        commands.entity(container).with_children(|parent| {
            side_chips(parent, side, focus.subject, weapon_name.as_deref());
            parent.spawn((
                Text::new(message),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.6, 0.5, 0.4)),
            ));
        });
        return;
    };

    let active = editor.active_role.clone();
    let space = editor.space;
    let present: Vec<String> = def.hardpoints.keys().cloned().collect();
    let hardpoint = active
        .as_ref()
        .and_then(|role| def.hardpoints.get(role))
        .cloned();

    // Bone names of the rig on screen, so the anchor picker offers what actually exists.
    // Weapon frames are model-origin relative, so the weapon side has no picker.
    let rig = match side {
        HardpointSide::Character => players.iter().next(),
        HardpointSide::Prop => props.iter().next(),
        HardpointSide::Weapon => None,
    };
    let bones: Vec<String> = rig.map_or_default(|root| {
        let joints = crate::assets::gizmos::joints_under(root, &children_q, &skinned_q);
        let mut names: Vec<String> = crate::assets::gizmos::bone_map(&joints, &names)
            .into_keys()
            .collect();
        names.sort();
        names
    });

    commands.entity(container).with_children(|parent| {
        side_chips(parent, side, focus.subject, weapon_name.as_deref());

        // Role chips. A role the def lacks is offered as "+ add"; clicking it creates the
        // hardpoint and selects it, which brings up the nudge rows and anchor picker.
        let hand_guess = guess_hand_bone(&bones);
        for role in side.roles().iter().copied() {
            let exists = present.iter().any(|r| r == role);
            let is_active = active.as_deref() == Some(role);
            let (label, color) = match (is_active, exists) {
                (true, _) => (format!("* {role}"), Color::srgb(0.85, 0.92, 0.85)),
                (false, true) => (format!("  {role}"), Color::srgb(0.85, 0.92, 0.85)),
                (false, false) => (format!("+ add {role}"), Color::srgb(0.6, 0.75, 0.7)),
            };
            let clicked = role.to_string();
            let fallback = hand_guess.clone();
            row(parent, label, is_active, color).observe(
                move |_: On<Activate>, mut editor: ResMut<HardpointEditor>, mut docs: SideDocs| {
                    if !exists {
                        // Weapon frames are model-origin relative: no bone to guess.
                        let side = editor.side;
                        let fallback = fallback.clone().filter(|_| side.has_bones());
                        if let Some(def) = docs.def_mut(side) {
                            add_role(def, &clicked, fallback);
                        }
                    }
                    editor.select_role(&clicked);
                },
            );
        }

        if present.is_empty() {
            parent.spawn((
                Text::new("no hardpoints yet - click '+ add grip' to place one"),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.7, 0.75, 0.6)),
            ));
        }

        // First guesses read off the model file (`suggest_hardpoints`): every role this
        // side lacks, or the selected one again. A starting point for the nudge rows.
        if side
            .roles()
            .iter()
            .any(|role| !present.iter().any(|r| r == role))
        {
            row(
                parent,
                "suggest the missing frames".to_string(),
                false,
                SUGGEST_COLOR,
            )
            .observe(
                |_: On<Activate>, mut editor: ResMut<HardpointEditor>, mut docs: SideDocs| {
                    let side = editor.side;
                    let Some(def) = docs.def_mut(side) else {
                        return;
                    };
                    let Some(shape) = ModelShape::read(def, side) else {
                        return;
                    };
                    for role in side.roles() {
                        if !def.hardpoints.contains_key(*role)
                            && let Some(frame) = shape.frame(role)
                        {
                            def.hardpoints.insert((*role).to_string(), frame);
                        }
                    }
                    editor.touch();
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

        // Which way each row goes. A weapon's own axes are its model axes, so it has no
        // switch; on a bone-anchored frame "model" and "anchor" differ, often wildly.
        let (moves, turns) = match (side, space) {
            (HardpointSide::Weapon, _) => (
                ["move X (barrel)", "move Y (up)", "move Z (side)"],
                ["turn X (roll)", "turn Y (yaw)", "turn Z (pitch)"],
            ),
            (HardpointSide::Character, NudgeSpace::Model) => (
                ["move X (left)", "move Y (up)", "move Z (fwd)"],
                ["turn X (pitch)", "turn Y (yaw)", "turn Z (roll)"],
            ),
            _ => (
                ["move X", "move Y", "move Z"],
                ["turn X", "turn Y", "turn Z"],
            ),
        };
        if side.has_bones() {
            let label = match space {
                NudgeSpace::Model => "moves along: the model's axes",
                NudgeSpace::Anchor => "moves along: the anchor bone's axes",
            };
            row(parent, label.to_string(), false, SUGGEST_COLOR).observe(
                |_: On<Activate>, mut editor: ResMut<HardpointEditor>| {
                    editor.space = editor.space.toggled();
                    editor.touch();
                },
            );
            parent.spawn((
                Text::new("arrows on the frame: red X, green Y, blue Z"),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.55, 0.7, 0.8)),
            ));
        }

        for (axis, name) in moves.into_iter().enumerate() {
            nudge_row(
                parent,
                name.to_string(),
                axis,
                COARSE_TRANSLATION,
                FINE_TRANSLATION,
                false,
            );
        }
        for (axis, name) in turns.into_iter().enumerate() {
            nudge_row(
                parent,
                name.to_string(),
                axis,
                COARSE_ROTATION,
                FINE_ROTATION,
                true,
            );
        }

        let suggested = role.clone();
        row(
            parent,
            "suggest this frame (replaces it)".to_string(),
            false,
            SUGGEST_COLOR,
        )
        .observe(
            move |_: On<Activate>, mut editor: ResMut<HardpointEditor>, mut docs: SideDocs| {
                let side = editor.side;
                let Some(def) = docs.def_mut(side) else {
                    return;
                };
                if let Some(frame) =
                    ModelShape::read(def, side).and_then(|shape| shape.frame(&suggested))
                {
                    def.hardpoints.insert(suggested.clone(), frame);
                }
                editor.touch();
            },
        );

        let removed = role.clone();
        row(
            parent,
            "remove this hardpoint".to_string(),
            false,
            Color::srgb(0.95, 0.7, 0.6),
        )
        .observe(
            move |_: On<Activate>, mut editor: ResMut<HardpointEditor>, mut docs: SideDocs| {
                if let Some(def) = docs.def_mut(editor.side) {
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
                move |_: On<Activate>, mut editor: ResMut<HardpointEditor>, mut docs: SideDocs| {
                    if let Some(def) = docs.def_mut(editor.side) {
                        ensure_role(def, &for_role).anchor = Some(chosen.clone());
                    }
                    editor.touch();
                },
            );
        }
    });
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
    focus: Res<EditorFocus>,
    mut commands: Commands,
    container_q: Query<Entity, With<WeaponListContainer>>,
    mut last: Local<Option<(Option<String>, Option<String>, Subject)>>,
) {
    let equipped = doc
        .def
        .as_ref()
        .and_then(crate::character_editor::doc::weapon_def_path);
    // Follows a model swap (which changes what is equipped), a weapon opened from the
    // library (held without being equipped), and the list itself.
    let key = Some((equipped.clone(), focus.held_weapon.clone(), focus.subject));
    let changed = *last != key;
    if !models.weapon_list_dirty && !changed {
        return;
    }
    let Ok(container) = container_q.single() else {
        return;
    };
    *last = key;
    commands.entity(container).despawn_related::<Children>();

    let weapons = models.weapon_defs.clone();
    let held = focus.held_weapon.clone();
    let is_character = doc.def.as_ref().is_some_and(|def| {
        matches!(
            def.model_type,
            crate::assets::asset_definition::ModelType::Player(_)
        )
    });
    models.weapon_list_dirty = false;

    commands.entity(container).with_children(|parent| {
        let note = if focus.subject == Subject::Prop {
            Some("a model on its own holds no weapon")
        } else if !is_character {
            Some("no character open")
        } else {
            None
        };
        if let Some(note) = note {
            parent.spawn((
                Text::new(note),
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
        .observe(|_: On<Activate>, mut docs: Docs| {
            docs.request(ConfirmAction::EquipWeapon(None));
        });

        for weapon_path in weapons {
            let is_equipped = equipped.as_deref() == Some(weapon_path.as_str());
            let is_held = held.as_deref() == Some(weapon_path.as_str());
            // `*` is what the character carries in its def; "(held)" is a weapon opened
            // from the library and only borrowed — equipping it makes it the character's.
            let label = format!(
                "{} {}{}",
                if is_equipped { "*" } else { " " },
                def_stem(&weapon_path),
                if is_held { "  (held)" } else { "" }
            );
            let chosen = weapon_path.clone();
            row(parent, label, is_equipped, Color::srgb(0.9, 0.88, 0.75)).observe(
                move |_: On<Activate>, mut docs: Docs| {
                    docs.request(ConfirmAction::EquipWeapon(Some(chosen.clone())));
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
