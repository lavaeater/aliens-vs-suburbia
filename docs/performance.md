# Performance on weak hardware

Findings from 2026-11-05, measured on the low end of what we want to run on: a ThinkPad T14
Gen 1 (i7-10610U, 4 cores / 8 threads, UHD Graphics CML GT2, 32 GB, 1920x1080, Mesa 26.2.6
on COSMIC Wayland, Bluetooth AAC headphones).

Symptom: dreadful audio stutter, while the picture mostly kept up.

## Summary

- **Audio stutter: a compiler setting, not hardware or drivers.** `~/.cargo/config.toml` set
  `codegen-backend = "cranelift"` for the dev profile. Removed; audio no longer underruns.
- **Frame rate: per-pixel lighting and shadows, on a GPU held back by power and heat.** Level 1
  runs at about 24 fps and the OSM map at about 17 fps. Turning off the moon's shadows alone
  gives about 32 and 24.
- **Drivers are fine.** ANV, firmware and adapter selection are all correct.
- **Realtime audio added.** On Linux the game now opens a native PipeWire stream on a
  realtime thread.

## Audio

### Cause

Cranelift applies to dependencies too, and does none of LLVM's inlining or vectorization, so
`opt-level = 3` in `Cargo.toml` doesn't deliver optimised dependencies. Cargo still prints
`opt-level=3`, which hides this. Check the rustc command line with `cargo build -v` and look
for `codegen-backend=`.

The synths suffered most: the voice loops do per-sample work, with atomics, on one audio
thread. Each silent engine still fills its lanes every block.

Debug assertions were not the cause: turning them off for `rusty_music` changed nothing.

| Build | Band, silent (offline) | Band, playing (offline) | Audio thread (live) | Underrun warnings |
|---|---|---|---|---|
| Cranelift, everything | 55% of a core | 149% | ~92% | 19 in 60 s |
| LLVM dependencies | 1.0% | 12% | ~18% | 0 in 50 s |
| Release (no LTO) | 1.1% | 14% | 9-32% | 0 in 60 s |
| LLVM + PipeWire realtime | not measured | not measured | 13-16% | 0 in 50 s, each map |

"Playing" has all 13 engines hitting a note four times a second, which is deliberately heavy.
Underruns are logged at most once every 3 s, so real counts are higher.

The band was: bass, pad, arp, four drums, lead, three stingers and two spawn point hums. Every
spawn point adds a synth, so this grows with the map, but cheaply with LLVM (about 0.7% each
while playing).

### Realtime priority

The audio thread used to run at normal priority, competing with Bevy's task pools.

`cpal`'s `realtime-dbus` feature on its own does **nothing** on a desktop: its ALSA host only
promotes hw-like PCM types, and `default` here is the `pipewire-alsa` plugin. So `Cargo.toml`
also enables the `pipewire` feature. `default_host()` then picks native PipeWire when it is
running, and falls back to ALSA otherwise.

Verified live: device `DeviceId(PipeWire, "output_default")`, thread `pw_out` at `RR 10`, zero
errors in `pw-top`.

Linux builds now need libpipewire, libdbus and clang.

`rodio` (through `bevy_audio`) still opens a second, silent 44.1 kHz stream over ALSA. It costs
almost nothing, so it was left alone.

## Frame rate

### Power

The usual RAPL entry shows 51 W, which is misleading. The limit in force is the MMIO one:

```
intel-rapl:0        package-0  PL1=51W  PL2=51W
intel-rapl-mmio:0   package-0  PL1=17W  PL2=51W
```

Those 17 W are shared by the CPU and the iGPU. Under sustained load both settle at about
1.8 GHz and 900 MHz; the GPU rarely reaches its 1150 MHz. The system was about 67% idle during
play, with CPU threads waiting for the GPU, so **frames are GPU-bound**.

A 100 W USB-C supply raises PL1 to 28 W, but adds no frames: level 1 stayed at 23 fps and the
OSM map at 17. The CPU takes the extra power (3.4 GHz instead of 1.9), the GPU stays near
950 MHz, and the package reaches 96-98 C. Not tested yet: disabling turbo to leave that heat
budget to the GPU.

Temperatures reached 97-99 C in some runs, and the package logged about 74,000 throttle events
in one boot (builds included).

Stay on the `performance` power profile. `balanced` was slower: 20 vs 23 fps on level 1, 15 vs
17 on the OSM map.

### Where a frame goes

`--perf`, dev build, OSM map, one setting changed at a time. The two baselines were 58.0 and
53.9 ms, so differences under about 4 ms mean little:

| Change | Frame | FPS | Main opaque pass | Rest of frame |
|---|---|---|---|---|
| Baseline | 56 ms | 18 | 35 ms | 21 ms |
| Tonemapping and dither off | 55 ms | 18 | 35 ms | 21 ms |
| MSAA off | 56 ms | 18 | 36 ms | 20 ms |
| Shadow filter `Hardware2x2` | 53 ms | 19 | 31 ms | 21 ms |
| No lamp shadows | 47 ms | 21 | 32 ms | 14 ms |
| Flashlight off | 49 ms | 21 | 29 ms | 20 ms |
| **No moon shadows** | 40 ms | 25 | 31 ms | 9 ms |
| Map lamps off | 33 ms | 31 | 18 ms | 14 ms |
| Everything unlit | 26 ms | 38 | 3.4 ms | 23 ms |

What this shows:

- **Geometry and overdraw are not the problem.** About 21,000 vertices, 14,000 triangles and
  2M fragments a frame, which is about one layer of 1080p.
- **Lighting is about 90% of the main pass**, about 20 ns per pixel. Map lamps are about half
  of the pass, because their ranges overlap at `SPACING`.
- **The main pass scales linearly with pixels.** A window that reached 0.63x the pixels took
  0.63x the time. Earlier notes here said resolution helped little: that test's resize to
  960x540 never fully applied, reaching 1.26M pixels and not 0.5M.
- **The moon's shadows are the most expensive single thing.** The night moon is a directional
  light with shadow maps on, at the default cascade count, for barely visible light.
- **Bevy has no timing spans for shadow passes**, so they are only visible as "rest of frame".
  The rest of frame also contains whatever else isn't timed.

### Combinations

| Change | OSM map | Level 1 |
|---|---|---|
| Baseline | 18 fps | 24 fps |
| No moon shadows | 24 fps | 32 fps |
| + `Hardware2x2` | 27 fps | not measured |
| + 6 shadow lamps | 29 fps | 36 fps |
| + 0 shadow lamps | 33 fps | not measured |
| + 6 lamps, MSAA and tonemapping off | 32 fps | not measured |

Render scale should come on top of these, but hasn't been measured properly yet.

### Lamps

OSM map, after the moon change, one setting changed at a time. GPU clock drifts with heat (1135
down to about 960 MHz over a few runs), so options were bracketed by fresh baselines, which
settled at about 42 ms (24 fps):

| Change | Frame | FPS | Lamps planned | Lights in reach, avg / max |
|---|---|---|---|---|
| Baseline, spacing 5 | 42 ms | 24 | 13,325 | 2.5 / 5 |
| Spacing 7 | 35 ms | 28 | 6,774 | 1.9 / 4 |
| Spacing 8 | 34 ms | 30 | 5,207 | 1.5 / 3 |
| Spacing 7, ranges x0.8 | 31 ms | 32 | 6,774 | not measured |

Single runs on a hotter or cooler GPU, so read these as rough:

- Ranges x0.7 cut about 4 ms, x0.5 about 7 ms.
- Hiding all point lights (lanterns, neon, spawn glows) cut about 9 ms from a 38-41 ms frame.
- Hiding street lamps cut about 10-13 ms, partly because they carry most shadow maps.
- Hiding floodlights changed nothing measurable.

Back to back on a cool GPU (about 1140 MHz), with ranges x0.8, the OSM map ran at 32 fps at
spacing 5 and 36 fps at spacing 7. So shorter ranges give most of the gain. Screenshots show
spacing 7 removing most coloured pools from view, leaving the flashlight and little else. Turbo
was off for these two runs, so they don't compare directly with the earlier tables.

**Kept: spacing 5, ranges x0.8.** It keeps the colour and most of the gain.

Unexplained: on level 1, spacing 7 drops `fragment_shader_invocations` from 1.59M to 0.90M and
moves GPU time from the main pass to the rest of the frame, though screenshots show the same
geometry. Don't compare level 1 pass timings across lamp counts until that is understood.

**Cluster layout is not worth tuning.** 2, 4 or 8 depth slices instead of 24 made no difference.
4096 clusters is a hard limit: Bevy panics above it.

The overlap is moderate, not extreme. The cost comes from the number of lit pixels times a few
lights each, so it only falls by removing lights or shrinking their reach, and both change how
levels look.

## Measuring

```bash
cargo run --release -- --play --perf
cargo run --release -- --play --perf --level assets/maps/map-01.osm.map
```

`--perf` (`src/perf.rs`) logs frame time, entity count and render pass timings every 5 seconds.
It also presents uncapped and skips story crawls.

Uncapped matters: with vsync, a window the compositor wasn't showing ran at exactly 1.000 fps.
That was probably a hidden window, though not confirmed. It makes the game look far slower than
it is.

Audio checks while the game runs:

```bash
pw-top                                        # ERR column, and busy time of the game's stream
ps -L -o tid,cls,rtprio,pcpu,comm -p <pid>    # pw_out should be RR
```

The log shows `audio output stream encountered underrun or overrun` on any underrun.

Power checks:

```bash
grep . /sys/class/powercap/intel-rapl-mmio:0/constraint_*_power_limit_uw
cat /sys/devices/system/cpu/cpu0/thermal_throttle/package_throttle_count
cat /sys/class/drm/card1/gt_act_freq_mhz
```

## Limits of these numbers

- Audio was checked by counters, not by ear.
- FPS moved between 11 and 17 on the same OSM setup across runs, so small differences mean
  little.
- The player stood idle, with no large fights. Expect lower numbers in real play.
- Stack sampling and the offline DSP bench were temporary code and aren't in the repo.

## Next steps

1. Done: the Night preset's moon no longer casts shadows (`LightingPreset::sky_shadows`). The
   tables above are from before that change.
2. Try `Hardware2x2` filtering, and a smaller lamp shadow budget, on low-end hardware.
3. Done: lamps stay at spacing 5, with every range cut to x0.8.
4. Test disabling turbo, and measure real render scale.
5. Build the quality system from those: render scale, shadow budget and filter, picked from the
   adapter type at startup.
6. Optionally skip the lane fills for idle synth voices. Not needed with LLVM, at about 1% of a
   core, but it would help on any slower machine.
