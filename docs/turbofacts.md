# Turbofacts in Bevy

A plan to port the **turbofacts** system from `turbo-rocket-ultra` (Kotlin/libGDX) to
this Bevy project. Turbofacts is a tiny *data-driven narrative / game-logic engine*:
the game writes typed "facts" about the world into a flat key->value store, and
declarative "stories" watch those facts via rule criteria and fire consequences when
their rules pass. It decouples *what is true* from *what should happen* — designers can
add win/lose conditions, tutorials, story beats, spawn modifiers etc. without touching
systems code.

This document describes how to reproduce the same capability idiomatically in Bevy 0.18,
leaning into **resources** for the fact store and **messages + an optional observer
layer** for setting and reacting to fact changes.

---

## 1. What the Kotlin system does (reference)

Source: `turbo-rocket-ultra/core/src/main/kotlin/turbofacts/`.

### Facts (`TurboFactsOfTheWorld`, `Factoid`)
- A single `mutableMapOf<String, Factoid>` keyed by a dotted string (`multiKey("a","b")`
  -> `"a.b"`).
- `Factoid.Fact<T>` is a sealed hierarchy: `BooleanFact`, `IntFact`, `FloatFact`,
  `StringFact`, `StringListFact`, `SetFact<V>`.
- Typed setters/getters: `setBooleanFact`, `getInt`, `addToInt`, `addToStringList`,
  `intOrDefault`, etc. Getters auto-create a default fact if missing.
- Every mutation calls `updated(key)` -> `onFactUpdated(key)` callback **unless** inside a
  `silent { ... }` block (used for bulk initialization without triggering reactions).
- `factsFor(*key)` does prefix/wildcard queries over keys (`"enemy.*.dead"`,
  or "contains" matching) — this powers the `All*`/`Any*` criteria.
- In the real game `onFactUpdated` pushes a `Message.FactUpdated(key)` onto the message bus
  (see `dependencies/Context.kt:136`).

### Criteria (`Criterion.kt`)
A sealed hierarchy of predicates over the fact store. Families:
- Booleans: `SingleBoolean.IsTrue/IsFalse`, `AnyBoolean`, `AllBooleans` (the last two run
  over `factsFor` query results).
- Ints: `SingleInt`, `AnyInts`, `AllInts` with `moreThan/lessThan/equals`; plus
  `IntVersusInt` (compare two fact keys).
- Strings: `SingleString`, `AnyString`, `AllStrings` with `equals/contains`.
- Collections: `StringListContains`, `StringListSize`, `SetContains`, `SetSize`.
- Each criterion implements `checkRule(): Boolean` and an optional `toTextToken(): String?`
  used for text (de)serialization.

### Rules & Stories (`TurboRule`, `TurboStory`, `TurboStoryManager`)
- `TurboRule` = a named `List<Criterion>`; passes when **all** criteria pass (AND).
- `TurboStory` = name/description, a list of rules (story passes when **all** rules pass),
  a `Consequence`, plus flags:
  - `repeat` — may fire again after re-initialization.
  - `exclusive` — when it fires, stop checking the rest this tick.
  - `initializer` — runs on activation (typically seeds facts in a `silent {}` block).
  - `specificityScore` = total criteria count; stories are sorted **most-specific-first**.
  - Internal `storyIsFinished` / `needsInit` latch so a story fires once until re-init.
- `TurboStoryManager` holds the sorted stories, an `isActive` flag, and a `needsChecking`
  dirty flag. `checkIfNeeded()` early-returns unless active **and** dirty; the dirty flag
  is set by the `FactUpdated` message. So stories are only re-evaluated when a fact changed.
- `GameScreen` calls `storyManager.checkIfNeeded()` each frame.

### Consequences (`story/consequence/`)
- `Consequence { apply() }` interface. Implementations: `EmptyConsequence`,
  `SetFactConsequence` (writes a fact back), `SimpleConsequence` (lambda),
  `ConversationConsequence`. Consequences commonly write *more* facts, which re-triggers
  the dirty flag — facts cascade.

### Authoring / persistence
- `TurboRuleBuilder` / `TurboStoryBuilder` — Kotlin DSL (`story { rule { isTrue(...) } }`).
- `StoryLoader` — JSON story files in assets.
- `StoryTextParser` / `StoryTextSerializer` — line-oriented text format embedded in map
  files (`story <name>` / `rule` / criterion tokens / `then` / `setFact ...`).
- `FactPersistence` — save/load the fact map to JSON (only scalar facts; lists/sets are
  runtime-only).

### Sibling systems that *write* facts
- `FactSystem` (`systems/facts/FactSystem.kt`) — an interval system that derives facts from
  ECS state every 1s (e.g. "no boss entities -> `BossIsDead = true`").
- Gameplay systems set facts directly (`EnemyDeathSystem`, `EnemySpawnSystem`, HUD reads
  them, etc.).

---

## 2. Target design in Bevy

Map the pieces as follows:

| Kotlin | Bevy |
|--------|------|
| `TurboFactsOfTheWorld` singleton | `Facts` **Resource** |
| `onFactUpdated` callback + `Message.FactUpdated` | `FactChanged` **Message** (+ optional observer trigger) |
| `TurboStoryManager` | `StoryStore` **Resource** + `check_stories` system |
| `Factoid.Fact<T>` sealed class | `FactValue` enum |
| `Criterion` sealed class | `Criterion` enum (data, not trait objects) |
| `Consequence` interface | `Consequence` enum + a `run_consequences` system / observer |
| `FactSystem` interval rules | normal Bevy systems writing into `Facts` |
| `silent {}` | a method that mutates without queuing `FactChanged` |

New module: `src/facts/` with a `FactsPlugin`, registered in `GamePlugin`
(`src/game_state/game_state_plugin.rs`).

```
src/facts/
  mod.rs
  facts_plugin.rs        // FactsPlugin: resources, messages, systems
  fact_value.rs          // FactValue enum + typed accessors
  facts_resource.rs      // Facts resource (the store) + set/get/silent
  criterion.rs           // Criterion enum + evaluate(&Facts)
  consequence.rs         // Consequence enum + apply
  story.rs               // Rule, Story, StoryStore
  builder.rs             // ergonomic constructors / DSL-ish helpers
  persistence.rs         // save/load via RON
  text_format.rs         // optional: parse/serialize stories from map files
  messages.rs            // FactChanged, StoryFired, SetFact
```

### 2.1 The fact store as a Resource

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FactValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
    TextList(Vec<String>),
    TextSet(HashSet<String>),
}

#[derive(Resource, Default)]
pub struct Facts {
    map: HashMap<String, FactValue>,
    /// keys mutated since last drained; the plugin turns these into FactChanged msgs.
    dirty: Vec<String>,
    silent: bool,
}
```

Keys: keep the dotted-string convention. Provide a `fact_key(&[&str]) -> String` helper
(equivalent of `multiKey`) and a `FactKey` newtype if we want type safety later. Define
well-known keys as `const &str` constants in a `keys` module — the direct analog of
`Factoids.kt` (`pub const BOSS_IS_DEAD: &str = "BossIsDead";` …).

Accessors mirror the Kotlin API but return `Result`/`Option` instead of throwing:
- `set_bool(&mut self, key, value)`, `set_int`, `set_float`, `set_text`.
- `add_to_int(key, delta)`, `add_to_text_list`, `add_to_text_set`, etc.
- `get_bool(key) -> bool` (default-creating, like Kotlin) **plus** `try_bool(key) -> Option<bool>`
  for the non-mutating case. Decide per-getter whether auto-default is worth the `&mut`
  requirement — prefer `bool_or(default)` style that takes `&self` where possible and only
  the explicit `*_or_default` variants take `&mut`.
- Type mismatch (writing an Int over a Bool) returns `Err`/logs a warning rather than
  panicking — Bevy systems shouldn't panic on data.

Mutation path records the key:
```rust
fn touch(&mut self, key: &str) {
    if !self.silent { self.dirty.push(key.to_string()); }
}
```

`silent` block becomes a closure helper:
```rust
pub fn silent(&mut self, f: impl FnOnce(&mut Facts)) {
    self.silent = true; f(self); self.silent = false;
}
```

`factsFor` wildcard/prefix query -> `fn query<'a>(&'a self, pattern: &str) -> impl Iterator<Item = (&'a String, &'a FactValue)>`
supporting the single-`*` prefix/suffix match and the "contains" fallback.

### 2.2 Reacting to fact changes — messages + observers

This is the part the user wants to explore. Two complementary mechanisms, both fed by the
same `dirty` list:

**(a) `FactChanged` message (the workhorse).**
```rust
#[derive(Message, Clone)]
pub struct FactChanged { pub key: String, pub value: FactValue }
```
A system at the start of the schedule drains `Facts::dirty` into `FactChanged` messages:
```rust
fn emit_fact_changes(mut facts: ResMut<Facts>, mut w: MessageWriter<FactChanged>) {
    for key in facts.dirty.drain(..).collect::<Vec<_>>() {
        if let Some(v) = facts.get_raw(&key) { w.write(FactChanged { key, value: v.clone() }); }
    }
}
```
Any system can `MessageReader<FactChanged>` to react (HUD refresh, audio sting, analytics).
This is the direct analog of `Message.FactUpdated`. It fits the project's existing
`add_message::<T>()` + `MessageReader`/`MessageWriter` idiom (see `building_events.rs`,
`animation_plugin.rs`).

**(b) Observers (targeted, immediate reactions).**
Bevy observers fire synchronously when an event is triggered, which is ideal for "when
*this specific* fact changes, do X right now" without polling. Option:
- A global observer on a `FactChanged`-as-`Event` trigger, OR
- Per-key reaction registration. Because observers key off event *type* (not data), to get
  per-key dispatch we'd either (i) keep one observer that matches on `key` inside, or
  (ii) use Bevy's entity-targeted observers: represent "watchers" as entities and trigger
  the event at them. For a fact system the simplest is **one global observer** that
  forwards to interested parties; per-key filtering stays data-driven.

**Recommendation:** make `FactChanged` the canonical mechanism (message). Add an observer
layer only where a system genuinely needs synchronous, same-frame reaction (e.g. gameplay
that must not lag a frame behind a fact flip). Keep the fact mutation itself in plain
methods on the resource — don't route every `set_*` through a message, that would be noisy
and ordering-sensitive. Mutate the resource, let the drain system emit the change signal
once per frame per key.

**Setting facts from far-away systems without `ResMut<Facts>`.** Provide a `SetFact`
message so systems that only have a `MessageWriter` can request a write:
```rust
#[derive(Message)]
pub enum SetFact { Bool(String,bool), Int(String,i64), AddInt(String,i64), Text(String,String), /* … */ }
```
An `apply_set_fact` system reads them into `ResMut<Facts>`. This mirrors how the Kotlin code
lets gameplay poke facts indirectly, and avoids `ResMut<Facts>` contention across many
systems. (Direct `ResMut<Facts>` is still fine where convenient.)

### 2.3 Criteria as data (enum, not trait objects)

Kotlin uses a sealed class with `checkRule()`. In Rust, prefer a serializable enum over
`Box<dyn>` so stories can be loaded from RON and stored in a resource cheaply:

```rust
#[derive(Clone, Serialize, Deserialize)]
pub enum Criterion {
    BoolIs   { key: String, expected: bool },
    AnyBool  { key: String, expected: bool },   // query over key pattern
    AllBool  { key: String, expected: bool },
    IntCmp   { key: String, op: NumOp, value: i64 },
    AnyIntCmp{ key: String, op: NumOp, value: i64 },
    AllIntCmp{ key: String, op: NumOp, value: i64 },
    IntVsInt { lhs: String, op: NumOp, rhs: String },
    FloatCmp { key: String, op: NumOp, value: f32 },
    TextEq   { key: String, value: String },
    TextContains { key: String, value: String },
    ListContains { key: String, value: String },
    ListSize { key: String, op: NumOp, value: usize },
    SetContains  { key: String, value: String },
    SetSize  { key: String, op: NumOp, value: usize },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum NumOp { Lt, Gt, Eq }

impl Criterion {
    pub fn evaluate(&self, facts: &Facts) -> bool { /* match self */ }
    pub fn to_token(&self) -> Option<String> { /* for text serialization */ }
}
```

This collapses the large Kotlin class hierarchy (`SingleInt`/`AnyInts`/`AllInts` × ops)
into `{ scope, op, value }` data, which is more compact and trivially serializable.

### 2.4 Rules, Stories, StoryStore

```rust
pub struct Rule { pub name: String, pub criteria: Vec<Criterion> }
impl Rule { fn passes(&self, f: &Facts) -> bool { self.criteria.iter().all(|c| c.evaluate(f)) } }

pub struct Story {
    pub name: String,
    pub description: String,
    pub repeat: bool,
    pub exclusive: bool,
    pub rules: Vec<Rule>,
    pub consequences: Vec<Consequence>,
    pub init_facts: Vec<(String, FactValue)>, // seeded silently on activation
    finished: bool,
    needs_init: bool,
}
impl Story {
    fn specificity(&self) -> usize { self.rules.iter().map(|r| r.criteria.len()).sum() }
    fn passes(&self, f: &Facts) -> bool { self.rules.iter().all(|r| r.passes(f)) }
}

#[derive(Resource, Default)]
pub struct StoryStore { stories: Vec<Story>, active: bool, needs_checking: bool }
```

`StoryStore::add` keeps the **sort-by-specificity-descending** behavior so more-specific
stories win. `activate()` sets `active`, runs each story's initializer (seed `init_facts`
via `facts.silent`).

### 2.5 The check loop

Replace `checkIfNeeded()` (called every frame, gated by dirty flag) with a Bevy system
gated the same way:

```rust
fn check_stories(
    mut store: ResMut<StoryStore>,
    mut facts: ResMut<Facts>,
    mut fired: MessageWriter<StoryFired>,
) {
    if !store.active || !store.needs_checking { return; }
    store.needs_checking = false;
    // iterate (already specificity-sorted); collect consequences to apply after the borrow
    for story in &mut store.stories { /* check finished, passes(); latch; queue consequences */ }
    // apply queued consequences -> writes more facts -> sets needs_checking again next drain
}
```

`needs_checking` is re-armed by a tiny system reading `FactChanged`:
```rust
fn arm_story_check(mut r: MessageReader<FactChanged>, mut store: ResMut<StoryStore>) {
    if !r.is_empty() { store.needs_checking = true; r.clear(); }
}
```

**System ordering** (one `add_systems(Update, (...).chain())`, `run_if(in_state(InGame))`):
1. `apply_set_fact` (drain `SetFact` -> `Facts`)
2. `emit_fact_changes` (drain `Facts::dirty` -> `FactChanged`)
3. `arm_story_check` (FactChanged -> `needs_checking`)
4. `check_stories` (evaluate + apply consequences; consequences mutate `Facts`, re-dirtying)

Consequence writes land in `dirty` and get emitted next frame, giving the same cascading
behavior as Kotlin (consequence sets a fact -> dirty -> re-check). The CLAUDE.md gotcha
about shared dirty flags and intra-tuple ordering applies directly here — this chain order
matters.

### 2.6 Consequences

```rust
#[derive(Clone, Serialize, Deserialize)]
pub enum Consequence {
    SetFact { key: String, value: FactValue },
    AddInt  { key: String, delta: i64 },
    // Game-specific effects emit a message rather than touch unrelated systems:
    Emit(StoryEffect),
}
```
Pure fact writes apply directly to `Facts`. For side effects that the Kotlin version did
inline (start cutscene, send `LevelComplete` message, fire a state-machine event),
emit a `StoryEffect` message and let the relevant plugin handle it — keeps the facts module
free of dependencies on camera/ui/state. `StoryFired { story_name }` is also emitted for
observers/telemetry.

### 2.7 Authoring API (the DSL)

Kotlin's `story { rule { isTrue(...) }; consequence = {...} }` becomes builder functions /
a small fluent API:
```rust
let s = story("Level Complete")
    .repeat(false)
    .exclusive(true)
    .rule("win", |r| r
        .is_true(keys::LEVEL_STARTED)
        .is_true(keys::BOSS_IS_DEAD)
        .is_true(keys::ALL_OBJECTIVES_TOUCHED))
    .then(Consequence::SetFact { key: keys::LEVEL_COMPLETE.into(), value: FactValue::Bool(true) })
    .build();
```
Implement with a `StoryBuilder` and `RuleBuilder` (`is_true`, `int_more_than`, etc.) that
push `Criterion` values — direct analog of `TurboRuleBuilder` extension functions.

### 2.8 Persistence & data loading

- **Save/load** (`FactPersistence`): serialize the scalar subset of `Facts.map` to RON
  (`assets/`-external, e.g. project-root `facts-save.ron`, matching how `game-settings.ron`
  lives at root). Skip `TextList`/`TextSet` (runtime-only), same as Kotlin. `FactValue`
  already derives `Serialize`/`Deserialize`, so this is mostly `ron::to_string`.
- **Story files**: since `Criterion`/`Consequence`/`Story` are all `Serialize`/`Deserialize`,
  prefer **RON story files** under `assets/stories/*.ron` over the bespoke JSON loader —
  this matches the project's RON-everywhere convention (`assets/defs`, `assets/maps`).
  Load with the asset server or a simple `ron::de` at startup.
- **Map-embedded stories**: the Kotlin `StoryTextParser` reads a line format from map files.
  Our `MapFile` (`src/general/components/map_components.rs`) can gain an optional
  `stories: Vec<Story>` field (it's serde-friendly), so editor-authored stories ride along
  in `assets/maps/*.ron` with no separate text parser. Only build `text_format.rs` if we
  specifically want the terse hand-typed syntax in the map editor.

### 2.9 Deriving facts from ECS (the `FactSystem` analog)

Plain Bevy systems that read world state and write facts, run on a timer or every frame:
```rust
fn derive_boss_dead(bosses: Query<(), With<Boss>>, mut sf: MessageWriter<SetFact>) {
    if bosses.is_empty() { sf.write(SetFact::Bool(keys::BOSS_IS_DEAD.into(), true)); }
}
```
Examples relevant to *this* game: alien count -> `EnemyCount`, wave index -> facts,
`TeamWallet` coins -> `Coins`, living players -> `LivingPlayerCount`, all-tiles-destroyed,
etc. These feed win/lose stories. Use `run_if` + a fixed timestep or a `Time`-based gate if
per-frame is overkill (the Kotlin one ran at 1s intervals).

---

## 3. How this game would use it

Concrete first stories to validate the system (mirroring `StoryHelper`):
- **Level start**: `LevelStarted == false` & start-message present -> set `LevelStarted`,
  emit a "level starting" `StoryEffect`.
- **Level complete**: `AllAliensDead`/objective facts -> `LevelComplete`, emit effect that
  the UI/score system listens for.
- **Level failed**: `LivingPlayerCount == 0` & `LevelStarted` -> `LevelFailed`.
- **Wave/accelerating-spawn modifiers**: facts read by `WaveManager`
  (`src/alien/wave_manager.rs`) instead of hardcoded progression.

Existing systems read facts instead of bespoke flags: HUD (`src/ui/spawn_ui.rs`) can show
fact-driven values; `WaveManager`, abilities, economy can gate behavior on facts.

---

## 4. Implementation order

1. `facts/` module skeleton + `FactsPlugin` registered in `GamePlugin`; `Facts` resource
   with `FactValue`, typed set/get, `silent`, `dirty`, `query`. Unit tests on the store.
2. `FactChanged` message + `emit_fact_changes` drain system + `SetFact` message/apply. Wire
   a debug `MessageReader<FactChanged>` log to confirm the signal path.
3. `Criterion` enum + `evaluate` + unit tests against a `Facts` fixture.
4. `Rule`, `Story`, `StoryStore`, `check_stories`, `arm_story_check`, consequence apply.
   Chain ordering as in 2.5.
5. `StoryBuilder`/`RuleBuilder` DSL; port the 3-4 `StoryHelper` stories as code.
6. RON persistence (`Facts` save/load) and RON story loading; optionally `MapFile.stories`.
7. `StoryEffect` message + handlers in ui/state plugins; replace ad-hoc win/lose flags.
8. ECS-derived fact systems (the `FactSystem` analog) for this game's quantities.
9. (Optional) observer layer for facts needing synchronous reactions; (optional) map-editor
   text format.

### Notes / gotchas specific to this project
- **ASCII only** in any UI text emitted from consequences (CLAUDE.md font constraint).
- **Messages, not Events**: use `#[derive(Message)]` + `add_message::<T>()` — matches the
  Bevy 0.18 conventions already documented in CLAUDE.md. `MessageReader::clear()` /
  draining matters for the dirty-flag arming.
- **Intra-tuple system order**: the dirty-flag chain (2.5) is exactly the
  `nodes_dirty` vs `nodes_ui_dirty` class of gotcha called out in CLAUDE.md — keep it
  `.chain()`ed and documented.
- Avoid trait-object criteria/consequences; the enum approach keeps everything `Reflect`/
  serde-friendly and inspector-visible (`bevy-inspector-egui`).

---

## 5. Refactor: facts/stories as the level-flow brain

Steps 1-8 landed the engine and wired it in *alongside* the existing `LevelTracker` flow
(`game_integration.rs` mirrors world state into facts but nothing consumes the result except
a debug log). This section is the plan to make facts/stories the **authority** for level
flow, so win/lose/score decisions live in declarative stories instead of the hand-rolled
`level_state_system`.

### 5.1 Current (pre-refactor) ownership

- `LevelTracker` resource holds `level_state: LevelState` (NotStarted/InProgress/Completed/
  Failed) plus the counters (`aliens_killed`, `aliens_reached_goal`, `aliens_to_spawn`, …).
- `level_state_system` (`src/game_state/score_keeper.rs`) computes win/lose **inline** every
  frame: win = all waves done && all spawned aliens killed; lose = too many escaped, or all
  players dead. On Completed/Failed it waits `end_delay` then writes `GotoState(Menu)`.
- `wave_system` (`src/alien/wave_manager.rs`) gates on `LevelState::InProgress`.
- Score lives in per-player `Score` components and `LevelTracker` counters, fed by
  `GameTrackingEvent` messages from collision / goal / spawn / throw systems.

### 5.2 Target ownership

Keep the low-level bookkeeping (events -> `Score`/`LevelTracker` counters) — that's the
ECS-derived-fact source. Move the **decisions** into stories:

1. **Derived condition facts** (`derive_world_facts`, the `FactSystem` analog). Each frame,
   translate raw `LevelTracker`/queries into the small set of boolean/int facts stories read,
   using `set_if_changed` so unchanging values never dirty the store:
   - `ALL_ALIENS_DEAD`  = all waves done && kill target met (&& target > 0).
   - `ALL_PLAYERS_DEAD` = at least one player exists && all at <= 0 health (the
     non-empty guard the old inline code had).
   - `TOO_MANY_ALIENS_ESCAPED` = `aliens_reached_goal >= aliens_win_cut_off`.
   - plus mirrored scalars for HUD/telemetry/stories: `ENEMY_KILL_COUNT`, `ENEMY_COUNT`,
     `LIVING_PLAYER_COUNT`, `COINS`, `CURRENT_WAVE`, `WAVE_COUNT`, `ALL_WAVES_DONE`,
     `ALIENS_ESCAPED`, `ALIENS_ESCAPED_CUTOFF`, `ALIENS_TO_SPAWN`, `SHOTS_FIRED`,
     `SHOTS_HIT` (the last two summed from `Score`).

2. **Stories own the verdict** (`stories.rs`, `base_stories()`):
   - `Level Start`: `LevelStarted == false` -> set `LevelStarted`, reset the win/lose
     condition facts (its `init_facts`), emit `level_starting`.
   - `Aliens Cleared` (win): `LevelStarted && ALL_ALIENS_DEAD` -> set `LevelComplete`.
   - `Level Complete`: `LevelStarted && LevelComplete` -> set `GotoNextLevel`, emit
     `level_complete`. (Unchanged — already covered by a unit test.)
   - `Level Failed (players)`: `LevelStarted && ALL_PLAYERS_DEAD` -> `LevelFailed`, emit
     `level_failed`.
   - `Level Failed (escaped)`: `LevelStarted && TOO_MANY_ALIENS_ESCAPED` -> `LevelFailed`,
     emit `level_failed`. (OR across lose conditions = separate stories, since a story
     ANDs all its rules.)

3. **`level_state_system` becomes a reactor.** It no longer computes anything — it reads
   `StoryEffect` messages and maps them to `LevelState`: `level_starting` -> InProgress,
   `level_complete` -> Completed, `level_failed` -> Failed. It keeps only the `end_delay`
   countdown -> `GotoState(Menu)`. This deletes the duplicated win/lose math; the stories are
   the single source of truth, and the effect log in `game_integration.rs` already proves the
   signal path.

4. **Waves** stay gated on `LevelState::InProgress` (now itself story-driven via
   `level_starting`), and additionally publish `CURRENT_WAVE`/`WAVE_COUNT`/`ALL_WAVES_DONE`
   facts. Fact-*driven* wave modifiers (spawn-rate multipliers, bonus waves keyed off facts)
   are a later step — noted in 5.4.

### 5.3 Why this split

- Designers can add/replace win-lose conditions in RON (`assets/stories/*.ron` or
  `MapFile.stories`) without recompiling — e.g. a survival map drops `Aliens Cleared` and
  adds a time-based story (see section 6).
- Edge cases that the inline code special-cased (no players yet -> not a loss) become a
  single derived bool (`ALL_PLAYERS_DEAD`) instead of scattered guards.
- The cascade is automatic: `derive_world_facts` flips `ALL_ALIENS_DEAD` -> `Aliens Cleared`
  sets `LevelComplete` -> `Level Complete` sets `GotoNextLevel` + emits -> reactor transitions.

### 5.4 Out of scope for this pass (follow-ups)

- Per-player score as facts (`player.<slot>.kills`) — keep the `Score` component for now.
- Fact-driven wave tuning (`WaveManager` reading spawn-rate facts).
- Replacing `LevelTracker.level_state` entirely with facts (the enum still gates several
  systems; collapsing it is a bigger churn than this pass warrants).

---

## 6. Time-based facts (design only — do not implement yet)

The game already has a clock (`Res<Time>`; `level_state_system` accumulates `end_delay` from
`time.delta_secs()`, and `wave_manager` counts down `wave_timer`). To make time available to
**stories**, expose it as facts so criteria like "survive 60s", "sudden death after 5
minutes", or "accelerate spawns after 90s" become data, not code.

### 6.1 Two shapes for a time fact

**(a) Whole-second derived fact (recommended).** Add `ELAPSED_SECONDS` (and a per-level
`LEVEL_ELAPSED_SECONDS`) and write them from a derived-fact system using the **same
`set_if_changed` discipline as the other derived facts**: accumulate a float locally, but
only push the *truncated integer second* into the store. That gives 1 Hz granularity and,
crucially, only dirties the store ~once per second instead of every frame — so story
re-checks stay cheap. Integer seconds also pair naturally with the existing `IntCmp` /
`IntVsInt` criteria (`int_more_than(LEVEL_ELAPSED_SECONDS, 60)`).

```rust
// sketch — NOT wired yet
#[derive(Resource, Default)]
struct LevelClock { secs: f32 }

fn tick_level_clock(time: Res<Time>, mut clock: ResMut<LevelClock>, mut facts: ResMut<Facts>) {
    clock.secs += time.delta_secs();
    set_if_changed(&mut facts, keys::LEVEL_ELAPSED_SECONDS, clock.secs as i64);
}
```

**(b) Tick message.** A repeating 1s `Timer` that emits `SetFact::AddInt(ELAPSED_SECONDS, 1)`.
Simpler to reason about (monotonic counter), but the `LevelClock` accumulator in (a) is just
as small and avoids a second timer abstraction. Prefer (a).

### 6.2 Gating and resetting

- **Pause when not playing.** Gate the tick system on `LevelState::InProgress` (or, once 5.2
  lands, on `LevelStarted && !LevelComplete && !LevelFailed`) so the clock doesn't run on the
  menu/end screen.
- **Reset per level.** Zero `LevelClock` on `OnEnter(InGame)` and seed
  `LEVEL_ELAPSED_SECONDS = 0` as an `init_fact` on the `Level Start` story (silent), so a
  replayed level starts the clock fresh. Keep a never-reset `ELAPSED_SECONDS` too if we want
  total-session time for telemetry.

### 6.3 What time facts unlock

- **Survival win**: `story("Survive") { rule { is_true(LevelStarted); int_more_than(LEVEL_ELAPSED_SECONDS, 120) } } -> set LevelComplete`.
- **Escalation**: a derived `DIFFICULTY_TIER` int bumped from elapsed seconds, read by a
  (future) fact-driven `WaveManager` to scale `spawn_rate_per_minute`.
- **Sudden death / soft timeout**: after N seconds with the level unfinished, emit an effect
  that spikes spawns or ends the level.
- **HUD**: a `mm:ss` readout fed by `LEVEL_ELAPSED_SECONDS`, ASCII-only.

### 6.4 Cost note

The single rule: **never write a continuously-changing float into `Facts` every frame** — it
would set `needs_checking` every frame and defeat the dirty-flag gate (the CLAUDE.md
`nodes_dirty` gotcha again). Quantize to whole seconds (or coarser) before it touches the
store.
