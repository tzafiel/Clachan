use image::{Rgba, RgbaImage};
use std::fs;
use std::path::Path;

// Canonical Biome RGB codes (shared by LibreSprite editing & your Rust chunk streamer)
pub const BIOME_DEEP_OCEAN: [u8; 3] = [23, 52, 92];
pub const BIOME_SHALLOW_SEA: [u8; 3] = [41, 114, 184];
pub const BIOME_STARTING_GLEN: [u8; 3] = [102, 189, 75];
pub const BIOME_RAINFOREST: [u8; 3] = [34, 115, 82];
pub const BIOME_HEATHER_MOOR: [u8; 3] = [145, 72, 150];
pub const BIOME_HIGH_MUNROS: [u8; 3] = [206, 214, 224];
pub const BIOME_LOWLAND_FARMS: [u8; 3] = [162, 217, 85];
pub const BIOME_COASTAL_CLIFF: [u8; 3] = [107, 107, 128];
pub const BIOME_URBAN_CITY: [u8; 3] = [214, 134, 64];
pub const BIOME_CLAIMABLE: [u8; 3] = [252, 232, 146];

fn hash_f32(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix.wrapping_mul(374761393) as u32)
        .wrapping_add(iy.wrapping_mul(668265263) as u32)
        .wrapping_add(seed.wrapping_mul(1442695041));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);

    let a = hash_f32(ix, iy, seed);
    let b = hash_f32(ix + 1, iy, seed);
    let c = hash_f32(ix, iy + 1, seed);
    let d = hash_f32(ix + 1, iy + 1, seed);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for oct in 0..4 {
        sum += value_noise(x * freq, y * freq, seed + oct * 31) * amp;
        freq *= 2.05;
        amp *= 0.5;
    }
    sum
}

fn ellipse_mask(x: f32, y: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> f32 {
    let dx = (x - cx) / rx;
    let dy = (y - cy) / ry;
    (1.0 - (dx * dx + dy * dy)).clamp(0.0, 1.0)
}

fn stamp_disc(img: &mut RgbaImage, cx: i32, cy: i32, r: i32, rgb: [u8; 3]) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let px = cx + dx;
                let py = cy + dy;
                if (0..256).contains(&px) && (0..256).contains(&py) {
                    img.put_pixel(px as u32, py as u32, Rgba([rgb[0], rgb[1], rgb[2], 255]));
                }
            }
        }
    }
}

fn ensure_land_disc_if_water(
    biomes: &mut RgbaImage,
    elevations: &mut RgbaImage,
    cx: i32,
    cy: i32,
    radius: i32,
    land_biome: [u8; 3],
) {
    if cx < 0 || cy < 0 || cx >= 256 || cy >= 256 {
        return;
    }
    let center = biomes.get_pixel(cx as u32, cy as u32).0;
    let is_water = center[..3] == BIOME_DEEP_OCEAN || center[..3] == BIOME_SHALLOW_SEA;
    if !is_water {
        return;
    }

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let px = cx + dx;
            let py = cy + dy;
            if px < 0 || py < 0 || px >= 256 || py >= 256 {
                continue;
            }
            biomes.put_pixel(
                px as u32,
                py as u32,
                Rgba([land_biome[0], land_biome[1], land_biome[2], 255]),
            );
            let height = 72u8.saturating_add((radius - dx.abs().max(dy.abs())) as u8 * 2);
            elevations.put_pixel(px as u32, py as u32, Rgba([height, height, height, 255]));
        }
    }
}

pub fn generate_world(out_dir: &str, seed: u32) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("Failed creating {out_dir}: {e}"))?;

    let mut biomes = RgbaImage::new(256, 256);
    let mut elev_img = RgbaImage::new(256, 256);

    for py in 0..256u32 {
        for px in 0..256u32 {
            let x = px as f32;
            let y = py as f32;
            let wx = x + (fbm(x * 0.03, y * 0.03, seed + 7) - 0.5) * 34.0;
            let wy = y + (fbm(x * 0.03, y * 0.03, seed + 19) - 0.5) * 34.0;
            let fjord_detail = (fbm(wx * 0.11, wy * 0.11, seed + 223) - 0.5) * 0.22;
            let coast_noise = (fbm(wx * 0.055, wy * 0.055, seed) - 0.5) * 0.42 + fjord_detail;
            let ridge_noise = fbm(wx * 0.09, wy * 0.09, seed + 101);

            // 1. Sculpt Scottish-inspired landmass + Outer Hebrides + Northern Isles
            let highlands = ellipse_mask(wx, wy, 162.0, 108.0, 56.0, 62.0) * 1.15;
            let lowlands = ellipse_mask(wx, wy, 166.0, 206.0, 58.0, 40.0) * 0.95;
            let west_peninsula = ellipse_mask(wx, wy, 114.0, 146.0, 24.0, 38.0) * 0.85;
            let hebrides_n = ellipse_mask(wx, wy, 62.0, 82.0, 14.0, 26.0) * 0.80;
            let hebrides_s = ellipse_mask(wx, wy, 56.0, 126.0, 11.0, 18.0) * 0.72;
            let northern_isles = ellipse_mask(wx, wy, 168.0, 26.0, 22.0, 10.0) * 0.76;
            let pirate_isle = ellipse_mask(wx, wy, 42.0, 190.0, 10.0, 10.0) * 0.72;

            // Carve the Firth of Clyde (west) and Firth of Forth (east) sea inlets
            let firth_clyde = ellipse_mask(wx, wy, 106.0, 190.0, 26.0, 14.0) * 0.65;
            let firth_forth = ellipse_mask(wx, wy, 212.0, 186.0, 28.0, 11.0) * 0.62;
            let sea_loch = ellipse_mask(wx, wy, 96.0, 128.0, 12.0, 16.0) * 0.48;

            let raw_land = highlands
                .max(lowlands)
                .max(west_peninsula)
                .max(hebrides_n)
                .max(hebrides_s)
                .max(northern_isles)
                .max(pirate_isle)
                - firth_clyde
                - firth_forth
                - sea_loch
                + coast_noise;
            let stirling_neck = ellipse_mask(x, y, 158.0, 160.0, 5.0, 4.0) * 0.82;
            let stirling_isthmus = ellipse_mask(x, y, 160.0, 179.0, 9.0, 7.0) * 0.82;
            let isthmus_core = if (154.0..=166.0).contains(&x) && (163.0..=184.0).contains(&y) {
                0.55
            } else {
                0.0
            };
            let raw_land = raw_land.max(stirling_neck.max(stirling_isthmus.max(isthmus_core)));

            // 2. Compute Elevation (0..255; sea level = 64)
            let (biome, elev_u8) = if raw_land < 0.14 {
                let depth = ((raw_land + 0.35).clamp(0.0, 0.49) / 0.49 * 48.0) as u8;
                (BIOME_DEEP_OCEAN, depth)
            } else if raw_land < 0.24 {
                let shallow = 48 + ((raw_land - 0.14) / 0.10 * 15.0) as u8;
                (BIOME_SHALLOW_SEA, shallow)
            } else {
                // Land elevation sculpted by region
                let munro_boost = ellipse_mask(wx, wy, 152.0, 74.0, 48.0, 34.0) * 115.0;
                let glen_bowl = ellipse_mask(wx, wy, 166.0, 134.0, 26.0, 22.0) * 32.0;
                let elev = (72.0 + (raw_land - 0.24) * 95.0 + ridge_noise * 48.0 + munro_boost
                    - glen_bowl)
                    .clamp(66.0, 255.0);
                let e = elev as u8;
                let biome_x = wx + (fbm(wx * 0.03, wy * 0.03, seed + 307) - 0.5) * 36.0;
                let biome_y = wy + (fbm(wx * 0.03, wy * 0.03, seed + 401) - 0.5) * 36.0;

                let b = if e >= 176 {
                    BIOME_HIGH_MUNROS
                } else if biome_y > 176.0 && e < 126 {
                    BIOME_LOWLAND_FARMS
                } else if biome_x < 142.0 && biome_y > 92.0 && biome_y < 176.0 && biome_x > 84.0 {
                    BIOME_RAINFOREST
                } else if biome_x >= 142.0
                    && biome_x <= 198.0
                    && biome_y >= 106.0
                    && biome_y <= 164.0
                    && e < 142
                {
                    BIOME_STARTING_GLEN
                } else if biome_x > 196.0 && raw_land < 0.36 {
                    BIOME_COASTAL_CLIFF
                } else {
                    BIOME_HEATHER_MOOR
                };
                (b, e)
            };

            biomes.put_pixel(px, py, Rgba([biome[0], biome[1], biome[2], 255]));
            elev_img.put_pixel(px, py, Rgba([elev_u8, elev_u8, elev_u8, 255]));
        }
    }

    // 3. Ensure anchors on water receive a land disc before city/settlement stamps.
    for &(cx, cy, biome, radius) in &[
        (164, 136, BIOME_CLAIMABLE, 5),
        (188, 194, BIOME_URBAN_CITY, 5),
        (134, 192, BIOME_URBAN_CITY, 5),
        (158, 180, BIOME_URBAN_CITY, 5),
        (168, 86, BIOME_URBAN_CITY, 5),
        (94, 128, BIOME_CLAIMABLE, 5),
        (202, 122, BIOME_CLAIMABLE, 5),
        (60, 82, BIOME_CLAIMABLE, 5),
        (140, 68, BIOME_CLAIMABLE, 5),
        (42, 190, BIOME_CLAIMABLE, 4),
    ] {
        ensure_land_disc_if_water(&mut biomes, &mut elev_img, cx, cy, radius, biome);
    }

    // Stamp iconic cities & strategic claimable frontier sites.
    stamp_disc(&mut biomes, 188, 194, 5, BIOME_URBAN_CITY); // Edinburgh
    stamp_disc(&mut biomes, 134, 192, 5, BIOME_URBAN_CITY); // Glasgow
    stamp_disc(&mut biomes, 158, 180, 3, BIOME_URBAN_CITY); // Stirling
    stamp_disc(&mut biomes, 168, 86, 4, BIOME_URBAN_CITY); // Inverness

    // Claimable Clan Frontier Sites (plant banner to build keeps/villages/shipyards)
    for &(cx, cy) in &[
        (164, 136), // Starter Croft Valley (Glen Albyn)
        (94, 128),  // Tidal Island Keep (West Sea Loch)
        (202, 122), // East Cliff Fortress
        (60, 82),   // Hebridean Smuggler Cove
        (140, 68),  // High Munro Pass Fort
        (42, 190),  // Southern Corsair Atoll
    ] {
        stamp_disc(&mut biomes, cx, cy, 2, BIOME_CLAIMABLE);
    }

    let biomes_path = Path::new(out_dir).join("world_biomes.png");
    let elev_path = Path::new(out_dir).join("world_elevation.png");
    let anchors_path = Path::new(out_dir).join("world_anchors.ron");

    biomes
        .save(&biomes_path)
        .map_err(|e| format!("Save failed: {e}"))?;
    elev_img
        .save(&elev_path)
        .map_err(|e| format!("Save failed: {e}"))?;

    // 4. Write human-readable world_anchors.ron for cities, roads, wind lanes & Z-portals
    let anchors_ron = format!(
        r#"// Clachan Master World Blueprint (256x256 chunks = 16,384x16,384 tiles, 5 Z-layers)
(
    world_seed: {seed},
    chunk_size_tiles: 64,
    world_chunks: (256, 256),
    starting_chunk: (164, 136),

    settlements: [
        (id: "glen_albyn_croft", name: "Glen Albyn Starter Croft", chunk: (164, 136), radius: 2, faction: "Player", claimable: true),
        (id: "edinburgh",        name: "Edinburgh (Auld Reekie)",  chunk: (188, 194), radius: 5, faction: "Crown",  claimable: false),
        (id: "glasgow",          name: "Glasgow Shipyards",        chunk: (134, 192), radius: 5, faction: "Guild",  claimable: false),
        (id: "stirling",         name: "Stirling Bridge & Castle", chunk: (158, 180), radius: 3, faction: "Crown",  claimable: false),
        (id: "inverness",        name: "Inverness Garrison",       chunk: (168, 86),  radius: 4, faction: "Crown",  claimable: false),
        (id: "eilean_keep",      name: "Tidal Island Keep",        chunk: (94, 128),  radius: 2, faction: "RivalClan", claimable: true),
        (id: "east_cliff_fort",  name: "Dunnottar Cliff Fortress", chunk: (202, 122), radius: 2, faction: "Neutral",   claimable: true),
        (id: "hebrides_cove",    name: "Stornoway Smuggler Cove",  chunk: (60, 82),   radius: 2, faction: "Smugglers", claimable: true),
        (id: "munro_pass",       name: "Clan MacRae Stronghold",   chunk: (140, 68),  radius: 2, faction: "RivalClan", claimable: true),
    ],

    // Splines carved smoothly across chunks by the chunk streamer
    roads: [
        (id: "wades_military_road", width_tiles: 4, waypoints: [(158, 180), (162, 158), (164, 136), (166, 112), (168, 86)]),
        (id: "royal_lowland_way",   width_tiles: 5, waypoints: [(134, 192), (158, 180), (174, 188), (188, 194)]),
        (id: "rainforest_track",    width_tiles: 3, waypoints: [(164, 136), (138, 138), (114, 134), (94, 128)]),
    ],

    // Dynamic naval shipping & prevailing wind corridors
    sea_lanes: [
        (id: "atlantic_trade_route", waypoints: [(134, 192), (96, 194), (66, 170), (48, 126), (52, 72), (110, 26)]),
        (id: "north_sea_patrol",     waypoints: [(188, 194), (226, 180), (228, 122), (212, 62), (168, 26)]),
        (id: "corsair_run",          waypoints: [(60, 82), (34, 132), (42, 190), (88, 224)]),
    ],

    // Vertical World Portals connecting Z = 0 down to Z = -1, -2, -3
    depth_portals: [
        (id: "edinburgh_vaults",   from_z: 0,  to_z: -1, chunk: (188, 194), kind: "CityVaults"),
        (id: "rainforest_sinkhole",from_z: 0,  to_z: -2, chunk: (118, 144), kind: "BioluminescentRuins"),
        (id: "sea_loch_grotto",   from_z: 0,  to_z: -2, chunk: (94, 128),  kind: "TidalSeaCave"),
        (id: "munro_iron_delve",  from_z: 0,  to_z: -1, chunk: (146, 76),  kind: "MountainMine"),
        (id: "deep_forge_breach", from_z: -2, to_z: -3, chunk: (152, 92),  kind: "MagmaForgeGate"),
    ],
)
"#
    );

    fs::write(&anchors_path, anchors_ron).map_err(|e| format!("Failed saving anchors: {e}"))?;
    println!("Generated Clachan World Blueprint in {out_dir}:");
    println!("  - world_biomes.png    (256x256 chunks = 16,384x16,384 tiles)");
    println!("  - world_elevation.png (256x256 grayscale heightmap)");
    println!("  - world_anchors.ron   (settlements, Wade's Road, sea lanes, Z-portals)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn water_settlement_anchors_are_stamped_as_land_discs() {
        let water = Rgba([
            BIOME_DEEP_OCEAN[0],
            BIOME_DEEP_OCEAN[1],
            BIOME_DEEP_OCEAN[2],
            255,
        ]);
        let mut biomes = RgbaImage::from_pixel(256, 256, water);
        let mut elevations = RgbaImage::from_pixel(256, 256, Rgba([12, 12, 12, 255]));

        ensure_land_disc_if_water(&mut biomes, &mut elevations, 94, 128, 5, BIOME_CLAIMABLE);

        assert_eq!(biomes.get_pixel(94, 128).0[..3], BIOME_CLAIMABLE);
        assert_eq!(biomes.get_pixel(99, 128).0[..3], BIOME_CLAIMABLE);
        assert_eq!(biomes.get_pixel(100, 128).0[..3], BIOME_DEEP_OCEAN);
        assert!(elevations.get_pixel(94, 128)[0] > 64);
    }

    #[test]
    fn generated_map_has_a_connected_stirling_isthmus_and_land_anchors() {
        let out_dir =
            std::env::temp_dir().join(format!("clachan-world-gen-{}-{}", std::process::id(), 1744));
        if out_dir.exists() {
            fs::remove_dir_all(&out_dir).unwrap();
        }
        generate_world(out_dir.to_str().unwrap(), 1744).unwrap();
        let biomes = image::open(out_dir.join("world_biomes.png"))
            .unwrap()
            .to_rgba8();

        for y in 176..=182 {
            for x in 156..=164 {
                let color = biomes.get_pixel(x, y).0;
                assert_ne!(color[..3], BIOME_DEEP_OCEAN);
                assert_ne!(color[..3], BIOME_SHALLOW_SEA);
            }
        }

        let start = (162u32, 120u32);
        let target = (166u32, 200u32);
        let mut visited = vec![false; 256 * 256];
        let mut pending = VecDeque::from([start]);
        visited[(start.1 * 256 + start.0) as usize] = true;
        while let Some((x, y)) = pending.pop_front() {
            for (next_x, next_y) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if next_x >= 256 || next_y >= 256 {
                    continue;
                }
                let index = (next_y * 256 + next_x) as usize;
                let color = biomes.get_pixel(next_x, next_y).0;
                let is_land = color[..3] != BIOME_DEEP_OCEAN && color[..3] != BIOME_SHALLOW_SEA;
                if is_land && !visited[index] {
                    visited[index] = true;
                    pending.push_back((next_x, next_y));
                }
            }
        }
        assert!(visited[(target.1 * 256 + target.0) as usize]);

        for (x, y) in [
            (164, 136),
            (188, 194),
            (134, 192),
            (158, 180),
            (168, 86),
            (94, 128),
            (202, 122),
            (60, 82),
            (140, 68),
            (42, 190),
        ] {
            let color = biomes.get_pixel(x, y).0;
            assert_ne!(color[..3], BIOME_DEEP_OCEAN, "anchor ({x}, {y})");
            assert_ne!(color[..3], BIOME_SHALLOW_SEA, "anchor ({x}, {y})");
        }

        fs::remove_dir_all(out_dir).unwrap();
    }
}
