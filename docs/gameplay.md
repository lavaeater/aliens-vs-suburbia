# Gameplay MVP

Style is cool, but style without gameplay is not good enough. We need to nail the gameplay down so we can test gameplay with friends and family and then we can tweak the style of the game as we go.

## Needed gameplay features

### Multiplayer

### Split screen

### Weapons

We already have different weapons implemented. Do we have different ammo types?

### Items

This could be first aid kits etc.

### Ammo

We need to have different types of ammo.

### Pickups

Items to be found on map.

### Health

Tracking of health - implemented.

### Damage

Different types of weapons do different amounts of damage and we need to apply this damage to players and enemies and walls and towers etc.

### Death

What happens when players die? I think we should keep track of players lives.

#### Suggested changes

- [ ] Three lives per player
- [ ] After player death, cooldown before respawn
- [ ] Drops all items and weapons
- [ ] Player either respawns around area where he died
- [ ] Or the player can elect to spawn near some other living player by switching between living players

### Loot Drops

Have a look at the loot drop system in ~/projects/java/turbo-rocket-ultra, I think I implemented a loot drop system there, in Kotlin. Anyhoo, the idea was just an implementation of Drop Tables - i.e a list of loot and their weights, this can easily be made into human-redable resources. It is important to allow for a None Option. In Rust I think the implementation would be an Enum with a None(weight) and then Drop(weight, item) or something like that. 

### Towers

We have towers, need to evaluate them some more.

### Tower construction

Works reasonably well.

### Thrown weapons

This is thrown grenades or molotovs

### Explosions

We need explosions! What are explosions? Well, they are a thing that goes boom! at some coordinate in the game and then depending on how close or far from that coordinate stuff is, it takes different amounts of damage - and is also pushed away at different amounts (simulating a pushing shock wave).

### Stories (goals, objectives, win and loss conditions)

We have this, I think to a very satisfying level.

### Human-readable definition formats

Have for most.

### Gamepad support

Not tested.

### Game setup screen

Yes, with selection for player, as well, of course. What could be better is of course the player selection input methods.

#### Suggested changes

- [ ] For keyboard, pressing Enter enables the player
- [ ] Then left - right on keyboard moves between selectable characters
- [ ] Pressing Enter again either starts game or marks player as Ready - if all players are ready, starts game
- [ ] For gamepad, pressing X enables the player
- [ ] Then left-right on DPad or Left stick moves between players
- [ ] Pressing X again starts game or marks player as ready - if all players are ready, game starts

### At least 4 playable characters

This is on me, the human developer, to get done.

And checking just right now, Mesh2Motion has added 10 player characters and extra monsters, so we are GOOD TO GO!

### At least 5 enemies

Also at least partially on me to design and code some enemies.

### Maps

With this point is we need to sit down and actually make like five maps that we connect together into a story.

### Transitions

This is wipes, fades etc between screens, level starts etc.

### Filters or VFX

Sort of like a CRT Screen effect, or pixelation effect.

### HUD Information

The Hud should simply be that for every human player we display the character name, current health, current weapon, current ammo / total ammo for current weapon.
This should be a simple quarter of the screen at the bottom of the screen.

### On-Screen Crawls

So, this could be a "dialog" that shows up and then having a crawl that conveys information to the player, like the opening crawl in Star Wars for instance.
