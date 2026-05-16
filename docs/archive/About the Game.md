# DevLog
## Sun, 21st of Jan
So, I have actually managed to understand and add the space_editor project to my project. This will enable creating prefabs and levels for the game, hopefully!
### What is next?
This took a lot of energy, not surprisingly, but in the end we reached "something". The next step is to actually make a level and also return the "game" to a playable status. 

## Thu, 4th of Jan
I am allowed to do and implement anything I want to in the game. And what do I want to do? 

I want to have a finished game. That's what I want.

## Wednesday 27th of December
I've been thinking about "hi and low" for a minute. What is that about? Well, take controller support, that is kind of in the middle, but could perhaps mostly be considered a "low" concept, low-level, a detailed thing, whereas multiplayer could be seen as "high-level". I need to work on both to have fun. High level is also stuff like "story engine" and things like that. 

## Friday 1st of December
## 10:07 - stalling
I started working on visual pazazz... instead of the game. The game is all that counts. To make the game not look like shit, I think we could use a statemachine for the animation - so it idles when idle, you know.
I would love to use mixamo animations. Perhaps give that a whirl?
Did I load animations separately then? I did, didn't I?

## Saturday 18th of November
I have lots of items and todos - but I think I forever will love the idea of "gameplay" when I make games. I want to build AI, I want to make something that is fun. 
So the next thing is for me to make the destroy-map behavior, otherwise we cant make the gameplay we want.

Also, we need to make some towers etc.

Oh, let's make smaller floor tiles as well, mate!

## Tuesday 14th of November
I love the vision below. It gives me goals, it gives me ideas of features needed. Like for instance the idea that the aliens are entering the house, that means that we need to construct an actual house, with furniture for instance. And then perhaps our simple map thing won't do? Or do we have some other way of doing that map-thing?

Well, we *could* have a map that consists of *bit layers* and those bit layers... I mean, I have used for the FlagSet, 

Anyways, the aliens have to spawn somewhere and then move through the map to somewhere else, and we can simply use a flag to define the path the alien should take.

Along the way, it will do nothing, eh?

Or, it could be that we need to build the labyrinth for them, but the alien needs a behavior that is "move to goal". We also need a flag that is "alien spawns here."
# First Vision

I always forget what I want to make. I dream up these huge visions of what to do and I vaccilate between reasonable scopes and insane scopes. But I know that if I want to actually make a **game** I have to write a concept **down**.

So here's the concept.

*Interior - family dinner*
The family is having dinner and some friendly banter is going on.
The dinner is interrupted by murderous aliens that wish to feast upon the families of suburbia.

The game is a dynamic tower defense game, so the aliens are entering through, say, one door and are working their way across the room - but the family can build obstacles to put in their way. They can also throw plates, glasses, knives and such. The mother is an engineer and the kids are geniuses - and the dad is there too. 
They build automatic guns from kitchen items, power tools and surveillance equipment - combined with raspberry pi computers etc to automatically gun down the aliens.

They pile up furniture and stuff to stop the onslaught of aliens. 

The aliens will be homages to all alien creatures in all films etc.