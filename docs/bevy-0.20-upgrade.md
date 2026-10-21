# Bevy 0.19 -> 0.20 upgrade analysis

Written 2026-10-21. Bevy 0.20.0 shipped 2026-10-08
([migration guide](https://bevy.org/learn/migration-guides/0-19-to-0-20/)).

## Verdict

**Not yet, and do leafwing first.** Two dependencies have no Bevy 0.20 release or branch,
and one of them (`bevy_seedling`) sits under `rusty_music`, so the soundtrack would have
to be switched off to upgrade today. The leafwing integration costs nothing extra to carry
across: leafwing 0.21 -> 0.22 is only a version bump ("updated to Bevy 0.20.0", no API
change). It also deletes the hand-rolled keyboard/gamepad code, which would otherwise have
to be ported.

Upgrade once `bevy_mod_outline` and `bevy_seedling` publish 0.20 releases. Expect about
2-4 days of mechanical work, plus a playtest pass for render-order and UI-size
regressions.

## Dependencies

| Crate | Now | Bevy 0.20 version | Status |
|---|---|---|---|
| `bevy` | 0.19.1 | 0.20.0 | released 2026-10-08 |
| `leafwing-input-manager` | 0.21 | **0.22.0** | released 2026-10-15, version bump only |
| `avian3d` | 0.7 | **0.8.0** | released 2026-10-20, has a [migration guide](https://github.com/avianphysics/avian/blob/main/migration-guides/0.7-to-0.8.md) (collider context renames, CCD rework, contact recycling) |
| `bevy-inspector-egui` | 0.37 | **0.38.0** | released 2026-10-17 (pulls `bevy_egui` 0.43) |
| `bevy_skein` | 0.6 | 0.7.0-rc.1 | RC only, but its `^0.20.0-rc.1` requirement accepts 0.20.0 |
| `bevy_mod_outline` | 0.13 | **none** | **blocker**: no release, and no `bevy-0.20` branch on master yet. Workarounds: patch it ourselves, or feature-gate the outlines (`src/player/` outline rendering) |
| `bevy_seedling` | 0.8 (git rev) | **none** | **blocker**: no release or branch. `rusty_music` depends on it |

## Our own crates (submodules and workspace members)

All of them pin `bevy = "0.19"` and need a version bump and a fix pass. Size is a rough
guide to the work.

| Crate | Lines | What 0.20 touches |
|---|---|---|
| `lava_ui_builder` | ~5.4k | The heaviest. UI-wide `Interaction` deprecation (`InteractionPalette` builds on it, which gives warnings, not errors), `BorderRadius` fields become `CornerRadius`, `Val::resolve` gains em/rem args, `TextFont` default size becomes `FontSize::Rem(1.)`, `scenes.rs` needs the BSN syntax changes (`@` scene refs, `--` separators) |
| `bevy_wind_waker_shader` | ~1.1k | **Shaders must move to WESL.** All three `.wgsl` files use `#import bevy_pbr::...` and `#ifdef PREPASS_PIPELINE`, which naga_oil handled and which no longer exist. Rename them to `.wesl`, update the module paths (`bevy_pbr::mesh_view_bindings` becomes `bevy_pbr::render::...`), and change `iter_many` (3 call sites, now yields `Result`, so use `.matched()`). Used by `spawn_aliens`, `map_systems`, `towers`, `throwing_system`, `main.rs` |
| `rusty_music` | ~17k | Blocked on `bevy_seedling`. Apart from that, mostly fine (little UI, few `Name::new`) |
| `turbofacts` | ~2.3k | `bevy_log` only. Version bump |
| `bevy_mod_inverse_kinematics` | ~0.4k | Version bump. Maybe the `bevy_math` -> `bevy_shape` split |

## The game crate (~45k lines): exposure by breaking change

Counts are grep hits in `src/` and are only an indication.

- **Shaders / WESL**: no shaders in the game itself. The risk is in `bevy_wind_waker_shader` (see above).
- **Text size** (`TextFont` default becomes `Rem(1.)`, 110 `font_size` sites): it renders the
  same at the default `RemSize` of 20, so nothing visibly changes unless someone touches
  `RemSize`. No action needed. It is an opportunity for UI scaling later.
- **`BorderRadius`** (24 game + 8 builder sites): mechanical `CornerRadius::circular` / `.into()` rewrite.
- **Flat pointer events** (`Pointer<Click>` becomes `PointerClick`): 3 sites in `poly_pizza/ui.rs`.
- **`ui::Interaction` / `ui::widgets::Button` deprecated**: the game uses feathers `Activate`
  observers almost everywhere, so this mostly hits `lava_ui_builder`. Deprecation means warnings, not errors.
- **Primitives move to `bevy_shape`, curves to `bevy_curve`**: the game uses prelude imports
  (`Cuboid`, `Sphere`, ...; ~25 sites) and no `bevy::math::primitives::` paths, so the
  prelude should cover it. Avian also moves to `bevy_shape`.
- **Exclusive systems unified** (`&mut World` anywhere in params): 1 site, should keep working.
- **`Entity::PLACEHOLDER` -> `Option<Entity>`** for `current_camera()` / `auxiliary_entity`: 2 hits to check.
- **`Name::from(&str)` needs `'static`**: 17 + 9 + 7 `Name::new` calls. Ones that pass a
  borrowed non-static `&str` need `.to_string()`.
- **"Many" query iterators yield `Result`**: only in the shader crate.
- **Built-in schedules order sets weakly (`chain_weak`)**: the `Render` / `Core3d` /
  `ExtractSchedule` / **UI sets in `PostUpdate`** now order systems only by data access. Our
  `torso_twist` / `leg_ik` / hardpoint snapping run in `PostUpdate` with explicit
  `.after(AnimationSystems)` / `.before(TransformSystems::Propagate)`, which still hold. But
  this is the class of change that compiles cleanly and then breaks silently. **Playtest the
  twist, leg IK and weapon snap after the upgrade.**
- **Escape releases text-input focus and bubbles**: `character_editor/text_field.rs` is our
  own field, not `EditableText`, so it is unaffected. Watch the pause menu if it ever gains a text field.
- **UI rendering is retained**: an internal change. Check for UI flicker or ordering differences.
- **`Sprite` now uses `Mesh2d`**: 3 hits, health bars or 2D overlays. Check draw order.
- **avian 0.8**: collider context type renames and CCD rework. Throw / projectile physics
  (`general/projectiles.rs`, `throwing_system.rs`) need a playtest because CCD behaviour changed.

## Suggested order when the blockers clear

1. Bump `turbofacts`, `bevy_mod_inverse_kinematics`, `lava_ui_builder` (each its own commit or PR in its submodule).
2. `bevy_wind_waker_shader`: WESL port. This is the only genuinely non-mechanical piece. Test each of the three materials visually.
3. `rusty_music` once `bevy_seedling` has a 0.20 rev (drop the `[patch.crates-io]` pin if it is published).
4. Game crate: bump `bevy`, `avian3d` 0.8, `bevy-inspector-egui` 0.38, `bevy_skein` 0.7,
   `leafwing-input-manager` 0.22, `bevy_mod_outline`. Fix compile errors, then run
   `cargo clippy` (the workspace denies `unwrap_used` and similar, so the `Result`-yielding
   iterators will show up there).
5. Playtest: torso twist (F7 A/B), leg IK, weapon snap in the character editor, throws and
   grenades (CCD), outlines, the HUD and menus.
