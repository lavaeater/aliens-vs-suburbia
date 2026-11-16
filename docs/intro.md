# The campaign intro

Before the first level of a fresh campaign, the camera follows a copter down through the
clouds and low across the town to the players' start. The team goes down ropes, the
camera settles into the game's own isometric view, and play begins while the copter
climbs away. Code: `src/intro/` (`plan.rs` is the flight as pure, tested maths).

## Setting it up

In `assets/campaigns/<name>/campaign.ron` (every field may be left out):

```ron
intro: Some((
    music: "music/intro.mp3",      // inside assets/, mp3 / ogg / wav; played once
    music_db: 0.0,
    copter: (
        model: "",                 // a .glb inside assets/, nose along -Z; "" = built-in stand-in
        scale: 1.0,
        turn_degrees: 0.0,         // for a model whose nose is not along -Z
        rotors: [                  // nodes of the model to spin
            // (node: "MainRotor", axis: (0.0, 1.0, 0.0), rpm: 380.0),
        ],
    ),
    timing: (dive: 10.0, low_pass: 9.0, flare: 4.0, rappel: 5.0, handoff: 3.0),
    clouds: (base: 70.0, top: 115.0),   // metres above the players' start
)),
```

`timing` is seconds per part: over and down through the clouds, low over the town, the
flare into the hover, the ropes, and the camera's hand-off. Play begins at their sum
(31 s by default). Fit them to the track.

## While it runs

- The level is loaded and alive underneath (townsfolk, lamps).
- `spawn_players` waits, and the map's `SpawnPlayer`s are held back. Stand-ins in each
  player's model go down the ropes, and the real players spawn where they land.
- The stories are off (`StoryStore::active`), so there is no `LevelStarted`, no intro
  crawl, and no waves or spawners until the players are in.
- `camera_follow` and the camera settings wait. The intro flies a perspective camera,
  and the hand-off is a dolly zoom down to a 3 degree view matching the orthographic
  one, then switches to it.
- The generative music is ducked (`Mix::duck_db`) until the intro's track ends.
- The map's background plane is stretched under the whole flight, so its edges don't
  show from the air.
- On a level cut from a base map (`map::compose`, the `gis` levels), the whole base is
  drawn round the level while flying (`backdrop.rs`): its ground, walls, roads, radhus
  and trees, all at once (`streaming::spawn_scenery`), lined up with the level
  (`compose::level_offset`: back by the region's corner, down by the ground the level was
  lowered by), with no colliders, pathfinding or grass. The level's own chunk scenery
  (`ChunkScenery`, its border wall too) is hidden meanwhile; its lamps and grass stay.
  The copter's flight clears the town's hills, not only the level's. At the hand-over
  the backdrop is despawned and the level shows again.

Esc / Enter / Space / Start / Cross skip to the hand-off. `--no-intro` leaves it out.
`--intro-at <secs>` starts the flight that far in (for looking at one moment, e.g. with
`--shot`). A retry of the first level does not replay it.

## Clouds

`clouds.rs` and `assets/shaders/clouds.wesl`, after Sebastian Lague's raymarched clouds:
a box drawn from the inside. Each pixel marches the view ray through it, reading density
from a tiling 64^3 noise texture made on the CPU at startup (Worley over value noise for
the shape, three Worley octaves to erode the edges). It is lit by Beer-Lambert towards
the light, with a two-lobed Henyey-Greenstein phase. The look's numbers are in
`cloud_params`.

## Known gaps

- The town round the level has no lamps: at night only the level is lit.

- Seen from above, the deck reads weak. It looks best from the side and from below.
- The stand-ins play a fall/jump/idle clip from their own model only (no external
  animation sources).
- Clouds do not cut against the copter when it is inside them (no depth read).
- Mouse/keys are free during the hand-off. A player who moves then shifts the final
  framing slightly.
