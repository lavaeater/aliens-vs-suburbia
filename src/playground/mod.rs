//! The playground: the live game, running inside a two-pane tweaking screen.
//!
//! It deliberately does *not* have its own `GameState`. The playground runs in
//! `GameState::InGame` with a [`PlaygroundSession`] resource inserted, so every gameplay
//! system — physics, control, animation, throwing, health — works there untouched, and so
//! does every gameplay system added in the future. See `docs/playground.md` for why this
//! was chosen over a separate state.
//!
//! Only the handful of things that genuinely differ between a real match and a sandbox are
//! gated, via [`state::in_playground`] / [`state::in_normal_game`].

pub mod dummies;
pub mod plugin;
pub mod state;
pub mod ui;
