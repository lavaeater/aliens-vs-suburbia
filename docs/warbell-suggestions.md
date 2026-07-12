# Warbell — what we can re-use and learn from

An analytical sweep of `example-games/warbell` (the GitHub game "Warbell", formerly
"D: Tileworld") for ideas worth porting into **aliens-vs-suburbia (AvS)**.

Warbell is a Bevy **0.19** game (AvS is on 0.18), a knight-defends-castle RTS-ish
siege game. It is striking for two reasons the author leans into hard:

1. **Almost everything visible is procedurally generated geometry** — trees, animals,
   props, ground, the hero — built in Rust as merged, vertex-coloured low-poly meshes.
   Almost no GLB asset files. (AvS is the opposite: GLB models + `AssetDefinition`.)
2. **A genuinely polished custom render pipeline** — procedural IBL, atmosphere/fog,
   screen-space god rays, custom bokeh DoF, outline, colour grade, bloom, SSAO — all on
   a single camera, with a hardware-aware quality system.

Both are directly relevant to "rendering / asset stuff." Below, ranked roughly by
value-to-effort for AvS.

---

## Tier 1 — High value, port-worthy

### 1. Hardware-aware + tunable graphics quality system (`src/quality.rs`, 812 lines)

The single most reusable, self-contained subsystem. Worth studying even if we adopt
nothing else.

- **Boots on a sensible preset by inspecting the GPU.** It reads
  `RenderAdapterInfo` → `wgpu::DeviceType` at startup and picks Low for
  integrated/virtual/CPU adapters, Ultra for discrete. No config, no first-run stutter
  on weak machines.
  - Gotcha they document: pin `wgpu` to the *same major* Bevy depends on (29.x for 0.19)
    with `default-features = false`, so cargo unifies the crate and the `DeviceType`
    types actually match. We'd pin to whatever 0.18's wgpu major is.
- **Presets are just canned fills of a flat `GraphicsSettings` struct** (one field per
  player-facing control: shadow level, AA, AO, bloom, DoF, god rays…). `apply_quality`
  translates those high-level enums into the actual render components/uniforms. Tweaking
  any one control flips the preset to `Custom`. The whole struct is `Serialize` so it
  saves cleanly — *nothing in it is a render-API type*.
- **Effects toggle by inserting/removing camera components** (e.g. add/remove the SSAO
  or god-rays component) — a clean Bevy-idiomatic on/off that we can copy directly.

**For AvS:** we currently ship one fixed look. A small version of this (detect adapter →
pick shadow-map size + MSAA/SMAA + whether outline/bloom run) would make the game
playable on laptops and is mostly mechanical to add. The `GraphicsSettings`-struct →
`apply` pattern is the part to steal wholesale.

### 2. Procedural low-poly mesh "contract" (`src/palette.rs`, `src/trees.rs`, `src/props.rs`)

This is the heart of their asset story and a real performance technique, not just an
aesthetic. The contract (documented in their CLAUDE.md and the verified-API doc):

- **One shared white `StandardMaterial` for thousands of props.** Colour lives in the
  mesh `ATTRIBUTE_COLOR` (linear RGBA), not the material. Because every instance shares
  the same material+mesh handle, Bevy **auto-batches** them into very few draw calls.
- **Build parts as primitives, `tinted()` each (add COLOR), then `Mesh::merge`** into one
  mesh per prop. Helpers are tiny and copy-pasteable (`tinted`, `merged`, `trunk_part`,
  `foliage`, `cone_at` in `trees.rs:48-95`).
- **`duplicate_vertices()` then `compute_flat_normals()`** for the crisp faceted
  low-poly look. Order matters — `compute_flat_normals` panics on an indexed mesh, so
  duplicate first.
- **Baked "painterly" per-facet shading into vertex colours** (`bake_facet_shading` in
  `trees.rs:125`): facets facing up lighten, down-facing darken, plus a vertical
  dark-skirt→lit-crown gradient. They note realtime lighting *alone* can't produce this —
  a constant ambient/IBL fill washes a low-poly blob flat. This is a cheap trick that
  makes untextured geometry look hand-painted.
- **Central palette module** (`palette.rs`): all colours as `0xRRGGBB` consts with
  `lin()`/`srgb()`/`lin_scaled()` helpers, so linear-for-mesh vs sRGB-for-UI is never
  confused. Heavily commented with *why* each colour was chosen.

**For AvS:** we are GLB-based and shouldn't rewrite our pipeline. But:
- For **cheap, plentiful decor** (rubble, debris, simple suburbia props, projectiles,
  pickups, build-preview tiles) a procedural merged-mesh + vertex-colour approach would
  add visual variety with near-zero asset cost and excellent batching.
- The **shared-material + `ATTRIBUTE_COLOR` batching insight** is worth knowing
  regardless: if we ever spawn many copies of one thing, sharing a material handle and
  tinting via vertex colour keeps draw calls down.
- The **`bake_facet_shading` trick** would visibly improve any low-poly mesh we generate.

### 3. Screenshot / clip capture harness driven by env vars (`src/capture.rs`)

Warbell can't be screen-captured externally (true for AvS too — a Bevy window). Their
solution is excellent for "did my visual change actually work" loops and for trailers:

- `FOREST_SHOT=path.png` → warms up ≥120 frames AND ≥6s (so cold pipelines / IBL settle),
  saves a PNG, exits.
- `FOREST_CLIP=dir` (+ frames/fps/warmup/orbit) → frame-sequence recorder → ffmpeg
  GIF/video, with a clamped fixed timestep so the per-frame encode stall doesn't ruin
  motion.
- A whole family of `FOREST_*` **staging** vars place the camera, set time-of-day, drop
  the player, boot a specific level/state, etc., so a single `cargo run` produces an
  exact framed shot with no interaction.
- They even run it **headless in the cloud** (Xvfb + Mesa llvmpipe software Vulkan) — see
  `.claude/skills/visual-debug-cloud/SKILL.md`.

**For AvS:** a `AVS_SHOT=...` capture-and-exit harness plus a couple of staging vars
(skip menu → straight into InGame, place camera, spawn a wave) would let us — and Claude
— verify rendering/UI changes from screenshots instead of "trust me." High value, low
effort; it's an isolated plugin.

---

## Tier 2 — Worth studying, selective borrow

### 4. The single-camera post-FX stack (`scene.rs`, `godrays.rs`, `dof.rs`, `outline.rs`, `grade.rs`, `postfx.rs`, + `assets/shaders/*.wgsl`)

A polished daytime look built entirely on Bevy components + a few custom WGSL passes:
procedural **gradient-cubemap IBL**, `Atmosphere` + `DistanceFog`, **screen-space god
rays**, a **custom CoC bokeh DoF** (over Bevy's plain DoF), outline, colour grade, bloom,
SSAO, AgX tonemapping. Custom shaders live in `assets/shaders/` (`terrain.wgsl`,
`water.wgsl`, `creature.wgsl`, `godrays.wgsl`, `dof.wgsl`, `outline.wgsl`).

Most relevant takeaways for AvS:

- **AvS already uses `bevy_mod_outline`.** Warbell's `outline.wgsl` is a self-rolled
  screen-space outline — a reference if we ever want to drop the dependency or want a
  different style, but not an obvious win over what we have.
- **Procedural IBL from a gradient cubemap** (`scene.rs`) is a cheap way to get pleasant
  ambient lighting without shipping an HDRI — relevant since AvS controls its own look.
- **Custom WGSL `Material` examples** that actually compile against Bevy are scarce; these
  six shaders are a good study set if we ever want a custom ground/water/forcefield shader.

**Hard-won warning they document (applies to us): the pipeline is SINGLE-CAMERA.** They
tried the "proper" two-camera / `RenderLayers` first-person view-model pattern and it
failed three ways (ambiguous `Single<Camera3d>` queries silently freezing the main
camera; a 2nd HDR+Tonemapping camera corrupting output — Bevy #17530; and the heavy
prepass+post stack throwing a wgpu validation error that quits the app). If AvS ever adds
a second 3D camera (minimap, portrait, view-model), read `CLAUDE.md` lines ~285-302
first.

### 5. The verified-API reference doc (`docs/specs/bevy-0-18-1-polished-static-3d-scene-verified-apis.md`)

A long, **verified-against-a-compiling-project** cheat-sheet for Bevy **0.18.1** post-FX,
fog, SSAO/TAA, lights, custom `Material` + WGSL, meshes and instancing — exactly our Bevy
version. This is genuinely useful reference material for AvS rendering work; the mesh
merge/tint/flat-normal API forms it documents are the same ones we'd use. Worth keeping a
pointer to it.

### 6. Procedurally-baked SFX synth (`src/audio/synth.rs`)

Bakes gameplay stings (pickup, level-up, shatter, chest, low-HP, thunder…) into in-memory
WAV `AudioSource`s at startup — pure Rust, deterministic xorshift noise, no asset files.
Note the `wav` Bevy feature is required or the in-memory sources panic on play.

**For AvS:** we already have generative *music* via the `rusty_music` submodule, so we're
philosophically aligned, but we don't have procedural **SFX**. This is a clean, dependency-
free way to get placeholder (or final) UI/pickup/impact stings without sourcing clips.
Each sting "graduates out" of the synth when a real recording lands — a nice workflow.

---

## Tier 3 — Architecture & process patterns

### 7. Two-crate split: pure logic vs. rendering

`crates/core` (`tileworld_core`) is **zero-dep, deterministic, `f64`, no Bevy/IO/render**
game logic (pathfinding, waves, economy, inventory, upgrades) with ~268 unit tests; the
Bevy front-end wraps those stores as Resources and mostly *drives* them. This makes the
gameplay numbers test-gated independent of the engine.

**For AvS:** our logic (wave manager, economy/`TeamWallet`, pathfinding grid, abilities)
is currently entangled with Bevy. We don't need to refactor, but the pattern is worth
remembering: anything we want to unit-test cheaply could be pulled into a tiny pure module/
crate. Lower priority — it's a big refactor for an existing codebase.

### 8. Determinism via per-tile seeded RNG

Scatter/placement uses `mulberry32` seeded per-tile, so the world is reproducible
("feels-the-same" parity, not byte-exact). If AvS ever wants reproducible procedural maps
or deterministic waves for testing, this is the lightweight approach.

### 9. Save/load as a *logic snapshot*, not an ECS dump (`src/savegame.rs`)

The world is built once and persists in-process; saving serializes the run-state
**resources** (+ a few world flags) to one JSON slot rather than dumping entities. They
codify a sharp invariant: *anything a player earns has TWO obligations — persist it AND
reset it on New Game.* If AvS adds save/continue, this resource-snapshot approach is far
simpler than trying to serialize the live ECS.

### 10. Custom Claude skills worth imitating (`.claude/skills/`)

The author ships project-specific skills that are good templates:
- **`model-viewer`** — boots an in-binary single-model viewer (`FOREST_VIEW`) on a clean
  3-point-lit stage to inspect one mesh fast (~25s vs ~6min full-game). AvS has an
  `asset_browser` and `model_showcase` already — a headless screenshot wrapper around them
  would be the equivalent.
- **`visual-debug-cloud`** — headless screenshots via Xvfb + Mesa software Vulkan.
- **`trailer-maker`** / **`release`** — frame-sequence trailer assembly and release-cutting.

These pair with the capture harness (#3); the skills are mostly thin wrappers around the
`FOREST_*` env vars.

---

## Things NOT worth copying for AvS

- **Wholesale switch to procedural geometry.** AvS's GLB + `AssetDefinition` + asset-browser
  pipeline is a real strength and a different design. Borrow the procedural technique for
  *cheap decor/props/FX only*, not for characters.
- **The full post-FX stack as-is.** It's tuned for one fixed daytime island look and a
  single camera; AvS has different needs (isometric, suburbia). Cherry-pick (IBL,
  hardware presets, capture), don't transplant.
- **`f64` everywhere.** That choice exists purely for JS/three.js parity with their dead
  original. AvS has no such constraint — stay `f32`.

---

## Suggested first steps (highest value, lowest risk)

1. **Capture harness** (`AVS_SHOT=path.png` → warm up, save PNG, exit) + 2-3 staging env
   vars. Self-contained, unlocks screenshot-based verification immediately.
2. **Hardware-aware quality preset** at startup (adapter `DeviceType` → shadow/AA/effects),
   even before a full settings UI. Big playability win on weak GPUs.
3. **Procedural vertex-coloured decor** experiment: one merged-mesh prop (e.g. rubble or a
   simple bush) sharing a white material, with `bake_facet_shading`, to validate the
   batching + look before deciding how far to take it.

Reference points in the warbell tree:
`src/quality.rs` · `src/capture.rs` · `src/palette.rs` · `src/trees.rs` · `src/props.rs` ·
`src/scene.rs` · `src/audio/synth.rs` · `assets/shaders/*.wgsl` ·
`docs/specs/bevy-0-18-1-polished-static-3d-scene-verified-apis.md` · `CLAUDE.md`.
