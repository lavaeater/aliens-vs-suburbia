# The `.map` text format

The format the levels are kept in (`assets/maps/*.map`): the same data as a RON map
(`MapFile`), one letter per tile, editable in any text editor. Code:
`src/map/text_format.rs`. The map editor saves `.map`; RON still reads and writes, for
tools and for later.

Paths in a map (defs, stories, weapons, loot bags) are inside `assets/`, without the
prefix: `defs/Barrel.ron`, like a def's `model_path`. Older maps wrote
`assets/defs/Barrel.ron`; that still reads, and is saved in the short form
(`src/assets/paths.rs`). `cargo run -- --validate` checks every one of them.

## Using it

- **Convert:** `cargo run -- --convert-map assets/maps/level_01.map` writes
  `assets/maps/level_01.ron` (and the other way round for a `.ron`). Pass an output path
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
at: 2, 2 | def: defs/door.ron | rotation: 45

[houses]
house: bungalow | at: 4, 6 | rotation: 90

[waves]
enemy: defs/Alien.ron | count: 5 | rate: 10
count: 20 | rate: 30
```

### `[properties]`

| key | meaning |
|-----|---------|
| `name` | display name (optional) |
| `stories` | the level's turbofacts story file, e.g. `stories/level_01.ron` (optional; see `docs/crawls.md`) |
| `loot` | random loot bags on the open floor: `1-5 per 20` = 1 to 5 bags for every 20 open tiles (`3 per 20` for an exact count). A fresh spread every time the level loads; never on spawns, goals, water, walls or placements (optional) |
| `loot_table` | the loot table the bags roll, a file stem in `assets/loot/` (default `trash_bag`) |
| `loot_bags` | the defs the bags are drawn with, comma separated, one picked per bag (default `defs/Trash Bag.ron, defs/Trash Bags.ron`) |
| `escape_limit` | how many aliens may reach the goal before the level is lost (default 10) |
| `weapons` | weapon defs every player carries on this level instead of their own, comma separated, the first one held (e.g. `defs/Snowball.ron` for a snowballs-only level) |
| `coins` | the team starts the level with at least this many coins (for towers) |
| `seed` | random seed (optional) |
| `generated` | `true` = ignore `[tiles]` and generate the map from `seed` |
| `width`, `height` | size of a *generated* map only. A hand-made map's size comes from `[tiles]`: the longest row and the number of rows. |

### `[tiles]`

One line per row, one character per tile. Spaces inside a row are ignored (use them to
line things up); rows shorter than the longest are padded with void. Everything except
`v` has a floor under it.

| letter | tile |
|--------|------|
| `v` | void (nothing; only outside the walls - a hole inside the map shows the background, so fill it with `w`) |
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

### `[decorations]`, `[placements]`, `[houses]`, `[waves]`

One entry per line: `key: value` fields separated by `|`, in any order. Values can
contain spaces (paths often do) but not `|`. An unknown key is an error.

| section | fields |
|---------|--------|
| decorations | `at: x, y` + `model:` (path under `assets/`), optional `rotation:` (degrees) and `scale:` (default 1) |
| placements | `at: x, y` + `def:` (a def `.ron`), optional `rotation:` (degrees, multiples of 45) |
| houses | `house:` (a name in `assets/houses/`, made in the house editor) + `at: x, y` (the top-left tile of its footprint), optional `rotation:` (degrees clockwise, multiples of 90). The game stamps the house into the tiles when the level loads: walls block everyone, doors are floor, and anything under it is replaced. Only written when the map has houses. |
| waves | optional `enemy:` (enemy def; omitted = the built-in alien), `count:` (default 10), `rate:` (spawns per minute, default 20) |

`x` is the column and `y` the row, both counted from 0 at the top left of `[tiles]`.
