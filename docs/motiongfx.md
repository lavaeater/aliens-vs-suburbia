# MotionGfx: timelines for choreographed moments

Notes from a look at [`bevy_motiongfx`](https://crates.io/crates/bevy_motiongfx)
(repo: <https://github.com/voxell-tech/motiongfx>), 2026-10-30. Nothing is built yet; this
is the case for a spike.

## What it is

A motion graphics framework: you build a **timeline** in plain Rust ("move this entity's
`Transform.translation.x` to +6 over 1 s with a quad ease-out, then ...") and a player
plays it at any speed, either direction, or seeks to any point without re-simulating.
Actions are relative (each starts from the field's current value), so steps chain.
It animates component fields (`path!(Transform.translation.x)`) and asset fields
(`path!(StandardMaterial.base_color)`, by untyped asset id). `RealtimePlayer` is a
controller that drives a timeline's time.

```rust
let mut b = motiongfx.create_builder();
let track = b
    .act(entity, path!(Transform.translation.x), |x| x + 6.0)
    .with_ease(ease::quad::ease_out)
    .play(s(1))
    .compile();
commands.spawn((
    motiongfx.add_timeline(b.compile(track)),
    RealtimePlayer::new().with_playing(true),
));
```

## Versions

| Bevy | bevy_motiongfx |
|------|----------------|
| 0.19 (us) | 0.3 - 0.4 |
| 0.20 | 0.5 (2026-10-22) |

So: **0.4 now**, 0.5 with the Bevy 0.20 upgrade (`docs/bevy-0.20-upgrade.md`). It is
young - first release 2026-01, six releases since, ~570 downloads - so expect API churn
between versions.

## Not the crawls

Considered and rejected for `crawl` / `lava_ui_builder::crawl`:

- **Little to replace.** The moving parts are a scroll position at constant speed and a
  fade; the bulk of the code is the queue, layout, input, the `Cutscene` pause and the
  facts, none of which a timeline does.
- **The typewriter is not a tween.** Revealing characters would be a tweened "chars
  shown" number plus our own system writing the text - the bespoke code stays.
- **Crawls are data; timelines are code.** Crawls are RON files edited without a
  recompile. The crate has optional `ron` / `motiongfx_scene` features; whether a
  timeline can be loaded from a file is unverified. If not, crawls would lose their main
  strength.

## Where it would pay

Anything choreographed - several things moving in sequence, where each would otherwise
be its own one-off system:

- **Boss intro (level 5)**: camera pans to the spawn, Alpaking Evolved rises out of it,
  the title slams in, camera returns to the players. Seeking to the end is a free skip.
  The favourite for the spike.
- **Intermission / level title cards**: slide-ins, staggered lines, a count-up of kills
  or coins.
- **HUD flourishes**: the pickup toast, the wave banner, coin pops.

## Spike plan

1. Add `bevy_motiongfx = "0.4"`, `BevyMotionGfxPlugin` in `GamePlugin`.
2. Build the boss intro (or the intermission card, as a smaller first step).
3. Answer:
   - How does the builder API feel for a sequence of 5-10 steps?
   - Does a timeline run on real time while `Time<Virtual>` is paused
     (`PauseState::Cutscene`)? Which clock does `RealtimePlayer` read?
   - Can `camera_follow` be gated off for the duration and handed back cleanly
     (`CameraFocus` smoothing will want a starting point)?
   - Can timelines be authored in RON (`ron` feature, `motiongfx_scene`)? If yes,
     cutscenes could sit in the level folder next to the crawls
     (`level_05_cutscene_boss.ron`, `assets::content`).
   - Animating UI: `Node` fields (`left`, `top` are `Val`) - interpolable, or do we
     tween a plain f32 and copy it over?
