# Gore sound effects

Drop `.wav` files here and the game plays them automatically — no code changes.
`src/gore/sfx.rs` scans this folder at startup and groups files by **filename prefix**.
Until you add files, the game just runs quiet.

## Naming

A file is matched to a category if its (lower-cased) name **starts with** one of these
prefixes. Add as many variants per category as you like — one is chosen at random per
event, with slight pitch/volume jitter so repeats don't sound identical.

| Prefix   | Plays when...                                   | Example files              |
|----------|-------------------------------------------------|----------------------------|
| `hit`    | a non-lethal hit lands (bullet/ball/melee)      | `hit1.wav`, `hit2.wav`     |
| `death`  | something dies                                  | `death_squelch.wav`        |
| `gib`    | layered under a death (wet burst)               | `gib1.wav`                 |
| `fire`   | a fire field ignites (molotov)                  | `fire_whoosh.wav`          |
| `shoot`  | a shot is fired                                 | `shoot_pistol.wav`         |
| `bark`   | a one-liner caption appears                     | `bark1.wav` ... `bark9.wav`|
| `heartbeat` | a player is bleeding out (quickens near death) | `heartbeat.wav`         |

Anything that doesn't match a prefix is ignored. A single looping-style thud works well
for `heartbeat` — the despair system re-triggers it on a tightening cadence, so keep the
sample short (one beat).

## Format gotcha

`bevy_seedling`'s loader rejects some wavs with `malformed fmt_pcm chunk` even when other
tools play them fine. If a sample won't load, re-encode it:

```
ffmpeg -i in.wav -c:a pcm_s16le out.wav
```

## Tuning

Per-category base volume and the concurrent-voice cap live at the top of
`src/gore/sfx.rs` (`emit_combat_sfx` gains, `MAX_VOICES`).
