# The retro look

Goal: a pixel-art finish on the 3D game, somewhere between Moebius, Hyperlight Drifter and
Return of the Obra Dinn. Built in layers, each one switchable on its own.

```
3D scene (flat shading) -> small image (camera snapped to its texels)
  -> [edge pass: depth/normal outlines]           (planned)
  -> canvas quad: grade + ink + palette + dither  (src/retro, assets/shaders/retro_canvas.wesl)
  -> window, scaled up by whole pixels, shifted by the sub-texel remainder
```

## Looks (`mode.rs`, `shading.rs`)

The canvas is one part of the game's look. `LookMode` (the top row of the F12 panel,
**Ctrl+K** / Shift+Ctrl+K, `--look <plain|retro|toon|toon-retro|psx>` for one run):

| Look | Meshes drawn with | Picture |
|---|---|---|
| Plain | `StandardMaterial` | straight to the window |
| Retro | `StandardMaterial` | the retro canvas (below) |
| Toon | flat toon shader (`bevy_wind_waker_shader::flat`) | straight to the window |
| Toon + retro | flat toon shader | the retro canvas |
| PSX | `bevy_psx`'s material (vertex snapping, colour steps) | the canvas as a plain low-res upscale |

Toon and PSX are material swaps (`shading.rs`), done and undone at any time, in a match only
(menus and editors stay on `StandardMaterial`). Every mesh keeps the material it was
given in `BaseMaterial` (recorded on every insert of a `MeshMaterial3d<StandardMaterial>`)
and is drawn with one toon or PSX copy per base material; an edit of the base (a fade, a
tint) is copied into it. So: **fade or tint a mesh through `BaseMaterial`'s handle, never
`MeshMaterial3d<StandardMaterial>`** (a toon or PSX mesh has none), and swap a material by
inserting a new `MeshMaterial3d<StandardMaterial>`.

- **Toon** rows: shadow (brightness of the shadow side) and cool (leans it blue-violet),
  light and warm (the lit side), levels (posterize per channel, off..32), dark at (below
  this light level the surface bands down to black: unlit ground at night is dark; off =
  never darker than the shadow), lamp hue (how much coloured lamps tint). The two-band
  split itself is the shader's `ZAtoon.png` ramp.
- **PSX** rows: pixels (of the low-res picture), wobble (vertex snap grid, lower wobbles
  more), colours (steps per channel), palette (any of ours, matched in linear light), dither
  and its pattern. `bevy_psx`'s own plugin is not used (it spawns an upscale camera and
  converts every material for good); its material shader is loaded from a copy,
  `src/retro/psx_material.wesl`, under the handle the material asks for. Copy it again
  when the `bevy_psx` rev changes.
- **Scene** rows in every look: outline width (0 off), fog, dread (`DespairSettings::max_strength`),
  grass. Fog, dread and grass are for the run; everything else is saved.

## The retro canvas (`src/retro/`)

- **Low-res render.** The game camera renders into an image of window / `pixel_size`
  texels (plus one texel of overscan per side). A `Camera2d` on render layer 31 draws it
  on a quad, `pixel_size` screen pixels per texel, nearest filtered.
- **Snapping** (`snap.rs`, pure and tested). Each frame, after `camera_follow`, the camera
  is moved onto its own texel grid (along its right/up axes), and the quad is shifted by
  the remainder. Pixels stay on the world, the camera still glides. Orthographic only: a
  perspective camera is left unsnapped and shimmers (P switches projection).
- **Palette + dither** (`retro_canvas.wesl`). Each texel goes to the nearest two palette
  colours in OKLab and an 8x8 Bayer threshold picks between them by how far between them
  it lies. The pattern is offset by the camera's texel index, so it does not swim when
  the camera pans.
- **Palettes** are Lospec `.hex` files in `assets/palettes/` (one `rrggbb` per line, `;` comments, max
  256). Drop any palette from lospec.com in and it shows up in the cycle. The bigger ones
  (`suburb-*`, `drifter-*`, `moebius-*`, the number is the colour count) come from
  `tools/make_palettes.py`: ramps dark to light, even in OKLab lightness, shadows leaning
  blue and highlights yellow; edit the ramp tables there and rerun.
- **Grade** (in the shader, OKLab, before the palette): exposure, contrast, colour
  (chroma), violet (shadows lean violet and lift a little: the Giraud / Sable night),
  warmth (highlights lean warm).
- **Ink**: a line on the dark side of every lightness step bigger than "ink edge" in the
  low-res picture, so it is one texel thick. Colour edges only: two surfaces of the same
  colour get no line between them; the depth/normal pass below would fix that.
- **Palettes from pictures**: `tools/extract_palette.py <png> <name> <count>` (k-means in
  OKLab, chroma-weighted so small bright accents survive). `giraud-*` and `rover-*` come
  from `docs/gfx-reference/`. The generated `*-128` sets are 16 ramps x 8.
- **Mouse aim** goes through `RetroView::cursor_to_viewport`, the window cursor mapped
  into the small image.
- Off in the character editor (its camera is clipped to a pane).

**F12** opens the panel: the look, then every setting of it with `<` / `>` buttons (only the
sections the look uses), the scene, and under "Light" the sky
(moon / none / sun, same as Ctrl+L), its height and turn in degrees, its strength and extra
ambient (`SkyLightSettings` in `general/systems/lights_systems.rs`). Any change flashes a
value at the top of the screen and is saved to `retro-settings.ron` (project root,
committed like `game-settings.ron`; palettes by name; `mode`, `toon`, `psx`, `outline`). Hotkeys
(`look_keys`, each a panel row, Shift steps back): **Ctrl+K** next look, **Ctrl+J** next palette (first is full colour),
**Ctrl+H** dither strength (0 / 0.2 / 0.35 / 0.7 / 1), **Ctrl+U / Ctrl+I** smaller / bigger pixels
(PSX's in PSX), **Ctrl+L** sky, **Ctrl+O** fog, **Ctrl+Y** grass.

Retro with `moebius-64` (`DEFAULT_PALETTE`) when there is no file. Flags: `--no-retro` / `--plain` (Plain for the run), `--psx` (PSX),
`--palette <name>`, `--retro-shots <dir>` (one screenshot per palette,
then quit; run under XWayland with `env -u WAYLAND_DISPLAY` if the window is not visible,
Wayland stops drawing hidden windows).

## Next

1. Flat shading underneath: in, as the Toon + retro look. Tune it.
2. Edge pass: depth + normal prepass on the game camera, a post pass at low res that
   draws a dark line on depth jumps and a light one on normal creases. Then try dropping
   `bevy_mod_outline`.
3. Moebius variant: hatching instead of dither in the shadow bands, wobbly ink lines,
   warm light / cool shadow.
4. Presets: save a named set of panel settings ("giraud", "rover dusk") and step through them.

Known: MSAA stays on for the game camera; switching it at runtime broke a render pass
(depth/colour sample count mismatch). The palette hides the soft edges anyway.
