# Playground

To get where we are going, we now need to construct a **playground** where we can tweak all things related to, for instance, player movement, gun handling, animations, shooting, etc.

The playground should be a combination of the regular "Play the Game" where we load a map and we play the game and the Asset Browser.

The Playground should be its own choice when we start the game. 

I imagine a two-pane window, one pane is the UI used to tweak and modify stuff, stuff that is then saved to gamesettings.ron or the model def ron-file, like amy.ron. The other pane is the player model - and in this case we can walk around with the player model as we would in-game. The map is small and contains say, three static enemies that simply die and respan if attacked. We will add pickups and stuff for tweaking to the map.

The models that should be available to load and modify should be explicitly imported - i.e. some kind of file selector lets us look up a model file, we import it - which simply means we create a ron-file for it and from then on it is automatically listed in a drop-down or list or something so we can select it and work on it. 

We should be able to toggle Avian 3D debug view here. We should be able to render model skeletons, and also, importantly, render widgets for model hardpoints - and move / add / remove them if needed.

Come up with a plan below and then get to work

## Claude's plan

### The one real architectural decision

The playground needs the *whole* game running — physics, control, animation, throwing,
towers, collision, health. Today that behaviour is gated on `in_state(GameState::InGame)`
in 41 places across 20 files.

Two ways to get it:

**A. A new `GameState::Playground`.** Every gameplay system needs a shared run condition
(`in_state(InGame).or(in_state(Playground))`). Mechanical, but permanent: every gameplay
system added from now on silently does nothing in the playground until someone remembers
to widen its condition. That's a footgun that never goes away.

**B. Enter `GameState::InGame` with a `PlaygroundSession` resource inserted.** All 41 sites
keep working untouched, and so does every system added later. The handful of things that
*must* differ get run conditions of their own.

**Going with B.** The cost is that `GameState::InGame` no longer means "playing the real
game" — it means "the simulation is live". A `PlaygroundSession` resource plus two run
conditions (`in_playground` / `in_normal_game`) make the distinction explicit where it
matters. This is reversible: if it gets muddy, promoting it to a real state later is the
same mechanical edit as A.

The pieces that need `in_normal_game`:

| Thing | Why |
|---|---|
| `load_map_one` (`OnEnter(InGame)`) | playground loads its own map |
| `spawn_ui` (HUD) | replaced by the playground's two-pane UI |
| wave spawning | `WaveManager::default()` has hardcoded waves; playground clears them |
| score keeper / game-over transition | no win/lose in a sandbox |

### Stages

**1 — Session, map, two-pane shell.** `PlaygroundSession` resource + run conditions;
`Playground` menu button (inserts the resource, sets `InGame`); `assets/maps/playground.ron`,
a small walled arena with `waves: []`; playground setup clears `WaveManager`. Left pane =
tweak UI (empty scaffold), right pane = the game camera, restricted with the same
`Camera::viewport` trick `sync_viewer_viewport` uses in the asset browser. Three
`TargetDummy` enemies that take damage, die, and respawn on a timer. **Outcome: walk around
and shoot dummies in a two-pane window.**

**2 — Model list + import + live swap.** Left pane lists `assets/defs/*.ron` filtered to
`ModelType::Player`. A file browser over `assets/packs/**` (reusing `scan_folder`) imports a
`.glb`, writing a minimal def ron — from then on it's in the list. Selecting one despawns
and respawns the player from that def. **Outcome: import a model, walk around as it.**

**3 — Debug views.** Panel toggles for avian debug gizmos (the existing F3), skeleton
overlay, and hardpoint frame widgets. `draw_skeleton_gizmos` / `draw_hardpoint_gizmos`
currently read `AssetBrowserState`, so the drawing gets lifted into functions taking the
bones/hardpoints directly, called from both the browser and the playground.

**4 — Live hardpoint editing.** List roles on the selected def, add/remove, nudge
translation and rotation with the browser's existing nudge-row widget — applied to the
*live* player and its equipped weapon, so you see the gun move while standing in the map.
Save writes back to the def ron.

**5 — Animation + settings.** Bind animation keys to clip tag paths and trigger clips on
the live player; sliders for the `GameSettings` fields worth feeling in motion (camera
yaw/pitch/zoom, `player_speed_multiplier`) saving to `game-settings.ron`.

Stage 1 is the one that proves the architecture; 2–5 are additive panels on top of it.

## Progress

### Stage 1 — built

`src/playground/`: `state.rs` (session + run conditions), `plugin.rs`, `ui.rs` (two-pane
shell + viewport clipping), `dummies.rs`. Map at `assets/maps/playground.ron`. Menu button
plus a `--playground` flag that skips the menu.

Verified with a probe system: 1 player, 3 dummies, camera viewport clipped to
`([330, 0], [938, 1359])` — i.e. exactly the right-hand pane.

Three things worth recording:

- **The corner `EnemyExit` tile is load-bearing.** `move_towards_goal_system` logs
  "Has no goal" *every frame* when no `AlienGoal` entity exists — not per alien, just
  unconditionally. A goal-less arena floods the log. The corner tile exists only to shut
  that up; nothing walks to it.
- **Dummies drop their AI data.** `Alien`'s `#[require]` list pulls in `MoveTowardsGoalData`
  and `AvoidWallsData`. A `Static` body ignores the velocity those systems write, but they
  still run A* every frame, so the dummy explicitly `remove`s them after spawn. Being a
  real `Alien` otherwise matters: `auto_aim` targets `With<Alien>`, so aiming and the whole
  shooting chain behave as they do in a match.
- **`AlienCounter` must be incremented on spawn.** `collision_handling_system` decrements it
  on every alien death; a dummy that was never counted underflows the `u32`.

### Stage 2 — built

`src/playground/models.rs`: `PlaygroundModels` holds the imported-def list, the import
browser's folder state, and the pending swap. Two mouse-driven lists in the left pane —
the keyboard is busy walking the character, so binding Up/Down/Enter there would fight the
game.

Selecting a def goes through `PlayerRoster` rather than poking the player entity, so the
swap takes exactly the path a real match takes: `spawn_players` derives scene, ability,
throw rate, animation graph and equipped weapon from the def.

**The swap needs two frames.** `spawn_players` skips its `SpawnPlayer` event while a player
still exists, and the despawn only lands when commands are applied — so `swap_player_model`
parks the position in `pending_position` and finishes on a later frame once the old player
is actually gone. Respawning at the old player's `Position` means a swap does not teleport
you.

Import writes a minimal def (`model_path` + `ModelType::Player`, nothing else) and refuses
to overwrite an existing one — re-importing a model you had already tuned would silently
reset its scale, hardpoints and animation bindings. Scale stays at the default because
computing it needs the mesh AABB, which needs the model loaded; the asset browser is still
the place for that.

Verified end-to-end with a probe: 4 player defs found, 15 folders under `assets/packs`,
and selecting `amy.ron` respawned the player with `packs/mesh2motion/amy.glb` loaded — with
no torso-twist warning, i.e. the spine chain resolved on the new rig.

### Stage 3 — built

`src/assets/gizmos.rs` now holds the skeleton and hardpoint drawing, called by both the
asset browser and `src/playground/debug.rs`. Three panel toggles: physics colliders,
skeleton, hardpoint frames.

**The joint selection, not the drawing, is what differs between the two screens.** The
browser has one model in an empty scene, so `all_joints` — every skinned mesh in the world
— is correct there. The playground has the player *and* three skinned dummies: a probe
found 16 skinned meshes in the scene, of which 65 joints belong to the player. Using the
browser's approach would have drawn all four skeletons on top of each other. Hence
`joints_under(root)`, which walks *down* from the character: a joint's `ChildOf` chain leads
through the armature, but the `SkinnedMesh` component sits on the mesh entity, which is a
sibling of the armature rather than an ancestor of the joints.

The physics toggle writes avian's `PhysicsGizmos` config directly and `sync_physics_toggle`
reads it back, so the button and `F3` cannot disagree — there is one source of truth rather
than a mirrored flag that drifts.

Hardpoints come from `PlayerAssetDef`, which `spawn_players` already populates for slot 0,
so no extra plumbing was needed. Only the *character's* hardpoints are drawn; the equipped
weapon's own frames need its def at hand, which is stage 4's job anyway.

### Stage 4 — built

`src/playground/hardpoints.rs` plus a panel section: role chips, translation and rotation
nudge rows (coarse/fine, `<< < > >>`), an anchor-bone picker built from the *live* rig, a
remove button, and Save.

Edits go into `PlayerAssetDef` — the def resource `spawn_players` already populates — and
two things then follow without extra plumbing:

- the hardpoint overlay reads that resource, so the gizmo moves as you nudge;
- `keep_weapons_snapped` rebuilds the weapon's transform *every frame* from
  `WeaponModel::char_grip`, so pushing the edited grip into that component is all it takes
  for the held weapon to follow. That is why `WeaponModel` gained a `set_char_grip`.

Nothing touches disk until Save. Verified end-to-end: nudging the grip +0.5 on Y moved the
equipped pistol exactly 0.5, and turning it +45 from 180 stored -135 rather than 225.

**Rotation is wrapped to (-180, 180].** Without it the numbers grow without bound as you
spin a hardpoint round — identical on screen, nonsense in the saved `.ron`.

A new role inherits an existing role's anchor bone rather than defaulting to the model
origin, which would drop the gizmo at the character's feet and read as a bug.

### Stage 5 — built

**Settings: reused, not rebuilt.** The HUD already carries `spawn_camera_panel` and
`spawn_model_panel` (F1 / F2), and their update systems already run for the whole of
`InGame` — only `spawn_ui` was gated out of the playground. So the playground spawns those
two panels and gets live camera yaw/pitch/zoom, projection, player speed and the model
transform sliders for four lines of code instead of a duplicate set that would drift.

**Animation:** `src/playground/animation.rs` plus a panel listing every key, what it
currently resolves to, and the tag paths it can be re-bound to. Clicking a key plays it on
the character standing in the arena — the quickest way to find out that a rig's "wave" is
a T-pose.

Re-binding takes effect with no respawn: `build_player_anim_graph` folds
`animation_bindings` and `clip_tags` into the signature it compares against `last_sig`, so
mutating `PlayerAssetDef` makes it rebuild the graph on the next frame. Verified: amy's
`wave` resolved to `Greeting RT`; re-binding it to the `building` tag re-resolved to
`Interact RT` within three seconds, with the `AnimationStore` rebuilt.

The key strings are `AnimationKey::default_search()`, which is what `resolved_clip` is
called with at runtime — so what you bind is exactly what the game looks up. A binding
whose tag matches no clip is called out explicitly rather than shown as unbound; it looks
bound but plays nothing, which is the failure worth surfacing.

### Known issues

- **Health bars are offset by the width of the left pane.** ~~Fixed~~ — see below. `lava_ui_builder`'s
  `world_follower_system` positions UI nodes from `Camera::world_to_viewport`, which returns
  *viewport*-relative coordinates, while the node itself is laid out in window space. With a
  full-screen camera the two agree; with the playground's clipped viewport they differ by the
  pane origin. The fix is one line in the submodule — add
  `camera.logical_viewport_rect().map(|r| r.min).unwrap_or(Vec2::ZERO)` to `pos` — but
  `lava_ui_builder` is a separate repo, so that is a deliberate call to make rather than a
  drive-by edit.

  **Fixed** in `lava_ui_builder` 39ebfae, along with a second bug in the same function that
  was never playground-specific: `UiScale` multiplies every `Val::Px`, so followers rendered
  at `scale` times their intended position — dragged toward the top-left corner in *any*
  window narrower than `LavaTheme::ui_width` (1920). Confirmed by probe: a button authored
  `size_px(160, 40)` computes to `[105, 26]` at scale 0.66.
- **The default player model has no torso twist.** Without a roster the playground spawns
  `Character Soldier`, whose rig has no mixamo spine bones, so the twist logs
  "bones never appeared on this rig" and disables itself. Picking `amy` from the model list
  fixes it for the session; making the playground *default* to the last model used is a
  small follow-up.
