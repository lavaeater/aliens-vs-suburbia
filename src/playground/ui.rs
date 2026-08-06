//! The playground's two-pane shell: a tweaking panel on the left, the live game on the
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

use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::Player;
use crate::playground::debug::{toggle_physics_gizmos, PlaygroundDebug};
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::playground::animation::{
    bind, resolution_label, tag_paths, unbind, AnimationEditor, PLAYABLE_KEYS,
};
use crate::playground::hardpoints::{
    ensure_role, nudge_rotation, nudge_translation, save_player_def, save_weapon_def,
    HardpointEditor, HardpointSide, PlaygroundWeaponDef, COARSE_ROTATION, COARSE_TRANSLATION,
    FINE_ROTATION, FINE_TRANSLATION,
};
use crate::playground::models::{def_stem, PlaygroundModels};
use crate::playground::state::PlaygroundSession;
use crate::ui::spawn_ui::StateMarker;

/// The right-hand pane. Its computed rectangle drives the game camera's viewport.
#[derive(Component)]
pub struct PlaygroundViewportPane;

/// Container for the tweak controls, so later stages can refill it without rebuilding
/// the whole screen.
#[derive(Component)]
pub struct PlaygroundPanel;

/// Holds one clickable row per imported player def.
#[derive(Component)]
pub struct ModelListContainer;

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

/// Holds the hardpoint role chips, nudge rows and bone picker.
#[derive(Component)]
pub struct HardpointContainer;

/// Holds the animation key list and tag-binding picker.
#[derive(Component)]
pub struct AnimationContainer;

pub fn spawn_playground_ui(commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands, Some(theme.clone()));

    ui.component::<StateMarker>()
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_row();

    let t = theme.text.clone();
    let hint = TextTheme { label_size: 11.0, label_color: Color::srgb(0.45, 0.6, 0.5), ..t.clone() };
    let section = TextTheme { label_size: 12.0, label_color: Color::srgb(0.55, 0.8, 0.65), ..t.clone() };

    ui.with_child(|left| {
        left.modify_node(|mut n| {
            n.width = Val::Percent(26.0);
            n.min_width = Val::Px(260.0);
            n.max_width = Val::Px(520.0);
            n.height = Val::Percent(100.0);
        })
        .display_flex()
        .flex_column()
        .gap_px(6.0)
        .padding_all_px(10.0)
        .bg_color(Color::srgba(0.04, 0.07, 0.10, 0.97));

        left.with_child(|c| { c.insert_bundle(lava_ui_builder::header("Playground", &t)); });
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

        // ── Debug overlays ───────────────────────────────────────────────
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("DEBUG VIEW", &section)); });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .insert(DebugTogglesContainer)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch);
        });

        // ── Imported models ──────────────────────────────────────────────
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("MODEL", &section)); });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .insert(ModelListContainer).insert(ScrollPosition::default())
             .modify_node(|mut n| {
                 n.align_self = AlignSelf::Stretch;
                 n.max_height = Val::Px(220.0);
                 n.overflow = Overflow::scroll_y();
             });
        });

        // ── Import browser ───────────────────────────────────────────────
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("IMPORT", &section)); });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("", &TextTheme {
                label_size: 10.0, label_color: Color::srgb(0.5, 0.7, 0.9), ..t.clone()
            }))
            .insert(ImportPathLabel)
            .modify_node(|mut n| n.overflow = Overflow::clip());
        });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .insert(ImportBrowserContainer).insert(ScrollPosition::default())
             .modify_node(|mut n| {
                 n.align_self = AlignSelf::Stretch;
                 n.flex_grow = 1.0;
                 n.min_height = Val::Px(120.0);
                 n.overflow = Overflow::scroll_y();
             });
        });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label("", &hint)).insert(ImportStatusLabel);
        });

        // ── Hardpoints ───────────────────────────────────────────────────
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("HARDPOINTS", &section)); });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .insert(HardpointContainer).insert(ScrollPosition::default())
             .modify_node(|mut n| {
                 n.align_self = AlignSelf::Stretch;
                 n.max_height = Val::Px(260.0);
                 n.overflow = Overflow::scroll_y();
             });
        });

        // ── Animation ────────────────────────────────────────────────────
        left.with_child(|c| { c.insert_bundle(lava_ui_builder::label("ANIMATION", &section)); });
        left.with_child(|c| {
            c.insert_bundle(lava_ui_builder::label(
                "click a key to play it   [F1] camera  [F2] model",
                &hint,
            ));
        });
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(2.0)
             .insert(AnimationContainer).insert(ScrollPosition::default())
             .modify_node(|mut n| {
                 n.align_self = AlignSelf::Stretch;
                 n.max_height = Val::Px(260.0);
                 n.overflow = Overflow::scroll_y();
             });
        });

        // Spare room for whatever comes next.
        left.with_child(|c| {
            c.display_flex().flex_column().gap_px(6.0).insert(PlaygroundPanel)
             .modify_node(|mut n| n.align_self = AlignSelf::Stretch);
        });

        left.add_button_observe(
            "Back to Menu",
            |b| { b.size_px(160.0, 40.0).font_size(16.0); },
            |_: On<Activate>, mut next_state: ResMut<NextState<GameState>>| {
                next_state.set(GameState::Menu);
            },
        );
    });

    // Right pane: empty node, exists only to give the game camera a rectangle.
    ui.with_child(|right| {
        right.modify_node(|mut n| {
            n.flex_grow = 1.0;
            n.height = Val::Percent(100.0);
        })
        .insert(PlaygroundViewportPane);
    });

    ui.build();
}

/// Clip the game camera to the right-hand pane. Runs every frame so the viewport tracks
/// window resizes and any future resizing of the panel.
pub fn sync_playground_viewport(
    panes: Query<(&ComputedNode, &UiGlobalTransform), With<PlaygroundViewportPane>>,
    mut cameras: Query<&mut Camera, With<GameCamera>>,
    windows: Query<&Window>,
) {
    let Ok((node, transform)) = panes.single() else { return };
    let Ok(mut camera) = cameras.single_mut() else { return };
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
pub fn clear_playground_viewport(mut cameras: Query<&mut Camera, With<GameCamera>>) {
    for mut camera in cameras.iter_mut() {
        camera.viewport = None;
    }
}

/// Leaving `InGame` ends the session; without this the resource would leak into the next
/// real match and quietly suppress the HUD and the map.
pub fn end_playground_session(mut commands: Commands) {
    commands.remove_resource::<PlaygroundSession>();
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

/// Rebuild the debug toggle rows. Cheap: three rows, only on change.
pub fn rebuild_debug_toggles(
    mut debug: ResMut<PlaygroundDebug>,
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
    ];
    commands.entity(container).with_children(|parent| {
        for (index, (on, label)) in rows.into_iter().enumerate() {
            let text = format!("[{}] {label}", if on { "x" } else { " " });
            row(parent, text, on, Color::srgb(0.85, 0.9, 0.95)).observe(
                move |_: On<Activate>,
                      mut debug: ResMut<PlaygroundDebug>,
                      mut store: ResMut<GizmoConfigStore>| {
                    match index {
                        0 => {
                            toggle_physics_gizmos(&mut store);
                            // `sync_physics_toggle` picks the new value up and marks the
                            // panel dirty, so there is one source of truth.
                        }
                        1 => debug.toggle_skeleton(),
                        _ => debug.toggle_hardpoints(),
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
        .spawn((
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                ..Default::default()
            },
        ))
        .with_children(|strip| {
            strip.spawn((
                Node { width: Val::Px(86.0), ..Default::default() },
                Text::new(label),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
            ));
            for (text, delta) in [
                ("<<", -coarse), ("<", -fine), (">", fine), (">>", coarse),
            ] {
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
                              mut player_def: ResMut<PlayerAssetDef>,
                              mut weapon_def: ResMut<PlaygroundWeaponDef>| {
                            let Some(role) = editor.active_role.clone() else { return };
                            let side = editor.side;
                            // Only reach for the side being edited: `as_mut()` on the other
                            // would mark it changed and re-push an unedited def at the live
                            // weapon every click.
                            let def = match side {
                                HardpointSide::Character => player_def.0.as_mut(),
                                HardpointSide::Weapon => weapon_def.def.as_mut(),
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
        (HardpointSide::Character, HardpointSide::Character.label().to_string()),
        (HardpointSide::Weapon, weapon_label),
    ] {
        let is_active = which == side;
        let marker = if is_active { "*" } else { " " };
        row(parent, format!("{marker} {label}"), is_active, Color::srgb(0.9, 0.85, 0.7)).observe(
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
    player_def: Res<PlayerAssetDef>,
    weapon_def: Res<PlaygroundWeaponDef>,
    mut commands: Commands,
    container_q: Query<Entity, With<HardpointContainer>>,
    players: Query<Entity, With<Player>>,
    children_q: Query<&Children>,
    skinned_q: Query<&bevy::mesh::skinning::SkinnedMesh>,
    names: Query<&Name>,
    mut last_def: Local<Option<String>>,
) {
    // A model swap replaces the def, so the panel has to follow it. The weapon def is
    // reloaded by `sync_weapon_def` in the same breath, so both sides reset together.
    let current = player_def.0.as_ref().map(|d| d.model_path.clone());
    if *last_def != current {
        *last_def = current;
        editor.active_role = None;
        editor.side = HardpointSide::Character;
        editor.character_dirty = false;
        editor.weapon_dirty = false;
        editor.status.clear();
        editor.ui_dirty = true;
    }
    if !editor.ui_dirty {
        return;
    }
    editor.ui_dirty = false;

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let side = editor.side;
    let weapon_name = weapon_def.name().map(str::to_string);
    let def = match side {
        HardpointSide::Character => player_def.0.as_ref(),
        HardpointSide::Weapon => weapon_def.def.as_ref(),
    };

    let Some(def) = def else {
        let message = match side {
            HardpointSide::Character => "no player def loaded",
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
    let hardpoint = active.as_ref().and_then(|role| def.hardpoints.get(role)).cloned();
    let status = editor.status.clone();
    let dirty = editor.dirty(side);
    // Which file the Save button writes. Worth spelling out: the two sides write different
    // files, and the weapon's is reached by an explicit path rather than its model stem.
    let save_target = match side {
        HardpointSide::Character => crate::assets::asset_definition::AssetDefinition::def_path(
            &def.model_path,
        )
        .display()
        .to_string(),
        HardpointSide::Weapon => weapon_def.def_path.clone().unwrap_or_default(),
    };

    // Bone names of the live rig, so the anchor picker offers what actually exists.
    // Weapon frames are model-origin relative, so the picker is character-side only.
    let bones: Vec<String> = players
        .iter()
        .next()
        .filter(|_| side == HardpointSide::Character)
        .map(|player| {
            let joints = crate::assets::gizmos::joints_under(player, &children_q, &skinned_q);
            let mut names: Vec<String> = crate::assets::gizmos::bone_map(&joints, &names)
                .into_keys()
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default();

    commands.entity(container).with_children(|parent| {
        side_chips(parent, side, weapon_name.as_deref());

        // Role chips.
        for role in side.roles().iter().copied() {
            let exists = present.iter().any(|r| r == role);
            let is_active = active.as_deref() == Some(role);
            let marker = if is_active { "*" } else if exists { "-" } else { "+" };
            let clicked = role.to_string();
            row(parent, format!("{marker} {role}"), is_active, Color::srgb(0.85, 0.92, 0.85))
                .observe(move |_: On<Activate>, mut editor: ResMut<HardpointEditor>| {
                    editor.select_role(&clicked);
                });
        }

        let Some(hardpoint) = hardpoint else { return };
        let role = active.clone().unwrap_or_default();

        parent.spawn((
            Text::new(format!(
                "anchor: {}\nT {:.3} {:.3} {:.3}\nR {:.1} {:.1} {:.1}",
                hardpoint.anchor.as_deref().unwrap_or("(model origin)"),
                hardpoint.translation[0], hardpoint.translation[1], hardpoint.translation[2],
                hardpoint.rotation_euler_deg[0],
                hardpoint.rotation_euler_deg[1],
                hardpoint.rotation_euler_deg[2],
            )),
            TextFont::default().with_font_size(10.0),
            TextColor(Color::srgb(0.6, 0.8, 0.95)),
        ));

        for (axis, name) in ["move X", "move Y", "move Z"].into_iter().enumerate() {
            nudge_row(parent, name.to_string(), axis, COARSE_TRANSLATION, FINE_TRANSLATION, false);
        }
        for (axis, name) in ["turn X", "turn Y", "turn Z"].into_iter().enumerate() {
            nudge_row(parent, name.to_string(), axis, COARSE_ROTATION, FINE_ROTATION, true);
        }

        let removed = role.clone();
        row(parent, "remove this hardpoint".to_string(), false, Color::srgb(0.95, 0.7, 0.6))
            .observe(move |_: On<Activate>,
                           mut editor: ResMut<HardpointEditor>,
                           mut player_def: ResMut<PlayerAssetDef>,
                           mut weapon_def: ResMut<PlaygroundWeaponDef>| {
                let def = match editor.side {
                    HardpointSide::Character => player_def.0.as_mut(),
                    HardpointSide::Weapon => weapon_def.def.as_mut(),
                };
                if let Some(def) = def {
                    def.hardpoints.remove(&removed);
                }
                editor.active_role = None;
                editor.touch();
            });

        let save_label = match (side, dirty) {
            (HardpointSide::Character, true) => "SAVE character def (unsaved edits)",
            (HardpointSide::Character, false) => "SAVE character def",
            (HardpointSide::Weapon, true) => "SAVE weapon def (unsaved edits)",
            (HardpointSide::Weapon, false) => "SAVE weapon def",
        };
        row(parent, save_label.to_string(), dirty, Color::srgb(0.7, 0.95, 0.75))
            .observe(move |_: On<Activate>,
                      mut editor: ResMut<HardpointEditor>,
                      player_def: Res<PlayerAssetDef>,
                      weapon_def: Res<PlaygroundWeaponDef>| {
                let side = editor.side;
                let saved = match side {
                    HardpointSide::Character => {
                        player_def.0.as_ref().map(save_player_def)
                    }
                    HardpointSide::Weapon => weapon_def
                        .def
                        .as_ref()
                        .zip(weapon_def.def_path.as_deref())
                        .map(|(def, path)| save_weapon_def(def, path)),
                };
                if let Some(status) = saved {
                    editor.status = status;
                    editor.set_dirty(side, false);
                    editor.ui_dirty = true;
                }
            });

        if !save_target.is_empty() {
            parent.spawn((
                Text::new(format!("-> {save_target}")),
                TextFont::default().with_font_size(9.0),
                TextColor(Color::srgb(0.45, 0.55, 0.6)),
            ));
        }

        if !status.is_empty() {
            parent.spawn((
                Text::new(status),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.6, 0.85, 0.6)),
            ));
        }

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
                let for_role = role.clone();
                row(
                    parent,
                    "clear anchor (use model origin)".to_string(),
                    false,
                    Color::srgb(0.95, 0.85, 0.6),
                )
                .observe(move |_: On<Activate>,
                               mut editor: ResMut<HardpointEditor>,
                               mut weapon_def: ResMut<PlaygroundWeaponDef>| {
                    if let Some(def) = weapon_def.def.as_mut() {
                        ensure_role(def, &for_role).anchor = None;
                    }
                    editor.touch();
                });
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
                      mut player_def: ResMut<PlayerAssetDef>| {
                    if let Some(def) = player_def.0.as_mut() {
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
    player_def: Res<PlayerAssetDef>,
    mut commands: Commands,
    container_q: Query<Entity, With<AnimationContainer>>,
    mut last_def: Local<Option<String>>,
) {
    // Follow model swaps, and redraw when a binding edit changes what keys resolve to.
    let current = player_def.0.as_ref().map(|d| d.model_path.clone());
    if *last_def != current {
        *last_def = current;
        editor.selected_key = None;
        editor.status.clear();
        editor.ui_dirty = true;
    }
    if !editor.ui_dirty {
        return;
    }
    editor.ui_dirty = false;

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let Some(def) = player_def.0.as_ref() else {
        commands.entity(container).with_children(|parent| {
            parent.spawn((
                Text::new("no player def loaded"),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(0.6, 0.5, 0.4)),
            ));
        });
        return;
    };

    let selected = editor.selected_key;
    let labels: Vec<(AnimationKey, String, String)> = PLAYABLE_KEYS
        .iter()
        .map(|&key| (key, key.default_search().to_string(), resolution_label(def, key)))
        .collect();
    let tags = tag_paths(def);
    let bound_tag = selected.and_then(|key| {
        def.animation_bindings.get(key.default_search()).cloned()
    });
    let status = editor.status.clone();
    let dirty = editor.dirty;

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

        row(parent, "back to idle".to_string(), false, Color::srgb(0.7, 0.8, 0.9)).observe(
            |_: On<Activate>,
             editor: Res<AnimationEditor>,
             mut anim_mw: MessageWriter<AnimationEvent>,
             players: Query<Entity, With<Player>>| {
                let Some(key) = editor.selected_key else { return };
                for player in players.iter() {
                    anim_mw.write(AnimationEvent(AnimationEventType::LeaveAnimState, player, key));
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
                Text::new("this def tags no clips - tag them in the asset browser"),
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
                      mut player_def: ResMut<PlayerAssetDef>| {
                    if let Some(def) = player_def.0.as_mut() {
                        bind(def, key, &chosen);
                    }
                    editor.touch();
                },
            );
        }

        row(parent, "clear binding".to_string(), false, Color::srgb(0.95, 0.8, 0.6)).observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  mut player_def: ResMut<PlayerAssetDef>| {
                if let Some(def) = player_def.0.as_mut() {
                    unbind(def, key);
                }
                editor.touch();
            },
        );

        let save_label = if dirty { "SAVE def (unsaved edits)" } else { "SAVE def" };
        row(parent, save_label.to_string(), dirty, Color::srgb(0.7, 0.95, 0.75)).observe(
            |_: On<Activate>,
             mut editor: ResMut<AnimationEditor>,
             player_def: Res<PlayerAssetDef>| {
                if let Some(def) = player_def.0.as_ref() {
                    editor.status = save_player_def(def);
                    editor.dirty = false;
                    editor.ui_dirty = true;
                }
            },
        );

        if !status.is_empty() {
            parent.spawn((
                Text::new(status),
                TextFont::default().with_font_size(10.0),
                TextColor(Color::srgb(0.6, 0.85, 0.6)),
            ));
        }
    });
}

/// A binding edit changes what every key resolves to, so the list has to be redrawn.
pub fn refresh_animation_panel_on_def_change(
    player_def: Res<PlayerAssetDef>,
    mut editor: ResMut<AnimationEditor>,
) {
    if player_def.is_changed() {
        editor.ui_dirty = true;
    }
}

pub fn rebuild_model_list(
    mut models: ResMut<PlaygroundModels>,
    mut commands: Commands,
    container_q: Query<Entity, With<ModelListContainer>>,
) {
    if !models.list_dirty {
        return;
    }
    models.list_dirty = false;

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let defs = models.defs.clone();
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
            let label = format!("{} {}", if is_selected { "*" } else { " " }, def_stem(&def_path));
            let clicked = def_path.clone();
            row(parent, label, is_selected, Color::srgb(0.85, 0.92, 0.85))
                .observe(move |_: On<Activate>, mut m: ResMut<PlaygroundModels>| {
                    m.select(&clicked);
                });
        }
    });
}

pub fn rebuild_import_browser(
    mut models: ResMut<PlaygroundModels>,
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
        **text = models.status.clone();
    }

    let Ok(container) = container_q.single() else { return };
    commands.entity(container).despawn_related::<Children>();

    let at_root = models.browse_folder.is_empty();
    let folders = models.folders.clone();
    let files = models.files.clone();
    commands.entity(container).with_children(|parent| {
        if !at_root {
            row(parent, "[..]".to_string(), false, Color::srgb(0.75, 0.88, 1.0))
                .observe(|_: On<Activate>, mut m: ResMut<PlaygroundModels>| m.leave_folder());
        }
        for name in folders {
            let entered = name.clone();
            row(parent, format!("[dir] {name}"), false, Color::srgb(0.75, 0.88, 1.0))
                .observe(move |_: On<Activate>, mut m: ResMut<PlaygroundModels>| {
                    m.enter_folder(&entered);
                });
        }
        for file in files {
            let imported = file.clone();
            let name = file.rsplit('/').next().unwrap_or(&file).to_string();
            row(parent, format!("+ {name}"), false, Color::srgb(0.9, 0.85, 0.7))
                .observe(move |_: On<Activate>, mut m: ResMut<PlaygroundModels>| {
                    m.import(&imported);
                    m.browser_dirty = true;
                });
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
        assert_eq!(v.physical_size.to_array(), [1200, 900], "clamped to the window edge");
    }

    #[test]
    fn a_zero_sized_pane_is_refused_rather_than_sent_to_the_gpu() {
        assert!(pane_viewport(0.0, 0.0, 0.0, 0.0, 1600, 900).is_none(), "first frame");
        assert!(pane_viewport(1600.0, 0.0, 100.0, 900.0, 1600, 900).is_none(), "fully off-screen");
    }

    #[test]
    fn a_negative_origin_is_pulled_back_to_the_window_corner() {
        let v = pane_viewport(-20.0, -10.0, 500.0, 400.0, 1600, 900).expect("valid rect");
        assert_eq!(v.physical_position.to_array(), [0, 0]);
    }
}
