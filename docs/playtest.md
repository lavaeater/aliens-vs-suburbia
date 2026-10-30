# Playtest

## Purpose

This is to be considered a sort of spike. Take the game in its current state and take it to a play-testable state. To do that we simply have to decide what needs to be in a play-testable game - but also what the playtest should be. I think of the playtest as an event. It could be inviting friends or family to play the game in some form or another, together, on a big screen.

## The Playtest

The game supports four players, so up to four players, a big screen and some snacks.

## The Game

### Crashes

Weird crashes need to be found. How do we do that? We make a stupid player-ai for controlling the player and walking hither and dither and basically playing the game to find crashes?

### Levels

We need 5+ levels, showcasing our ability to write "stories" integrated with turbofacts to control the flow of the game, moving from one level to another in the game and of course having a splash screen at the end with some kind of win message. There should be some challenge to actually clearing a level and also fun things to do. One level can very well be an actual tower defense level with snowball tossing as the only weapon as in South Park Tower Defense

#### Status (2026-10-24)

The campaign is in (`assets/campaigns/main/`, `src/campaign/`): five levels, a card between
levels (next level / retry after a loss), and a victory screen after level five. Start
Game plays it from level 1; `cargo run -- --play --level N` jumps straight to level N, and
**End** wins the current level on the spot (to skip ahead during a playtest).

| # | Level | Map | What is special |
|---|-------|-----|-----------------|
| 1 | Maple Drive | `level_01.map` | the intro; the strange letter (a story item); aliens then bunnies |
| 2 | The Community Centre | `level_02.map` | three bungalows (houses from the house editor); find fuel, spare parts and fuses (story objectives on the HUD) before clearing the street counts |
| 3 | Snow Day | `level_03.map` | snowballs only (`weapons:`), 250 coins for towers, a zig-zag path, only 8 may escape |
| 4 | Night of the Golems | `level_04.map` | two spawns, stone golems, barrels to blow up, loot bags; radio chatter at wave 2 |
| 5 | The Mothership | `level_05.map` | the boss wave (Alpaking Evolved) with its own announcement; only 5 may escape |

Each level is a folder, `assets/campaigns/main/level_0N/`, with its map, its stories
(`level_0N_story.ron`) and crawls (`level_0N_crawl_*.ron`): an intro, mid-level chatter driven by `CurrentWave` or items found, and
an outro. A level's rules are map properties: `escape_limit`, `weapons`, `coins`
(`docs/map-text-format.md`).

Untested by hand so far: balance, the snowball's look and feel (a small rock model, white
projectile), the story item sizes, and the flow through all five levels in one sitting.

### Playable Characters

We have several playable mesh2motion characters downloaded and ready to go. We need to move and modify the hardpoint for them - for a quick fix we could simply copy the ones we have in male-hazmat to the others and work from there, as it is already tweaked to "perfection".

### Voice Acting

Record dialog, stings, readings of the crawls etc. 

### SFX

Monster SFX

### Monsters

We need to add more and varied monsters. I'd love to have humanoid models but twist and get freaky with them. I'd love to have an enemy or monster that has tentacles.

