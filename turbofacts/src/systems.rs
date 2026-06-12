use bevy::prelude::{MessageReader, MessageWriter, ResMut};

use super::facts_resource::Facts;
use super::messages::{FactChanged, SetFact, StoryEffect, StoryFired};
use super::story::StoryStore;

/// Applies queued [`SetFact`] requests into the [`Facts`] resource. Runs before
/// `emit_fact_changes` so the resulting mutations are picked up the same frame.
pub fn apply_set_fact(mut facts: ResMut<Facts>, mut reader: MessageReader<SetFact>) {
    for req in reader.read() {
        match req {
            SetFact::Bool(k, v) => facts.set_bool(k, *v),
            SetFact::Int(k, v) => facts.set_int(k, *v),
            SetFact::AddInt(k, v) => {
                facts.add_to_int(k, *v);
            }
            SetFact::Float(k, v) => facts.set_float(k, *v),
            SetFact::AddFloat(k, v) => {
                facts.add_to_float(k, *v);
            }
            SetFact::Text(k, v) => facts.set_text(k, v.clone()),
            SetFact::AddToList(k, v) => facts.add_to_text_list(k, v.clone()),
            SetFact::RemoveFromList(k, v) => facts.remove_from_text_list(k, v),
            SetFact::AddToSet(k, v) => facts.add_to_text_set(k, v.clone()),
            SetFact::RemoveFromSet(k, v) => facts.remove_from_text_set(k, v),
        }
    }
}

/// Drains the [`Facts`] dirty list into [`FactChanged`] messages, one per changed key.
pub fn emit_fact_changes(mut facts: ResMut<Facts>, mut writer: MessageWriter<FactChanged>) {
    if !facts.has_dirty() {
        return;
    }
    for key in facts.drain_dirty() {
        if let Some(value) = facts.get_raw(&key) {
            writer.write(FactChanged {
                key,
                value: value.clone(),
            });
        }
    }
}

/// Arms a story re-check when any fact changed this frame. The dirty-flag gate that keeps
/// `check_stories` from re-evaluating every frame (analog of `TurboStoryManager.needsChecking`).
pub fn arm_story_check(mut reader: MessageReader<FactChanged>, mut store: ResMut<StoryStore>) {
    if !reader.is_empty() {
        store.needs_checking = true;
    }
    reader.clear();
}

/// Evaluates stories (already specificity-sorted) when active and armed, applying the
/// consequences of any whose rules pass. Consequence fact writes re-dirty [`Facts`], so the
/// change cascades to the next frame. Mirrors `TurboStoryManager.checkIfNeeded`.
pub fn check_stories(
    mut store: ResMut<StoryStore>,
    mut facts: ResMut<Facts>,
    mut fired_writer: MessageWriter<StoryFired>,
    mut effect_writer: MessageWriter<StoryEffect>,
) {
    if !store.active || !store.needs_checking {
        return;
    }
    store.needs_checking = false;

    let mut fired_stories: Vec<String> = Vec::new();
    let mut effects: Vec<String> = Vec::new();
    for story in &mut store.stories {
        if story.check_and_apply(&mut facts, &mut effects) {
            fired_stories.push(story.name.clone());
            if story.exclusive {
                break;
            }
        }
    }

    for story in fired_stories {
        fired_writer.write(StoryFired { story });
    }
    for effect in effects {
        effect_writer.write(StoryEffect { effect });
    }
}
