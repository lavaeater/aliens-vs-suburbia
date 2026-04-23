What is auto-aim? It is a testament to the fact that this game is about tactical and co-operative skill, not necessarily hand-eye-coordination (for the most part). This means that the player always hits what they throw stuff at! 

How? We make the player "target" the closes victim in range. This means we need to have an internal "aim-vector" that we use, I guess.

Nah, we don't. We just PICK a target, everytime we throw. The target we pick is the one that is closest with lowest angle from forward. The player should be in control but also always hit.

It shouldn't be a collider, it should be a spatial query, of course, at time of throw. Ta-daaa.
