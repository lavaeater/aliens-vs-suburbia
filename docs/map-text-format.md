# The `.map` text format

A hand-editable companion to the RON maps in `assets/maps/`. Same data (`MapFile`), one
letter per tile, editable in any text editor. Code: `src/map/text_format.rs`.

## Using it

- **Convert:** `cargo run -- --convert-map assets/maps/level_01.ron` writes
  `assets/maps/level_01.map` (and the other way round for a `.map`). Pass an output path
  as a second argument to put it elsewhere. Existing files are never overwritten without
  `--force`. Conversion is lossless both ways.
- **Drop-in:** when the game loads a level by its `.ron` path (`level_01`, the editor's
  `playground`), a `.map` with the same name next to it is used instead. Delete or rename
  the `.map` to go back to the `.ron`.
- The TUI map editor (`--map-editor --file foo.map`) opens and saves either format by
  extension.

## Layout

```text
# Lines starting with # are comments. Blank lines are ignored.
[properties]
name: Backyard

[tiles]
wwwwwwwww
wpffffffw
wffvvfffe
wxfffAAfw

[legend]
A: grass + blocks_enemies

[decorations]
at: 3, 1 | model: packs/nature/Pine.glb | rotation: 90 | scale: 1.2

[placements]
at: 2, 2 | def: assets/defs/door.ron | rotation: 45

[waves]
enemy: assets/defs/Alien.ron | count: 5 | rate: 10
count: 20 | rate: 30
```

### `[properties]`

| key | meaning |
|-----|---------|
| `name` | display name (optional) |
| `seed` | random seed (optional) |
| `generated` | `true` = ignore `[tiles]` and generate the map from `seed` |
| `width`, `height` | size of a *generated* map only. A hand-made map's size comes from `[tiles]`: the longest row and the number of rows. |

### `[tiles]`

One line per row, one character per tile. Spaces inside a row are ignored (use them to
line things up); rows shorter than the longest are padded with void. Everything except
`v` has a floor under it.

| letter | tile |
|--------|------|
| `v` | void (nothing) |
| `f` | floor |
| `w` | wall (floor, blocks everyone) |
| `e` | enemy spawn |
| `p` | player spawn |
| `x` | goal (enemy exit) |
| `g` / `m` / `s` / `r` / `~` | grass / mud / snow / rock / water |

### `[legend]`

Defines extra letters, or redefines the built-ins: `<letter>: <flag> + <flag> ...`, or a
raw tile number (`A: 261`). Flags: `nothing`, `water`, `grass`, `floor`, `mud`, `snow`,
`rock`, `blocks_players`, `blocks_enemies`, `enemy_spawn`, `player_spawn`, `enemy_exit`.
Converting a RON map writes one of these for every tile value that has no built-in
letter.

### `[decorations]`, `[placements]`, `[waves]`

One entry per line: `key: value` fields separated by `|`, in any order. Values can
contain spaces (paths often do) but not `|`. An unknown key is an error.

| section | fields |
|---------|--------|
| decorations | `at: x, y` + `model:` (path under `assets/`), optional `rotation:` (degrees) and `scale:` (default 1) |
| placements | `at: x, y` + `def:` (a def `.ron`), optional `rotation:` (degrees, multiples of 45) |
| waves | optional `enemy:` (enemy def; omitted = the built-in alien), `count:` (default 10), `rate:` (spawns per minute, default 20) |

`x` is the column and `y` the row, both counted from 0 at the top left of `[tiles]`.
