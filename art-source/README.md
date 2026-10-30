# art-source

Models the game does not load: rigging experiments, raw downloads and work files, kept
for Blender and for later. Nothing under `assets/` points here, and the game, the asset
browser and the character editor never see these files.

To use one in the game, move it into `assets/packs/<pack>/` and give it a def
(`assets/defs/<name>.ron`, e.g. by importing it in the character editor).

- `work-models/` - Mesh2Motion rigging and animation experiments (swat, tifa, julie,
  male/female base meshes and animation libraries).
- `mesh2motion/` - Mesh2Motion characters with no def: cyberpunk-voxel, female-default,
  male-180-armored, ugly-man, spider.
- `models/` - an early fox companion, a spider and a humanoid monster.
- `toon-shooter-characters/` - the toon-shooter pack's Enemy and Hazmat characters.
