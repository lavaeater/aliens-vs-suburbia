# Performance on weak hardware

Findings from 2026-11-05, measured on the low end of what we want to run on: a ThinkPad T14
Gen 1 (i7-10610U, 4 cores / 8 threads, UHD Graphics CML GT2, 32 GB, 1920x1080, Mesa 26.2.6
on COSMIC Wayland, Bluetooth AAC headphones).

Symptom: dreadful audio stutter, while the picture mostly kept up.

## Summary

- **Audio stutter: a compiler setting, not hardware or drivers.** `~/.cargo/config.toml` set
  `codegen-backend = "cranelift"` for the dev profile. Removed; audio no longer underruns.
- **Frame rate: a power limit.** CPU and GPU share a sustained 17 W. Level 1 runs at about
  24 fps and the OSM map at about 17 fps.
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

Temperatures reached 97-99 C in some runs, and the package logged about 74,000 throttle events
in one boot (builds included).

Stay on the `performance` power profile. `balanced` was slower: 20 vs 23 fps on level 1, 15 vs
17 on the OSM map.

### Graphics settings

Release build, OSM map, 1080p, uncapped:

| Setting | FPS | Gain |
|---|---|---|
| Baseline | 15.1 | - |
| MSAA off | 15.7 | +4% |
| 4 shadow lamps (from 16) | 17.6 | +17% |
| 0 shadow lamps | 20.1 | +33% |
| 960x540 | 20.6 | +36% |
| No MSAA + 4 lamps + 960x540 | 25.9 | +72% |

Level 1 went from 21 to 31 fps with all three.

So a quality system (`docs/not-right-now/warbell-suggestions.md`) would need render scale and
the `SHADOW_LAMPS` budget. MSAA barely matters here. Aim for a steady 30 fps on this class of
machine, not 60.

### Where a frame goes

`--perf`, dev build:

| Pass | Level 1 | OSM map |
|---|---|---|
| Frame time | 41.8 ms | 59.1 ms |
| `main_opaque_pass_3d` (GPU) | 28.3 ms | 39.8 ms |
| `msaa_writeback` | 0.5 ms | 0.5 ms |
| `clustering` | 0.3 ms | 0.4 ms |
| `upscaling` | 0.2 ms | 0.2 ms |
| Everything else timed | 0.1 ms | 0.1 ms |

The main opaque pass is about two thirds of the frame. **Shadow passes do not appear in these
numbers**, so the unaccounted 12-18 ms per frame may be shadows, but that isn't measured.

Open question: quarter resolution only gained 36%, which is less than a pass dominated by pixel
shading should give. Finding out why comes before building a quality system.

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

1. Find out what the main opaque pass spends its time on, and time the shadow passes.
2. Build the quality system from what that shows: render scale and shadow budget first, picked
   from the adapter type at startup.
3. Optionally skip the lane fills for idle synth voices. Not needed with LLVM, at about 1% of a
   core, but it would help on any slower machine.
