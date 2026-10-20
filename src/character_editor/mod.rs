//! The character editor: the live game, running inside a two-pane tweaking screen.
//!
//! It deliberately does *not* have its own `GameState`. The character editor runs in
//! `GameState::InGame` with a [`EditorSession`] resource inserted, so every gameplay
//! system — physics, control, animation, throwing, health — works there untouched, and so
//! does every gameplay system added in the future. See `docs/playground.md` (the
//! editor's previous life) for why this was chosen over a separate state, and
//! `docs/character-editor.md` for where it is going.
//!
//! Only the handful of things that genuinely differ between a real match and a sandbox are
//! gated, via [`state::in_editor`] / [`state::in_normal_game`].

pub mod aim_bones;
pub mod animation;
pub mod animation_tab;
pub mod debug;
pub mod doc;
pub mod focus;
pub mod hardpoints;
pub mod inspect;
pub mod library;
pub mod model_tab;
pub mod models;
pub mod plugin;
pub mod prefs;
pub mod prop;
pub mod props_tab;
pub mod shell;
pub mod state;
pub mod suggest_hardpoints;
pub mod text_field;
pub mod ui;
