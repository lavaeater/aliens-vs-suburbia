This is fairly easy.

It could be a separate behavior or it could be included in the pathfinding one... but when the pathfinding algorithm returns no path, the alien should simply move towards the goal diagonally and start attacking anything in it's way - until a path becomes available. 

It should probably be its own behavior. 

When this is done, we have "something" that resembles a game!

Oooh, in fact, what we need to do is for the aliens to plot a course to the closes obstacle and just start hacking away. But hoooow do we keep track of that? Well, we have to reconfigure the grid completely then?!

Aah, targets are acquired using the grid, when a grid is illegal, we find the closest tower / obstacle and find a path to it.

It is still approach goal, I guess?

# Next steps

Now I have to add that complex feature that I love from the South Park: Tower Defense game - the enemies just destroying your map if you happen to build an unsolvable maze!

But how do we do it? 

Well, here's my idea - when an alien **cannot find a path** through the maze, we send an event