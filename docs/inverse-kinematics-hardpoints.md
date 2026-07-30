# Hardpoints & Inverse Kinematics for Held Weapons

Design doc for making characters dynamically pick up and hold weapons (pistols,
SMGs, rifles) without hand-tuning every character × weapon combination or opening
Blender for each model.

Status: **design agreed, not yet built.** Builds on the asset-browser socket
tool (`src/asset_browser/`), the skeleton overlay (`B`), and the `Attachment`
type in `src/assets/asset_definition.rs`.

---

## 1. The problem

Today an `Attachment` is `(bone, offset_transform)`, tuned by hand in the asset
browser for one specific character holding one specific weapon. That's **N × M**
hand-tuning: every character paired with every gun needs its own offset. It also
doesn't survive animation intent well and is tedious to author.

We want: **any character can hold any weapon, positioned correctly, automatically.**

## 2. The idea: hardpoints

Put a **hardpoint** on both the character and the weapon, then align them.

- The character's hand has a **grip hardpoint** — "a weapon grip goes here, oriented like this."
- The weapon has a **grip hardpoint** — "this is the part a hand grips, oriented like this."

To hold the weapon, make the two hardpoints **coincide**. Author one grip per
character hand and one grip per weapon, and every combination works:

> **N + M authored frames instead of N × M.** This is the whole payoff.

A rifle gets more than one hardpoint:

| Hardpoint | On the character | On the rifle |
|---|---|---|
| `grip` (trigger hand) | right-hand bone | the pistol grip |
| `foregrip` (support hand) | left hand (via IK, see §9) | the handguard / foregrip |
| `stock` (optional) | shoulder/clavicle | the buttstock |
| `sight` (optional) | head/eye | the aim-down-sight point |

## 3. Important correction: a hardpoint is a *frame*, not a vector

The intuition "a hardpoint is a point + a direction, align them" is *almost*
right, but a single direction leaves the weapon free to **roll** around that
direction. A pistol aligned only by its barrel direction could still have its
grip pointing sideways or up.

To fully pin orientation you need a **second axis** (e.g. which way the grip
points). Position + two axes = a full orientation. So:

> **A hardpoint is a small coordinate frame: a position and a rotation, stored
> relative to a bone (character) or the model origin (weapon).**

In Bevy that's just a `Transform` (translation + quaternion). Conveniently, the
asset-browser nudge tool **already produces exactly this** — the offset transform
we author is a frame. We're not inventing new authoring, just reinterpreting the
data and adding a matching frame on the weapon.

## 4. The snap math (where the quaternion pain lives — and stays)

All the frame algebra is one idea: *make weapon-grip-frame equal hand-grip-frame.*

Let (all as `Transform` / `Isometry3d`, composed as matrices):

- `hand_bone` = world transform of the character's hand bone (from animation)
- `grip_offset` = the character's grip hardpoint, **local to the hand bone**
- `weapon_grip` = the weapon's grip hardpoint, **local to the weapon origin**

We want the weapon placed so that its grip frame lands on the hand's grip frame.
If we parent the weapon to the hand bone, its required **local** transform is:

```
weapon_local = grip_offset * inverse(weapon_grip)
```

Read it right-to-left: `inverse(weapon_grip)` moves the weapon so its grip sits
at the weapon's origin; `grip_offset` then moves that to where the hand's grip
frame is. Parenting to `hand_bone` supplies the animated world placement for free.

**This is the only quaternion-heavy spot, and it's ~3 lines.** Everything else in
the codebase keeps thinking in named hardpoints and `Transform`s. The plan is to
wrap this (and the IK below) in small, tested helper functions so the rest of the
game — and future-you — never hand-multiplies quaternions.

## 5. Data model

Weapons become a first-class model type (or gain props) carrying their
hardpoints; characters carry theirs. Sketch (final field names TBD in §12):

```rust
/// A named coordinate frame relative to some anchor.
struct Hardpoint {
    /// For characters: the bone this frame is relative to (e.g. "mixamorigRightHand").
    /// For weapons: empty = relative to the model origin.
    anchor: String,
    translation: [f32; 3],
    rotation_euler_deg: [f32; 3], // authored in degrees; converted to quat at use
}

// Character def gains:
hardpoints: HashMap<String, Hardpoint>,   // e.g. { "grip": ..., "foregrip_target_pole": ... }

// Weapon def (new ModelType::Weapon(WeaponProps) or Item extension) gains:
hardpoints: HashMap<String, Hardpoint>,   // e.g. { "grip": ..., "foregrip": ..., "stock": ... }
hands: WeaponHands,                        // OneHanded | TwoHanded
```

The current `Attachment` type is effectively a hardpoint already; it can be
folded into this or kept as the "manual override" escape hatch.

## 6. Authoring in the asset browser (reuse what we built)

No new tool needed, just repurposed:

- **Skeleton overlay (`B`)** = the debug view for where hardpoints sit.
- **Bone list + cycle (`,` / `.`)** = pick which bone a character hardpoint anchors to.
- **Nudge controls (Pos / Rot / Scale)** = author the frame precisely, live.
- The live preview shows the *actual snapped weapon* using §4, so you tune the
  hardpoint frames and immediately see any weapon click into place.

Authoring flow:
1. Load a **character**, place its `grip` hardpoint in the hand (nudge until a
   reference weapon sits right). Save to the character def.
2. Load a **weapon**, place its `grip` hardpoint on the grip (and `foregrip` /
   `stock` for rifles). Save to the weapon def.
3. Any character + any weapon now snaps automatically — no per-pair tuning.

## 7. Runtime snap (deferred, but simple)

When a character equips a weapon in-game: load the weapon scene, find the hand
bone entity by name, compute `weapon_local` (§4) from the two defs' hardpoints,
spawn the weapon as a child of the hand bone with that local transform. It then
follows animation automatically. (This is the same mechanism the browser preview
already uses, just driven by hardpoint data instead of a hand-tuned offset.)

## 8. Two-handed weapons: split FK + IK

Don't try to satisfy all hardpoints with one solve. Decompose:

- **Dominant (trigger) hand → forward kinematics.** Parent the rifle to the right
  hand via the `grip` hardpoints (§4). The rifle is now fully placed and moves
  with the right hand through any animation.
- **Support (off) hand → inverse kinematics.** The rifle's world position is fixed
  by the right hand, so the rifle's `foregrip` hardpoint is a **known world
  point**. Solve the **left arm** so the left hand lands on it (§9).
- **Head / lean (optional, later)** → an aim constraint on the neck toward `sight`.

This avoids ever needing a general multi-constraint solver.

## 9. The IK: two-bone analytic (your 2D stick, in 3D)

An arm is a **two-bone chain** (upper arm + forearm, i.e. shoulder → elbow →
wrist). Two-bone IK has a **closed-form** solution — no iterative solver:

1. **Elbow angle** from the **law of cosines**, using the three lengths
   (upper-arm, forearm, shoulder-to-target distance). This is the same
   Pythagoras/trig as the 2D stick — literally one `acos`.
2. **Shoulder aim**: rotate the upper arm so the chain points from the shoulder
   toward the target.
3. **Pole vector (elbow hint)**: the one thing 2D didn't need. In 3D the elbow's
   bend plane is free, so we supply a hint direction (e.g. "elbows point down and
   out") to disambiguate. Usually a per-character constant or a hardpoint.

Clamp when the target is out of reach (arm fully extended points at it).

### What's actually hard here

Not the trig — the **bookkeeping**:

- Results must become **quaternions in each bone's local space**, respecting the
  bone's rest orientation and roll. Mixamo/mesh2motion rigs have arbitrary bone
  axes, so "rotate the upper arm to aim at X" is not a clean world-space rotation.
- It must run **after** the `AnimationPlayer` samples the clip but **before**
  global-transform propagation, so we override the animated pose. That's a
  specific Bevy system-ordering slot.

Mitigations: keep the solver a small, unit-tested pure function
(`solve_two_bone(root, mid, tip_target, pole) -> (root_rot, mid_rot)`); build it
against the amy rig with the **skeleton overlay as the live debug view**; start
with position-only targeting before adding hand orientation.

## 10. Bevy integration reality

- **No built-in IK.** `bevy_mod_inverse_kinematics` exists but has historically
  lagged Bevy versions — verify it builds on 0.18 before relying on it; writing
  our own two-bone solver (~50–100 lines) is likely faster and fully in our control.
- **System ordering.** The IK/aim systems run in `PostUpdate` after
  `bevy_animation`'s sampling and before `TransformSystems::Propagate`. Getting
  this slot right is essential; wrong order = IK gets stomped by the animation or
  never propagates.
- **Bone lookup.** Reuse the `SkinnedMesh.joints` + `Name` approach already used
  by the skeleton overlay and attachment preview.
- **Quaternion containment.** All quat math lives in `snap` (§4) and
  `solve_two_bone` (§9). Nothing else multiplies quaternions.

## 11. Staging plan

| Stage | Deliverable | Effort | Risk |
|---|---|---|---|
| **1. Hardpoint snapping** | Weapon-side hardpoint data; author grip frames in the browser; auto-compute the snap so any one-handed weapon clicks into any hand. Rifles held in the dominant hand only (support hand ignored). | Medium | Low |
| **2. Two-bone IK** | Analytic support-hand IK onto the rifle's `foregrip`. Position-only first, then hand orientation. | Medium–High | Moderate |
| **3. Polish / runtime** | Head-aim toward `sight`, pole-vector tuning, and in-game equip/spawn driven by hardpoint data. | Medium | Low–Moderate |

**Do stage 1 first.** It delivers most of the "dynamic" feel, de-risks stage 2
by getting hardpoint frames authored and visualized before any IK math, and stays
useful even if IK is deprioritized (one-handed weapons just work).

## 12. Decisions (locked) & open questions

**Locked:**

- **Weapon model type** → new `ModelType::Weapon(WeaponProps { hands })`. First-class
  gameplay category; gameplay stats added later.
- **Hardpoint storage** → dedicated top-level `hardpoints: HashMap<String, Hardpoint>`
  on `AssetDefinition`, where `Hardpoint { anchor: Option<String>, translation,
  rotation_euler_deg }` (`anchor = Some(bone)` for characters, `None` = model origin
  for weapons). The existing `Attachment` stays as a manual "bolt this fixed prop
  here" escape hatch.
- **Rotation storage** → authored as XYZ Euler degrees (human-readable RON, matches
  the nudge UI); converted to quaternion at use via `assets::hardpoint`.
- **Dominant hand** → implicit in the character's `grip` hardpoint anchor bone; no
  separate flag. Resurfaces only for stage-2 support-arm IK.

**Still open (not blocking stage 1):**

- Role vocabulary is convention: `grip`, `foregrip`, `stock`, `sight`.
- **Character hardpoints per skeleton family**: Mixamo/mesh2motion characters share
  bone names, so one authored `grip` could apply to a whole family — worth a "copy
  hardpoints from" convenience later.
- Weapon gameplay stats (damage, fire rate, ammo, projectile) — deferred.

## 13. Progress

- ✅ **Snap math** (`src/assets/hardpoint.rs`): `frame_from_euler`, `hardpoint_frame`,
  `weapon_local`, `weapon_world`, `transform_from_frame` — unit-tested (weapon grip
  lands exactly on character grip; world == parented-local).
- ✅ **Data model**: `ModelType::Weapon(WeaponProps)`, `Hardpoint`, and
  `AssetDefinition.hardpoints` added and round-tripped through the browser's
  load/save.
- ✅ **Browser authoring UI**: `-- Hardpoints --` section — role chips
  (grip/foregrip/stock/sight), anchor a character role to the selected bone, Pos/Rot
  nudge, delete; `H` toggles RGB axis-gizmo frames; "Preview selected weapon" snaps a
  saved weapon onto a character's `grip` live via `weapon_local`.
- ✅ **In-game equip** (`src/player/systems/equip.rs`): `PlayerProps.weapon` names a
  weapon def path; `PendingEquip::resolve` reads both defs at spawn, and
  `equip_pending_weapons` waits for the skeleton, finds the grip bone *under that
  character*, and spawns the weapon as its child using the shared
  `hardpoint::snap_transform`. Browser preview and in-game equip call the same
  function, so they can't drift apart.
- ⏳ **Stage 2**: two-bone analytic IK for the support hand onto the rifle `foregrip`.
  Needs a `foregrip` on the weapon and the arm chain (shoulder/elbow/hand bones) on
  the character — currently only `grip` is authored.

## 14. Glossary (for when the brain hurts)

- **Frame**: a position + orientation; a little set of XYZ axes floating in space.
  A `Transform` is a frame. "Aligning two frames" = making their positions and
  axes coincide.
- **Quaternion**: the compact, gimbal-lock-free way to store a 3D rotation. We
  author rotations as Euler degrees and let helpers convert; we never read raw
  quat components by hand.
- **Hardpoint**: a named frame we attach to a model to say "this part connects here."
- **Pole vector**: a hint direction that tells a two-bone IK solve which way the
  elbow (or knee) should bend, since the straight-line solution is ambiguous in 3D.
- **FK (forward kinematics)**: bones drive the end — rotate the shoulder, the hand
  follows. What animation clips do.
- **IK (inverse kinematics)**: the end drives the bones — put the hand *here*, and
  solve what the shoulder/elbow must do.
