# Infestation levels

A level mode next to the classic one (aliens walk to a goal, too many escaping loses).
On an infestation level there is no goal: spawners open up around a suburb, monsters
come out of them and go for the neighbours in their yards, and the players have to kill
the monsters and destroy the spawners. The default campaign, `assets/campaigns/neighbourhood`,
is one such level, Juniper Close, on a generated map; `cargo run -- --campaign main` plays
the older campaign.

## Pieces

| Where | What |
|-------|------|
| `map::neighbourhood` | The generator (`generator: neighbourhood`). `ghx_proc_gen` (0.9, the core crate, no Bevy dependency) lays out a grid of 10-tile cells as lots or street pieces (straight, corner, T, crossing, cul-de-sac) with two sockets, `road` and `verge`, so streets always join up; the middle cell is pinned to a crossing. Drawn as asphalt (`Rock`), paving (`Floor`) and lawn (`Grass`). 4-8 lots on a street get a house from `assets/houses` that fits a lot (the bungalow, today), long side along the street, turned so its door opens onto the street or the back, the yard on the door's side with 1-2 neighbours in it. The other lots are gardens with trees and bushes. `cargo test print_a_neighbourhood -- --ignored --nocapture` (optionally `SEED=n`) prints one. |
| `infestation/spawners.rs` | One spawner every `spawner_every` seconds (the first after 10 s), on open lawn or street at least 14 m from every player and 8 m from other spawners. Each sends `spawner_waves` waves drawn from the map's `[waves]`, 8-16 s apart, then one monster every 12-22 s until it is destroyed. 400 health, on the `Alien` layer and faction so it is shot like one, with a pulsing red light, the spawn point hum, and an explosion when it goes. |
| `infestation/hunters.rs` | The monsters. Each picks the nearest prey every ~0.8 s - a neighbour counts as 30% nearer than a player - and walks to it along the pathfinding grid (A*), straight in for the last 2.5 m; within 0.8 m it plays its attack and touch damage does the rest. A hunter sets its own velocity and facing (the goal-walking AI is taken off it) and has a player's body. |
| `infestation/neighbours.rs` | The neighbours: any character def placed on a map (`map_loader` writes `SpawnNeighbour`), here the people in `assets/defs/people` (kept out of `assets/defs` so the player setup does not offer them). They do their def's `Idle` (watering the garden, on the phone) until a monster is within 5 m, then run (`Run`) away from it and towards a player within 15 m, calling for help (`Wave`) when one is on them. `Faction::Player` on the player layer: bullets and blasts pass them, claws do not. |
| `infestation/mod.rs` | Starts on `LoadMap` from `MapFile::infestation`, takes the map's waves off the `WaveManager`, and every frame sets the facts below and the `ObjectiveText` line. It holds `LevelTracker::aliens_to_spawn` at kills + living monsters + what the spawners still have to send (one for each standing spawner's endless trickle, one while any are unopened), so the base `Aliens Cleared` story wins once the last spawner and its monsters are gone. |
| `ui/minimap.rs` | Top right in every match: the tiles one pixel each, players green, neighbours blue, monsters red, spawners dark red; turned with the camera's yaw. |

## Facts

`SpawnersTotal`, `SpawnersDestroyed`, `SpawnersLeft` (unopened + standing),
`NeighboursTotal`, `NeighboursAlive`, `NeighboursLost` (every neighbour dead; false on a
level without neighbours). Juniper Close's story (`juniper_close_story.ron`) sets
`Objective.spawners` when `SpawnersLeft` hits 0 and fails the level on `NeighboursLost`.

## Not done / to tune

- Not playtested by hand: monster and neighbour scale and facing, the spawner's look,
  speeds and health, the minimap's placement and turn. A 90 s smoke run spawned
  neighbours, two spawners and their monsters with no errors.
- Only the bungalow fits a 10-tile lot; `house_1` (16 x 7) is never used.
- The map editor does not list `assets/defs/people`, so placing a neighbour by hand means
  writing the placement (`at: x, y | def: defs/people/old-lady.ron`).
- `kinematic_movement` ignores `CharacterControl::speed` (every classic alien walks at
  1 m/s); hunters set their own velocity, so `speed` in Monster/Spider defs does apply.
