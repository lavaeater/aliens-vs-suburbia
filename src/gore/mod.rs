//! The gore / over-the-top-violence layer.
//!
//! Combat systems emit [`components::DamageDealt`] and [`components::EntityDied`];
//! everything visceral (blood, gibs, scorch, wet SFX) subscribes to those. See
//! `docs/ultraviolence.md` for the full plan.

pub(crate) mod barks;
pub(crate) mod blood;
pub(crate) mod components;
pub(crate) mod fire;
pub(crate) mod gibs;
pub(crate) mod sfx;
pub(crate) mod plugin;
pub(crate) mod systems;
pub(crate) mod terrain;
