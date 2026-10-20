# Character Editor

One screen for everything we do to a model: find it, import it, size it, tag its clips,
bind its animations, place its hardpoints, hand it a gun, and then walk around and shoot
things with it. It replaces both the **Asset Browser** (`src/asset_browser/`) and the
**Playground** (`src/playground/`). When it reaches parity, both are deleted.

## Why

We have two editors that overlap:

| | Asset Browser | Playground |
|---|---|---|
| Scene | one model, empty void | the live game, an arena, three dummies |
| Model list | every `.glb` under a folder, keyboard nav | imported player defs only, mouse |
| Writes | the whole def, rebuilt from its own working fields (`export_definition`) | `PlayerAssetDef` / `PlaygroundWeaponDef` in place, Save per side |
| Hardpoints | yes, plus a reference weapon preview | yes, live, both sides, plus muzzle tracers |
| Animation | clip tags (text edit) + bindings, cycle clips with `[` `]` | play keys through the real state machine, re-bind to tags |

Both edit hardpoints and animation bindings. Each holds its own copy of the def. Changes
made in one are easy to lose by saving from the other. We confused the two while building
them, and we will again. The fix is one editor, one working copy of the def, and one Save.

## Goals

- **One place.** Every authoring feature from either tool lives here, or is deliberately
  dropped (see *Parity checklist*).
- **Two stages, one model.** **Studio** shows the model held still on a turntable, for
  precise work. **Live** is the running game with the model walking and shooting, to check
  how it feels. Toggle with one key, without reloading.
- **An arena to drive in.** Both stages happen in a small walled arena
  (`assets/maps/playground.ron`). In Live you control the character exactly as in a match,
  with **keyboard and mouse or a gamepad**, so movement, aim and firing can be tried with
  both control schemes. No enemies are spawned. The arena is for handling, not combat.
- **Any model type.** Players, enemies, weapons, towers, terrain and items all open here.
  Live is only offered where it means something (see *Stages*).
- **A working copy that cannot be silently lost.** A dirty marker, one Save, and a prompt
  before switching models or leaving with unsaved edits.
- **Readable at a glance.** Tabs instead of one endless scrolling column, a toolbar over
  the viewport, and a status bar that always shows what is open, whether it is dirty, and
  which keys do what.

## Non-goals

- Map editing (`src/map_editor/` stays separate).
- Retargeting animations between rigs with different bone names.
- Two-bone IK authoring (separate work, `docs/inverse-kinematics-hardpoints.md`).
- Undo history beyond "revert to saved". A full undo stack is a possible follow-up.

## Architecture

### Session, not a new GameState

Keep the playground's decision (see `docs/playground.md`, *The one real architectural
decision*). The editor runs in `GameState::InGame` with an `EditorSession` resource, so
every gameplay system works in Live with no extra gating. `PlaygroundSession` becomes
`EditorSession`. `in_playground` / `in_normal_game` become `in_editor` / `in_normal_game`.

Studio does **not** leave `InGame`. It is the playground's inspect mode generalised:
- The follow camera is swapped for the orbit camera (`inspect.rs`).
- The player is held still (`hold_player_still`).
- The bind pose or a chosen clip is written last (`hold_rest_pose`).

So Studio and Live are flags on one session, which is why switching between them is
instant.

For non-player models, Studio spawns the def's scene as a plain prop at the arena centre,
the way the browser's `spawn_viewer_model` does, and the player is despawned. The arena
floor gives scale and lighting that the browser's void never had.

**No enemies.** The playground's target dummies (`dummies.rs`) are not carried over, and
waves stay silenced. Practice targets come back later as an opt-in toggle (plan step 8),
not as something always standing in the arena.

### One document: `EditorDoc`

The core change. Replace `AssetBrowserState`'s scattered working fields, and the
playground's split between `PlayerAssetDef` and `PlaygroundWeaponDef`, with:

```rust
#[derive(Resource)]
pub struct EditorDoc {
    /// Where it saves. `None` = previewing a file that has not been imported yet.
    pub def_path: Option<String>,
    /// The def being edited, whole. Panels mutate fields on it directly.
    pub def: AssetDefinition,
    /// What is on disk, for the dirty check and "revert".
    pub saved: Option<AssetDefinition>,
}
```

Plus a second, optional `EditorDoc` for the **held weapon**, edited from the character's
hardpoint tab. This is today's weapon side, generalised.

Rules:
- **Panels edit `doc.def` and nothing else.** No mirrored fields, no per-panel copies.
- **The live entity follows the doc.** For a player in Live, a sync system copies
  `doc.def` into `PlayerAssetDef`, so `build_player_anim_graph`, the hardpoint overlay and
  `keep_weapons_snapped` keep working through the paths they already use. Changes that
  need a respawn (`model_path`, `model_type`, `scale` on a player) are detected by
  comparing fields and go through the existing two-frame swap (`swap_player_model`).
- **Dirty = `Some(def) != saved`.** It is computed, never a flag, so it cannot drift.
  `AssetDefinition` needs `PartialEq` for this.
- **Save writes `doc.def` to `doc.def_path`, whole.** No rebuilding from pieces. This
  removes the browser's `export_definition` failure mode, where a field it did not know
  about (`aim_bones`, until someone remembered) was dropped on re-export.
- **One rebuild flag per panel**, never shared. See *Known Gotchas* in `CLAUDE.md`; the
  model list and weapon list emptying each other is the latest example.

### Input focus

Live needs WASD, mouse aim and fire. Editing needs typing (tag names), arrow keys (the
library) and the mouse (buttons). The browser and playground each solved half of this.

- **Pointer over the left pane** means the keyboard goes to the editor and game input is
  suppressed. Mouse clicks on the pane never fire the gun.
- **A text field open** (tag edit, search box) captures every key until Enter or Escape,
  as `tag_edit_clip` does today.
- **Studio** means the game never gets input. The keys go to the orbit camera and editor
  shortcuts.
- **Live with the pointer over the viewport** means the game gets input, as now.
- **A gamepad always drives the character in Live**, wherever the pointer is. It has no
  way to click the panel, so it never conflicts, and you can hold the pad in one hand while
  nudging a hardpoint with the mouse in the other. Pads are bound the same way as in a match
  (`assign_gamepads`, `GamepadBindings`), including the fallback that hands a connected pad
  to the lone keyboard player. In Studio the pad is ignored, like the keyboard.

This needs one gate (an `EditorInputFocus` resource) read by `keyboard_input`, `mouse_aim`
and the fire path in `in_editor` sessions, instead of each screen guessing.

### Live-capable types

| `ModelType` | Studio | Live |
|---|---|---|
| `Player` | yes | walk as it (today's playground) |
| `Enemy` | yes (rig, clips, sizing) | no, enemies are out of scope for now |
| `Weapon` | yes, alone on the turntable | held by the last-worn character |
| `Tower` | yes | later: placed in the arena |
| `Terrain`, `Item` | yes | later: placed in the arena (barrels explode, pickups pick up) |

Player and Weapon Live come almost for free from the playground, and they are the scope.
Everything marked *later* is optional, after parity.

## Layout

```
+-----------------------------------------------------------------------------------+
| [Library] amy.ron *            Player   [Studio|Live]   [Save] [Revert]   [Menu]  |  <- top bar
+---------------------------+-------------------------------------------------------+
| Model  Anim  Hardpts  Attach  Props  View  |  (viewport toolbar, overlaid)        |
|--------------------------------------------| [Orbit][Follow] [Skel][HP][Phys]     |
|                                            | [Bind pose] [Frame HP] [Frame all]   |
|   active tab's contents,                   |                                       |
|   scrolls on its own                       |           the game camera,            |
|                                            |           clipped to this pane        |
|                                            |                                       |
|                                            |                                       |
+--------------------------------------------+---------------------------------------+
| amy.ron (unsaved)  grip <- mixamorigRightHand   [F11] orbit  [F12] bind pose  ...  |  <- status bar
+-----------------------------------------------------------------------------------+
```

- **Top bar:** the open doc's name, a `*` when dirty, its type, the Studio/Live switch,
  Save/Revert, and Back to Menu. The name opens the **Library**.
- **Library:** an overlay or drawer over the left pane, not a permanent section. It is
  where you go to pick something, not where you work. It merges the browser's folder
  browser with the playground's model list and import browser:
  - a search box, plus type filter chips (`Player Enemy Weapon Tower Terrain Item New`);
  - every `.glb` / `.gltf` under `assets/`, each marked as imported (with its type), not
    imported, or anims-only;
  - arrow keys and Enter, as in the browser, as well as the mouse;
  - opening an un-imported file previews it in Studio with `def_path: None`, and the top
    bar offers **Import as <type>**. Nothing is written until then. This keeps the
    browser's "look before you import" and the playground's "never clobber an existing
    def";
  - anims-only files offer **Add as animation source to <open doc>** instead (today's
    `add_animation_source`).
- **Tabs:** one tab is visible at a time, and each scrolls on its own. This replaces the
  collapsible single column, which is what made things get lost. The last tab is
  remembered in prefs.
- **Viewport toolbar:** the debug toggles and camera buttons move here from the left pane,
  so they sit next to the thing they affect.
- **Status bar:** what is selected (role, bone, clip), the save state, and context-
  sensitive key hints. These replace the hint labels scattered through today's panels.

All UI text stays ASCII (the embedded font has nothing else).

### Tabs

**Model**
- Model path, with a re-point option for when a `.glb` moves.
- Height: the target height in metres, and the measured AABB height. The scale is computed
  from those (browser: `compute_model_height`, `target_height_m`, `+`/`-`).
- Hidden nodes: a toggle list of mesh nodes (browser: `rebuild_node_list`).
- Mesh/clip/bone counts from `gltf_info`, with a warning if it has no meshes.

**Animation** (as built; the clip tags this section first planned were dropped
2026-10-19 for a plain key -> clip map plus Mesh2Motion rig presets)
- Keys: each `AnimationKey`, the clip it resolves to and why (def / rig preset / by name /
  borrowed from a fallback key). A def entry naming a clip the model lacks is flagged.
- Picking a key previews it on the character (`PreviewAnimation`, which plays the clip
  outright; going through the `CharacterState` stack only ever played the handful of keys
  that change what the stack resolves to, and Death stuck). "stop preview" returns to the
  stack.
- Under the picked key: every clip from the model and its sources; click one to map the
  key to it, "use default" to drop the entry and go back to the preset.
- Sources: add (from the Library); their clips appear as `<stem>|<clip>`.

**Hardpoints**
- A side switch: **character** / **held weapon** (today's playground `HardpointSide`).
  When the open doc *is* a weapon, there is only one side.
- Role chips, add/remove, anchor-bone picker (from the live rig), and nudge rows for
  translation and rotation with coarse/fine steps. Rotation stays wrapped to (-180, 180].
- Suggestions (`suggest_hardpoints.rs`): "suggest the missing frames" fills every role a
  side lacks, "suggest this frame" redoes the selected one. A character's come from its
  bind pose using Mesh2Motion's prop rule (rest pose faces +Z, palms down; the gun runs
  straight out from the hand, top toward the thumb, 9 cm along the hand and 3.5 cm below the
  palm, scaled to the rig's height): grip on the right hand, foregrip on the left, stock in
  the right shoulder pocket (on the clavicle), sight at the right eye. Rigs with no hand bone
  hold on the forearm at the knuckles. A weapon's come from its mesh profile, assuming the
  barrel runs +X with +Y up: muzzle at the front on the barrel's line, grip on the first part
  hanging below the barrel (two-handed: skipping the stock butt and shallow receiver
  undersides), foregrip under the barrel ahead of it, stock at the back, sight on top above
  the grip.
- Weapon picker: equip any weapon def. In Studio this is the browser's reference-weapon
  preview; in Live it is the real equip. It is one control backed by
  `hardpoint::snap_transform` in both cases.
- `F` frames the selected hardpoint (orbit camera).
- The muzzle tracer check: hold fire in Live and watch the tracer leave the barrel.

**Attach** (browser only today)
- Bone sockets for fixed props: pick the bone (`,` / `.`), pick the model, and nudge the
  offset.
- Shown for every type, but mostly useful for characters and enemies.

**Props**
- The model type picker, and the type-specific props editor (browser:
  `rebuild_type_picker`). Changing the type of an imported def asks first.
- Player: ability, throw rate, weapon, extra weapons, starting ammo.
- Aim bones: edit the torso-twist chain (bone and weight). **New.** Neither tool edits it
  today; the browser only carries it through.

**View**
- The camera, model and gait panels the playground spawns today (F1 / F2 / F10), embedded
  here instead of floating.
- Overlay toggles: physics, skeleton, hardpoints, gait. These mirror the viewport toolbar
  and share one source of truth (`PlaygroundDebug`, renamed).

## Parity checklist

Every feature, where it comes from, and where it lands. Nothing gets deleted until every
row is ticked.

| Feature | From | Lands in |
|---|---|---|
| Browse folders under `assets/` | Browser, Playground | Library |
| Keyboard nav: arrows, PgUp/PgDn, Enter, Backspace | Browser | Library |
| Preview without importing | Browser | Library -> Studio with `def_path: None` |
| Import (write a minimal def, never clobber) | Playground | Library / top bar |
| Mesh-less detection, add as source | Playground | Library, Animation tab |
| Orbit + zoom viewer | Browser (`orbit_viewer`), Playground (`OrbitCamera`) | Studio, using `OrbitCamera` (the better one) |
| Height / scale from AABB | Browser | Model tab |
| Hidden nodes | Browser | Model tab |
| Model type + props | Browser | Props tab |
| Clip tags (text edit) | Browser | Animation tab |
| Cycle clips `[` `]` | Browser | Animation tab, Studio |
| Bindings (edit) | Browser, Playground | Animation tab |
| Play a key through the state machine | Playground | Animation tab, Live |
| Animation sources | Browser, Playground | Animation tab |
| Skeleton overlay (`B`) | Browser, Playground | View tab / toolbar |
| Hardpoint gizmos (`H`) | Browser, Playground | View tab / toolbar |
| Physics debug (F3), gait gizmos | Playground | View tab / toolbar |
| Character hardpoints, nudge, anchor | Browser, Playground | Hardpoints tab |
| Weapon-side hardpoints, muzzle | Playground | Hardpoints tab |
| Reference weapon preview | Browser | Hardpoints tab (weapon picker, Studio) |
| Equip weapon | Playground | Hardpoints tab (weapon picker, Live) |
| Attachments (bone sockets) | Browser | Attach tab |
| Bind pose hold (F12) | Playground | toolbar, Studio |
| Frame hardpoint / character | Playground | toolbar, `F` / `Home` |
| Camera / model / gait sliders | Playground (HUD panels) | View tab |
| Target dummies | Playground | **Dropped**, comes back as opt-in practice targets (step 8) |
| Keyboard + gamepad control in the arena | Playground | Live |
| Remember last model | Playground (`playground-prefs.ron`) | `editor-prefs.ron`, plus the last tab |
| Export def (`I`) | Browser | Save (`Ctrl+S`) |
| Foldable sections | Both | Replaced by tabs |
| Aim bones | neither | Props tab (new) |

## Plan

Each step leaves the game building and the editor usable. The Asset Browser and the
Playground keep working until step 9, so nothing is lost mid-migration.

### 1. Rename and re-home

`git mv src/playground src/character_editor`. Rename the types: `PlaygroundSession` ->
`EditorSession`, `PlaygroundModels` -> `EditorLibrary`, `PlaygroundDebug` ->
`EditorOverlays`. Add a menu button **Character Editor** and an `--editor` flag, keeping
`--playground` as an alias. The old Playground menu entry points here too. Stop spawning
the target dummies (`spawn_dummy_posts`, `respawn_dummies`), leaving an empty arena. Move
`gltf_info` to `src/assets/`, since the Library and the browser both need it.

*Done when:* the same screen as today's playground opens as "Character Editor".

### 2. `EditorDoc`

Introduce `EditorDoc` and the held-weapon doc. Derive `PartialEq` on `AssetDefinition`
and its parts. Re-point the existing panels (hardpoints, animation, weapon list) at the
doc. Add the sync system from doc to `PlayerAssetDef`. Replace the per-side Save buttons
with the top bar's Save/Revert and dirty marker. Add the unsaved-changes prompt on model
switch and exit.

*Done when:* every playground edit goes through the doc, `*` appears and disappears
correctly, and Revert restores the file's contents on the live model.

### 3. Shell: top bar, tabs, toolbar, status bar

Build the new layout with today's content sorted into tabs (Hardpoints, Animation, View).
Move the debug toggles to the viewport toolbar. This is the moment to fix scrolling for
good: each tab is one scroll container with `flex_shrink: 0` children, so there are no
nested scroll lists inside a scrolling pane.

*Done when:* nothing lives in one long column any more and every section is reachable
without scrolling past others.

### 4. Input focus

`EditorInputFocus` and the gates in `keyboard_input`, `mouse_aim` and firing. The text
field capture, ported from the browser's tag editor as a small reusable widget, since the
Library search needs one too.

*Done when:* clicking a panel button never shoots, and typing a tag name never walks the
character.

### 5. Studio / Live

Generalise inspect mode into the Studio stage: orbit camera, hold still, optional bind
pose. Spawn non-player models as a static prop in Studio. Make Live the
default for players and Studio for everything else.

**Weapons and enemies are first-class docs, not just things reachable through a
character.** Opening a Weapon def makes *it* the `EditorDoc`: it stands alone on the
turntable in Studio, and its hardpoints (grip, muzzle, ...) are edited directly. Live with
a weapon open hands it to the last-worn character, so the same edits can be checked in a
hand. Opening an Enemy def puts the enemy in Studio, where its sizing, clip tags and
bindings are edited like a player's. The model list in the Library therefore lists every
def type, not only players.

The switch needs its own key. The spec's original `Tab` is taken (weapon switching), and
F1 to F12 are all in use, so it lives on the top bar as a button.

*Done when:* the top-bar switch flips between the two instantly on a player, a weapon def
opens standing alone with its hardpoints editable, and an enemy def opens in Studio.

### 6. Library

The merged browser: search, type chips, import state per file, keyboard and mouse,
preview-before-import, Import as <type>, and add-as-source for anim libraries. It replaces
the playground's MODEL and IMPORT sections and the browser's folder and file lists.

*Done when:* any `.glb` under `assets/` can be previewed, imported as any type and opened,
without leaving the editor.

### 7. Port the browser-only panels

In order of how often they are needed:
1. **Model tab:** height/scale from AABB, hidden nodes.
2. **Animation tab:** clip tags with text edit, raw clip playback, merged with the
   existing bindings list.
3. **Props tab:** the type picker and props editor.
4. **Hardpoints:** reference-weapon preview in Studio, merged into the weapon picker.
5. **Attach tab:** bone sockets.

Each one is ported to edit `EditorDoc`, not copied with its own state.

*Done when:* every row of the parity checklist is ticked.

### 8. New

Aim-bone editor (Props tab). Last tab and Studio/Live mode remembered in
`editor-prefs.ron`. Optional after that: a **Practice targets** toggle in the View tab
that brings back the playground's respawning dummies (the code is in git history), and
Live for towers and terrain.

### 9. Retire the old screens

Remove the Asset Browser and Playground menu entries, `GameState::AssetBrowser`,
`src/asset_browser/` and anything left in the old playground module. Update `CLAUDE.md`
(module map, the AssetDefinition section's "authored in the asset browser" mentions) and
`docs/hardpoints.md`. Archive `docs/playground.md` to `docs/archive/`.

*Done when:* `grep -ri "asset.browser\|playground" src` is empty, apart from the
`--playground` alias.

## Proposed order of work

1. **Steps 1 and 2 first, together.** The rename is cheap, and `EditorDoc` is the change
   that actually prevents "edited in one, saved from the other". Everything after it is
   built on the doc, so building any panel before it means building that panel twice.
2. **Step 4 (input focus) before step 3 (shell).** The shell adds more clickable surface
   over a live game, and every new button is another chance to fire the gun or walk off
   while clicking. It is small and self-contained.
3. **Step 3, the shell.** It is the visible payoff, and it gives the ported panels a place
   to land, rather than being ported into the old column and moved again.
4. **Step 5, Studio/Live.** Most of it exists as inspect mode. It needs to come before the
   Library, because previewing an un-imported or non-player file has to happen in Studio.
5. **Step 6, the Library.** From here on the Asset Browser is only needed for its panels.
6. **Step 7, porting, in the listed order.** Height and clip tags come first because every
   newly imported model needs them before it is usable. After 7.2 a new character can go
   from `.glb` to walking around without opening the old browser.
7. **Step 9, retire,** as soon as the parity checklist is complete. Do not let the two old
   screens linger "just in case"; that is how we got here.
8. **Step 8, new features,** after the retirement. None of them block it, and doing them
   last keeps the migration from growing.

Rough size: steps 1, 2, 4 and 9 are small to medium. Steps 3, 5 and 6 are medium. Step 7
is the bulk, since `asset_browser/ui.rs` alone is about 2,000 lines, though much of that is
re-plumbing state that `EditorDoc` makes unnecessary.

## Open questions

- **Auto-frame on open?** Studio always uses the arena. Some models (tiny items, huge
  terrain) may want the camera to frame them on open, which `Home` already does by hand.

Decided: the arena stays for every model, Live supports keyboard and gamepad, enemies
are out of scope (practice targets are an opt-in later), and the left pane uses **tabs**,
not folding sections.

## Progress

### Steps 1 and 2 — built (2026-10-14)

**Step 1.** `src/playground/` is now `src/character_editor/`, with `EditorSession`,
`in_editor`, `EditorLibrary`, `EditorOverlays`, `EditorPrefs` and
`CharacterEditorPlugin`. The menu button reads **Character Editor**; `--editor` boots
straight in and `--playground` still works. The target dummies are gone, so the arena is
empty. `gltf_info` moved to `src/assets/`. Prefs are written to `editor-prefs.ron`, and
an old `playground-prefs.ron` is still read if the new file does not exist yet.

**Step 2.** `src/character_editor/doc.rs`: `DefDoc` (`def_path`, `def`, `saved`) and two
resources, `EditorDoc` for the character and `WeaponDoc` for the gun it holds.
`AssetDefinition` derives `PartialEq` so dirty can be computed. What changed for the
panels:

- The hardpoint, animation, weapon and import panels edit the docs. None of them writes
  `PlayerAssetDef` or saves any more, and the per-panel Save buttons and dirty flags are
  gone. Adding an animation source no longer saves straight away; it is an edit like any
  other.
- A **doc bar** sits under the title (it is not in the scroll path of anything, so the
  question below can't be scrolled away): the character and weapon names with `*
  (unsaved)`, then **Save** (every dirty doc), **Revert** and **Menu**. Back to Menu moved
  up here from the bottom of the pane.
- **Asking first:** opening another model or leaving checks both docs. Equipping a
  different gun only checks the weapon doc, because the character doc just gains an
  edit. Either way the bar asks *Save, then continue* / *Discard* / *Cancel*.
- **Equipping is an edit, not a save.** It writes `PlayerProps::weapon` into the doc.
  `sync_doc_to_live` sees that the live character carries something else and respawns
  it from the doc.

The live character follows the doc through `sync_doc_to_live`. Fields the game reads
while running are copied into `PlayerAssetDef`. Fields it only reads at spawn
(`needs_respawn`) trigger a respawn, and the doc rides along in the new
`PlayerRoster::def_overrides`, so `spawn_players` spawns the unsaved working copy rather
than the file. The respawn uses the existing swap, which now tracks `awaiting_spawn`.
Until the new character exists, `PlayerAssetDef` still holds the old def, and comparing
against it would ask for another respawn.

Equipping resolves the gun's frames from the files on disk (`PendingEquip::resolve`), so
`apply_grip_to_equipped_weapon` and `apply_weapon_def_to_equipped_weapon` now also run
on `Added<EquippedWeapon>`. Without that, a respawn would show the saved grip and muzzle
instead of the ones being edited.

The doc systems run as one chained pipeline (swap -> adopt -> weapon doc -> live ->
weapon frames), so each sees the previous step's result in the same frame.

Verified in-window by hand: model switching, saving, the unsaved-changes question and
equipping all work. One bug turned up and is fixed: a role the def did not have yet could
not be added, because the nudge rows only appear for a hardpoint that exists. Missing
roles now read `+ add <role>`; clicking one creates it, anchored on a guess at the right
hand (`guess_hand_bone`) when the character has no hardpoints to inherit an anchor from.

### Steps 4 and 3 — built (2026-10-14)

**Step 4: input focus.** `control::components::GameInputBlock` is a resource the
keyboard and mouse input read. It exists only while the editor runs, so a real match
never sees it. `character_editor::focus` sets it each frame from the hover map:

- **Pointer over a panel** (anything under a screen root other than the viewport pane,
  including the floating F1/F2/F10 panels and the viewport toolbar): the aim holds still,
  so clicking a button no longer swings the character and its gun at it.
- **Inspecting** (orbit camera or bind pose): the keyboard is blocked as well.
- **A text field has focus** (`EditorInputFocus::text_capture`): the keyboard is blocked.
  Nothing sets this yet; the Library search and the tag editor will.

Two departures from the spec, on purpose:

- **WASD is not blocked over the panel.** The panels have no keyboard shortcuts, and
  losing movement because the cursor happens to rest on the pane would be a surprise.
  Revisit if a tab gains shortcuts.
- **There is no reusable text-field widget yet.** It would have nothing to use it. It
  comes with its first user.

A blocked keyboard still lets key *releases* through (`key_event_passes`), so a key held
when the block starts is not stuck down when it lifts. The mouse never fired anything, so
there was no fire path to gate. Gamepads are never blocked.

**Step 3: the shell** (`character_editor::shell`). The layout is now:

- **Top bar:** the title and the doc bar (names, `*`, Save / Revert / Menu, and the
  unsaved-changes question).
- **Left pane:** a tab strip and one scrolling body per tab:
  - **Library:** the model list and the import browser. Interim, until step 6 turns it
    into a drawer.
  - **Hardpoints:** the weapon picker, then the hardpoint editor.
  - **Animation:** the key list.
- **Viewport:** the debug toggles moved onto a toolbar floating over the game.
- **Status bar:** the open doc, the selected hardpoint and its anchor bone, an
  INSPECTING marker, and the key hints that used to be scattered through the pane.

The folding sections and the per-list height caps are gone from the editor. Each tab is
one scroll container (`min_height: 0`, `overflow: scroll_y`) holding plain columns, so
nothing scrolls inside something else that scrolls.

The tab strip is local to the editor rather than a `lava_ui_builder` widget, for now.
The Model, Attach and Props tabs arrive with their panels in step 7.

**Not verified in-window yet.**

### Step 5 — built (2026-10-14)

**Studio / Live.** Studio is the orbit camera with the character held still. It was
already the playground's inspect mode, so the switch is a pair of buttons on the top bar
(and F11 still works); there is no separate stage state to drift from it. Bind pose (F12)
stays its own toggle.

**Every def type opens.** The Library lists every def in `assets/defs/`, grouped
(CHARACTERS, WEAPONS, ENEMIES, TOWERS, TERRAIN, ITEMS). `*` marks what is open; `-` marks
the character worn and the weapon held when something else is the subject.
`EditorFocus::subject` says which kind is open:

- **A character** opens as before: worn and walked around as.
- **A weapon** opens into the worn character's hand *without editing the character*.
  `EditorFocus::held_weapon` overrides the character's `PlayerProps::weapon` in the live
  def only (`doc::expected_live`), so fixing a gun's muzzle does not leave the character
  with unsaved changes. The Hardpoints tab opens on the weapon side. Equipping from the
  weapon list makes a gun the character's own again and clears the borrowed one.
- **Anything else** (enemy, tower, terrain, item) opens as a **prop**
  (`character_editor::prop`): a bare scene spawned where the character's feet were, with
  the character hidden and held still. Studio is forced on, because there is nothing to
  drive (`Subject::has_live`, `keep_studio_for_props`). The orbit camera frames the prop
  by its mesh bounds (`prop::mesh_bounds`), so a skeleton-less barrel frames as well as a
  rigged alien. The Hardpoints tab edits the prop's own frames (`HardpointSide::Prop`,
  every role including `muzzle`, bones from the prop's rig). The skeleton and hardpoint
  overlays draw the prop instead of the character.

A third doc, `PropDoc`, holds the prop. One Save writes all three; opening another
model or leaving asks first if any of them has edits. The doc actions moved onto one
`Docs` system param (`request`, `perform`, `save_all`, `discard_for`, `revert_all`), and
`SideDocs` reaches the doc for a hardpoint side without touching the others.

Departures from the spec, on purpose:

- **A weapon opens in the hand, not alone on the turntable.** Grip placement only makes
  sense against a hand, and the muzzle check needs the gun to fire. A weapon alone, for
  when no character is worn, can come later as a prop.
- **A prop is static.** No animation graph: the Animation tab says so for a prop. Its clip
  tags and bindings come with the Animation tab port in step 7. Hidden nodes and scale
  editing are step 7's Model tab. The prop respawns on a scale change, ready for it.

**Not verified in-window yet.**

### Step 6 — built (2026-10-14)

The Library tab is now one list (`character_editor::library`), built by the pure
`library_rows` from the imported defs and the files on disk:

- **A search box** (`text_field.rs`, the first text field; it sets
  `EditorInputFocus::text_capture` while it has focus, so typing never walks the
  character). An empty query browses the current folder. A query matches def names and
  every model file under `assets/` (indexed once on entry, `scan_all_files`), capped at
  `SEARCH_LIMIT`. Matching ignores case.
- **Type chips** (All, Character, Weapon, Enemy, Tower, Terrain, Item) filter the
  imported defs.
- **Every file says what it is** (`FileState`): imported (and as what), `+ new`, or
  `~ anims`. The geometry check reads the file, so the answers are cached per path.
- **Preview before import.** Clicking a new file shows it in Studio as a prop with no
  file behind it (`DefDoc::preview`, `ConfirmAction::Preview`). A preview is never
  dirty, so leaving it never asks. While it shows, the bar under the search box offers
  **import as** one of the six kinds (`Docs::import_as`). Importing writes the def,
  refuses to overwrite an existing one (it opens it instead), carries over anything set
  on the preview, and opens the new def as what it now is.
- **Animation libraries** add themselves to the open character's `animation_sources`, as
  in the playground.
- **Keyboard:** with the search box focused, Up / Down move a `>` highlight over the rows
  (headings skipped, wrapping), Enter opens it (or the first match), and Escape or a
  click elsewhere gives the keyboard back.

The old MODEL list and IMPORT browser are gone, and with them `EditorLibrary::import`
and `new_player_def`.

Departure from the spec: **the Library stays a tab**, not a drawer over the pane. It
works as a tab, and a drawer can come with the polish pass if it is missed.

Smoke-tested: the editor ran for 40 s with no panic or error in the log. **The new UI
itself has not been clicked through.**

### Step 7 — mostly built (2026-10-14)

Three new tabs; the strip is now Library, Model, Animation, Hardpoints, Props. Every tab
edits whatever is open (`doc::SideDocs::subject_mut`), so the same tab works on a
character, a weapon or an enemy.

**Model** (`model_tab.rs`):
- What the file holds: meshes, bones and clips. `gltf_info` now also reports mesh node
  names and the joint count.
- The scale, with factor buttons.
- The height measured off the model on screen (`measure_subject`, mesh bounds rounded
  to a millimetre), with **set height** buttons that work out the scale from it. This is
  the asset browser's target height, against the model in the arena.
- Hidden nodes as toggles: a prop updates in place (`apply_prop_hidden_nodes`), a
  character respawns.

**Animation** (`animation_tab.rs`):
- The key bindings as before, now on any subject; keys only play on a character in the
  arena.
- A **CLIPS** list underneath: every clip of the model and its sources (`all_clips`),
  each with its tag. Pick one, type a path into the field and press Enter (`set_clip_tag`
  tidies the path and moves it off any clip that had it), or reuse an existing path.
- Legacy `animation_mapping` defs are migrated as they open (`migrate_legacy_mapping`).
  That shows as an unsaved edit until saved.

**Props** (`props_tab.rs`):
- The type, which asks before resetting the stats.
- Every plain stat of the type, editable: numbers with coarse/fine nudges (never below
  zero; whole numbers round), switches, and pick-one choices. Switching an enemy to
  Ranged, or a tower to Slow, starts the new variant at usable values.
- Nested data (resistances, explosions, projectiles, ammo lists) is shown and left to the
  `.ron`.
- The asset browser could only display these.
- A changed type takes effect once saved and reopened. A character doc whose type
  changed stops being worn rather than respawn-looping (guard in `sync_doc_to_live`).

Things the code revealed:

- **A character's def scale is never read in game.** `spawn_players` sizes the player
  from `ModelSettings` (the F2 panel). Enemies, towers, terrain, items and weapons do use
  theirs. So the Model tab offers scale and set-height for everything but a character,
  and says why for a character. Making players honour their def scale is a separate
  decision.
- **Weapon hidden nodes and scale show on the next equip**, not live. The tab says so.

Not ported, on purpose:

- **Raw clip playback** (the browser's `[` `]`). A clip only plays through the
  character's animation graph, which holds bound clips only. Binding a clip to a key and
  clicking the key does the same job.
- **Attachments (bone sockets).** Nothing in the game reads `attachments`; it was
  superseded by hardpoints. Port it if it is still wanted, otherwise drop the field.
- **Reference-weapon preview** is covered by step 5: open a weapon and it is in the hand.

Smoke-tested: 40 s with no panic or error. **The new tabs have not been clicked through.**

### Step 8 — built, ahead of step 9 (2026-10-14)

Done before retiring the old screens, because it is additive and risk-free, and step 9
deletes code you may still want to compare against.

**Torso twist chain** (`aim_bones.rs`, a TORSO TWIST section at the bottom of the Props
tab for a character):

- With no chain of its own, the section says whether the rig actually has the mixamo
  spine the default needs. `male-simple` does not (`spine_01`..`spine_03`), which is why
  its torso never twisted.
- **Suggest from this rig** (`suggest_chain`) takes its spine bones in order, at most
  four, with weights rising up the back like the default chain.
- Bones can be added from the rig's torso bones (spine, chest, neck...) or removed,
  weights nudged, or the chain cleared back to the default.
- Edits respawn the character, which is where the chain is resolved.

**Prefs:** `editor-prefs.ron` now also remembers the last tab and Studio/Live. Studio
is only remembered while a character or weapon is open, so leaving on an enemy does not
start the next session in Studio. Every write goes through `EditorPrefs::update`, which
keeps the other fields; the old `remember` overwrote them.

Not done: the opt-in **practice targets**. It is optional, and the dummy code is in git
history when it is wanted.

Smoke-tested: 40 s with no panic or error.

