# Clachan sprite-sheet specification

All sheets are PNG, use a transparent RGBA background where noted, have no
spacing between frames, and are sampled with nearest-neighbor filtering.
One world meter is 16 pixels. Sheet coordinates and frame indices are
zero-based.

## Terrain — `terrain.png` (256 x 128)

Sixteen columns by eight rows of 16 x 16 pixel tiles. Terrain tiles are opaque;
the remaining rows and slots are reserved for later authored tiles.

| Row | Columns | Contents | World anchor |
|---:|---|---|---|
| 0 | 0–3 | Meadow grass variants 0–3 | Tile top-left |
| 0 | 4–7 | Peat bog variants 0–3 | Tile top-left |
| 0 | 8–11 | Granite crag variants 0–3 | Tile top-left |
| 0 | 12–15 | Interior floor-plank variants 0–3 | Tile top-left |
| 1 | 0–15 | Heather 4-neighbor masks | Tile top-left |
| 2 | 0–15 | Wade's Road 4-neighbor masks | Tile top-left |
| 3 | 0–3 | Loch-water animation frames 0–3 | Tile top-left |
| 3 | 4–15 | Shoreline masks 0–11; masks 12–15 use tile 11 | Tile top-left |
| 4 | 0–1 | Dry and rain-watered tilled soil | Tile top-left |
| 4 | 2–3 | Horizontal and vertical dry-stone wall | Tile top-left |
| 4 | 4–7 | Wall corners: NW, NE, SW, SE | Tile top-left |
| 4 | 8–11 | Wall end caps: W, E, N, S | Tile top-left |
| 5 | 0–5 | North stone wall, wainscoting, day window, rain window, hearth, doormat | Tile top-left |
| 5 | 6–15; 6–7 | Reserved | Tile top-left |

For rows 1 and 2, mask bits are N=1, E=2, S=4, W=8. A set bit means the
neighboring tile belongs to the same terrain type. Water uses columns 0–3
when surrounded by water; otherwise its 4-neighbor mask selects a shoreline
tile as listed above.

## Buildings — `buildings.png` (512 x 96)

Four transparent 128 x 96 pixel building sprites in one row. Each sprite is
scaled to its `BuildingRecord` footprint. The world position is the sprite's
top-left corner.

| Slot / columns | Building |
|---:|---|
| 0 / 0–127 | Callum's Stone Bothy |
| 1 / 128–255 | MacRae Smithy |
| 2 / 256–383 | The Heather Alehouse |
| 3 / 384–511 | Wade's Toll Watch-Post |

## Trees — `trees.png` (128 x 48)

Four transparent 32 x 48 pixel sprites in one row. All use a bottom-center
ground anchor. The top 32 pixels are the canopy region; the bottom 16 pixels
are the trunk/base region.

| Slot / columns | Sprite |
|---:|---|
| 0 / 0–31 | Scots Pine variant 0 |
| 1 / 32–63 | Scots Pine variant 1 |
| 2 / 64–95 | Felled Scots Pine stump |
| 3 / 96–127 | Timber gibbet frame |

When the player passes behind the upper canopy, render the canopy at alpha
0.45 while leaving the trunk/base opaque.

## Props — `props.png` (256 x 128)

Sixteen columns by eight rows of transparent 16 x 16 pixel frames. Object
sprites are bottom-center anchored unless noted.

| Row | Column | Sprite | Anchor |
|---:|---:|---|---|
| 0 | 0 | Tall grass | Bottom-center |
| 0 | 1 | Fern bracken | Bottom-center |
| 0 | 2 | Gorse bush | Bottom-center |
| 0 | 3 | Fallen log | Bottom-center |
| 0 | 4 | Fieldstone boulder | Bottom-center |
| 0 | 5 | Burial cairn | Bottom-center |
| 0 | 6–7 | Campfire frames 0–1 | Bottom-center |
| 0 | 8–15 | Reserved | Bottom-center |
| 1 | 0 | Rug tile | Tile top-left |
| 1 | 1–2 | Bed upper / lower | Each half's top-left |
| 1 | 3 | Table | Bottom-center |
| 1 | 4 | Anvil | Bottom-center |
| 1 | 5 | Forge | Bottom-center |
| 1 | 6 | Bar counter | Bottom-center |
| 1 | 7 | Barrel | Bottom-center |
| 1 | 8 | Weapon rack | Bottom-center |
| 1 | 9 | Iron cell bars | Bottom-center |
| 1 | 10–15; 2–7 | Reserved | Bottom-center |

## Characters — `player.png`, `villager.png`, `redcoat.png` (64 x 128 each)

Each transparent sheet is four columns by four rows of 16 x 32 pixel frames.
Frames are bottom-center anchored at the character's feet (1 x 2 world
meters).

| Row | Facing |
|---:|---|
| 0 | Down |
| 1 | Up |
| 2 | Left |
| 3 | Right |

| Column | Animation |
|---:|---|
| 0 | Idle |
| 1 | Walk 1 |
| 2 | Idle |
| 3 | Walk 2 |

## Mounts and deer — `bramble.png`, `deer.png` (128 x 128 each)

Each transparent sheet is four columns by four rows of 32 x 32 pixel frames.
Frames are bottom-center anchored at the feet (2 x 2 world meters). Facing
rows match the character sheets.

| Column | Animation |
|---:|---|
| 0 | Idle |
| 1 | Trot 1 |
| 2 | Trot 2 |
| 3 | Trot 3 |

## User interface — `ui.png` (128 x 64)

Eight columns by four rows of transparent 16 x 16 pixel tiles. UI coordinates
are screen-space pixels; icons use their top-left corner as the anchor.

| Row | Columns | Contents |
|---:|---|---|
| 0 | 0–4 | Dirk, flintlock, whistle, bandage, peat-fire kit icons |
| 0 | 5–7 | Backpack, shilling, heart icons |
| 1–3 | 0–2 | 3 x 3 panel nine-slice: top, middle, bottom rows |
| 1–3 | 3–7 | Reserved |

## Placeholder sheet notes

The committed PNGs are muted color-coded painting templates with 1-pixel
guides. Sprite sheets retain transparent backgrounds; terrain tiles are
opaque. Paint directly over the guides and preserve the dimensions, slot
order, anchors, and alpha around sprite silhouettes.
