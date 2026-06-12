use serde::{Deserialize, Serialize};

use super::fact_value::FactValue;
use super::facts_resource::Facts;

/// What a story does when its rules pass. The Rust analog of Kotlin's `Consequence`
/// interface. Pure fact writes apply directly to [`Facts`]; side effects that need other
/// systems (cutscenes, UI, state transitions) are surfaced as a named [`StoryEffect`] that
/// the relevant plugin handles, keeping the facts module dependency-free.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Consequence {
    SetFact { key: String, value: FactValue },
    AddInt { key: String, delta: i64 },
    /// Emits a [`StoryEffect`] with this name for game systems to react to.
    Emit { effect: String },
}

impl Consequence {
    /// Applies the consequence. Fact writes mutate `facts`; `Emit` pushes a named effect
    /// onto `effects` for the caller to dispatch as messages.
    pub fn apply(&self, facts: &mut Facts, effects: &mut Vec<String>) {
        match self {
            Consequence::SetFact { key, value } => match value {
                FactValue::Bool(b) => facts.set_bool(key, *b),
                FactValue::Int(i) => facts.set_int(key, *i),
                FactValue::Float(f) => facts.set_float(key, *f),
                FactValue::Text(t) => facts.set_text(key, t.clone()),
                FactValue::TextList(_) | FactValue::TextSet(_) => {
                    bevy::log::warn!("SetFact consequence on '{}' ignores collection values", key);
                }
            },
            Consequence::AddInt { key, delta } => {
                facts.add_to_int(key, *delta);
            }
            Consequence::Emit { effect } => effects.push(effect.clone()),
        }
    }
}
