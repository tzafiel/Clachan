# Clachan
A deterministic 2D historical sandbox set in 1744 Scotland, with an LDtk-authored world, pixel-art rendering, and emergent simulation.

The public landing page is served from the repository root; the playable browser build is in `play/`.

## World pipeline

Edit `assets/world/clachan.ldtk` with LDtk 1.5.3. The importer draws `Ground`,
`Tiles`, and `AutoLayer` content first, sorts sprites by their world-space foot,
then draws `Overhead` layers above them. Entity sprites and colliders are defined
in `assets/packs/base/entities.ron`. Press **F6** to reload the LDtk project and
its texture cache; press **F3** to show debug footprints.

When the active LDtk level has no ground tiles, `draw_ground_pass()` renders a
deterministic procedural floor from the streamed terrain; painted ground tiles
take precedence over that fallback.

The biome map remains available with **M**. Regenerate its 256 × 256 chunk
blueprint (64 tiles per chunk) with:

```sh
cargo run --manifest-path tools/clachan_forge/Cargo.toml -- generate-world assets/world 1744
```

The game loads the biome map, elevation map, and anchor data from `assets/world/`
at startup. It keeps a 3 × 3 chunk simulation window active around the player.
The forge also retains `flash` for palette-snapping and outlining sprite sheets.

## Sprite forge

Place source artwork in `raw_art/` with target dimensions and an asset type in
the filename, such as `dirt_road_48x48_tile.webp` or
`bothy_128x96_sprite.webp`. Ingest and normalize the files with:

```sh
cargo run --bin clachan_forge -- ingest
```

Tile images are written to `assets/tilesets/`; sprite images go to
`assets/sprites/`. Near-white backgrounds connected to the image edge are
removed from sprites; tile RGB values are preserved during ingestion.
