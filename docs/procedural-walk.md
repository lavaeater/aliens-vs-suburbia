# The procedural walk

> **Off by default.** The clip's own legs look better today, so that is what ships. `F9` —
> or the gait panel's Legs toggle — turns this on. What is left to do is at the bottom.

With it on, the player's legs are not animated: the clip still drives the whole body, but
every frame the thigh and shin rotations — and the pelvis height — are overwritten by a
solve that puts the feet on the ground and keeps them there. `F9` toggles it back off for an
A/B against the clip's own legs, and nothing needs undoing when it goes off, because the
animation rewrites those bones from the clip on the very next frame.

Three pieces: `src/player/systems/gait.rs` decides where the feet go, in world space, and
knows nothing about the engine. `src/player/systems/leg_ik.rs` finds the leg chains, keeps
the ground and the hips honest, and calls `arm_ik::solve_elbow` — a leg is the same two-bone
problem as an arm. The tuning panel is `F10` in the playground, and the "gait targets"
overlay on the debug panel draws what it is doing.

## The one idea

A planted foot does not move. It is nailed to a *world* position and stays there while the
hips travel over it; only the swinging foot moves. Animating both feet in character space is
what produces skating, and no amount of tuning fixes it, because the character translates
out from under any local-space pose however good the curve.

The cycle is driven by **distance travelled, not time**. One stride of ground covered is one
full cycle, always. Stride length and speed therefore cannot disagree: walk slowly and the
steps are as long as they were, they just take longer to happen. Sprint and they come
quicker. There is no playback rate to keep in sync with movement speed, which is the thing
that always drifts in a clip-driven walk.

## The parameters

All of them are stated at **human scale** — a person with a 0.85 m leg — and rescaled to
whatever rig is wearing them, so the same numbers mean the same walk on a 30 cm character.
Whatever you set is then **clamped to what the legs can actually reach**, which is the part
that surprises people, and it is why the panel's bottom half exists.

| | what it does |
|---|---|
| **Stride** | Metres of ground per full cycle (two steps). This is the master control for step length: bigger means fewer, longer steps at the same speed. |
| **Stance** | How far apart the feet are, across the direction of travel. |
| **Step up** | How high the swinging foot lifts at mid-step. |
| **Fore/aft** | Shifts the whole footfall pattern forward or back relative to the hips, in strides. Negative puts the feet down further back — the same thing as the body riding further forward over them. |
| **Hip bob** | How far the hips rise and fall over the cycle, as a fraction of leg length. Highest over the planted foot, lowest between steps, twice per cycle. |
| **Hips** | The mean hip height, as a fraction of leg length. `anim` hands it back to the animation. |
| **Knee max / min** | What the knee will do, as the interior angle at the joint: 180 degrees is a leg straightened into a stilt, small angles are a heel folded up under the body. These are anatomy rather than choreography, and they set the reach ceiling below. |
| **Duty** | The fraction of the cycle each foot spends on the ground. Above 0.5 there is always a foot down (a walk); below, a flight phase (a run); 0.5 exactly is a march. |
| **Speed** | `GameSettings::player_speed_multiplier`, the character's actual movement speed. Not a gait parameter — it is what the gait is responding to. |

## Why your stride does nothing (the reach ceiling)

A leg whose hip rides `h` above the ground can put its foot at most

```
sqrt(L² - h²)
```

away from directly underneath itself. That number collapses as `h` approaches `L`, and rigs
are modelled standing up straight, right at the top of that curve. swat-2 stands at 96% of
full leg extension: a 17 cm leg with **4 cm** of horizontal reach. Ask for a longer stride
and the foot simply cannot get there; the leg straightens and stops short, and you get a
character walking on tiptoe over ground it never touches.

`L` is not `upper + lower`. That is the length of a leg with a knee that locks dead
straight, which no knee does, and it is also the singularity of the two-bone solve — at full
extension the knee's bend direction is undefined and a hair of noise flips it anywhere it
likes. `L` is the law of cosines across the leg's own triangle at the angle the joint
actually permits:

```
L = sqrt(upper² + lower² - 2·upper·lower·cos(knee max))
```

Two things fall out of the same limits. The **fold** limit (`Knee min`) says how near the
ankle can get to the hip, which is the ceiling on `Step up`: you cannot lift a foot closer
to the hip than a folded knee allows. And the solve clamps its target into the ring between
the two, so when the gait does ask for something impossible the foot comes off its mark
rather than the knee going somewhere a knee does not go.

Note that limits near full extension cost almost no reach — cosine is flat there, so 175
degrees rather than 180 gives up about 0.1% of the leg. What they buy is a knee that never
locks or inverts. Take `Knee max` down to 150 and you will see the reach ceiling drop
properly, along with the stride that depends on it.

So the stride is capped rather than allowed to lie:

```
stride ≤ reach / (duty/2 + |fore-aft|)
```

Three consequences worth knowing:

- **Lowering `Hips` is what buys stride.** At 96% you have 28% of a leg's worth of reach; at
  90%, 44%; at 85%, 53%. A centimetre of hip height is worth several of stride, which is why
  the gait takes the pelvis over at all, and why real walking drops the hips as the legs
  scissor apart.
- **`Stance` and `Fore/aft` are spent out of the same budget.** They are all distances from
  under the hip, so a wide stance leaves less for the stride, and a large bias leaves less
  still — the bias pushes one end of the stance further out, so the whole stride has to
  shrink to keep the trailing foot reachable.
- **`Step up` is capped too**, by how far the knee folds: the foot cannot be lifted nearer
  the hip than a folded knee allows. Crouch the character down and the ceiling drops with
  it, because a knee that is already folded has less left to give.

The `as walked` rows at the bottom of the panel show what actually came out: the rig's scale,
its leg length, where its hips are, the reach, the **fitted stride**, and the resulting steps
per second. If the fitted stride stops following the slider, you have hit the ceiling and the
next thing to change is `Hips`.

## Reading the overlay

Turn on "gait targets" in the playground's debug panel. **Left is orange, right is blue**,
consistently — the first question the overlay gets asked is which foot you are looking at.

- a **ring** around the hips: the reach limit. Anything outside it is a foot the legs cannot
  get to.
- a **line** with a big cross at one end and a faint one at the other: where that foot should
  touch down, where it should leave the ground, and the stance in between. The body travels
  the length of that line while the foot holds still.
- a **sphere**: where the foot is right now. While it is in the air it is joined back to the
  footprint it left.

## Things it does not do yet

- **Feet keep the clip's orientation.** They will not lie flat on a slope, or roll heel to
  toe.
- **The ground is a plane** under the character, taken from the model's own origin. No
  raycast, so stairs and ramps are not handled.
- **Only the knee is constrained.** The hip and ankle have no limits, so the solve will
  happily rotate a thigh further than a hip joint would allow. The knee was first because it
  is the one that decides how far the character can reach, and so how long a stride it can
  take. See `docs/inverse-kinematics-hardpoints.md` for the same solver's use on arms.
- **Limits live in the gait settings, not on the character.** They are anatomy, so they
  belong in the model's `.ron` def beside `aim_bones` and `hardpoints`. They are here for
  now because here is what has a tuning panel attached to it.
