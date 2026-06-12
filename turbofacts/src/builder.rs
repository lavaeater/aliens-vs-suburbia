//! Ergonomic constructors for stories and rules — the Rust analog of Kotlin's
//! `TurboStoryBuilder` / `TurboRuleBuilder` DSL.
//!
//! ```ignore
//! let s = story("Level Complete")
//!     .exclusive(true)
//!     .rule("win", |r| {
//!         r.is_true(keys::LEVEL_STARTED)
//!          .is_true(keys::BOSS_IS_DEAD)
//!          .is_true(keys::ALL_OBJECTIVES_TOUCHED);
//!     })
//!     .set_true(keys::LEVEL_COMPLETE)
//!     .emit("level_complete")
//!     .build();
//! ```

use super::consequence::Consequence;
use super::criterion::{Criterion, NumOp};
use super::fact_value::FactValue;
use super::story::{Rule, Story};

/// Entry point: start building a story with the given name.
pub fn story(name: impl Into<String>) -> StoryBuilder {
    StoryBuilder::new(name)
}

pub struct StoryBuilder {
    name: String,
    description: String,
    repeat: bool,
    exclusive: bool,
    rules: Vec<Rule>,
    consequences: Vec<Consequence>,
    init_facts: Vec<(String, FactValue)>,
}

impl StoryBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        StoryBuilder {
            name: name.into(),
            description: String::new(),
            repeat: true,
            exclusive: false,
            rules: Vec::new(),
            consequences: Vec::new(),
            init_facts: Vec::new(),
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn repeat(mut self, repeat: bool) -> Self {
        self.repeat = repeat;
        self
    }

    pub fn exclusive(mut self, exclusive: bool) -> Self {
        self.exclusive = exclusive;
        self
    }

    /// Adds a named rule, configured via a closure on a [`RuleBuilder`].
    pub fn rule(mut self, name: impl Into<String>, build: impl FnOnce(&mut RuleBuilder)) -> Self {
        let mut rb = RuleBuilder {
            name: name.into(),
            criteria: Vec::new(),
        };
        build(&mut rb);
        self.rules.push(Rule {
            name: rb.name,
            criteria: rb.criteria,
        });
        self
    }

    // --- init facts (seeded silently on activation) -------------------------

    pub fn init_bool(mut self, key: impl Into<String>, value: bool) -> Self {
        self.init_facts.push((key.into(), FactValue::Bool(value)));
        self
    }

    pub fn init_int(mut self, key: impl Into<String>, value: i64) -> Self {
        self.init_facts.push((key.into(), FactValue::Int(value)));
        self
    }

    pub fn init_float(mut self, key: impl Into<String>, value: f32) -> Self {
        self.init_facts.push((key.into(), FactValue::Float(value)));
        self
    }

    pub fn init_text(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.init_facts
            .push((key.into(), FactValue::Text(value.into())));
        self
    }

    // --- consequences -------------------------------------------------------

    pub fn set_fact(mut self, key: impl Into<String>, value: FactValue) -> Self {
        self.consequences.push(Consequence::SetFact {
            key: key.into(),
            value,
        });
        self
    }

    pub fn set_true(self, key: impl Into<String>) -> Self {
        self.set_fact(key, FactValue::Bool(true))
    }

    pub fn set_false(self, key: impl Into<String>) -> Self {
        self.set_fact(key, FactValue::Bool(false))
    }

    pub fn set_int(self, key: impl Into<String>, value: i64) -> Self {
        self.set_fact(key, FactValue::Int(value))
    }

    pub fn add_int(mut self, key: impl Into<String>, delta: i64) -> Self {
        self.consequences.push(Consequence::AddInt {
            key: key.into(),
            delta,
        });
        self
    }

    pub fn emit(mut self, effect: impl Into<String>) -> Self {
        self.consequences.push(Consequence::Emit {
            effect: effect.into(),
        });
        self
    }

    pub fn build(self) -> Story {
        let mut s = Story::new(self.name, self.rules, self.consequences);
        s.description = self.description;
        s.repeat = self.repeat;
        s.exclusive = self.exclusive;
        s.init_facts = self.init_facts;
        s
    }
}

/// Accumulates [`Criterion`]s for a single rule. Methods mirror the Kotlin
/// `TurboRuleBuilder` extension functions.
pub struct RuleBuilder {
    name: String,
    criteria: Vec<Criterion>,
}

impl RuleBuilder {
    fn push(&mut self, c: Criterion) -> &mut Self {
        self.criteria.push(c);
        self
    }

    // booleans
    pub fn is_true(&mut self, key: impl Into<String>) -> &mut Self {
        self.push(Criterion::BoolIs { key: key.into(), expected: true })
    }
    pub fn is_false(&mut self, key: impl Into<String>) -> &mut Self {
        self.push(Criterion::BoolIs { key: key.into(), expected: false })
    }
    pub fn any_true(&mut self, pattern: impl Into<String>) -> &mut Self {
        self.push(Criterion::AnyBool { pattern: pattern.into(), expected: true })
    }
    pub fn any_false(&mut self, pattern: impl Into<String>) -> &mut Self {
        self.push(Criterion::AnyBool { pattern: pattern.into(), expected: false })
    }
    pub fn all_true(&mut self, pattern: impl Into<String>) -> &mut Self {
        self.push(Criterion::AllBool { pattern: pattern.into(), expected: true })
    }
    pub fn all_false(&mut self, pattern: impl Into<String>) -> &mut Self {
        self.push(Criterion::AllBool { pattern: pattern.into(), expected: false })
    }

    // ints
    pub fn int_more_than(&mut self, key: impl Into<String>, value: i64) -> &mut Self {
        self.push(Criterion::IntCmp { key: key.into(), op: NumOp::Gt, value })
    }
    pub fn int_less_than(&mut self, key: impl Into<String>, value: i64) -> &mut Self {
        self.push(Criterion::IntCmp { key: key.into(), op: NumOp::Lt, value })
    }
    pub fn int_equals(&mut self, key: impl Into<String>, value: i64) -> &mut Self {
        self.push(Criterion::IntCmp { key: key.into(), op: NumOp::Eq, value })
    }
    pub fn int_more_than_fact(&mut self, lhs: impl Into<String>, rhs: impl Into<String>) -> &mut Self {
        self.push(Criterion::IntVsInt { lhs: lhs.into(), op: NumOp::Gt, rhs: rhs.into() })
    }
    pub fn int_less_than_fact(&mut self, lhs: impl Into<String>, rhs: impl Into<String>) -> &mut Self {
        self.push(Criterion::IntVsInt { lhs: lhs.into(), op: NumOp::Lt, rhs: rhs.into() })
    }

    // floats
    pub fn float_more_than(&mut self, key: impl Into<String>, value: f32) -> &mut Self {
        self.push(Criterion::FloatCmp { key: key.into(), op: NumOp::Gt, value })
    }
    pub fn float_less_than(&mut self, key: impl Into<String>, value: f32) -> &mut Self {
        self.push(Criterion::FloatCmp { key: key.into(), op: NumOp::Lt, value })
    }

    // text
    pub fn text_equals(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.push(Criterion::TextEq { key: key.into(), value: value.into() })
    }
    pub fn text_contains(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.push(Criterion::TextContains { key: key.into(), value: value.into() })
    }

    // collections
    pub fn list_contains(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.push(Criterion::ListContains { key: key.into(), value: value.into() })
    }
    pub fn list_size_more_than(&mut self, key: impl Into<String>, value: usize) -> &mut Self {
        self.push(Criterion::ListSize { key: key.into(), op: NumOp::Gt, value })
    }
    pub fn list_size_equals(&mut self, key: impl Into<String>, value: usize) -> &mut Self {
        self.push(Criterion::ListSize { key: key.into(), op: NumOp::Eq, value })
    }
    pub fn set_contains(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.push(Criterion::SetContains { key: key.into(), value: value.into() })
    }
    pub fn set_size_equals(&mut self, key: impl Into<String>, value: usize) -> &mut Self {
        self.push(Criterion::SetSize { key: key.into(), op: NumOp::Eq, value })
    }
}
