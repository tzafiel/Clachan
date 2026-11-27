use crate::entities::{DeltaEntry, DeltaLedger};
use crate::world::TerrainKind;
use image::{ImageFormat, RgbaImage};
use macroquad::prelude::{FilterMode, ImageFormat as MacroquadImageFormat, Texture2D, Vec2};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const CHUNK_SIZE: i32 = 64;

const BIOME_DEEP_OCEAN: [u8; 3] = [23, 52, 92];
const BIOME_SHALLOW_SEA: [u8; 3] = [41, 114, 184];
const BIOME_STARTING_GLEN: [u8; 3] = [102, 189, 75];
const BIOME_RAINFOREST: [u8; 3] = [34, 115, 82];
const BIOME_HEATHER_MOOR: [u8; 3] = [145, 72, 150];
const BIOME_HIGH_MUNROS: [u8; 3] = [206, 214, 224];
const BIOME_LOWLAND_FARMS: [u8; 3] = [162, 217, 85];
const BIOME_COASTAL_CLIFF: [u8; 3] = [107, 107, 128];
const BIOME_URBAN_CITY: [u8; 3] = [214, 134, 64];
const BIOME_CLAIMABLE: [u8; 3] = [252, 232, 146];

const BIOMES_PATH: &str = "assets/world/world_biomes.png";
const ELEVATION_PATH: &str = "assets/world/world_elevation.png";
const ANCHORS_PATH: &str = "assets/world/world_anchors.ron";

#[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
const EMBEDDED_BIOMES: &[u8] = include_bytes!("../assets/world/world_biomes.png");
#[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
const EMBEDDED_ELEVATION: &[u8] = include_bytes!("../assets/world/world_elevation.png");
#[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
const EMBEDDED_ANCHORS: &[u8] = include_bytes!("../assets/world/world_anchors.ron");

#[derive(Clone, Debug, Deserialize)]
pub struct WorldAnchors {
    pub world_seed: u32,
    pub chunk_size_tiles: u32,
    pub world_chunks: (u32, u32),
    pub starting_chunk: (u32, u32),
    pub settlements: Vec<SettlementAnchor>,
    pub roads: Vec<RoadAnchor>,
    pub sea_lanes: Vec<PolylineAnchor>,
    pub depth_portals: Vec<DepthPortal>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SettlementAnchor {
    pub id: String,
    pub name: String,
    pub chunk: (u32, u32),
    pub radius: u32,
    pub faction: String,
    pub claimable: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoadAnchor {
    pub id: String,
    pub width_tiles: u32,
    pub waypoints: Vec<(u32, u32)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PolylineAnchor {
    pub id: String,
    pub waypoints: Vec<(u32, u32)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DepthPortal {
    pub id: String,
    pub from_z: i32,
    pub to_z: i32,
    pub chunk: (u32, u32),
    pub kind: String,
}

pub struct WorldBlueprint {
    pub anchors: WorldAnchors,
    biomes: RgbaImage,
    elevations: RgbaImage,
    road_segments: Vec<RoadSegment>,
    map_texture: Texture2D,
}

impl WorldBlueprint {
    pub fn load() -> Result<Self, String> {
        let biome_bytes = load_asset_bytes(BIOMES_PATH, embedded_biomes())?;
        let elevation_bytes = load_asset_bytes(ELEVATION_PATH, embedded_elevation())?;
        let anchors_bytes = load_asset_bytes(ANCHORS_PATH, embedded_anchors())?;

        let biomes = image::load_from_memory_with_format(&biome_bytes, ImageFormat::Png)
            .map_err(|error| format!("Failed decoding {BIOMES_PATH}: {error}"))?
            .to_rgba8();
        let elevations = image::load_from_memory_with_format(&elevation_bytes, ImageFormat::Png)
            .map_err(|error| format!("Failed decoding {ELEVATION_PATH}: {error}"))?
            .to_rgba8();
        let anchors_text = std::str::from_utf8(&anchors_bytes)
            .map_err(|error| format!("{ANCHORS_PATH} is not valid UTF-8: {error}"))?;
        let anchors: WorldAnchors = ron::from_str(anchors_text)
            .map_err(|error| format!("Failed parsing {ANCHORS_PATH}: {error}"))?;

        if anchors.chunk_size_tiles != CHUNK_SIZE as u32 {
            return Err(format!(
                "{ANCHORS_PATH} declares {} tiles per chunk; expected {CHUNK_SIZE}",
                anchors.chunk_size_tiles
            ));
        }
        let expected = anchors.world_chunks;
        if biomes.dimensions() != expected || elevations.dimensions() != expected {
            return Err(format!(
                "World blueprint images must be {}x{} chunks",
                expected.0, expected.1
            ));
        }
        if anchors.starting_chunk.0 >= expected.0 || anchors.starting_chunk.1 >= expected.1 {
            return Err("Starting chunk is outside the world blueprint.".to_string());
        }
        if anchors.roads.iter().any(|road| road.waypoints.len() < 2) {
            return Err("Every road anchor must have at least two waypoints.".to_string());
        }

        let road_segments = build_road_segments(&anchors.roads);
        let map_texture =
            Texture2D::from_file_with_format(&biome_bytes, Some(MacroquadImageFormat::Png));
        map_texture.set_filter(FilterMode::Nearest);

        Ok(Self {
            anchors,
            biomes,
            elevations,
            road_segments,
            map_texture,
        })
    }

    pub fn map_texture(&self) -> &Texture2D {
        &self.map_texture
    }

    fn biome_at(&self, chunk_x: u32, chunk_y: u32) -> Biome {
        let pixel = self.biomes.get_pixel(chunk_x, chunk_y).0;
        Biome::from_rgb([pixel[0], pixel[1], pixel[2]])
    }

    fn elevation_at(&self, chunk_x: u32, chunk_y: u32) -> u8 {
        self.elevations.get_pixel(chunk_x, chunk_y)[0]
    }

    fn road_radius_at(&self, world_x: f32, world_y: f32) -> Option<f32> {
        self.road_segments
            .iter()
            .filter(|segment| {
                point_segment_distance_squared(world_x, world_y, segment.a, segment.b)
                    <= segment.radius * segment.radius
            })
            .map(|segment| segment.radius)
            .max_by(f32::total_cmp)
    }

    fn world_origin_tile(&self) -> (f32, f32) {
        world_origin_tile(self.anchors.starting_chunk)
    }

    pub fn world_tile_for_local(&self, local: Vec2) -> Vec2 {
        let (origin_x, origin_y) = self.world_origin_tile();
        Vec2::new(origin_x + local.x, origin_y + local.y)
    }

    pub fn chunk_for_local(&self, local: Vec2) -> (u32, u32) {
        chunk_for_local(
            local,
            self.anchors.starting_chunk,
            self.anchors.world_chunks,
        )
    }

    pub fn clamp_local(&self, local: Vec2) -> Vec2 {
        let (origin_x, origin_y) = self.world_origin_tile();
        let max_x = self.anchors.world_chunks.0 as f32 * CHUNK_SIZE as f32 - 1.0 - origin_x;
        let max_y = self.anchors.world_chunks.1 as f32 * CHUNK_SIZE as f32 - 1.0 - origin_y;
        Vec2::new(
            local.x.clamp(-origin_x, max_x),
            local.y.clamp(-origin_y, max_y),
        )
    }

    pub fn terrain_at(&self, local: Vec2, world_seed: u32) -> TerrainKind {
        self.terrain_at_with_chunk(local, world_seed, None)
    }

    fn terrain_at_with_chunk(
        &self,
        local: Vec2,
        world_seed: u32,
        active_chunk: Option<&ActiveChunk>,
    ) -> TerrainKind {
        let global = self.world_tile_for_local(local);
        if global.x < 0.0
            || global.y < 0.0
            || global.x >= self.anchors.world_chunks.0 as f32 * CHUNK_SIZE as f32
            || global.y >= self.anchors.world_chunks.1 as f32 * CHUNK_SIZE as f32
        {
            return TerrainKind::LochWater;
        }
        let tile_x = global.x.floor() as i32;
        let tile_y = global.y.floor() as i32;
        let chunk_x = tile_x.div_euclid(CHUNK_SIZE) as u32;
        let chunk_y = tile_y.div_euclid(CHUNK_SIZE) as u32;
        let biome = active_chunk
            .map(|chunk| chunk.biome)
            .unwrap_or_else(|| self.biome_at(chunk_x, chunk_y));

        if self
            .road_radius_at(tile_x as f32 + 0.5, tile_y as f32 + 0.5)
            .is_some()
        {
            return TerrainKind::MudRoad;
        }

        let elevation = active_chunk
            .map(|chunk| chunk.elevation)
            .unwrap_or_else(|| self.elevation_at(chunk_x, chunk_y));
        let variation = hash_cell(world_seed, tile_x, tile_y) % 100;
        match biome {
            Biome::DeepOcean | Biome::ShallowSea => TerrainKind::LochWater,
            Biome::StartingGlen | Biome::Claimable => {
                if variation < 7 {
                    TerrainKind::PeatBog
                } else {
                    TerrainKind::Meadow
                }
            }
            Biome::Rainforest => {
                if variation < 9 {
                    TerrainKind::Heather
                } else {
                    TerrainKind::Meadow
                }
            }
            Biome::HeatherMoor => TerrainKind::Heather,
            Biome::HighMunros | Biome::CoastalCliff => TerrainKind::GraniteCrag,
            Biome::LowlandFarms => {
                if variation < 16 {
                    TerrainKind::TilledSoil
                } else {
                    TerrainKind::Meadow
                }
            }
            Biome::UrbanCity => TerrainKind::TilledSoil,
            Biome::Unknown if elevation < 64 => TerrainKind::LochWater,
            Biome::Unknown if elevation > 190 => TerrainKind::GraniteCrag,
            Biome::Unknown => TerrainKind::Meadow,
        }
    }
}

fn embedded_biomes() -> &'static [u8] {
    #[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
    {
        EMBEDDED_BIOMES
    }
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    {
        &[]
    }
}

fn embedded_elevation() -> &'static [u8] {
    #[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
    {
        EMBEDDED_ELEVATION
    }
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    {
        &[]
    }
}

fn embedded_anchors() -> &'static [u8] {
    #[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
    {
        EMBEDDED_ANCHORS
    }
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    {
        &[]
    }
}

fn load_asset_bytes(path: &str, embedded: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    {
        let _ = embedded;
        std::fs::read(path).map_err(|error| format!("{path}: {error}"))
    }
    #[cfg(not(all(debug_assertions, not(target_arch = "wasm32"))))]
    {
        let _ = path;
        Ok(embedded.to_vec())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Biome {
    DeepOcean,
    ShallowSea,
    StartingGlen,
    Rainforest,
    HeatherMoor,
    HighMunros,
    LowlandFarms,
    CoastalCliff,
    UrbanCity,
    Claimable,
    Unknown,
}

impl Biome {
    fn from_rgb(rgb: [u8; 3]) -> Self {
        match rgb {
            BIOME_DEEP_OCEAN => Self::DeepOcean,
            BIOME_SHALLOW_SEA => Self::ShallowSea,
            BIOME_STARTING_GLEN => Self::StartingGlen,
            BIOME_RAINFOREST => Self::Rainforest,
            BIOME_HEATHER_MOOR => Self::HeatherMoor,
            BIOME_HIGH_MUNROS => Self::HighMunros,
            BIOME_LOWLAND_FARMS => Self::LowlandFarms,
            BIOME_COASTAL_CLIFF => Self::CoastalCliff,
            BIOME_URBAN_CITY => Self::UrbanCity,
            BIOME_CLAIMABLE => Self::Claimable,
            _ => Self::Unknown,
        }
    }

    fn foliage_density(self) -> u32 {
        match self {
            Self::Rainforest => 48,
            Self::HeatherMoor | Self::CoastalCliff => 24,
            Self::StartingGlen | Self::Claimable => 16,
            Self::LowlandFarms => 8,
            Self::HighMunros => 6,
            Self::DeepOcean | Self::ShallowSea | Self::UrbanCity | Self::Unknown => 0,
        }
    }

    fn can_hold_trees(self) -> bool {
        self.foliage_density() > 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Foliage {
    pub id: u64,
    pub position: Vec2,
    pub scale: f32,
    pub chopped: bool,
}

#[derive(Clone, Copy, Debug)]
struct RoadSegment {
    a: Vec2,
    b: Vec2,
    radius: f32,
}

struct ActiveChunk {
    biome: Biome,
    elevation: u8,
    foliage: Vec<Foliage>,
}

pub struct WorldStream {
    pub blueprint: Arc<WorldBlueprint>,
    world_seed: u32,
    active_center: Option<(u32, u32)>,
    active_chunks: HashMap<(u32, u32), ActiveChunk>,
    foliage: Vec<Foliage>,
}

impl WorldStream {
    pub fn new(blueprint: Arc<WorldBlueprint>) -> Self {
        let world_seed = blueprint.anchors.world_seed;
        Self {
            blueprint,
            world_seed,
            active_center: None,
            active_chunks: HashMap::new(),
            foliage: Vec::new(),
        }
    }

    pub fn update(&mut self, local_player_position: Vec2, ledger: &DeltaLedger) {
        let center = self.blueprint.chunk_for_local(local_player_position);
        if self.active_center == Some(center) {
            return;
        }

        let destroyed: HashSet<u64> = ledger
            .entries
            .iter()
            .filter_map(|entry| match entry {
                DeltaEntry::FoliageChopped(id) => Some(*id),
                _ => None,
            })
            .collect();
        let width = self.blueprint.anchors.world_chunks.0;
        let height = self.blueprint.anchors.world_chunks.1;
        let mut chunks = HashMap::with_capacity(9);
        let mut foliage = Vec::new();

        for (cx, cy) in active_window_coords(center, width, height) {
            let chunk = self.generate_chunk(cx, cy, &destroyed);
            foliage.extend(chunk.foliage.iter().copied());
            chunks.insert((cx, cy), chunk);
        }

        self.active_center = Some(center);
        self.active_chunks = chunks;
        self.foliage = foliage;
    }

    pub fn active_chunk_count(&self) -> usize {
        self.active_chunks.len()
    }

    pub fn foliage(&self) -> &[Foliage] {
        &self.foliage
    }

    pub fn chop_foliage(&mut self, id: u64) -> bool {
        if let Some(tree) = self.foliage.iter_mut().find(|tree| tree.id == id) {
            if tree.chopped {
                return false;
            }
            tree.chopped = true;
            for chunk in self.active_chunks.values_mut() {
                if let Some(tree) = chunk.foliage.iter_mut().find(|tree| tree.id == id) {
                    tree.chopped = true;
                }
            }
            return true;
        }
        false
    }

    pub fn terrain_at(&self, local: Vec2) -> TerrainKind {
        let chunk = self.blueprint.chunk_for_local(local);
        self.blueprint
            .terrain_at_with_chunk(local, self.world_seed, self.active_chunks.get(&chunk))
    }

    pub fn clamp_local(&self, local: Vec2) -> Vec2 {
        self.blueprint.clamp_local(local)
    }

    fn generate_chunk(&self, chunk_x: u32, chunk_y: u32, destroyed: &HashSet<u64>) -> ActiveChunk {
        let biome = self.blueprint.biome_at(chunk_x, chunk_y);
        let elevation = self.blueprint.elevation_at(chunk_x, chunk_y);
        let mut foliage = Vec::new();
        if biome.can_hold_trees() {
            for slot in 0..64u32 {
                let random = hash_chunk(self.world_seed, chunk_x, chunk_y, slot);
                if random % 100 >= biome.foliage_density() {
                    continue;
                }
                let tile_x = chunk_x as i32 * CHUNK_SIZE
                    + (slot % 8) as i32 * 8
                    + 2
                    + ((random >> 8) % 5) as i32;
                let tile_y = chunk_y as i32 * CHUNK_SIZE
                    + (slot / 8) as i32 * 8
                    + 2
                    + ((random >> 16) % 5) as i32;
                if self
                    .blueprint
                    .road_radius_at(tile_x as f32 + 0.5, tile_y as f32 + 0.5)
                    .is_some()
                {
                    continue;
                }
                let id = (chunk_y as u64 * self.blueprint.anchors.world_chunks.0 as u64
                    + chunk_x as u64)
                    * 64
                    + slot as u64;
                let world_tile = Vec2::new(tile_x as f32 + 0.5, tile_y as f32 + 0.5);
                let local_origin = self.blueprint.world_origin_tile();
                foliage.push(Foliage {
                    id,
                    position: Vec2::new(
                        world_tile.x - local_origin.0,
                        world_tile.y - local_origin.1,
                    ),
                    scale: 1.5 + ((random >> 24) & 0xFF) as f32 / 255.0 * 0.7,
                    chopped: destroyed.contains(&id),
                });
            }
        }
        ActiveChunk {
            biome,
            elevation,
            foliage,
        }
    }
}

fn build_road_segments(roads: &[RoadAnchor]) -> Vec<RoadSegment> {
    let mut segments = Vec::new();
    for road in roads {
        let points: Vec<Vec2> = road
            .waypoints
            .iter()
            .map(|(x, y)| {
                Vec2::new(
                    *x as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
                    *y as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
                )
            })
            .collect();
        let radius = road.width_tiles as f32 * 0.5;
        for i in 0..points.len() - 1 {
            let p0 = points[i.saturating_sub(1)];
            let p1 = points[i];
            let p2 = points[i + 1];
            let p3 = points[(i + 2).min(points.len() - 1)];
            let mut last = p1;
            for step in 1..=16 {
                let t = step as f32 / 16.0;
                let point = catmull_rom(p0, p1, p2, p3, t);
                segments.push(RoadSegment {
                    a: last,
                    b: point,
                    radius,
                });
                last = point;
            }
        }
    }
    segments
}

fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn point_segment_distance_squared(point_x: f32, point_y: f32, a: Vec2, b: Vec2) -> f32 {
    let point = Vec2::new(point_x, point_y);
    let segment = b - a;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance_squared(a);
    }
    let t = ((point - a).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(a + segment * t)
}

fn hash_chunk(seed: u32, chunk_x: u32, chunk_y: u32, slot: u32) -> u32 {
    let mut hash = seed
        ^ chunk_x.wrapping_mul(0x9E37_79B9)
        ^ chunk_y.wrapping_mul(0x85EB_CA6B)
        ^ slot.wrapping_mul(0xC2B2_AE35);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846C_A68B);
    hash ^ (hash >> 16)
}

fn hash_cell(seed: u32, x: i32, y: i32) -> u32 {
    hash_chunk(seed, x as u32, y as u32, 0)
}

fn active_window_coords(center: (u32, u32), width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut coords = Vec::with_capacity(9);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let x = center.0 as i32 + dx;
            let y = center.1 as i32 + dy;
            if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                coords.push((x as u32, y as u32));
            }
        }
    }
    coords
}

fn world_origin_tile(starting_chunk: (u32, u32)) -> (f32, f32) {
    (
        starting_chunk.0 as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
        starting_chunk.1 as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
    )
}

fn chunk_for_local(
    local: Vec2,
    starting_chunk: (u32, u32),
    world_chunks: (u32, u32),
) -> (u32, u32) {
    let (origin_x, origin_y) = world_origin_tile(starting_chunk);
    let max_x = world_chunks.0 as i32 - 1;
    let max_y = world_chunks.1 as i32 - 1;
    (
        ((origin_x + local.x).floor() as i32 / CHUNK_SIZE).clamp(0, max_x) as u32,
        ((origin_y + local.y).floor() as i32 / CHUNK_SIZE).clamp(0, max_y) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn generated_blueprint_ron_and_maps_match_the_runtime_schema() {
        let anchors: WorldAnchors =
            ron::from_str(include_str!("../assets/world/world_anchors.ron")).unwrap();
        assert_eq!(anchors.world_chunks, (256, 256));
        assert_eq!(anchors.chunk_size_tiles, 64);
        assert_eq!(anchors.starting_chunk, (164, 136));
        assert!(anchors
            .roads
            .iter()
            .any(|road| road.id == "wades_military_road"));

        for bytes in [
            include_bytes!("../assets/world/world_biomes.png").as_slice(),
            include_bytes!("../assets/world/world_elevation.png").as_slice(),
        ] {
            let image = image::load_from_memory_with_format(bytes, ImageFormat::Png).unwrap();
            assert_eq!(image.dimensions(), (256, 256));
        }
    }

    #[test]
    fn starting_position_resolves_to_the_anchor_chunk() {
        let start = Vec2::new(-8.5, 0.5);
        assert_eq!(chunk_for_local(start, (164, 136), (256, 256)), (164, 136));
    }

    #[test]
    fn sparse_chop_ids_are_unique_per_candidate_slot() {
        let first = (136u64 * 256 + 164) * 64;
        let neighbor = (136u64 * 256 + 165) * 64;
        assert_ne!(first, neighbor);
        assert_ne!(first + 63, neighbor);
    }

    #[test]
    fn active_window_streams_nine_chunks_and_clips_at_world_edges() {
        assert_eq!(active_window_coords((164, 136), 256, 256).len(), 9);
        assert_eq!(active_window_coords((0, 0), 256, 256).len(), 4);
    }

    #[test]
    fn road_splines_pass_through_their_anchored_waypoints() {
        let road = RoadAnchor {
            id: "test".to_string(),
            width_tiles: 4,
            waypoints: vec![(10, 10), (11, 10), (12, 10)],
        };
        let segments = build_road_segments(&[road]);
        let waypoint = Vec2::new(10.5 * CHUNK_SIZE as f32, 10.5 * CHUNK_SIZE as f32);
        assert!(segments.iter().any(|segment| {
            point_segment_distance_squared(waypoint.x, waypoint.y, segment.a, segment.b) < 0.01
        }));
    }
}
