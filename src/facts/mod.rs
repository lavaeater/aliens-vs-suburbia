//! Game-side glue for the [`turbofacts`] engine. The engine itself (facts store, criteria,
//! stories, persistence) now lives in the standalone `turbofacts` crate; this module hosts
//! only [`FactsGameIntegrationPlugin`], which wires that engine into this game's ECS state,
//! and re-exports the engine API so existing `crate::facts::*` paths keep working.

pub mod game_integration;

pub use game_integration::FactsGameIntegrationPlugin;
pub use turbofacts::*;
