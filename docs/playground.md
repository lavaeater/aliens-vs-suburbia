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

### Known issues

- **Health bars are offset by the width of the left pane.** `lava_ui_builder`'s
  `world_follower_system` positions UI nodes from `Camera::world_to_viewport`, which returns
  *viewport*-relative coordinates, while the node itself is laid out in window space. With a
  full-screen camera the two agree; with the playground's clipped viewport they differ by the
  pane origin. The fix is one line in the submodule — add
  `camera.logical_viewport_rect().map(|r| r.min).unwrap_or(Vec2::ZERO)` to `pos` — but
  `lava_ui_builder` is a separate repo, so that is a deliberate call to make rather than a
  drive-by edit.
- **The default player model has no torso twist.** Without a roster the playground spawns
  `Character Soldier`, whose rig has no mixamo spine bones, so the twist logs
  "bones never appeared on this rig" and disables itself. Stage 2's model picker is the fix.
