# UI

## HUD
We need a HUD that is useful for the player, showing scores, etc.

## Debug / Settings UI

We need a simple UI to modify camera settings, zoom settings and all sorts of things that we want to modify - all these settings should then of course be serialized to some kind of game-settings.ron file as we change them so that they are stored for the next run - which also would enable editing them. It is good if we make that format forward-compatible so that if we add properties and settings in the future, the old files still work but are updated upon the next save with any new properties. 