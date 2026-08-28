use bevy::platform::collections::HashMap;
use bevy::prelude::Resource;

use super::fact_value::FactValue;

/// Joins key fragments into a single dotted key, the Rust analog of Kotlin's `multiKey`.
///
/// `fact_key(&["enemy", "boss", "dead"]) == "enemy.boss.dead"`
pub fn fact_key(parts: &[&str]) -> String {
    parts.join(".")
}

/// The world's fact store. A flat `key -> FactValue` map plus a dirty list of keys mutated
/// since the last drain. The drain is consumed by `emit_fact_changes` to produce
/// `FactChanged` messages (see [`crate::facts::facts_plugin`]).
///
/// Mutations made inside [`Facts::silent`] do not record dirty keys, mirroring the Kotlin
/// `silent { ... }` block used for bulk initialization.
#[derive(Resource, Default, Debug)]
pub struct Facts {
    map: HashMap<String, FactValue>,
    dirty: Vec<String>,
    silent: bool,
}

impl Facts {
    fn touch(&mut self, key: &str) {
        if !self.silent {
            self.dirty.push(key.to_string());
        }
    }

    /// Runs `f` with change-signalling suppressed. Used to seed facts without triggering
    /// story re-evaluation. Mirrors `TurboFactsOfTheWorld.silent`.
    pub fn silent(&mut self, f: impl FnOnce(&mut Facts)) {
        let was_silent = self.silent;
        self.silent = true;
        f(self);
        self.silent = was_silent;
    }

    /// Drains the dirty key list. Called once per frame by the change-emitting system.
    pub fn drain_dirty(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dirty)
    }

    pub fn has_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    pub fn get_raw(&self, key: &str) -> Option<&FactValue> {
        self.map.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &FactValue)> {
        self.map.iter()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Inserts a value, warning (but not panicking) when it changes an existing fact's type.
    fn insert(&mut self, key: &str, value: FactValue) {
        if let Some(existing) = self.map.get(key)
            && existing.type_tag() != value.type_tag()
        {
            bevy::log::warn!(
                "fact '{}' changing type from {} to {}",
                key,
                existing.type_tag(),
                value.type_tag()
            );
        }
        self.map.insert(key.to_string(), value);
        self.touch(key);
    }

    // --- bool ---------------------------------------------------------------

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.insert(key, FactValue::Bool(value));
    }

    pub fn set_true(&mut self, key: &str) {
        self.set_bool(key, true);
    }

    pub fn set_false(&mut self, key: &str) {
        self.set_bool(key, false);
    }

    pub fn try_bool(&self, key: &str) -> Option<bool> {
        self.map.get(key).and_then(FactValue::as_bool)
    }

    /// Reads a bool, treating a missing fact as `false`. Non-mutating.
    pub fn bool(&self, key: &str) -> bool {
        self.try_bool(key).unwrap_or(false)
    }

    /// Reads a bool, inserting `default` if the fact is missing. Mirrors `boolOrDefault`.
    pub fn bool_or_default(&mut self, key: &str, default: bool) -> bool {
        if !self.map.contains_key(key) {
            self.set_bool(key, default);
        }
        self.bool(key)
    }

    // --- int ----------------------------------------------------------------

    pub fn set_int(&mut self, key: &str, value: i64) {
        self.insert(key, FactValue::Int(value));
    }

    pub fn try_int(&self, key: &str) -> Option<i64> {
        self.map.get(key).and_then(FactValue::as_int)
    }

    pub fn int(&self, key: &str) -> i64 {
        self.try_int(key).unwrap_or(0)
    }

    pub fn int_or_default(&mut self, key: &str, default: i64) -> i64 {
        if !self.map.contains_key(key) {
            self.set_int(key, default);
        }
        self.int(key)
    }

    pub fn add_to_int(&mut self, key: &str, delta: i64) -> i64 {
        let new = self.int(key) + delta;
        self.set_int(key, new);
        new
    }

    // --- float --------------------------------------------------------------

    pub fn set_float(&mut self, key: &str, value: f32) {
        self.insert(key, FactValue::Float(value));
    }

    pub fn try_float(&self, key: &str) -> Option<f32> {
        self.map.get(key).and_then(FactValue::as_float)
    }

    pub fn float(&self, key: &str) -> f32 {
        self.try_float(key).unwrap_or(0.0)
    }

    pub fn float_or_default(&mut self, key: &str, default: f32) -> f32 {
        if !self.map.contains_key(key) {
            self.set_float(key, default);
        }
        self.float(key)
    }

    pub fn add_to_float(&mut self, key: &str, delta: f32) -> f32 {
        let new = self.float(key) + delta;
        self.set_float(key, new);
        new
    }

    // --- text ---------------------------------------------------------------

    pub fn set_text(&mut self, key: &str, value: impl Into<String>) {
        self.insert(key, FactValue::Text(value.into()));
    }

    pub fn try_text(&self, key: &str) -> Option<&str> {
        self.map.get(key).and_then(FactValue::as_text)
    }

    pub fn text(&self, key: &str) -> &str {
        self.try_text(key).unwrap_or("")
    }

    pub fn text_or_default(&mut self, key: &str, default: impl Into<String>) -> &str {
        if !self.map.contains_key(key) {
            self.set_text(key, default);
        }
        self.text(key)
    }

    // --- text list ----------------------------------------------------------

    fn ensure_list(&mut self, key: &str) -> &mut Vec<String> {
        if !matches!(self.map.get(key), Some(FactValue::TextList(_))) {
            self.map
                .insert(key.to_string(), FactValue::TextList(Vec::new()));
        }
        match self.map.get_mut(key) {
            Some(FactValue::TextList(l)) => l,
            _ => unreachable!("ensure_list guarantees a TextList"),
        }
    }

    pub fn add_to_text_list(&mut self, key: &str, value: impl Into<String>) {
        self.ensure_list(key).push(value.into());
        self.touch(key);
    }

    pub fn remove_from_text_list(&mut self, key: &str, value: &str) {
        self.ensure_list(key).retain(|v| v != value);
        self.touch(key);
    }

    pub fn text_list(&self, key: &str) -> &[String] {
        match self.map.get(key) {
            Some(FactValue::TextList(l)) => l,
            _ => &[],
        }
    }

    // --- text set -----------------------------------------------------------

    fn ensure_set(&mut self, key: &str) -> &mut bevy::platform::collections::HashSet<String> {
        if !matches!(self.map.get(key), Some(FactValue::TextSet(_))) {
            self.map.insert(
                key.to_string(),
                FactValue::TextSet(bevy::platform::collections::HashSet::default()),
            );
        }
        match self.map.get_mut(key) {
            Some(FactValue::TextSet(s)) => s,
            _ => unreachable!("ensure_set guarantees a TextSet"),
        }
    }

    pub fn add_to_text_set(&mut self, key: &str, value: impl Into<String>) {
        self.ensure_set(key).insert(value.into());
        self.touch(key);
    }

    pub fn remove_from_text_set(&mut self, key: &str, value: &str) {
        self.ensure_set(key).remove(value);
        self.touch(key);
    }

    pub fn text_set_contains(&self, key: &str, value: &str) -> bool {
        match self.map.get(key) {
            Some(FactValue::TextSet(s)) => s.contains(value),
            _ => false,
        }
    }

    pub fn text_set_len(&self, key: &str) -> usize {
        match self.map.get(key) {
            Some(FactValue::TextSet(s)) => s.len(),
            _ => 0,
        }
    }

    /// Writes a [`FactValue`] of any variant. Collections are appended element-by-element
    /// (so they pick up `add_to_*` semantics). Used for seeding and loading.
    pub fn apply_value(&mut self, key: &str, value: FactValue) {
        match value {
            FactValue::Bool(b) => self.set_bool(key, b),
            FactValue::Int(i) => self.set_int(key, i),
            FactValue::Float(f) => self.set_float(key, f),
            FactValue::Text(t) => self.set_text(key, t),
            FactValue::TextList(items) => {
                for item in items {
                    self.add_to_text_list(key, item);
                }
            }
            FactValue::TextSet(items) => {
                for item in items {
                    self.add_to_text_set(key, item);
                }
            }
        }
    }

    // --- queries ------------------------------------------------------------

    /// Returns facts whose key matches `pattern`. Mirrors Kotlin `factsFor`:
    /// - a single `*` matches a prefix/suffix split (`"enemy.*.dead"`),
    /// - otherwise the pattern is matched as a substring (`contains`).
    pub fn query<'a>(
        &'a self,
        pattern: &'a str,
    ) -> impl Iterator<Item = (&'a String, &'a FactValue)> {
        let star_count = pattern.matches('*').count();
        let (start, end) = if star_count == 1 {
            let mut split = pattern.splitn(2, '*');
            (split.next().unwrap_or(""), split.next().unwrap_or(""))
        } else {
            ("", "")
        };
        self.map.iter().filter(move |(k, _)| {
            if star_count == 1 {
                k.starts_with(start) && k.ends_with(end)
            } else {
                k.contains(pattern)
            }
        })
    }
}
