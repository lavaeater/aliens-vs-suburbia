//! Editing a character's torso-twist chain (`AssetDefinition::aim_bones`), shown in the
//! Props tab.
//!
//! The twist spreads the aim over a few spine bones (`player::systems::torso_twist`). A
//! def without its own chain falls back to the mixamo `Spine`/`Spine1`/`Spine2` bones,
//! which a rig named any other way does not have — the twist is then switched off with a
//! warning, and the upper body never turns to the aim. This is where such a rig gets a
//! chain: suggested from its spine bones, or picked bone by bone, with a weight each.
//!
//! The chain is resolved when the character spawns, so an edit respawns it
//! (`doc::needs_respawn` covers `aim_bones`).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::assets::asset_definition::{AimBone, AssetDefinition};
use crate::character_editor::doc::{EditorFocus, SideDocs};
use crate::character_editor::ui::row;
use crate::player::systems::torso_twist::default_aim_bones;

/// Most bones a suggested chain takes. A twist spread over more than this stops reading
/// as the torso turning and starts reading as the whole back wobbling.
const MAX_SUGGESTED: usize = 4;

/// Words that mark a bone as part of the torso, for the suggestion and the picker.
const TORSO_WORDS: [&str; 5] = ["spine", "chest", "torso", "back", "neck"];

/// Weight steps for the nudges. Weights are proportions, normalized at use.
const WEIGHT_STEP: f32 = 0.05;

fn is_torso(bone: &str) -> bool {
    let lower = bone.to_lowercase();
    TORSO_WORDS.iter().any(|word| lower.contains(word))
}

/// A chain for a rig: its spine bones in name order (which is root-to-chest order for
/// `spine_01`.. and `Spine`, `Spine1`..), at most [`MAX_SUGGESTED`], with weights rising
/// up the back so the bend builds gradually — the same shape as the default chain.
pub fn suggest_chain(bones: &[String]) -> Vec<AimBone> {
    let mut spine: Vec<&String> = bones
        .iter()
        .filter(|bone| bone.to_lowercase().contains("spine"))
        .collect();
    spine.sort();
    spine.truncate(MAX_SUGGESTED);
    let total: f32 = (1..=spine.len()).map(|i| i as f32).sum();
    spine
        .into_iter()
        .enumerate()
        .map(|(index, bone)| AimBone {
            bone: bone.clone(),
            weight: ((index + 1) as f32 / total * 100.0).round() / 100.0,
        })
        .collect()
}

/// The default chain's bones this rig is missing — all of them, on a non-mixamo rig.
pub fn missing_default_bones(bones: &[String]) -> Vec<String> {
    default_aim_bones()
        .into_iter()
        .map(|b| b.bone)
        .filter(|bone| !bones.contains(bone))
        .collect()
}

pub fn nudge_weight(def: &mut AssetDefinition, bone: &str, delta: f32) {
    if let Some(aim) = def.aim_bones.iter_mut().find(|a| a.bone == bone) {
        aim.weight = ((aim.weight + delta).max(0.0) * 100.0).round() / 100.0;
    }
}

/// Add `bone` with a share like the others', or remove it if it is in the chain.
pub fn toggle_bone(def: &mut AssetDefinition, bone: &str) {
    if let Some(index) = def.aim_bones.iter().position(|a| a.bone == bone) {
        def.aim_bones.remove(index);
    } else {
        let weight = def
            .aim_bones
            .last()
            .map_or(0.3, |last| last.weight);
        def.aim_bones.push(AimBone {
            bone: bone.to_string(),
            weight,
        });
    }
}

const INFO: Color = Color::srgb(0.75, 0.85, 0.9);
const WARN: Color = Color::srgb(1.0, 0.75, 0.4);
const BUTTON: Color = Color::srgb(0.85, 0.92, 0.95);

fn text(parent: &mut ChildSpawnerCommands, value: impl Into<String>, color: Color) {
    parent.spawn((
        Text::new(value),
        TextFont::default().with_font_size(10.0),
        TextColor(color),
    ));
}

fn strip<'a>(parent: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
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

/// The TORSO TWIST section. `bones` are the live rig's, sorted; empty before it spawns.
pub fn spawn_aim_bones(parent: &mut ChildSpawnerCommands, def: &AssetDefinition, bones: &[String]) {
    parent.spawn((
        Text::new("TORSO TWIST"),
        TextFont::default().with_font_size(12.0),
        TextColor(Color::srgb(0.55, 0.8, 0.65)),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..Default::default()
        },
    ));

    if def.aim_bones.is_empty() {
        let missing = missing_default_bones(bones);
        if bones.is_empty() {
            text(parent, "using the default mixamo spine chain", INFO);
        } else if missing.is_empty() {
            text(parent, "using the default mixamo spine chain (this rig has it)", INFO);
        } else {
            text(
                parent,
                "this rig has no mixamo spine, so the torso does not twist - \
                 suggest a chain or pick its bones below",
                WARN,
            );
        }
    }

    for aim in &def.aim_bones {
        let missing = !bones.is_empty() && !bones.contains(&aim.bone);
        strip(parent).with_children(|line| {
            line.spawn((
                Text::new(format!(
                    "{}  {:.2}{}",
                    aim.bone,
                    aim.weight,
                    if missing { "  (not on this rig)" } else { "" }
                )),
                TextFont::default().with_font_size(11.0),
                TextColor(if missing { WARN } else { INFO }),
                Node {
                    min_width: Val::Px(150.0),
                    ..Default::default()
                },
            ));
            for (label, delta) in [("<", -WEIGHT_STEP), (">", WEIGHT_STEP)] {
                let bone = aim.bone.clone();
                row(line, label.to_string(), false, BUTTON).observe(
                    move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                        if let Some(def) = docs.subject_mut(focus.subject) {
                            nudge_weight(def, &bone, delta);
                        }
                    },
                );
            }
            let bone = aim.bone.clone();
            row(line, "remove".to_string(), false, Color::srgb(0.95, 0.7, 0.6)).observe(
                move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        toggle_bone(def, &bone);
                    }
                },
            );
        });
    }

    let suggestion = suggest_chain(bones);
    strip(parent).with_children(|line| {
        if !suggestion.is_empty() && suggestion != def.aim_bones {
            row(line, "suggest from this rig".to_string(), false, BUTTON).observe(
                move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        def.aim_bones.clone_from(&suggestion);
                    }
                },
            );
        }
        if !def.aim_bones.is_empty() {
            row(line, "use the default chain".to_string(), false, BUTTON).observe(
                |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        def.aim_bones.clear();
                    }
                },
            );
        }
    });

    let candidates: Vec<&String> = bones
        .iter()
        .filter(|bone| is_torso(bone) && !def.aim_bones.iter().any(|a| a.bone == **bone))
        .collect();
    if !candidates.is_empty() {
        text(parent, "add a bone:", INFO);
        strip(parent).with_children(|line| {
            for bone in candidates {
                let picked = bone.clone();
                row(line, format!("+ {bone}"), false, BUTTON).observe(
                    move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                        if let Some(def) = docs.subject_mut(focus.subject) {
                            toggle_bone(def, &picked);
                        }
                    },
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    /// The rig that started this: Unreal-style names, no mixamo spine.
    #[test]
    fn an_unreal_rig_gets_its_spine_suggested_in_order() {
        let bones = names(&["pelvis", "spine_03", "spine_01", "neck_01", "spine_02", "hand_r"]);
        let chain = suggest_chain(&bones);
        let order: Vec<&str> = chain.iter().map(|a| a.bone.as_str()).collect();
        assert_eq!(order, ["spine_01", "spine_02", "spine_03"]);
        assert!(
            chain.windows(2).all(|w| w[0].weight < w[1].weight),
            "weights rise up the back: {chain:?}"
        );
        assert_eq!(missing_default_bones(&bones).len(), 3);
    }

    #[test]
    fn a_mixamo_rig_is_missing_nothing() {
        let bones = names(&["mixamorigSpine", "mixamorigSpine1", "mixamorigSpine2"]);
        assert!(missing_default_bones(&bones).is_empty());
    }

    #[test]
    fn toggling_adds_then_removes_and_weights_stay_non_negative() {
        let mut def = AssetDefinition::default();
        toggle_bone(&mut def, "spine_01");
        assert_eq!(def.aim_bones.len(), 1);
        nudge_weight(&mut def, "spine_01", -5.0);
        assert_eq!(def.aim_bones[0].weight, 0.0);
        toggle_bone(&mut def, "spine_01");
        assert!(def.aim_bones.is_empty());
    }

    #[test]
    fn a_rig_without_a_spine_suggests_nothing() {
        assert!(suggest_chain(&names(&["root", "hips", "head"])).is_empty());
    }
}
