use serde::{Deserialize, Serialize};

use super::facts_resource::Facts;

/// Numeric comparison operator. `op.apply(a, b)` reads "a <op> b".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumOp {
    Lt,
    Gt,
    Eq,
}

impl NumOp {
    pub const fn apply_i64(self, a: i64, b: i64) -> bool {
        match self {
            Self::Lt => a < b,
            Self::Gt => a > b,
            Self::Eq => a == b,
        }
    }

    pub fn apply_f32(self, a: f32, b: f32) -> bool {
        match self {
            Self::Lt => a < b,
            Self::Gt => a > b,
            Self::Eq => (a-b).abs()< f32::EPSILON,
        }
    }

    pub const fn apply_usize(self, a: usize, b: usize) -> bool {
        match self {
            Self::Lt => a < b,
            Self::Gt => a > b,
            Self::Eq => a == b,
        }
    }

    const fn token(self) -> &'static str {
        match self {
            Self::Lt => "LessThan",
            Self::Gt => "MoreThan",
            Self::Eq => "Equals",
        }
    }
}

/// A predicate over the [`Facts`] store. The Rust analog of Kotlin's `Criterion` sealed
/// hierarchy, collapsed into a single serializable enum keyed on `{ scope, op, value }`.
///
/// "Any"/"All" variants run over a key pattern via [`Facts::query`] (the `factsFor` analog);
/// the single variants read one key, treating a missing fact as its type default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Criterion {
    BoolIs { key: String, expected: bool },
    AnyBool { pattern: String, expected: bool },
    AllBool { pattern: String, expected: bool },

    IntCmp { key: String, op: NumOp, value: i64 },
    AnyIntCmp { pattern: String, op: NumOp, value: i64 },
    AllIntCmp { pattern: String, op: NumOp, value: i64 },
    IntVsInt { lhs: String, op: NumOp, rhs: String },

    FloatCmp { key: String, op: NumOp, value: f32 },

    TextEq { key: String, value: String },
    TextContains { key: String, value: String },
    AnyTextEq { pattern: String, value: String },
    AllTextEq { pattern: String, value: String },

    ListContains { key: String, value: String },
    ListSize { key: String, op: NumOp, value: usize },

    SetContains { key: String, value: String },
    SetSize { key: String, op: NumOp, value: usize },
}

impl Criterion {
    pub fn evaluate(&self, facts: &Facts) -> bool {
        match self {
            Self::BoolIs { key, expected } => facts.bool(key) == *expected,
            Self::AnyBool { pattern, expected } => facts
                .query(pattern)
                .filter_map(|(_, v)| v.as_bool())
                .any(|b| b == *expected),
            Self::AllBool { pattern, expected } => {
                let mut bools = facts.query(pattern).filter_map(|(_, v)| v.as_bool()).peekable();
                // vacuously false when there are no matching bool facts, matching the
                // intent that "all X are true" requires at least one X.
                bools.peek().is_some() && bools.all(|b| b == *expected)
            }

            Self::IntCmp { key, op, value } => op.apply_i64(facts.int(key), *value),
            Self::AnyIntCmp { pattern, op, value } => facts
                .query(pattern)
                .filter_map(|(_, v)| v.as_int())
                .any(|i| op.apply_i64(i, *value)),
            Self::AllIntCmp { pattern, op, value } => {
                let mut ints = facts.query(pattern).filter_map(|(_, v)| v.as_int()).peekable();
                ints.peek().is_some() && ints.all(|i| op.apply_i64(i, *value))
            }
            Self::IntVsInt { lhs, op, rhs } => op.apply_i64(facts.int(lhs), facts.int(rhs)),

            Self::FloatCmp { key, op, value } => op.apply_f32(facts.float(key), *value),

            Self::TextEq { key, value } => facts.text(key) == value,
            Self::TextContains { key, value } => facts.text(key).contains(value.as_str()),
            Self::AnyTextEq { pattern, value } => facts
                .query(pattern)
                .filter_map(|(_, v)| v.as_text())
                .any(|t| t == value),
            Self::AllTextEq { pattern, value } => {
                let mut texts = facts.query(pattern).filter_map(|(_, v)| v.as_text()).peekable();
                texts.peek().is_some() && texts.all(|t| t == value)
            }

            Self::ListContains { key, value } => {
                facts.text_list(key).iter().any(|v| v == value)
            }
            Self::ListSize { key, op, value } => {
                op.apply_usize(facts.text_list(key).len(), *value)
            }

            Self::SetContains { key, value } => facts.text_set_contains(key, value),
            Self::SetSize { key, op, value } => op.apply_usize(facts.text_set_len(key), *value),
        }
    }

    /// A round-trippable text token for the line-oriented map story format. `None` for
    /// variants the text format does not (yet) support.
    pub fn to_token(&self) -> Option<String> {
        Some(match self {
            Self::BoolIs { key, expected } => {
                format!("bool{} {}", if *expected { "True" } else { "False" }, key)
            }
            Self::IntCmp { key, op, value } => format!("int{} {} {}", op.token(), key, value),
            Self::IntVsInt { lhs, op, rhs } => {
                format!("intVsInt{} {} {}", op.token(), lhs, rhs)
            }
            Self::FloatCmp { key, op, value } => {
                format!("float{} {} {}", op.token(), key, value)
            }
            Self::TextEq { key, value } => format!("textEquals {key} {value}"),
            Self::TextContains { key, value } => format!("textContains {key} {value}"),
            Self::ListContains { key, value } => format!("listContains {key} {value}"),
            Self::ListSize { key, op, value } => {
                format!("listSize{} {} {}", op.token(), key, value)
            }
            Self::SetContains { key, value } => format!("setContains {key} {value}"),
            Self::SetSize { key, op, value } => {
                format!("setSize{} {} {}", op.token(), key, value)
            }
            // Query-scoped variants are not emitted to the terse text format.
            Self::AnyBool { .. }
            | Self::AllBool { .. }
            | Self::AnyIntCmp { .. }
            | Self::AllIntCmp { .. }
            | Self::AnyTextEq { .. }
            | Self::AllTextEq { .. } => return None,
        })
    }
}
