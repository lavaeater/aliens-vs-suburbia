# The retro look

Goal: a pixel-art finish on the 3D game, somewhere between Moebius, Hyperlight Drifter and
Return of the Obra Dinn. Built in layers, each one switchable on its own.

```
3D scene (flat shading) -> small image (camera snapped to its texels)
  -> [edge pass: depth/normal outlines]           (planned)
  -> canvas quad: grade + ink + palette + dither  (src/retro, assets/shaders/retro_canvas.wgsl)
  -> window, scaled up by whole pixels, shifted by the sub-texel remainder
```

## What is in (`src/retro/`)

- **Low-res render.** The game camera renders into an image of window / `pixel_size`
  texels (plus one texel of overscan per side). A `Camera2d` on render layer 31 draws it
  on a quad, `pixel_size` screen pixels per texel, nearest filtered.
- **Snapping** (`snap.rs`, pure and tested). Each frame, after `camera_follow`, the camera
  is moved onto its own texel grid (along its right/up axes), and the quad is shifted by
  the remainder. Pixels stay on the world, the camera still glides. Orthographic only: a
  perspective camera is left unsnapped and shimmers (P switches projection).
- **Palette + dither** (`retro_canvas.wgsl`). Each texel goes to the nearest two palette
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

**F12** opens the panel: every setting with `<` / `>` buttons, and under "Light" the sky
(moon / none / sun, same as Ctrl+L), its height and turn in degrees, its strength and extra
ambient (`SkyLightSettings` in `general/systems/lights_systems.rs`). Any change flashes a
summary at the top of the screen and is saved to `retro-settings.ron` (project root,
committed like `game-settings.ron`; palette by name). Hotkeys: **Ctrl+K** on/off, **Ctrl+J** next palette (Shift+Ctrl+J back; first is full colour),
**Ctrl+H** dither strength (0 / 0.2 / 0.35 / 0.7 / 1), **Ctrl+U / Ctrl+I** smaller / bigger pixels.

On from the start with `moebius-64` (`DEFAULT_PALETTE`). Flags: `--no-retro` (start in full colour),
`--palette <name>`, `--retro-shots <dir>` (one screenshot per palette,
then quit; run under XWayland with `env -u WAYLAND_DISPLAY` if the window is not visible,
Wayland stops drawing hidden windows).

## Next

1. Flat shading underneath: re-enable `FlatShaderPlugin` (commented out in `main.rs`) and
   compare.
2. Edge pass: depth + normal prepass on the game camera, a post pass at low res that
   draws a dark line on depth jumps and a light one on normal creases. Then try dropping
   `bevy_mod_outline`.
3. Moebius variant: hatching instead of dither in the shadow bands, wobbly ink lines,
   warm light / cool shadow.
4. Presets: save a named set of panel settings ("giraud", "rover dusk") and step through them.

Known: MSAA stays on for the game camera; switching it at runtime broke a render pass
(depth/colour sample count mismatch). The palette hides the soft edges anyway.
