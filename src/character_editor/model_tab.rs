//! The Model tab: what the file holds, how big the model is, and which parts are hidden.
//!
//! - **Info**: the model file, its meshes, bones and clips (`gltf_info`, off the file).
//! - **Size**: the scale, nudged by factors, and the height measured off the model on
//!   screen. For anything that uses its def's scale in game (enemies, towers, terrain,
//!   items, weapons) a target height sets the scale from that measurement — the asset
//!   browser's "target height", against the model in the arena rather than in a void.
//!   A character is the exception: in game it is sized by the model panel (F2), and its
//!   def's scale is not read, so the tab says so rather than offering a control that does
//!   nothing.
//! - **Hidden nodes**: every mesh node in the file, click to hide or show it. A character
//!   respawns to show the change (`doc::needs_respawn`); a prop updates in place
//!   ([`apply_prop_hidden_nodes`]).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::assets::asset_definition::AssetDefinition;
use crate::assets::gltf_info;
use crate::character_editor::doc::{
    EditorDoc, EditorFocus, PropDoc, SideDocs, Subject, WeaponDoc, subject_doc,
};
use crate::character_editor::prop::{EditorProp, mesh_extents};
use crate::character_editor::ui::row;
use crate::player::components::Player;
use crate::player::systems::equip::EquippedWeapon;

/// The tab's contents.
#[derive(Component)]
pub struct ModelContainer;

/// Measured off the model on screen, for the height readout and "set height".
#[derive(Resource, Debug, Default)]
pub struct ModelMeasure {
    /// World height of the subject's meshes, metres. `None` until it has spawned.
    pub height: Option<f32>,
    /// The scale the measured model was spawned at, so a target height can be turned into
    /// a scale: `scale * target / height`.
    pub at_scale: f32,
}

/// Scale factors offered as buttons, smallest step in the middle.
const SCALE_STEPS: [(&str, f32); 6] = [
    ("x0.5", 0.5),
    ("x0.9", 0.9),
    ("x0.99", 0.99),
    ("x1.01", 1.01),
    ("x1.1", 1.1),
    ("x2", 2.0),
];

/// Heights offered as targets, metres.
const TARGET_HEIGHTS: [f32; 7] = [0.25, 0.5, 1.0, 1.5, 1.8, 2.0, 3.0];

/// Measurements below this are noise from a model still loading.
const MIN_HEIGHT: f32 = 1e-3;

/// The scale that makes a model measured `height` tall at `scale` stand `target` tall.
pub fn scale_for_height(scale: f32, height: f32, target: f32) -> Option<f32> {
    (height > MIN_HEIGHT && target > 0.0).then(|| scale * target / height)
}

/// Whether the game reads this def's scale. Characters are sized by the model panel.
pub const fn scale_is_used(subject: Subject) -> bool {
    !matches!(subject, Subject::Character)
}

/// Measure the subject on screen: the prop, the held weapon, or the character.
pub fn measure_subject(
    focus: Res<EditorFocus>,
    props: Query<(Entity, &EditorProp)>,
    players: Query<(Entity, Option<&EquippedWeapon>), With<Player>>,
    children: Query<&Children>,
    aabbs: Query<(&bevy::camera::primitives::Aabb, &GlobalTransform)>,
    weapon_doc: Res<WeaponDoc>,
    mut measure: ResMut<ModelMeasure>,
) {
    let (root, at_scale) = match focus.subject {
        Subject::Prop => match props.iter().next() {
            Some((entity, prop)) => (Some(entity), prop.scale),
            None => (None, 1.0),
        },
        Subject::Weapon => (
            players
                .iter()
                .find_map(|(_, equipped)| equipped.map(|e| e.0)),
            weapon_doc.def.as_ref().map_or(1.0, |d| d.scale),
        ),
        Subject::Character => (players.iter().next().map(|(entity, _)| entity), 1.0),
    };
    let height = root
        .and_then(|root| mesh_extents(root, &children, &aabbs))
        .map(|(min, max)| max.y - min.y)
        .filter(|h| *h > MIN_HEIGHT);
    // Rounded to a millimetre before comparing, so float jitter in an animated mesh does
    // not mark the resource changed (and the tab redrawn) every frame.
    let rounded = height.map(|h| (h * 1000.0).round() / 1000.0);
    if measure.height != rounded || (measure.at_scale - at_scale).abs() > f32::EPSILON {
        measure.height = rounded;
        measure.at_scale = at_scale;
    }
}

/// Show and hide the prop's mesh nodes to match its def. Cheap enough to run every frame:
/// it walks one model's hierarchy and only writes visibility that is wrong.
pub fn apply_prop_hidden_nodes(
    prop_doc: Res<PropDoc>,
    props: Query<Entity, With<EditorProp>>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Visibility)>,
) {
    let Some(prop) = props.iter().next() else {
        return;
    };
    let hidden = prop_doc
        .def
        .as_ref()
        .map_or(&[][..], |d| d.hidden_nodes.as_slice());
    let mut stack = vec![prop];
    while let Some(entity) = stack.pop() {
        if let Ok((name, mut visibility)) = nodes.get_mut(entity) {
            let wanted = if hidden.iter().any(|h| h == name.as_str()) {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *visibility != wanted {
                *visibility = wanted;
            }
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
}

fn text(parent: &mut ChildSpawnerCommands, value: impl Into<String>, size: f32, color: Color) {
    parent.spawn((
        Text::new(value),
        TextFont::default().with_font_size(size),
        TextColor(color),
    ));
}

fn heading(parent: &mut ChildSpawnerCommands, value: &str) {
    parent.spawn((
        Text::new(value),
        TextFont::default().with_font_size(12.0),
        TextColor(Color::srgb(0.55, 0.8, 0.65)),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..Default::default()
        },
    ));
}

fn strip<'a>(parent: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
    parent.spawn(Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(4.0),
        ..Default::default()
    })
}

const INFO: Color = Color::srgb(0.75, 0.85, 0.9);
const NOTE: Color = Color::srgb(0.7, 0.65, 0.5);
const BUTTON: Color = Color::srgb(0.85, 0.92, 0.95);

#[allow(clippy::too_many_arguments)]
pub fn rebuild_model_tab(
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    measure: Res<ModelMeasure>,
    mut commands: Commands,
    container_q: Query<Entity, With<ModelContainer>>,
    mut built: Local<Option<Entity>>,
    mut info_cache: Local<Option<(String, Option<gltf_info::GltfInfo>)>>,
) {
    let Ok(container) = container_q.single() else {
        return;
    };
    let changed = doc.is_changed()
        || weapon_doc.is_changed()
        || prop_doc.is_changed()
        || focus.is_changed()
        || measure.is_changed();
    if !changed && *built == Some(container) {
        return;
    }
    *built = Some(container);
    commands.entity(container).despawn_related::<Children>();

    let open = subject_doc(focus.subject, &doc, &weapon_doc, &prop_doc);
    let Some(def) = open.def.clone() else {
        commands.entity(container).with_children(|parent| {
            text(parent, "nothing open", 11.0, NOTE);
        });
        return;
    };
    let info = cached_info(&mut info_cache, &def);
    let subject = focus.subject;
    let name = open
        .name()
        .map_or_else(|| "(preview)".to_string(), str::to_string);
    let measure_height = measure.height;
    let at_scale = measure.at_scale;

    commands.entity(container).with_children(|parent| {
        heading(parent, "MODEL");
        text(
            parent,
            format!("{name}  ({})", subject_label(subject)),
            13.0,
            INFO,
        );
        text(parent, def.model_path.clone(), 10.0, INFO);
        match &info {
            Some(info) => text(
                parent,
                format!(
                    "{} meshes   {} bones   {} clips",
                    info.mesh_count,
                    info.joint_count,
                    info.animations.len()
                ),
                10.0,
                INFO,
            ),
            None => text(parent, "cannot read the file", 10.0, NOTE),
        }

        spawn_size(parent, &def, subject, measure_height, at_scale);
        spawn_hidden_nodes(parent, &def, info.as_ref(), subject);
    });
}

fn cached_info(
    cache: &mut Option<(String, Option<gltf_info::GltfInfo>)>,
    def: &AssetDefinition,
) -> Option<gltf_info::GltfInfo> {
    if cache
        .as_ref()
        .is_none_or(|(path, _)| *path != def.model_path)
    {
        *cache = Some((def.model_path.clone(), gltf_info::inspect(&def.model_path)));
    }
    cache.as_ref().and_then(|(_, info)| info.clone())
}

const fn subject_label(subject: Subject) -> &'static str {
    match subject {
        Subject::Character => "character",
        Subject::Weapon => "weapon",
        Subject::Prop => "model",
    }
}

fn spawn_size(
    parent: &mut ChildSpawnerCommands,
    def: &AssetDefinition,
    subject: Subject,
    height: Option<f32>,
    at_scale: f32,
) {
    heading(parent, "SIZE");
    text(parent, format!("scale {:.4}", def.scale), 12.0, INFO);
    match height {
        Some(h) => text(parent, format!("height on screen {h:.3} m"), 11.0, INFO),
        None => text(parent, "height: (waiting for the model)", 11.0, NOTE),
    }
    if !scale_is_used(subject) {
        text(
            parent,
            "a character's size in game comes from the model panel [F2]; its def scale \
             is not read, so it is not offered here",
            10.0,
            NOTE,
        );
        return;
    }
    strip(parent).with_children(|strip| {
        for (label, factor) in SCALE_STEPS {
            row(strip, label.to_string(), false, BUTTON).observe(
                move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        def.scale *= factor;
                    }
                },
            );
        }
    });
    if subject == Subject::Weapon {
        text(
            parent,
            "a weapon's scale shows on its next equip",
            10.0,
            NOTE,
        );
    }
    let Some(height) = height else { return };
    text(parent, "set height (m):", 10.0, INFO);
    strip(parent).with_children(|strip| {
        for target in TARGET_HEIGHTS {
            let Some(scale) = scale_for_height(at_scale, height, target) else {
                continue;
            };
            row(strip, format!("{target}"), false, BUTTON).observe(
                move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        def.scale = scale;
                    }
                },
            );
        }
    });
}

fn spawn_hidden_nodes(
    parent: &mut ChildSpawnerCommands,
    def: &AssetDefinition,
    info: Option<&gltf_info::GltfInfo>,
    subject: Subject,
) {
    heading(parent, "HIDDEN NODES");
    let Some(info) = info.filter(|i| !i.mesh_nodes.is_empty()) else {
        text(parent, "no named mesh nodes in this file", 10.0, NOTE);
        return;
    };
    text(
        parent,
        match subject {
            Subject::Character => "click to hide or show (the character respawns)",
            Subject::Weapon => "click to hide or show (shows on its next equip)",
            Subject::Prop => "click to hide or show",
        },
        10.0,
        INFO,
    );
    for node in &info.mesh_nodes {
        let hidden = def.hidden_nodes.contains(node);
        let label = format!("[{}] {node}", if hidden { " " } else { "x" });
        let toggled = node.clone();
        row(parent, label, !hidden, BUTTON).observe(
            move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                if let Some(def) = docs.subject_mut(focus.subject) {
                    toggle_hidden(def, &toggled);
                }
            },
        );
    }
}

/// Hide `node` if it shows, show it if hidden. Kept sorted so the saved list is stable.
pub fn toggle_hidden(def: &mut AssetDefinition, node: &str) {
    if let Some(index) = def.hidden_nodes.iter().position(|n| n == node) {
        def.hidden_nodes.remove(index);
    } else {
        def.hidden_nodes.push(node.to_string());
        def.hidden_nodes.sort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_height_scales_in_proportion() {
        // 2 m tall at scale 1 -> 1 m tall wants scale 0.5.
        assert_eq!(scale_for_height(1.0, 2.0, 1.0), Some(0.5));
        // Measured at scale 0.5 standing 1 m: 3 m wants 1.5.
        assert_eq!(scale_for_height(0.5, 1.0, 3.0), Some(1.5));
    }

    #[test]
    fn a_model_still_loading_offers_no_height() {
        assert_eq!(scale_for_height(1.0, 0.0, 1.0), None);
    }

    #[test]
    fn hiding_twice_shows_again_and_the_list_stays_sorted() {
        let mut def = AssetDefinition::default();
        toggle_hidden(&mut def, "Pistol");
        toggle_hidden(&mut def, "Knife");
        assert_eq!(def.hidden_nodes, vec!["Knife", "Pistol"]);
        toggle_hidden(&mut def, "Pistol");
        assert_eq!(def.hidden_nodes, vec!["Knife"]);
    }

    #[test]
    fn only_a_character_ignores_its_def_scale() {
        assert!(!scale_is_used(Subject::Character));
        assert!(scale_is_used(Subject::Prop));
        assert!(scale_is_used(Subject::Weapon));
    }
}
