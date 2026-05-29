# Aliens vs Suburbia

A 3D tower defense game built with Rust and Bevy. Defend your neighborhood against waves of aliens by building towers and using special abilities.

## Running the game

```bash
cargo run
```

For a faster build without debug overhead:

```bash
cargo run --release
```

## Map Editor TUI

The map editor is a terminal-based tool for creating and editing map files.

```bash
cargo run --features map-editor -- --map-editor
```

To open an existing map:

```bash
cargo run --features map-editor -- --map-editor --file assets/maps/map_1.ron
```

Maps are saved as `.ron` files under `assets/maps/`.

## Generating maps procedurally

```bash
cargo run -- --create-map
cargo run -- --create-map --seed 42 --w 40 --h 16 --output assets/maps/my_map.ron
```

## Help

```bash
cargo run -- --help
```

## Development

```bash
cargo check   # check for compile errors
cargo test    # run tests
```
