# Wall Collider Positioning

## Coordinate System

`TileDefinitions::new(1.0, 32.0, 9.5, 1.0)` produces:
- `tile_unit = 1.0 / 32.0 = 0.03125`
- `tile_width = 1.0` (one tile = one world unit)
- `wall_height = 0.296875`
- `floor_level = -0.59375` (the Y of the floor collider center and visual floor mesh)

Tile col/row N is centered at world x/z = `tile_width * N`. The visual floor mesh places tile N from `N - 0.5` to `N + 0.5`, so the left/north edge of the map is at `x/z = -0.5`.

The player spawns at `y = 1.0`, falls under gravity, and comes to rest with feet at approximately `y = floor_level + floor_half_height ≈ -0.531` and center at `y ≈ -0.031`.

## Wall Collider Setup (map_systems.rs)

Impassable tile colliders are greedy-rectangle-merged per category (ImpassableAll, ImpassablePlayer, ImpassableAlien) and spawned as static cuboids:

```rust
let block_half_h = 4.0_f32;
let block_y = tile_defs.floor_level;  // = -0.59375

let center = Vec3::new(
    tw * (col + max_col) as f32 / 2.0,
    block_y,
    tw * (row + max_row) as f32 / 2.0,
);

commands.spawn((
    RigidBody::Static,
    Collider::cuboid(tw * w / 2.0, block_half_h, tw * h / 2.0),
    *layers,
    Transform::from_translation(center),
    Position::from(center),
));
```

**Both `Transform` and `Position` must be set explicitly.** Avian3D's required components insert a default `Transform::identity()` when only `Position` is provided, but `GlobalTransform` is only propagated from `Transform` — not from `Position` — so the debug renderer and Avian's own transform propagation both need the explicit `Transform`.

## Why block_y = floor_level

The wall center is placed at `floor_level` (`y ≈ -0.594`) rather than above it. This matters for two reasons:

1. **Debug renderer alignment.** Avian3D's debug gizmos derive position from `GlobalTransform`. In the isometric camera (−40° pitch), an object at `y = 0.906` projects roughly one tile *behind/above* the floor tile at the same XZ — the collider boxes appear to float in the air. Centering at `floor_level` makes the gizmo outlines visually coincide with the floor tiles.

2. **Full Y coverage.** With `block_half_h = 4.0`, walls span `y = [-4.594, 3.406]`. The player at rest spans `y ≈ [-0.531, 0.469]`, which is fully inside this range. The large half-height also means the walls cover the player at any height during spawning/falling.

**Previous (wrong) formula:** `block_y = floor_level + block_half_h` placed the center at `y = 0.906`. The walls were geometrically correct and did block the player, but looked elevated in the debug view.

## Avian3D Static Body Positioning

Avian 0.6 (`PhysicsTransformConfig::propagate_before_physics = true` by default) runs its own transform propagation step — `mark_dirty_trees`, `propagate_parent_transforms`, `sync_simple_transforms` — at the start of every physics step (in `PrepareSet::Propagate`, before `TransformToPosition`). This means:

- You do **not** need to rely on Bevy's PostUpdate `TransformPropagate` to be up-to-date before physics runs. Avian does it itself.
- Setting `Transform::from_translation(center)` is sufficient to get `Position` correctly set via Avian's `transform_to_position` system.
- Setting both `Transform` and `Position` to the same value is fine and is slightly safer (Position is correct from frame 0 without waiting for the first physics tick).

The debug renderer (`debug_render/mod.rs` line 297: `let position = Position::from(transform)`) reads `GlobalTransform`, not `Position`. So `GlobalTransform` must be correct for the debug to show walls in the right place.

## Border Barriers

Invisible ImpassableAll barriers are spawned around the entire map perimeter to prevent players and aliens from leaving:

```rust
let border = tw * 3.0;   // 3-tile thick
let half_b = border / 2.0;  // = 1.5

// Example: east border
Vec3::new(map_w - tw / 2.0 + half_b, block_y, cz)
// = (20 - 0.5 + 1.5, block_y, 9.5) = (21.0, block_y, 9.5)
// Inner edge at x = 21.0 - 1.5 = 19.5 = east edge of tile col=19 ✓
```

The *inner* edge of each barrier is flush with the corresponding floor tile edge. The centers appear 1.5 tiles outside the visual map, which looks offset in the debug, but the physics is correct.

## Collision Layers

| Entity | Group | Mask |
|--------|-------|------|
| ImpassableAll walls | `ImpassableAll` | `Ball, Alien, Player` |
| ImpassablePlayer walls (192 tiles) | `ImpassablePlayer` | `Player` |
| ImpassableAlien walls (260/261 tiles) | `ImpassableAlien` | `Alien` |
| Player | `Player` | `Ball, ImpassableAll, ImpassablePlayer, Floor, Alien, Player, AlienSpawnPoint, AlienGoal` |

For collision to occur, both entities must be in each other's masks. The player correctly includes `ImpassablePlayer` in its mask.

## Player Physics

The player uses `RigidBody::Dynamic` with `LinearDamping(5.0)` and `AngularDamping(1.0)`. The movement system (`dynamic_movement_system.rs`) directly sets `LinearVelocity.x` and `.z` every Update frame, bypassing physics velocity responses. Avian's XPBD resolves wall penetration positionally (not via velocity), so the velocity override does not cause passability issues — the player is correctly stopped by walls. The `LinearDamping(5.0)` limits approach speed and prevents accumulated Y-velocity from destabilizing wall contacts.
