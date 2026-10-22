# Story crawls

Text that rolls up the screen (Star Wars) or types into a dialog box, triggered by the
facts engine. Two halves:

- **`lava_ui_builder::crawl`** (generic, no game knowledge): `CrawlSpec` data, the
  `CrawlQueue`, `ShowCrawl` / `AdvanceCrawl` / `ClearCrawls` / `CrawlFinished` messages,
  layout, scrolling, typing and fades. Runs on `Time<Real>`. Example:
  `cd lava_ui_builder && cargo run --example crawl`.
- **`src/crawl/`** (the game): the crawl library, the story hook, input, and pausing.

## From a fact to a crawl

```
fact changes -> story rules pass -> Emit(effect: "crawl:<name>")
             -> assets/crawls/<name>.ron queued -> shown
             -> on finish: fact CrawlFinished.<name> = true
```

1. Write the crawl: `assets/crawls/<name>.ron` (a `CrawlSpec`; every field optional, the
   id is the file name). The header of `assets/crawls/level_01_intro.ron` lists the fields.
   Plain ASCII only.
2. Write a story that emits it, usually in the level's story file (below):
   `consequences: [Emit(effect: "crawl:<name>")]`.
3. Chain off the end with `BoolIs(key: "CrawlFinished.<name>", expected: true)`.

`CrawlShowing` is true while a crawl is up or queued.

## Level stories

A map names its own story file: `stories: "assets/stories/level_01.ron"` in the `.ron`,
`stories: assets/stories/level_01.ron` under `[properties]` in the `.map`. It is a RON
list of `Story` (format: `turbofacts/assets/stories/example.ron`), loaded on top of the
base level-flow stories when the map loads. Useful keys: `LevelStarted`,
`LevelComplete`, `LevelFailed`, `EnemyKillCount`, `CurrentWave`, ... (`turbofacts/src/keys.rs`).

## Story items

An `Item` def with `kind: Key(id: "<id>")` (e.g. `assets/defs/story_letter.ron`) sets
`ItemFound.<id>` when picked up. Place it with the map editor or a `[placements]` line.

## Modal vs not

- `modal: true` (default): the match goes into `PauseState::Cutscene` (virtual time
  stopped, player input closed, like the pause menu). Fire / Place / Interact / Pause
  or Enter, keyboard or any pad, advance or skip.
- `modal: false`: plays over the running game; only Interact (E / Circle) and Enter
  advance it. Pair with `auto_advance` for typewriters.

The level-end transition (`level_state_system`) waits until every crawl has finished,
so an outro on `LevelComplete` plays before the game returns to the menu.

## Level 1 sample

`assets/stories/level_01.ron`: an intro scroll on `LevelStarted`, a non-modal
typewriter letter when `ItemFound.letter` (the papers placed at 7,10), an outro scroll on
`LevelComplete` (and not `LevelFailed`).

**Not playtested visually yet**: the scroll's tilt/speed, the letter def's scale (1.0)
and its placement are first guesses.
