//! The Animation tab: which clip each game key plays, for whatever is open (a character,
//! a weapon, an enemy).
//!
//! One row per `AnimationKey` with the clip it resolves to and where that came from (the
//! def, the rig preset, the name search, a fallback key). Picking a key opens the list of
//! every clip the model and its animation sources carry (`animation::clip_set`) right
//! under it; clicking one maps the key to it, and "use default" drops the def's entry.
//! On a character in the arena, picking a key also previews it (`PreviewAnimation`).
//!
//! Edits go into the open doc like any other; a character in the arena picks them up live
//! (`build_player_anim_graph` rebuilds on a change of `animations`).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::animation::animation_plugin::{ANIM_KEYS, AnimationKey, PreviewAnimation};
use crate::animation::clips::ClipSet;
use crate::assets::asset_definition::AssetDefinition;
use crate::character_editor::animation::{
    AnimationEditor, clip_set, map, resolution_label, unmap,
};
use crate::character_editor::doc::{
    EditorDoc, EditorFocus, PropDoc, SideDocs, Subject, WeaponDoc, subject_doc,
};
use crate::character_editor::ui::{AnimationContainer, row};
use crate::model_settings::resources::{AnimMapping, ModelSettings};
use crate::player::components::Player;

const KEY_COLOR: Color = Color::srgb(0.85, 0.9, 0.85);
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

/// The model and sources a cached `ClipSet` was read from.
type ClipCache = Option<(String, Vec<String>, ClipSet)>;

/// Redraw on any change to what is open or to its def.
#[allow(clippy::too_many_arguments)]
pub fn rebuild_animation_tab(
    mut editor: ResMut<AnimationEditor>,
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    model_settings: Res<ModelSettings>,
    mut commands: Commands,
    container_q: Query<Entity, With<AnimationContainer>>,
    mut last_subject: Local<Option<(Subject, Option<String>)>>,
    mut clips_cache: Local<ClipCache>,
) {
    let open = subject_doc(focus.subject, &doc, &weapon_doc, &prop_doc);
    let subject = Some((focus.subject, open.def_path.clone()));
    if *last_subject != subject {
        *last_subject = subject;
        editor.selected_key = None;
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

    let plays_live = focus.subject == Subject::Character;
    // Only players read the global `ModelSettings` mapping (see `build_player_anim_graph`).
    let settings = plays_live.then_some(&model_settings.anim_mapping);
    let clips = cached_clips(&mut clips_cache, def);
    let selected = editor.selected_key;

    commands.entity(container).with_children(|parent| {
        spawn_keys(parent, def, clips, settings, selected, plays_live);
    });
}

/// Listing clips reads the model and every source off disk; only do it when those
/// change, not on every mapping edit.
fn cached_clips<'a>(cache: &'a mut ClipCache, def: &AssetDefinition) -> &'a ClipSet {
    let stale = cache.as_ref().is_none_or(|(model, sources, _)| {
        *model != def.model_path || *sources != def.animation_sources
    });
    if stale {
        *cache = None;
    }
    let (_, _, clips) = cache.get_or_insert_with(|| {
        (
            def.model_path.clone(),
            def.animation_sources.clone(),
            clip_set(def),
        )
    });
    clips
}

fn spawn_keys(
    parent: &mut ChildSpawnerCommands,
    def: &AssetDefinition,
    clips: &ClipSet,
    settings: Option<&AnimMapping>,
    selected: Option<AnimationKey>,
    plays_live: bool,
) {
    heading(parent, "KEYS");
    if clips.names().next().is_none() {
        note(
            parent,
            "this model has no clips - add an animation file from the Library",
            WARN_COLOR,
        );
    }
    if let Some(rig) = clips.rig {
        note(
            parent,
            format!("mesh2motion {} rig: unmapped keys use its preset", rig.label()),
            NOTE_COLOR,
        );
    }
    note(
        parent,
        if plays_live {
            "click a key to preview it and pick its clip"
        } else {
            "click a key to pick its clip"
        },
        NOTE_COLOR,
    );

    if plays_live {
        row(
            parent,
            "stop preview".to_string(),
            false,
            Color::srgb(0.7, 0.8, 0.9),
        )
        .observe(
            |_: On<Activate>,
             mut preview_mw: MessageWriter<PreviewAnimation>,
             players: Query<Entity, With<Player>>| {
                for character in &players {
                    preview_mw.write(PreviewAnimation {
                        character,
                        key: None,
                    });
                }
            },
        );
    }

    for &key in ANIM_KEYS {
        let is_selected = selected == Some(key);
        let marker = if is_selected { "*" } else { " " };
        let label = format!(
            "{marker} {}  ->  {}",
            key.default_search(),
            resolution_label(clips, def, settings, key)
        );
        row(parent, label, is_selected, KEY_COLOR).observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  mut preview_mw: MessageWriter<PreviewAnimation>,
                  players: Query<Entity, With<Player>>| {
                editor.select(key);
                if plays_live {
                    for character in &players {
                        preview_mw.write(PreviewAnimation {
                            character,
                            key: Some(key),
                        });
                    }
                }
            },
        );
        if is_selected {
            spawn_clip_picker(parent, def, clips, settings, key);
        }
    }
}

/// Every clip, under the selected key; the one it plays now is highlighted.
fn spawn_clip_picker(
    parent: &mut ChildSpawnerCommands,
    def: &AssetDefinition,
    clips: &ClipSet,
    settings: Option<&AnimMapping>,
    key: AnimationKey,
) {
    let current = clips
        .resolve(key, Some(&def.animations), settings)
        .map(|r| r.clip);
    if def.animations.contains_key(&key) {
        row(
            parent,
            "    use default (preset / by name)".to_string(),
            false,
            Color::srgb(0.95, 0.8, 0.6),
        )
        .observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  focus: Res<EditorFocus>,
                  mut docs: SideDocs| {
                if let Some(def) = docs.subject_mut(focus.subject) {
                    unmap(def, key);
                }
                editor.touch();
            },
        );
    }
    for clip in clips.names() {
        let chosen = clip.to_string();
        let is_current = current.as_deref() == Some(clip);
        row(parent, format!("    {clip}"), is_current, CLIP_COLOR).observe(
            move |_: On<Activate>,
                  mut editor: ResMut<AnimationEditor>,
                  focus: Res<EditorFocus>,
                  mut docs: SideDocs| {
                if let Some(def) = docs.subject_mut(focus.subject) {
                    map(def, key, &chosen);
                }
                editor.touch();
            },
        );
    }
}
