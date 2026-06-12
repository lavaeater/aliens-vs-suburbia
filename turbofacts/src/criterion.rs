use serde::{Deserialize, Serialize};

use super::facts_resource::Facts;

/// Numeric comparison operator. `op.apply(a, b)` reads "a <op> b".
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum NumOp {
    Lt,
    Gt,
    Eq,
}

impl NumOp {
    pub fn apply_i64(self, a: i64, b: i64) -> bool {
        match self {
            NumOp::Lt => a < b,
            NumOp::Gt => a > b,
            NumOp::Eq => a == b,
        }
    }

    pub fn apply_f32(self, a: f32, b: f32) -> bool {
        match self {
            NumOp::Lt => a < b,
            NumOp::Gt => a > b,
            NumOp::Eq => a == b,
        }
    }

    pub fn apply_usize(self, a: usize, b: usize) -> bool {
        match self {
            NumOp::Lt => a < b,
            NumOp::Gt => a > b,
            NumOp::Eq => a == b,
        }
    }

    fn token(self) -> &'static str {
        match self {
            NumOp::Lt => "LessThan",
            NumOp::Gt => "MoreThan",
            NumOp::Eq => "Equals",
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
            Criterion::BoolIs { key, expected } => facts.bool(key) == *expected,
            Criterion::AnyBool { pattern, expected } => facts
                .query(pattern)
                .filter_map(|(_, v)| v.as_bool())
                .any(|b| b == *expected),
            Criterion::AllBool { pattern, expected } => {
                let mut bools = facts.query(pattern).filter_map(|(_, v)| v.as_bool()).peekable();
                // vacuously false when there are no matching bool facts, matching the
                // intent that "all X are true" requires at least one X.
                bools.peek().is_some() && bools.all(|b| b == *expected)
            }

            Criterion::IntCmp { key, op, value } => op.apply_i64(facts.int(key), *value),
            Criterion::AnyIntCmp { pattern, op, value } => facts
                .query(pattern)
                .filter_map(|(_, v)| v.as_int())
                .any(|i| op.apply_i64(i, *value)),
            Criterion::AllIntCmp { pattern, op, value } => {
                let mut ints = facts.query(pattern).filter_map(|(_, v)| v.as_int()).peekable();
                ints.peek().is_some() && ints.all(|i| op.apply_i64(i, *value))
            }
            Criterion::IntVsInt { lhs, op, rhs } => op.apply_i64(facts.int(lhs), facts.int(rhs)),

            Criterion::FloatCmp { key, op, value } => op.apply_f32(facts.float(key), *value),

            Criterion::TextEq { key, value } => facts.text(key) == value,
            Criterion::TextContains { key, value } => facts.text(key).contains(value.as_str()),
            Criterion::AnyTextEq { pattern, value } => facts
                .query(pattern)
                .filter_map(|(_, v)| v.as_text())
                .any(|t| t == value),
            Criterion::AllTextEq { pattern, value } => {
                let mut texts = facts.query(pattern).filter_map(|(_, v)| v.as_text()).peekable();
                texts.peek().is_some() && texts.all(|t| t == value)
            }

            Criterion::ListContains { key, value } => {
                facts.text_list(key).iter().any(|v| v == value)
            }
            Criterion::ListSize { key, op, value } => {
                op.apply_usize(facts.text_list(key).len(), *value)
            }

            Criterion::SetContains { key, value } => facts.text_set_contains(key, value),
            Criterion::SetSize { key, op, value } => op.apply_usize(facts.text_set_len(key), *value),
        }
    }

    /// A round-trippable text token for the line-oriented map story format. `None` for
    /// variants the text format does not (yet) support.
    pub fn to_token(&self) -> Option<String> {
        Some(match self {
            Criterion::BoolIs { key, expected } => {
                format!("bool{} {}", if *expected { "True" } else { "False" }, key)
            }
            Criterion::IntCmp { key, op, value } => format!("int{} {} {}", op.token(), key, value),
            Criterion::IntVsInt { lhs, op, rhs } => {
                format!("intVsInt{} {} {}", op.token(), lhs, rhs)
            }
            Criterion::FloatCmp { key, op, value } => {
                format!("float{} {} {}", op.token(), key, value)
            }
            Criterion::TextEq { key, value } => format!("textEquals {} {}", key, value),
            Criterion::TextContains { key, value } => format!("textContains {} {}", key, value),
            Criterion::ListContains { key, value } => format!("listContains {} {}", key, value),
            Criterion::ListSize { key, op, value } => {
                format!("listSize{} {} {}", op.token(), key, value)
            }
            Criterion::SetContains { key, value } => format!("setContains {} {}", key, value),
            Criterion::SetSize { key, op, value } => {
                format!("setSize{} {} {}", op.token(), key, value)
            }
            // Query-scoped variants are not emitted to the terse text format.
            Criterion::AnyBool { .. }
            | Criterion::AllBool { .. }
            | Criterion::AnyIntCmp { .. }
            | Criterion::AllIntCmp { .. }
            | Criterion::AnyTextEq { .. }
            | Criterion::AllTextEq { .. } => return None,
        })
    }
}
