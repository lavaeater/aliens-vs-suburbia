# BitFlag Map Design

## Vision

Tiles are described by `BitFlags<MapFeatures>` — a compact u64 where each bit
encodes one property. Multiple bits can coexist on the same tile (e.g. Water +
ImpassableForPlayers). From these flags the engine generates geometry, colliders,
and spawn points — no hard-coded wall meshes.

Long-term, a saved map file is just a 2D grid of u64 values. The renderer and
physics layers read the flags and build everything dynamically:
- Any tile with `ImpassableForPlayers` gets covered by a player collider.
- Any tile with `ImpassableForEnemies` gets an enemy collider.
- Terrain type bits (Water, Grass, Floor…) drive mesh/material selection.
- Addor bits (EnemySpawn, PlayerSpawn, EnemyExit) drive entity spawning.
- Water under a rock is still water — the collider sits on top, not replacing it.

## MapFeatures Bit Groups

**Terrain type** (mutually exclusive in practice — only one makes visual sense):
`Water`, `Grass`, `Floor`, `Mud`, `Snow`, `Rock`

**Passability addors** (stack onto any terrain):
`ImpassableForPlayers`, `ImpassableForEnemies`

**Functional addors** (auto-clear conflicting passability flag):
`EnemySpawn`  → clears `ImpassableForEnemies`
`PlayerSpawn` → clears `ImpassableForPlayers`
`EnemyExit`   → clears `ImpassableForEnemies`

**Special**:
`Nothing` — all bits zero; default; means "clear / void tile"

## key_to_feature Logic

Signature:
```rust
fn key_to_feature(key: &KeyCode, current: BitFlags<MapFeatures>) -> BitFlags<MapFeatures>
```

Rules:
1. **Terrain key** (`w/g/f/m/s/r`): clear all terrain bits from `current`, set
   the new terrain bit, preserve all addor bits. Also apply the terrain's
   implied passability defaults (see table below).
2. **Addor key** (`i/e/p/x/z`): toggle that single bit onto `current`.
   Addors that imply passability also clear the conflicting passability bit.
3. **Del / Backspace / `0`**: return `BitFlags::default()` (Nothing).
4. **Unknown key**: return `current` unchanged.

### Key bindings

| Key | Action |
|-----|--------|
| `w` | Set terrain = Water (+ ImpassableForPlayers by default) |
| `g` | Set terrain = Grass |
| `f` | Set terrain = Floor |
| `m` | Set terrain = Mud |
| `s` | Set terrain = Snow |
| `r` | Set terrain = Rock (+ ImpassableForPlayers + ImpassableForEnemies) |
| `i` | Toggle ImpassableForPlayers |
| `e` | Toggle ImpassableForEnemies |
| `p` | Toggle PlayerSpawn (clears ImpassableForPlayers) |
| `z` | Toggle EnemySpawn  (clears ImpassableForEnemies) |
| `x` | Toggle EnemyExit   (clears ImpassableForEnemies) |
| Del/Backspace/`0` | Clear all → Nothing |

## Implementation Steps

1. `src/map/mod.rs` — implement `key_to_feature` fully per the table above.
   Add a `TERRAIN_BITS` const mask for clearing terrain on terrain-key presses.
2. Add `#[derive(serde::Serialize, serde::Deserialize)]` to `MapFeatures` so
   flag grids can be saved as u64 arrays in `.ron` files (future work, stub now).
3. Wire `key_to_feature` into the ratatui map editor's tile painting so the
   editor can emit BitFlag tiles instead of raw u8 values (future work).
