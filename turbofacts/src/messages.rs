use bevy::prelude::Message;

use super::fact_value::FactValue;

/// Emitted once per changed key per frame by `emit_fact_changes`, draining
/// [`Facts`](super::Facts)'s dirty list. The canonical signal that something in the world
/// changed — the Rust analog of the Kotlin `Message.FactUpdated`. Systems react by reading
/// this (HUD refresh, audio, story re-checks, telemetry).
#[derive(Message, Clone, Debug)]
pub struct FactChanged {
    pub key: String,
    pub value: FactValue,
}

/// A request to write a fact, for systems that hold only a `MessageWriter` and want to avoid
/// `ResMut<Facts>` contention. Applied by `apply_set_fact` into the [`Facts`](super::Facts)
/// resource. Direct `ResMut<Facts>` mutation is still fine where convenient.
/// A named side effect requested by a story consequence (`Consequence::Emit`). Game plugins
/// listen for the names they care about (e.g. "level_complete" -> start cutscene). Keeps the
/// facts module free of dependencies on camera/ui/state.
#[derive(Message, Clone, Debug)]
pub struct StoryEffect {
    pub effect: String,
}

/// Emitted when a story's rules pass and its consequences are applied. Useful for telemetry
/// and observers.
#[derive(Message, Clone, Debug)]
pub struct StoryFired {
    pub story: String,
}

#[derive(Message, Clone, Debug)]
pub enum SetFact {
    Bool(String, bool),
    Int(String, i64),
    AddInt(String, i64),
    Float(String, f32),
    AddFloat(String, f32),
    Text(String, String),
    AddToList(String, String),
    RemoveFromList(String, String),
    AddToSet(String, String),
    RemoveFromSet(String, String),
}
