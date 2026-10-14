//! The Animation tab: bind game keys to tags, and tag the clips.
//!
//! Two halves, for whatever is open (a character, a weapon, an enemy):
//!
//! - **Keys.** Every game animation key, what it resolves to, and — once picked — the tag
//!   paths it can be bound to. On a character in the arena, clicking a key also plays it
//!   through the real state machine.
//! - **Clips.** Every clip the model and its animation sources carry
//!   (`animation::all_clips`), each with its tag. Picking one opens a field to type its
//!   tag path into (Enter to set it, empty to clear it), with the paths already in use
//!   offered underneath. This is the asset browser's clip tagging, moved here: tags are
//!   what make a clip bindable, so the two halves need to sit side by side.
//!
//! Edits go into the open doc like any other; a character in the arena picks them up live
//! (`build_player_anim_graph` rebuilds on a change of bindings or tags).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::assets::asset_definition::AssetDefinition;
use crate::character_editor::animation::{
    AnimationEditor, PLAYABLE_KEYS, all_clips, bind, resolution_label, set_clip_tag, tag_paths,
    unbind,
};
use crate::character_editor::doc::{
    EditorDoc, EditorFocus, PropDoc, SideDocs, Subject, WeaponDoc, subject_doc,
};
use crate::character_editor::text_field::{FieldKey, FocusedField, TextField, text_field_with};
use crate::character_editor::ui::{AnimationContainer, row};
use crate::player::components::Player;

/// The field a clip's tag is typed into.
#[derive(Component)]
pub struct ClipTagField;

const KEY_COLOR: Color = Color::srgb(0.85, 0.9, 0.85);
const TAG_COLOR: Color = Color::srgb(0.8, 0.85, 0.95);
const CLIP_COLOR: Color = Color::srgb(0.85, 0.85, 0.75);
const NOTE_COLOR: Color = Color::srgb(0.55, 0.7, 0.8);
const WARN_COLOR: Color = Color::srgb(0.7, 0.6, 0.45);

fn note(parent: &mut ChildSpawnerCommands, text: impl Into<String>, color: Color) {
    parent.spawn((
        Text::new(text),
        TextFont::default().with_font_size(10.0),
        TextColor(color),
    ));
}

fn heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::default().with_font_size(12.0),
        TextColor(Color::srgb(0.55, 0.8, 0.65)),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..Default::default()
        },
    ));
}

/// Redraw on any change to what is open or to its def.
#[allow(clippy::too_many_arguments)]
pub fn rebuild_animation_tab(
    mut editor: ResMut<AnimationEditor>,
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    mut commands: Commands,
    container_q: Query<Entity, With<AnimationContainer>>,
    mut last_subject: Local<Option<(Subject, Option<String>)>>,
    mut clips_cache: Local<Option<(String, Vec<String>, Vec<String>)>>,
) {
    let open = subject_doc(focus.subject, &doc, &weapon_doc, &prop_doc);
    let subject = Some((focus.subject, open.def_path.clone()));
    if *last_subject != subject {
        *last_subject = subject;
        editor.selected_key = None;
        editor.selected_clip = None;
        editor.ui_dirty = true;
    }
    if doc.is_changed() || weapon_doc.is_changed() || prop_doc.is_changed() {
        editor.ui_dirty = true;
    }
    if !editor.ui_dirty {
        return;
    }
    let Ok(container) = container_q.single() else {
        return;
    };
    editor.ui_dirty = false;
    commands.entity(container).despawn_related::<Children>();

    let Some(def) = open.def.as_ref() else {
        commands.entity(container).with_children(|parent| {
            note(parent, "nothing open", WARN_COLOR);
        });
        return;
    };

    // Listing clips reads the model and every source off disk; only do it when those
    // change, not on every tag edit.
    let clips = cached_clips(&mut clips_cache, def);
    let plays_live = focus.subject == Subject::Character;
    let selected_key = editor.selected_key;
    let selected_clip = editor.selected_clip.clone();
    let tags = tag_paths(def);
    let def = def.clone();

    commands.entity(container).with_children(|parent| {
        spawn_keys(parent, &def, &tags, selected_key, plays_live);
        spawn_clips(parent, &def, &clips, &tags, selected_clip.as_deref());
    });
}

fn cached_clips(
    cache: &mut Option<(String, Vec<String>, Vec<String>)>,
    def: &AssetDefinition,
) -> Vec<String> {
    let fresh = cache.as_ref().is_none_or(|(model, sources, _)| {
        *model != def.model_path || *sources != def.animation_sources
    });
    if fresh {
        *cache = Some((
            def.model_path.clone(),
            def.animation_sources.clone(),
            all_clips(def),
        ));
    }
    let mut clips = cache.as_ref().map_or_default(|(_, _, clips)| clips.clone());
    // Tags on clips the files do not list are still shown, so they can be cleared.
    for clip in def.clip_tags.keys() {
        if !clips.contains(clip) {
            clips.push(clip.clone());
        }
    }
    clips
}

fn spawn_keys(
    parent: &mut ChildSpawnerCommands,
    def: &AssetDefinition,
    tags: &[String],
    selected: Option<AnimationKey>,
    plays_live: bool,
) {
    heading(parent, "KEYS");
    note(
        parent,
        if plays_live {
            "click a key to play it and pick its binding"
        } else {
            "click a key to pick its binding"
        },
        NOTE_COLOR,
    );
    for key in PLAYABLE_KEYS {
        let is_selected = selected == Some(key);
        let marker = if is_selected { "*" } else { " " };
        let label = format!(
            "{marker} {}  ->  {}",
            key.default_search(),
            resolution_label(def, key)
        );
        row(parent, label, is_selected, KEY_COLOR).observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  mut anim_mw: MessageWriter<AnimationEvent>,
                  players: Query<Entity, With<Player>>| {
                editor.select(key);
                if plays_live {
                    for player in &players {
                        anim_mw.write(AnimationEvent(
                            AnimationEventType::GotoAnimState,
                            player,
                            key,
                        ));
                    }
                }
            },
        );
    }

    if plays_live {
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
                for player in &players {
                    anim_mw.write(AnimationEvent(
                        AnimationEventType::LeaveAnimState,
                        player,
                        key,
                    ));
                }
            },
        );
    }

    let Some(key) = selected else { return };
    let bound = def.animation_bindings.get(key.default_search()).cloned();
    note(
        parent,
        format!("bind '{}' to:", key.default_search()),
        NOTE_COLOR,
    );
    if tags.is_empty() {
        note(parent, "no clip is tagged yet - tag some in CLIPS below", WARN_COLOR);
    }
    for tag in tags {
        let is_bound = bound.as_deref() == Some(tag.as_str());
        let chosen = tag.clone();
        row(parent, tag.clone(), is_bound, TAG_COLOR).observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  focus: Res<EditorFocus>,
                  mut docs: SideDocs| {
                if let Some(def) = docs.subject_mut(focus.subject) {
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
              focus: Res<EditorFocus>,
              mut docs: SideDocs| {
            if let Some(def) = docs.subject_mut(focus.subject) {
                unbind(def, key);
            }
            editor.touch();
        },
    );
}

fn spawn_clips(
    parent: &mut ChildSpawnerCommands,
    def: &AssetDefinition,
    clips: &[String],
    tags: &[String],
    selected: Option<&str>,
) {
    heading(parent, "CLIPS");
    if clips.is_empty() {
        note(
            parent,
            "this model has no clips - add an animation file from the Library",
            WARN_COLOR,
        );
        return;
    }
    note(parent, "click a clip to tag it", NOTE_COLOR);
    for clip in clips {
        let tag = def.clip_tags.get(clip);
        let is_selected = selected == Some(clip.as_str());
        let label = format!(
            "{} {}  ->  {}",
            if is_selected { "*" } else { " " },
            clip,
            tag.map_or("(untagged)", String::as_str)
        );
        let color = if tag.is_some() { CLIP_COLOR } else { WARN_COLOR };
        let picked = clip.clone();
        row(parent, label, is_selected, color).observe(
            move |_: On<Activate>, mut editor: ResMut<AnimationEditor>| {
                editor.selected_clip = Some(picked.clone());
                editor.touch();
            },
        );

        if !is_selected {
            continue;
        }
        // The editor for the picked clip, right under it.
        parent.spawn((
            ClipTagField,
            text_field_with(tag.cloned().unwrap_or_default(), "type a tag path, Enter to set"),
        ));
        if !tags.is_empty() {
            note(parent, "or reuse a path:", NOTE_COLOR);
        }
        for path in tags {
            let chosen = path.clone();
            let for_clip = clip.clone();
            row(parent, format!("  {path}"), tag == Some(path), TAG_COLOR).observe(
                move |_: On<Activate>,
                      mut editor: ResMut<AnimationEditor>,
                      focus: Res<EditorFocus>,
                      mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        set_clip_tag(def, &for_clip, &chosen);
                    }
                    editor.touch();
                },
            );
        }
    }
}

/// Enter in the tag field sets the picked clip's tag.
pub fn commit_clip_tag(
    mut keys: MessageReader<FieldKey>,
    fields: Query<&TextField, With<ClipTagField>>,
    mut focused: ResMut<FocusedField>,
    mut editor: ResMut<AnimationEditor>,
    focus: Res<EditorFocus>,
    mut docs: SideDocs,
) {
    for key in keys.read() {
        let FieldKey::Submit(entity) = *key else {
            continue;
        };
        let Ok(field) = fields.get(entity) else {
            continue;
        };
        let Some(clip) = editor.selected_clip.clone() else {
            continue;
        };
        if let Some(def) = docs.subject_mut(focus.subject) {
            set_clip_tag(def, &clip, &field.value);
        }
        focused.0 = None;
        editor.touch();
    }
}
