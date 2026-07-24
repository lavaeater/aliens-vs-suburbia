# bevy_skein for Aliens vs Suburbia — analysis & plan

Should we move model authoring (hardpoints, model types, tags) from our `.ron` defs +
in-engine asset browser toward **Blender-authored components** via
[`bevy_skein`](https://github.com/rust-adventure/skein)?

**TL;DR recommendation:** adopt it *incrementally and additively*, starting with the one
place it's clearly better than what we have — **placing hardpoint frames**. Keep the `.ron`
def system as the runtime source of truth for now. Do not rip out the asset browser. See the
staged plan at the end.

---

## Where we are today

- `bevy_skein = "0.5.0"` is **already a dependency**, and `SkeinPlugin::default()` is
  **already registered** in `main.rs:151`. But nothing uses it yet — we author no reflected
  components in Blender. It's dormant.
- Our authoring pipeline: mesh2motion (rig + animation) -> Blender -> glTF/GLB, then an
  **in-engine asset browser** writes `assets/defs/<model>.ron` (`AssetDefinition`): model
  type, scale, hidden nodes, clip tags/bindings, animation sources, attachments, and
  **hardpoints** (`HashMap<String, Hardpoint>` — role -> frame as translation + XYZ euler,
  optionally anchored to a named bone).
- Hardpoints are consumed by `src/assets/hardpoint.rs` (unit-tested quaternion math) for both
  the browser preview and in-game equip (`src/player/systems/equip.rs`,
  `src/player/systems/shoot.rs`).

## What skein actually does

1. You mark a Rust type `#[derive(Component, Reflect)] #[reflect(Component)]` and register it.
2. A Blender addon pulls the app's **type registry** and lets you attach those components to
   objects / meshes / materials as **custom properties**, edited in Blender's UI.
3. On glTF export the component data rides along in glTF **`extras`** (Bevy 0.18+ uses the
   extension format).
4. `SkeinPlugin` deserializes those extras through **reflection** on `SceneRoot(...)` load and
   inserts the components onto the spawned entities — no manual wiring.

So skein is a transport for "this glTF node carries these Bevy components", authored visually
in Blender.

## The fit for *hardpoints* (the strong case)

This is where skein shines for us. A hardpoint is fundamentally *"a frame at a place on the
model"* — exactly what a Blender **Empty** is. Instead of typing euler degrees and a bone name
into the browser and nudging numerically, you would:

- In Blender, parent an Empty to the hand bone, name/tag it, and **move it in the viewport** to
  the grip. Attach a `Hardpoint { role: "grip" }` component to it.
- On the weapon, add Empties for `grip` and `muzzle` at the barrel tip, etc.
- On spawn, skein gives you those Empties as **entities already parented at the right local
  transform** — the Empty's transform *is* the frame. No `frame_from_euler`, no
  "anchor + euler offset relative to a named bone" bookkeeping.

Advantages over the current approach:

- **Placement is WYSIWYG in a real DCC.** Positioning a muzzle at a barrel tip is a drag, not a
  guess-and-nudge in degrees (cf. the placeholder muzzle we just had to eyeball in `Pistol.ron`).
- **No euler bookkeeping.** The transform is authored directly; we stop round-tripping through
  XYZ-euler-degrees.
- **It travels with the model.** Re-export from Blender and the frames come with it.

Caveats that do **not** go away:

- **Baked bone scale still bites.** An Empty parented under a mesh2motion bone inherits that
  rig's baked scale (~0.0136 for amy), so a weapon parented to it still needs the
  `weapon_local_scale` correction we already have. Skein changes *authoring*, not the scale math.
- We'd still want the **snap math** (`hardpoint::snap_transform`) to align a weapon's grip to a
  character's grip — skein just supplies the two frames as entities instead of `.ron` data.

## The fit for the *rest* of AssetDefinition (the weaker / mixed case)

Could `model_type`, `hidden_nodes`, `clip_tags`, `animation_bindings`, `attachments` migrate to
Blender components too? Partially, and with friction:

- **`ModelType` + props (`Player`/`Weapon`/`Tower`/...):** a natural reflected component on the
  root node. Enums with struct data reflect fine. Reasonable candidate.
- **`hidden_nodes`:** could become a `Hidden` marker component on the nodes themselves — arguably
  *cleaner* than a name list. Good candidate.
- **`clip_tags` / `animation_bindings`:** these are `HashMap<String,String>` keyed by clip names
  that live in the animation GLBs, not the mesh. Reflection of maps works but authoring 17
  string->string pairs as Blender custom properties is worse UX than our current tag browser.
  **Keep in `.ron`.**
- **`animation_sources`:** external file paths — not a per-node concept. **Keep in `.ron`.**

So skein is a great fit for *spatial/per-node* data (hardpoints, hidden flags, model type) and a
poor fit for *asset-graph* data (animation wiring, external sources).

## Costs, risks, and gotchas

- **Reflection requirements:** every Blender-authored type needs `Reflect` + `reflect(Component)`
  + `app.register_type::<T>()`. Our components already lean on `Reflect` (bevy_skein-adjacent
  `bevy_skein`/inspector usage), so the muscle exists.
- **Blender addon parity:** skein depends on a Blender addon matching the crate version and the
  glTF exporter. Upstream bugs (addon or glTF spec) can break authoring. It's a young project
  (~300 stars); treat it as evolving.
- **Two sources of truth:** if some data is in `.ron` and some in glTF extras, "where is this
  configured?" gets muddier. Mitigate by drawing a clear line (spatial -> Blender, asset-graph
  -> ron) and documenting it.
- **We'd lose in-engine live authoring** for whatever moves to Blender — the asset browser edits
  and previews inside the actual renderer with the actual camera. Blender round-trips
  (edit -> export -> reload) are slower per iteration, though Blender itself is a better 3D tool.
- **mesh2motion step:** need to confirm the mesh2motion -> Blender import preserves the bone
  hierarchy names we rely on and that the glTF exporter emits extras. One spike model settles this.

## Recommendation

Adopt skein **for hardpoint placement first**, as an *optional, additive input* — not a
replacement. Concretely: let Blender Empties tagged with a hardpoint role **populate the same
`hardpoints` map** we already consume, so the runtime (equip, shoot, snap math, tests) is
untouched. This gets the big win (visual frame placement) at low risk and keeps a single runtime
representation. Expand to `ModelType`/`hidden_nodes` only if the first stage feels good.

Reasons not to go all-in now:

- Our hardpoint system is **working, unit-tested, and WYSIWYG in-engine** — the thing skein would
  replace is the least broken part.
- Guns/gore momentum (see `ultraviolence.md`) is higher-value than an authoring-pipeline refactor.
- Additive adoption is reversible; a full migration is not.

## Staged plan (to consider later)

**Stage 0 — Spike (½ day).** Add a trivial reflected marker component, author it on one Empty in
one Blender file, export, confirm `SkeinPlugin` instantiates it on load. Verifies the whole
mesh2motion -> Blender -> extras -> Bevy path end to end. Go/no-go gate.

**Stage 1 — Hardpoints from Blender (2–3 days).**
- Define `#[derive(Component, Reflect)] #[reflect(Component)] struct HardpointMarker { role: String }`
  and `register_type`.
- Author `grip`/`muzzle`/`foregrip` Empties on amy + the pistol in Blender.
- On scene spawn, a system walks the spawned entities, and for each `HardpointMarker` reads its
  `Transform` (relative to its parent bone / model origin) and writes an entry into the existing
  `hardpoints` map (or a runtime equivalent). Everything downstream (`snap_transform`, equip,
  shoot, the 11 tests) stays as-is.
- Keep the asset browser as the fallback authoring path; defs without Blender hardpoints keep
  working.

**Stage 2 — Evaluate (½ day).** Compare authoring a new weapon's frames in Blender vs the
browser. If Blender wins decisively, proceed; if not, stop here — we've lost nothing.

**Stage 3 — Optional: `ModelType` + `hidden_nodes` via skein (2–3 days).** Author model type and
per-node hidden flags in Blender for new models; keep `.ron` as fallback and for animation data.
Draw the documented line: **spatial/per-node -> Blender, asset-graph/animation -> `.ron`.**

**Explicitly out of scope:** moving `clip_tags`, `animation_bindings`, or `animation_sources` to
Blender — they're asset-graph data and authored better in our tag browser.
