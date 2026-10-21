//! The editor's frame: top bar, tabbed left pane, the game viewport with its toolbar, and
//! a status bar along the bottom.
//!
//! ```text
//! +--------------------------------------------------------------+
//! | Character Editor   character: amy *   weapon: Pistol  [Save] |  top bar (doc bar)
//! +-------------------+------------------------------------------+
//! | Library Hardpts.. |  [x] skeleton  [ ] hardpoints  ...       |  toolbar, over the game
//! |-------------------|                                          |
//! |  the active tab,  |          the game camera, clipped        |
//! |  scrolling on its |          to this pane                    |
//! |  own              |                                          |
//! +-------------------+------------------------------------------+
//! | grip on mixamorigRightHand    [F11] orbit  [F12] bind pose   |  status bar
//! +--------------------------------------------------------------+
//! ```
//!
//! **Tabs, not folding sections.** One tab's contents fill the pane at a time and the tab
//! row never moves, so nothing can be pushed out of reach — which is what the old single
//! column of foldable sections kept doing. Each tab is its own scroll container, and the
//! lists inside it are plain columns: no list scrolls inside a pane that also scrolls.
//!
//! This module only builds the frame and switches tabs. What goes *in* each container is
//! rebuilt by the panel systems in `ui.rs`, keyed on the marker components declared here.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::character_editor::debug::EditorOverlays;
use crate::character_editor::doc::{EditorDoc, EditorFocus, PropDoc, Subject, WeaponDoc};
use crate::character_editor::hardpoints::{HardpointEditor, HardpointSide};
use crate::character_editor::library::{
    LibraryChips, LibraryImportBar, LibraryList, LibrarySearch, LibraryStatus,
};
use crate::character_editor::model_tab::ModelContainer;
use crate::character_editor::props_tab::PropsContainer;
use crate::character_editor::text_field::text_field;
use crate::character_editor::ui::{
    AnimationContainer, DebugTogglesContainer, DocBarContainer, EditorViewportPane,
    HardpointContainer, WeaponListContainer, row,
};
use crate::ui::spawn_ui::StateMarker;

/// The tabs, in strip order.
///
/// `Library` is interim: step 6 of `docs/character-editor.md` turns it into a drawer
/// opened from the top bar. Model, Attach and Props arrive with the panels they hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorTab {
    #[default]
    Library,
    Model,
    Animation,
    Hardpoints,
    Props,
}

impl EditorTab {
    pub const ALL: [Self; 5] = [
        Self::Library,
        Self::Model,
        Self::Animation,
        Self::Hardpoints,
        Self::Props,
    ];

    /// The tab a label names, for reading it back out of the prefs.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.label() == label)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Model => "Model",
            Self::Animation => "Animation",
            Self::Hardpoints => "Hardpoints",
            Self::Props => "Props",
        }
    }
}

/// The tab showing in the left pane.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTab(pub EditorTab);

/// The row of tab buttons.
#[derive(Component)]
pub struct TabStrip;

/// One tab's scrolling body. Only the active tab's is displayed.
#[derive(Component, Clone, Copy)]
pub struct TabBody(pub EditorTab);

/// The status bar's text.
#[derive(Component)]
pub struct StatusBarText;

const BAR_BG: Color = Color::srgba(0.03, 0.05, 0.08, 0.98);
const PANE_BG: Color = Color::srgba(0.04, 0.07, 0.10, 0.97);
const HEADING: Color = Color::srgb(0.55, 0.8, 0.65);
const HINT: Color = Color::srgb(0.45, 0.6, 0.5);

/// The keys worth knowing, always on show at the bottom.
const KEY_HINTS: &str = "WASD / left stick move   mouse / right stick aim   \
     [F11] orbit camera   [F12] bind pose   [F3] physics   [F7] torso twist   \
     [F1] camera   [F2] model   [F10] gait";

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont::default().with_font_size(size),
        TextColor(color),
    )
}

/// A heading inside a tab.
fn heading(value: &str) -> impl Bundle {
    (
        text(value, 12.0, HEADING),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..Default::default()
        },
    )
}

/// A column that holds a rebuilt list. Never shrinks: the tab scrolls, the list does not.
fn list_column() -> Node {
    Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        align_self: AlignSelf::Stretch,
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// A tab's body: fills the rest of the pane and scrolls on its own.
fn tab_body(tab: EditorTab) -> impl Bundle {
    (
        TabBody(tab),
        Node {
            display: Display::None,
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            // Without a zero minimum a flex child grows to fit its content and never
            // scrolls.
            min_height: Val::Px(0.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(8.0)),
            overflow: Overflow::scroll_y(),
            ..Default::default()
        },
        ScrollPosition::default(),
    )
}

pub fn spawn_editor_ui(mut commands: Commands) {
    commands
        .spawn((
            StateMarker,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
        ))
        .with_children(|root| {
            // ── Top bar ──────────────────────────────────────────────────────
            root.spawn((
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(16.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                BackgroundColor(BAR_BG),
            ))
            .with_children(|bar| {
                bar.spawn(text("Character Editor", 18.0, Color::srgb(0.85, 0.95, 0.9)));
                bar.spawn((
                    DocBarContainer,
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        flex_grow: 1.0,
                        ..Default::default()
                    },
                ));
            });

            // ── Middle: left pane + viewport ─────────────────────────────────
            root.spawn(Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..Default::default()
            })
            .with_children(|middle| {
                middle
                    .spawn((
                        Node {
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            width: Val::Percent(26.0),
                            min_width: Val::Px(280.0),
                            max_width: Val::Px(520.0),
                            ..Default::default()
                        },
                        BackgroundColor(PANE_BG),
                    ))
                    .with_children(|left| {
                        left.spawn((
                            TabStrip,
                            Node {
                                display: Display::Flex,
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(4.0),
                                row_gap: Val::Px(4.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                flex_shrink: 0.0,
                                ..Default::default()
                            },
                        ));
                        spawn_library_tab(left);
                        spawn_single_tab(left, EditorTab::Model, ModelContainer);
                        spawn_animation_tab(left);
                        spawn_hardpoints_tab(left);
                        spawn_single_tab(left, EditorTab::Props, PropsContainer);
                    });

                // The game shows through this pane; its rectangle becomes the camera's
                // viewport. The toolbar floats over its top edge.
                middle
                    .spawn((
                        EditorViewportPane,
                        Node {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                    ))
                    .with_children(|pane| {
                        pane.spawn((
                            DebugTogglesContainer,
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(6.0),
                                left: Val::Px(6.0),
                                right: Val::Px(6.0),
                                display: Display::Flex,
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(4.0),
                                row_gap: Val::Px(4.0),
                                ..Default::default()
                            },
                        ));
                    });
            });

            // ── Status bar ───────────────────────────────────────────────────
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                BackgroundColor(BAR_BG),
            ))
            .with_child((StatusBarText, text(KEY_HINTS, 11.0, HINT)));
        });
}

fn spawn_library_tab(left: &mut ChildSpawnerCommands) {
    left.spawn(tab_body(EditorTab::Library))
        .with_children(|tab| {
            tab.spawn((LibrarySearch, text_field("search models and files...")));
            tab.spawn((
                LibraryChips,
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    flex_shrink: 0.0,
                    ..Default::default()
                },
            ));
            tab.spawn((LibraryImportBar, list_column()));
            tab.spawn((LibraryStatus, text("", 10.0, Color::srgb(0.5, 0.7, 0.9))));
            tab.spawn((LibraryList, list_column()));
        });
}

/// A tab whose whole contents are one rebuilt column.
fn spawn_single_tab(left: &mut ChildSpawnerCommands, tab: EditorTab, marker: impl Component) {
    left.spawn(tab_body(tab)).with_children(|body| {
        body.spawn((marker, list_column()));
    });
}

fn spawn_hardpoints_tab(left: &mut ChildSpawnerCommands) {
    left.spawn(tab_body(EditorTab::Hardpoints))
        .with_children(|tab| {
            tab.spawn(heading("WEAPON"));
            tab.spawn((WeaponListContainer, list_column()));
            tab.spawn(heading("HARDPOINTS"));
            tab.spawn((HardpointContainer, list_column()));
        });
}

fn spawn_animation_tab(left: &mut ChildSpawnerCommands) {
    left.spawn(tab_body(EditorTab::Animation))
        .with_children(|tab| {
            tab.spawn((AnimationContainer, list_column()));
        });
}

/// Show the active tab's body, hide the rest, and redraw the strip so the active button
/// is marked. Runs on a change of tab, and on the first frame the strip exists.
pub fn sync_tabs(
    active: Res<ActiveTab>,
    mut commands: Commands,
    strips: Query<Entity, With<TabStrip>>,
    mut bodies: Query<(&TabBody, &mut Node)>,
    mut built_for: Local<Option<Entity>>,
) {
    let Ok(strip) = strips.single() else {
        return;
    };
    // A new session spawns a new strip, which needs building whatever the resource says.
    if !active.is_changed() && *built_for == Some(strip) {
        return;
    }
    *built_for = Some(strip);

    for (body, mut node) in &mut bodies {
        let wanted = if body.0 == active.0 {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != wanted {
            node.display = wanted;
        }
    }

    commands.entity(strip).despawn_related::<Children>();
    commands.entity(strip).with_children(|strip| {
        for tab in EditorTab::ALL {
            let is_active = tab == active.0;
            row(
                strip,
                tab.label().to_string(),
                is_active,
                Color::srgb(0.85, 0.92, 0.95),
            )
            .observe(move |_: On<Activate>, mut active: ResMut<ActiveTab>| {
                if active.0 != tab {
                    active.0 = tab;
                }
            });
        }
    });
}

/// Remember the tab and Studio/Live for next session. A prop forces Studio, so Studio is
/// only remembered while something with a Live is open — opening an enemy should not
/// make the next session start in Studio.
pub fn remember_tab_and_stage(
    active: Res<ActiveTab>,
    overlays: Res<EditorOverlays>,
    focus: Res<EditorFocus>,
    mut last: Local<Option<(EditorTab, bool)>>,
) {
    let studio = if focus.subject.has_live() {
        overlays.orbit_camera
    } else {
        last.is_some_and(|(_, studio)| studio)
    };
    let now = Some((active.0, studio));
    if last.is_none() {
        // The first frame is what was just loaded; nothing to write.
        *last = now;
        return;
    }
    if *last != now {
        *last = now;
        crate::character_editor::prefs::EditorPrefs::update(|prefs| {
            prefs.last_tab = Some(active.0.label().to_string());
            prefs.studio = studio;
        });
    }
}

/// What the status bar says: what is selected, then the keys.
pub fn status_line(
    character: Option<&str>,
    side: HardpointSide,
    role: Option<&str>,
    anchor: Option<&str>,
    inspecting: bool,
) -> String {
    let mut parts = Vec::new();
    if let Some(name) = character {
        parts.push(name.to_string());
    }
    if let Some(role) = role {
        let on = match side {
            HardpointSide::Weapon => "weapon origin".to_string(),
            HardpointSide::Character | HardpointSide::Prop => {
                anchor.unwrap_or("model origin").to_string()
            }
        };
        parts.push(format!("{} {role} on {on}", side.label()));
    }
    if inspecting {
        parts.push("INSPECTING - character holds still".to_string());
    }
    parts.push(KEY_HINTS.to_string());
    parts.join("   |   ")
}

pub fn update_status_bar(
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    editor: Res<HardpointEditor>,
    overlays: Res<EditorOverlays>,
    mut texts: Query<&mut Text, With<StatusBarText>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let side_doc = match editor.side {
        HardpointSide::Character => &doc.0,
        HardpointSide::Weapon => &weapon_doc.0,
        HardpointSide::Prop => &prop_doc.0,
    };
    let subject_doc = match focus.subject {
        Subject::Character => &doc.0,
        Subject::Weapon => &weapon_doc.0,
        Subject::Prop => &prop_doc.0,
    };
    let role = editor.active_role.as_deref();
    let anchor = role.and_then(|role| {
        side_doc
            .def
            .as_ref()?
            .hardpoints
            .get(role)?
            .anchor
            .as_deref()
    });
    let line = status_line(
        subject_doc.name(),
        editor.side,
        role,
        anchor,
        overlays.inspecting(),
    );
    if text.0 != line {
        text.0 = line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_round_trips_through_its_label() {
        for tab in EditorTab::ALL {
            assert_eq!(EditorTab::from_label(tab.label()), Some(tab));
        }
        assert_eq!(EditorTab::from_label("Gone"), None);
    }

    #[test]
    fn every_tab_is_in_the_strip_once() {
        for tab in EditorTab::ALL {
            assert_eq!(EditorTab::ALL.iter().filter(|t| **t == tab).count(), 1);
        }
    }

    #[test]
    fn the_status_line_names_the_selected_hardpoint_and_its_bone() {
        let line = status_line(
            Some("amy"),
            HardpointSide::Character,
            Some("grip"),
            Some("mixamorigRightHand"),
            false,
        );
        assert!(line.starts_with("amy"), "got {line}");
        assert!(
            line.contains("character grip on mixamorigRightHand"),
            "got {line}"
        );
    }

    #[test]
    fn weapon_frames_are_reported_against_the_weapon_origin() {
        let line = status_line(None, HardpointSide::Weapon, Some("muzzle"), None, false);
        assert!(
            line.contains("weapon muzzle on weapon origin"),
            "got {line}"
        );
    }

    /// Everything on screen goes through Bevy's ASCII-only default font.
    #[test]
    fn the_status_line_is_plain_ascii() {
        assert!(status_line(Some("amy"), HardpointSide::Character, None, None, true).is_ascii());
    }
}
