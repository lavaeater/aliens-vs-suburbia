# Firewheel 0.12 patched for Bevy 0.20

Copies of the published Firewheel 0.12.x crates (from crates.io) with only their
Bevy dependency versions bumped from 0.19.0 to 0.20.1 (plus any source fixes
listed below). Used by `../bevy_seedling` via `[patch.crates-io]` until
upstream Firewheel releases Bevy 0.20 support. Delete once that happens.

## Changes from the published crates

- All `bevy_*` / `bevy_macro_utils` dependencies: `0.19.0` -> `0.20.1`.
- `firewheel` (umbrella, 0.12.1): only adds a `glam-33` feature forwarding to `firewheel-core/glam-33`.
- `firewheel-core`:
  - New `glam-33` feature (Bevy 0.20 uses glam 0.33), mirroring every `glam-32` impl in
    `src/event.rs`, `src/vector.rs`, `src/diff/leaf.rs`.
  - `src/diff/notify.rs`: hand-written `Reflect` impl ported to bevy_reflect 0.20
    (`FromType::from_type()` -> `CreateTypeData::create_type_data(())`,
    `to_dynamic_struct` / `to_dynamic` now return `Result`).

Note: path dependencies aren't lint-capped, so `RUSTFLAGS=--deny warnings` will fail on
two pre-existing unused-import warnings in `firewheel-graph` for wasm32 builds.
