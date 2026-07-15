pub mod plugin;
pub mod state;
pub mod ui;
pub mod viewer;

/*
Hierarchical animation handling (implemented).

We don't control which clips a given model ships. So rather than picking, per
game-animation, which model clip to use, the browser lists every clip in the
model and lets you tag each with a free-form hierarchical path (e.g.
"Combat/Ranged/Shoot"). Click a clip in the "-- Clip Tags --" list to open a text
box; typing offers autocomplete chips from tag paths already in use.

Game animation keys are bound to tag paths via `animation_bindings`. At runtime
`AssetDefinition::resolved_clip` maps a game key -> bound tag path -> the clip
carrying that tag (tag paths are unique per clip). Persisted as `clip_tags` +
`animation_bindings` in the def; the legacy flat `animation_mapping` is still read
as a fallback and auto-migrated into tags/bindings when an old def is loaded.

NOTE: the in-browser bindings editor ("-- Key Bindings --" list, cycle with < / >)
is currently hidden — see `rebuild_binding_list` / `BindingContainer` in ui.rs,
kept for a future re-enable. Bindings still resolve from the persisted def.
 */