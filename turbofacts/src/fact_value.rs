use bevy::platform::collections::HashSet;
use serde::{Deserialize, Serialize};

/// A single typed fact value. The Rust analog of the Kotlin `Factoid.Fact<T>` sealed
/// hierarchy. Stored as the value type in the [`crate::facts::Facts`] resource.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FactValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
    TextList(Vec<String>),
    TextSet(HashSet<String>),
}

impl FactValue {
    /// Short type tag, used for diagnostics and the text format.
    pub fn type_tag(&self) -> &'static str {
        match self {
            FactValue::Bool(_) => "bool",
            FactValue::Int(_) => "int",
            FactValue::Float(_) => "float",
            FactValue::Text(_) => "text",
            FactValue::TextList(_) => "list",
            FactValue::TextSet(_) => "set",
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FactValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            FactValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            FactValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            FactValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_text_list(&self) -> Option<&Vec<String>> {
        match self {
            FactValue::TextList(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_text_set(&self) -> Option<&HashSet<String>> {
        match self {
            FactValue::TextSet(s) => Some(s),
            _ => None,
        }
    }

    /// Whether this value can be persisted. Mirrors the Kotlin behavior where list/set
    /// facts are runtime-only and skipped by [`crate::facts::persistence`].
    pub fn is_persistable(&self) -> bool {
        matches!(
            self,
            FactValue::Bool(_) | FactValue::Int(_) | FactValue::Float(_) | FactValue::Text(_)
        )
    }
}
