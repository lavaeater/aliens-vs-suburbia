---
name: turbofacts-port
description: The src/facts/ turbofacts engine port — design decisions and how it's wired
metadata:
  type: project
---

Ported the Kotlin `turbofacts` engine (from `~/projects/java/turbo-rocket-ultra`) into
`src/facts/` on branch `turbofacts`. Plan lives in `docs/turbofacts.md`.

**Design decision (user-confirmed):** messaging drives reactions, NOT observers. Facts are
mutated directly on the `Facts` resource; a per-frame `emit_fact_changes` system drains the
dirty-key list into `FactChanged` messages (the analog of Kotlin `Message.FactUpdated`).
`arm_story_check` sets `StoryStore.needs_checking` on any `FactChanged`, then `check_stories`
evaluates. The four facts systems are `.chain()`ed in this order — a shared-dirty-flag chain.

**Why:** keeps fact writes as plain resource methods (not message-routed/noisy), emits one
change signal per key per frame, and consequence writes cascade to the next frame.

**How to apply:** Criterion/Consequence/Story are serde enums (not trait objects) so stories
load from RON (`assets/stories/*.ron`). `FactsGameIntegrationPlugin` activates base stories
on `OnEnter(InGame)` and `derive_world_facts` mirrors ECS state into facts — this runs as
GROUNDWORK alongside the existing `LevelTracker` win/lose flow in `score_keeper.rs`, it does
NOT replace it yet. Story side effects surface as named `StoryEffect` messages
("level_complete" etc.) currently only logged — real gameplay handlers are the next step.
