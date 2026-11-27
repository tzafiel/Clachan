use crate::assets::AssetBank;
use crate::config::{PIXELS_PER_METER, VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::entities::{Entity, EntityKind, InventoryItem, PlayerId};
use crate::ldtk_loader::{EntityPack, LdtkEntityInstance, LdtkLayer, LdtkLevel, LdtkLoader};
use crate::render_sort::{sort_render_items, RenderItem, SortedRenderItem};
use crate::sim::Universe;
use crate::world::{Location, Season, TerrainKind, TraceKind};
use crate::world_stream::WorldStream;
use macroquad::file::load_string;
use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::prelude::*;
use serde::Deserialize;

const CANVAS_WIDTH: f32 = VIRTUAL_WIDTH as f32;
const CANVAS_HEIGHT: f32 = VIRTUAL_HEIGHT as f32;
const WEATHER_LINE_WIDTH_PX: f32 = 2.0;
const REGION_ATMOSPHERE_PATH: &str = "assets/data/regions.ron";

#[derive(Clone, Debug, Deserialize)]
pub struct RegionAtmosphere {
    pub id: String,
    pub gaelic_name: String,
    pub english_name: String,
    pub day_ambient: (f32, f32, f32),
    pub rain_ambient: (f32, f32, f32),
    pub dusk_ambient: (f32, f32, f32),
    pub night_ambient: (f32, f32, f32),
    pub sunbeam_outer: (f32, f32, f32, f32),
    pub sunbeam_core: (f32, f32, f32, f32),
    pub clear_bg: (u8, u8, u8),
}

impl RegionAtmosphere {
    pub fn pine_edge() -> Self {
        Self {
            id: "pine_edge".to_string(),
            gaelic_name: "Oir na Giùthsaich".to_string(),
            english_name: "The Pine-Edge".to_string(),
            day_ambient: (0.95, 0.97, 0.92),
            rain_ambient: (0.74, 0.80, 0.86),
            dusk_ambient: (0.78, 0.66, 0.58),
            night_ambient: (0.34, 0.40, 0.52),
            sunbeam_outer: (1.0, 0.97, 0.82, 0.24),
            sunbeam_core: (1.0, 0.99, 0.88, 0.32),
            clear_bg: (88, 138, 76),
        }
    }

    pub async fn load_starting_region() -> Self {
        match load_string(REGION_ATMOSPHERE_PATH).await {
            Ok(text) => match ron::from_str(&text) {
                Ok(region) => region,
                Err(error) => {
                    eprintln!(
                        "Failed parsing {REGION_ATMOSPHERE_PATH}: {error}; using pine_edge defaults"
                    );
                    Self::pine_edge()
                }
            },
            Err(error) => {
                eprintln!(
                    "Failed reading {REGION_ATMOSPHERE_PATH}: {error}; using pine_edge defaults"
                );
                Self::pine_edge()
            }
        }
    }

    pub fn clear_color(&self) -> Color {
        Color::from_rgba(self.clear_bg.0, self.clear_bg.1, self.clear_bg.2, 255)
    }

    fn day_color(&self) -> Color {
        Color::new(
            self.day_ambient.0,
            self.day_ambient.1,
            self.day_ambient.2,
            1.0,
        )
    }

    fn rain_color(&self) -> Color {
        Color::new(
            self.rain_ambient.0,
            self.rain_ambient.1,
            self.rain_ambient.2,
            1.0,
        )
    }

    fn dusk_color(&self) -> Color {
        Color::new(
            self.dusk_ambient.0,
            self.dusk_ambient.1,
            self.dusk_ambient.2,
            1.0,
        )
    }

    fn night_color(&self) -> Color {
        Color::new(
            self.night_ambient.0,
            self.night_ambient.1,
            self.night_ambient.2,
            1.0,
        )
    }

    fn sunbeam_outer_color(&self, strength: f32) -> Color {
        Color::new(
            self.sunbeam_outer.0,
            self.sunbeam_outer.1,
            self.sunbeam_outer.2,
            self.sunbeam_outer.3 * strength,
        )
    }

    fn sunbeam_core_color(&self, strength: f32) -> Color {
        Color::new(
            self.sunbeam_core.0,
            self.sunbeam_core.1,
            self.sunbeam_core.2,
            self.sunbeam_core.3 * strength,
        )
    }
}

fn mix_color(left: Color, right: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color::new(
        left.r + (right.r - left.r) * amount,
        left.g + (right.g - left.g) * amount,
        left.b + (right.b - left.b) * amount,
        left.a + (right.a - left.a) * amount,
    )
}

fn ambient_for_time(region: &RegionAtmosphere, time: f32) -> Color {
    let t = time.rem_euclid(1.0);
    let dawn_start = 0.20;
    let day_start = 0.30;
    let dusk_start = 0.68;
    let night_start = 0.80;
    let late_night = 0.92;
    if t < dawn_start {
        region.night_color()
    } else if t < day_start {
        mix_color(
            region.night_color(),
            region.day_color(),
            (t - dawn_start) / (day_start - dawn_start),
        )
    } else if t < dusk_start {
        region.day_color()
    } else if t < night_start {
        mix_color(
            region.day_color(),
            region.dusk_color(),
            (t - dusk_start) / (night_start - dusk_start),
        )
    } else if t < late_night {
        mix_color(
            region.dusk_color(),
            region.night_color(),
            (t - night_start) / (late_night - night_start),
        )
    } else {
        mix_color(
            region.night_color(),
            region.night_color(),
            (t - late_night) / (1.0 - late_night),
        )
    }
}

pub fn load_multiply_material() -> Material {
    let vertex = r#"#version 100
    attribute vec3 position;
    attribute vec2 texcoord;
    attribute vec4 color0;
    varying lowp vec2 uv;
    varying lowp vec4 color;
    uniform mat4 Model;
    uniform mat4 Projection;
    void main() {
        gl_Position = Projection * Model * vec4(position, 1);
        color = color0 / 255.0;
        uv = texcoord;
    }"#;

    let fragment = r#"#version 100
    varying lowp vec2 uv;
    varying lowp vec4 color;
    uniform sampler2D Texture;
    void main() {
        gl_FragColor = texture2D(Texture, uv) * color;
    }"#;

    load_material(
        ShaderSource::Glsl { vertex, fragment },
        MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::DestinationColor),
                    BlendFactor::Zero,
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect("Failed to compile multiply lightmap material")
}

pub fn draw_traces_pass(universe: &Universe) {
    let active_location = universe.player_location(PlayerId(1));
    for trace in &universe.traces {
        if trace.location != active_location {
            continue;
        }
        let fade = (1.0 - trace.age / trace.max_age).clamp(0.0, 1.0);
        match trace.kind {
            TraceKind::BootPrint => draw_ellipse(
                trace.pos.x,
                trace.pos.y,
                0.14,
                0.08,
                0.0,
                Color::new(0.08, 0.11, 0.12, 0.34 * fade),
            ),
            TraceKind::HoofPrint => draw_circle_lines(
                trace.pos.x,
                trace.pos.y,
                0.14,
                0.05,
                Color::new(0.09, 0.08, 0.07, 0.36 * fade),
            ),
            TraceKind::BloodDrop => draw_circle(
                trace.pos.x,
                trace.pos.y,
                0.09,
                Color::new(0.48, 0.06, 0.06, 0.55 * fade),
            ),
            TraceKind::Scorch => draw_circle(
                trace.pos.x,
                trace.pos.y,
                0.12,
                Color::new(0.12, 0.08, 0.05, 0.45 * fade),
            ),
        }
    }
}

pub fn draw_atmosphere_pass(
    universe: &Universe,
    region: &RegionAtmosphere,
    cam: Vec2,
    view_w_m: f32,
    view_h_m: f32,
) {
    let active_location = universe.player_location(PlayerId(1));

    for puff in &universe.smoke {
        if puff.location != active_location {
            continue;
        }
        let fade = (1.0 - puff.age / puff.max_age).clamp(0.0, 1.0);
        let color = if puff.is_ember {
            Color::new(1.0, 0.62, 0.22, 0.85 * fade)
        } else {
            Color::new(0.76, 0.78, 0.76, 0.24 * fade)
        };
        draw_circle(puff.pos.x, puff.pos.y, puff.radius * 0.45, color);
    }

    if active_location != Location::Overworld {
        return;
    }

    let time = universe.time;
    let left = cam.x - view_w_m * 0.5;
    let top = cam.y - view_h_m * 0.5;
    let mist = universe.weather.mist_intensity.max(0.18);
    for index in 0..7 {
        let mx = left + ((index as f32 * 5.7 + time * 0.35).rem_euclid(view_w_m + 8.0) - 4.0);
        let my = top + ((index as f32 * 3.1).rem_euclid(view_h_m));
        draw_ellipse(
            mx,
            my,
            4.2,
            1.1,
            0.0,
            Color::new(
                region.rain_ambient.0,
                region.rain_ambient.1,
                region.rain_ambient.2,
                0.07 * mist,
            ),
        );
    }

    if universe.weather.rain_intensity > 0.08 {
        let drops = (universe.weather.rain_intensity * 95.0) as i32;
        let wind_x = universe.weather.wind_vec.x.clamp(-3.0, 3.0) - 1.2;
        for index in 0..drops {
            let rx = left + ((index as f32 * 3.71 + time * wind_x * 2.2).rem_euclid(view_w_m));
            let ry = top + ((index as f32 * 2.19 + time * 14.5).rem_euclid(view_h_m));
            draw_line(
                rx,
                ry,
                rx + wind_x * 0.14,
                ry + 0.52,
                WEATHER_LINE_WIDTH_PX / PIXELS_PER_METER,
                Color::new(
                    region.rain_ambient.0,
                    region.rain_ambient.1,
                    region.rain_ambient.2,
                    0.38 * universe.weather.rain_intensity,
                ),
            );
        }
    }

    if universe.weather.snow_intensity > 0.08 {
        let flakes = (universe.weather.snow_intensity * 90.0) as i32;
        let wind_x = universe.weather.wind_vec.x.clamp(-2.0, 2.0);
        for index in 0..flakes {
            let x =
                left + ((index as f32 * 4.17 + time * (0.25 + wind_x * 0.08)).rem_euclid(view_w_m));
            let y = top + ((index as f32 * 2.67 + time * 2.2).rem_euclid(view_h_m));
            let size = 0.035 + (index % 3) as f32 * 0.012;
            draw_circle(
                x,
                y,
                size,
                Color::new(0.92, 0.96, 1.0, 0.72 * universe.weather.snow_intensity),
            );
        }
    }

    if universe.weather.season == Season::Winter || universe.weather.snow_intensity > 0.1 {
        let breath_color = Color::new(0.88, 0.94, 0.96, 0.22);
        for (index, entity) in universe.entities.iter().enumerate() {
            if entity.location != active_location
                || !entity.alive
                || !matches!(entity.kind, EntityKind::Player | EntityKind::Villager)
            {
                continue;
            }
            let phase = time * 1.7 + index as f32 * 1.91;
            let pulse = (phase.sin() * 0.5 + 0.5).powf(3.0);
            if pulse < 0.25 {
                continue;
            }
            let x = entity.pos.x + phase.sin() * 0.08;
            let y = entity.pos.y - 0.72 - pulse * 0.12;
            draw_circle(
                x,
                y,
                0.055 + pulse * 0.035,
                Color::new(
                    breath_color.r,
                    breath_color.g,
                    breath_color.b,
                    breath_color.a * pulse,
                ),
            );
        }
    }
}

pub fn draw_lightmap_pass(
    universe: &Universe,
    loader: &LdtkLoader,
    region: &RegionAtmosphere,
    z: i32,
    cam: Vec2,
    view_w_m: f32,
    view_h_m: f32,
) {
    let active_location = universe.player_location(PlayerId(1));
    let rain = universe.weather.rain_intensity;
    let snow = universe.weather.snow_intensity;
    let time = universe.time;
    let ambient = if active_location != Location::Overworld {
        Color::new(0.36, 0.34, 0.38, 1.0)
    } else {
        let time_ambient = ambient_for_time(region, universe.weather.time_of_day);
        let weather_ambient = mix_color(
            mix_color(time_ambient, region.rain_color(), rain),
            Color::new(0.82, 0.88, 0.96, 1.0),
            snow,
        );
        mix_color(
            weather_ambient,
            region.rain_color(),
            universe.weather.mist_intensity * 0.12,
        )
    };
    clear_background(ambient);

    if active_location == Location::Overworld && rain < 0.75 {
        let sun_strength = (1.0 - rain * 1.15).clamp(0.0, 1.0);
        let sway_x = (time * 0.65).sin() * 0.32;
        let sway_y = (time * 0.45).cos() * 0.16;
        let canopy_gaps = loader.light_markers(z, "CanopyGap");
        if !canopy_gaps.is_empty() {
            for (position, radius) in canopy_gaps {
                draw_circle(
                    position.x + sway_x,
                    position.y + sway_y,
                    radius,
                    region.sunbeam_outer_color(sun_strength),
                );
                draw_circle(
                    position.x + sway_x,
                    position.y + sway_y,
                    radius * 0.55,
                    region.sunbeam_core_color(sun_strength),
                );
            }
        } else {
            let gx0 = (cam.x - view_w_m * 0.6).div_euclid(9.0) as i32;
            let gx1 = (cam.x + view_w_m * 0.6).div_euclid(9.0) as i32;
            let gy0 = (cam.y - view_h_m * 0.6).div_euclid(7.0) as i32;
            let gy1 = (cam.y + view_h_m * 0.6).div_euclid(7.0) as i32;
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let hash = (gx.wrapping_mul(73856093) ^ gy.wrapping_mul(19349663)) as u32;
                    if hash % 3 == 0 {
                        let px = gx as f32 * 9.0 + (hash % 5) as f32 + sway_x;
                        let py = gy as f32 * 7.0 + ((hash / 5) % 4) as f32 + sway_y;
                        let radius = 2.4 + (hash % 3) as f32 * 0.7;
                        draw_circle(px, py, radius, region.sunbeam_outer_color(sun_strength));
                        draw_circle(
                            px,
                            py,
                            radius * 0.52,
                            region.sunbeam_core_color(sun_strength),
                        );
                    }
                }
            }
        }
    }

    let flicker = 1.0 + (time * 8.5).sin() * 0.06 + (time * 15.0).cos() * 0.03;
    for entity in &universe.entities {
        if entity.kind == EntityKind::Campfire && entity.alive && entity.location == active_location
        {
            draw_circle(
                entity.pos.x,
                entity.pos.y,
                4.6 * flicker,
                Color::new(1.0, 0.68, 0.34, 0.52),
            );
            draw_circle(
                entity.pos.x,
                entity.pos.y,
                2.2 * flicker,
                Color::new(1.0, 0.86, 0.58, 0.78),
            );
        }
    }
    if active_location == Location::Overworld {
        for (position, radius) in loader.light_markers(z, "Hearth") {
            draw_circle(
                position.x,
                position.y,
                radius * 1.4 * flicker,
                Color::new(1.0, 0.66, 0.32, 0.55),
            );
            draw_circle(
                position.x,
                position.y,
                radius * 0.65 * flicker,
                Color::new(1.0, 0.88, 0.60, 0.82),
            );
        }
    }
}

fn draw_text_ivory(text: &str, x: f32, y: f32, size: f32) {
    draw_text(
        text,
        x + 1.0,
        y + 1.0,
        size,
        Color::from_rgba(24, 18, 14, 255),
    );
    draw_text(text, x, y, size, Color::from_rgba(246, 242, 230, 255));
}

fn is_overhead_layer(layer: &LdtkLayer) -> bool {
    layer.identifier == "Overhead" || layer.identifier.starts_with("Overhead_")
}

fn draw_layer_tiles(level: &LdtkLevel, layer: &LdtkLayer, loader: &LdtkLoader) {
    let Some(tileset_path) = layer.tileset_rel_path.as_deref() else {
        return;
    };
    let Some(texture) = loader.texture_for_tileset(tileset_path) else {
        return;
    };
    let tile_size = layer.grid_size.max(1) as f32;
    let origin_x = (level.world_x + layer.offset_x) as f32 / PIXELS_PER_METER;
    let origin_y = (level.world_y + layer.offset_y) as f32 / PIXELS_PER_METER;
    for tile in layer.auto_layer_tiles.iter().chain(layer.grid_tiles.iter()) {
        draw_texture_ex(
            texture,
            origin_x + tile.px[0] / PIXELS_PER_METER,
            origin_y + tile.px[1] / PIXELS_PER_METER,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(
                    tile_size / PIXELS_PER_METER,
                    tile_size / PIXELS_PER_METER,
                )),
                source: Some(Rect::new(tile.src[0], tile.src[1], tile_size, tile_size)),
                flip_x: tile.f & 1 != 0,
                flip_y: tile.f & 2 != 0,
                ..Default::default()
            },
        );
    }
}

fn draw_tiles_for_pass(loader: &LdtkLoader, overhead: bool, z: i32) {
    for level in loader
        .project()
        .levels
        .iter()
        .filter(|level| level.world_depth == z)
    {
        let Some(layers) = level.layer_instances.as_ref() else {
            continue;
        };
        for layer in layers.iter().rev().filter(|layer| {
            layer.visible
                && matches!(layer.layer_type.as_str(), "Tiles" | "AutoLayer")
                && is_overhead_layer(layer) == overhead
        }) {
            draw_layer_tiles(level, layer, loader);
        }
    }
}

fn has_ground_tiles(loader: &LdtkLoader, z: i32) -> bool {
    loader
        .project()
        .levels
        .iter()
        .filter(|level| level.world_depth == z)
        .filter_map(|level| level.layer_instances.as_ref())
        .flatten()
        .any(|layer| {
            layer.visible
                && matches!(layer.layer_type.as_str(), "Tiles" | "AutoLayer")
                && !is_overhead_layer(layer)
                && (!layer.auto_layer_tiles.is_empty() || !layer.grid_tiles.is_empty())
        })
}

fn ground_tile_hash(tx: i32, ty: i32) -> u32 {
    let mut hash = (tx as u32).wrapping_mul(0x9E37_79B9) ^ (ty as u32).wrapping_mul(0x85EB_CA6B);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846C_A68B);
    hash ^ (hash >> 16)
}

fn ground_tile_color(terrain: TerrainKind, hash: u32) -> [u8; 3] {
    let (base, range) = match terrain {
        TerrainKind::LochWater => ([38, 67, 70], 7),
        TerrainKind::MudRoad => ([73, 52, 41], 8),
        TerrainKind::PeatBog => ([61, 48, 39], 7),
        TerrainKind::Heather => ([54, 59, 48], 8),
        TerrainKind::Meadow => ([43, 62, 46], 7),
        TerrainKind::GraniteCrag => ([77, 82, 79], 9),
        TerrainKind::TilledSoil => ([78, 59, 42], 8),
    };
    let variation = (hash % (range * 2 + 1)) as i16 - range as i16;
    [
        (base[0] as i16 + variation).clamp(0, 255) as u8,
        (base[1] as i16 + variation / 2).clamp(0, 255) as u8,
        (base[2] as i16 + variation / 3).clamp(0, 255) as u8,
    ]
}

fn draw_ground_pixel(tx: i32, ty: i32, x: u32, y: u32, width: u32, color: [u8; 3]) {
    let pixel = 1.0 / PIXELS_PER_METER;
    draw_rectangle(
        tx as f32 + x as f32 * pixel,
        ty as f32 + y as f32 * pixel,
        width as f32 * pixel,
        pixel,
        Color::from_rgba(color[0], color[1], color[2], 255),
    );
}

fn draw_ground_details(tx: i32, ty: i32, terrain: TerrainKind, hash: u32, time: f32) {
    match terrain {
        TerrainKind::Meadow | TerrainKind::Heather => {
            let fleck_color = if terrain == TerrainKind::Heather {
                [112, 82, 91]
            } else {
                [86, 103, 65]
            };
            for index in 0..1 + hash % 2 {
                let fleck_hash = ground_tile_hash(
                    tx.wrapping_add(index as i32 * 31),
                    ty.wrapping_sub(index as i32 * 17),
                );
                let size = 2 + fleck_hash % 2;
                let x = (fleck_hash >> 8) % (32 - size + 1);
                let y = (fleck_hash >> 16) % (32 - size + 1);
                let pixel = 1.0 / PIXELS_PER_METER;
                draw_rectangle(
                    tx as f32 + x as f32 * pixel,
                    ty as f32 + y as f32 * pixel,
                    size as f32 * pixel,
                    size as f32 * pixel,
                    Color::from_rgba(fleck_color[0], fleck_color[1], fleck_color[2], 255),
                );
            }
        }
        TerrainKind::MudRoad | TerrainKind::PeatBog | TerrainKind::TilledSoil => {
            for index in 0..2 {
                let grain_hash =
                    ground_tile_hash(tx.wrapping_add(index * 47), ty.wrapping_add(index * 29));
                let color = if grain_hash & 1 == 0 {
                    [104, 77, 55]
                } else {
                    [43, 37, 32]
                };
                let width = 1 + (grain_hash >> 24) % 2;
                draw_ground_pixel(
                    tx,
                    ty,
                    (grain_hash >> 8) % (32 - width + 1),
                    (grain_hash >> 16) % 32,
                    width,
                    color,
                );
            }
        }
        TerrainKind::LochWater if hash % 3 == 0 => {
            let phase = (time.max(0.0) * 1.5).floor() as i32;
            let x = ((hash >> 8) as i32 + phase).rem_euclid(30) as u32;
            let y = (hash >> 16) % 32;
            draw_ground_pixel(tx, ty, x, y, 2, [91, 124, 123]);
        }
        TerrainKind::LochWater => {}
        TerrainKind::GraniteCrag => {
            let x = (hash >> 8) % 32;
            let y = (hash >> 16) % 32;
            let color = if hash & 1 == 0 {
                [105, 108, 102]
            } else {
                [52, 61, 61]
            };
            draw_ground_pixel(tx, ty, x, y, 1, color);
        }
    }
}

fn draw_procedural_ground(
    world_stream: &WorldStream,
    camera: Vec2,
    view_width_m: f32,
    view_height_m: f32,
    time: f32,
) {
    let left = camera.x - view_width_m * 0.5;
    let right = camera.x + view_width_m * 0.5;
    let top = camera.y - view_height_m * 0.5;
    let bottom = camera.y + view_height_m * 0.5;
    let min_tx = left.floor() as i32;
    let max_tx = right.ceil() as i32 - 1;
    let min_ty = top.floor() as i32;
    let max_ty = bottom.ceil() as i32 - 1;

    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            let terrain = world_stream.terrain_at(vec2(tx as f32 + 0.5, ty as f32 + 0.5));
            let hash = ground_tile_hash(tx, ty);
            let color = ground_tile_color(terrain, hash);
            draw_rectangle(
                tx as f32,
                ty as f32,
                1.0,
                1.0,
                Color::from_rgba(color[0], color[1], color[2], 255),
            );
            draw_ground_details(tx, ty, terrain, hash, time);
        }
    }
}

pub fn draw_ground_pass(
    loader: &LdtkLoader,
    world_stream: &WorldStream,
    z: i32,
    camera: Vec2,
    view_width_m: f32,
    view_height_m: f32,
    time: f32,
) {
    draw_tiles_for_pass(loader, false, z);
    if !has_ground_tiles(loader, z) {
        draw_procedural_ground(world_stream, camera, view_width_m, view_height_m, time);
    }
}

fn sim_sprite_id(entity: &Entity) -> &'static str {
    match entity.kind {
        EntityKind::Player | EntityKind::Villager => "Highlander",
        EntityKind::Mount => "Horse",
        EntityKind::Deer => "RedStag",
        EntityKind::Campfire => "Campfire",
        EntityKind::Cairn | EntityKind::Corpse => "Artifact",
    }
}

fn debug_entity_box(position: Vec2, width: f32, height: f32) {
    draw_rectangle_lines(
        position.x,
        position.y,
        width,
        height,
        1.0 / PIXELS_PER_METER,
        Color::new(1.0, 0.1, 0.1, 0.85),
    );
}

fn draw_pack_sprite(
    loader: &LdtkLoader,
    pack: &EntityPack,
    position: Vec2,
    size: Vec2,
    debug_view: bool,
) {
    if let Some(texture) = loader.texture_for_sprite_by_path(&pack.sprite_path) {
        let source = select_sprite_frame(texture, pack.frame_size, Vec2::new(0.0, 1.0), 0.0, false);
        draw_texture_ex(
            texture,
            position.x,
            position.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                source: Some(source),
                ..Default::default()
            },
        );
        if debug_view {
            debug_entity_box(position, size.x, size.y);
        }
    } else if debug_view {
        debug_entity_box(position, size.x, size.y);
    }
}

pub fn select_sprite_frame(
    texture: &Texture2D,
    frame_size: (u32, u32),
    facing: Vec2,
    walk_phase: f32,
    moving: bool,
) -> Rect {
    let frame_width = frame_size.0.max(1);
    let frame_height = frame_size.1.max(1);
    let cols = (texture.width() as u32 / frame_width).max(1);
    let rows = (texture.height() as u32 / frame_height).max(1);

    let row = if facing.y >= facing.x.abs() {
        0
    } else if -facing.y >= facing.x.abs() {
        1
    } else if -facing.x > facing.y.abs() {
        2
    } else if facing.x > facing.y.abs() {
        3
    } else {
        0
    }
    .min(rows - 1);

    let col = if moving && cols >= 4 {
        let frame_count = cols - 1;
        1 + ((walk_phase.rem_euclid(std::f32::consts::TAU) * frame_count as f32
            / std::f32::consts::TAU)
            .floor() as u32
            % frame_count)
    } else {
        0
    }
    .min(cols - 1);

    Rect::new(
        (col * frame_width) as f32,
        (row * frame_height) as f32,
        frame_width as f32,
        frame_height as f32,
    )
}

fn entity_pack_position(
    level: &LdtkLevel,
    layer: &LdtkLayer,
    instance: &LdtkEntityInstance,
) -> Vec2 {
    Vec2::new(
        (level.world_x + layer.offset_x) as f32 / PIXELS_PER_METER
            + instance.px[0] / PIXELS_PER_METER,
        (level.world_y + layer.offset_y) as f32 / PIXELS_PER_METER
            + instance.px[1] / PIXELS_PER_METER,
    )
}

pub fn draw_y_sorted_pass(universe: &Universe, loader: &LdtkLoader, z: i32, debug_view: bool) {
    let ldtk_entities = loader.entity_instances(z);
    let mut render_items = Vec::new();
    let mut order = 0;
    for (index, entity) in universe.entities.iter().enumerate() {
        if entity.location == crate::world::Location::Overworld && !entity.buried {
            let identifier = sim_sprite_id(entity);
            if let Some(pack) = loader.entity_pack(identifier) {
                render_items.push(SortedRenderItem {
                    foot_y: entity.pos.y + pack.foot_offset_y,
                    order,
                    item: RenderItem::SimEntity(index),
                });
                order += 1;
            }
        }
    }
    for (index, entity) in ldtk_entities.iter().enumerate() {
        let level = &loader.project().levels[entity.level_index];
        let layer = &level.layer_instances.as_ref().unwrap()[entity.layer_index];
        let instance = &layer.entity_instances[entity.entity_index];
        if let Some(pack) = loader.entity_pack(&instance.identifier) {
            render_items.push(SortedRenderItem {
                foot_y: entity.world_y + pack.foot_offset_y,
                order,
                item: RenderItem::LdtkEntity(index),
            });
            order += 1;
        }
    }
    for (index, tree) in universe.world_stream.foliage().iter().enumerate() {
        let pack_name = if tree.chopped { "Rock" } else { "ScotsPine" };
        if let Some(pack) = loader.entity_pack(pack_name) {
            render_items.push(SortedRenderItem {
                foot_y: tree.position.y + pack.foot_offset_y,
                order,
                item: RenderItem::Foliage(index),
            });
            order += 1;
        }
    }
    sort_render_items(&mut render_items);

    for sorted in render_items {
        match sorted.item {
            RenderItem::SimEntity(index) => {
                let entity = &universe.entities[index];
                let pack_name = sim_sprite_id(entity);
                let Some(pack) = loader.entity_pack(pack_name) else {
                    continue;
                };
                let size = vec2(
                    pack.frame_size.0 as f32 / PIXELS_PER_METER,
                    pack.frame_size.1 as f32 / PIXELS_PER_METER,
                );
                let position = vec2(entity.pos.x - size.x * 0.5, entity.pos.y - size.y);
                if matches!(
                    entity.kind,
                    EntityKind::Player
                        | EntityKind::Villager
                        | EntityKind::Mount
                        | EntityKind::Deer
                ) {
                    draw_ellipse(
                        entity.pos.x,
                        entity.pos.y - 0.08,
                        size.x * 0.32,
                        0.12,
                        0.0,
                        Color::new(0.025, 0.035, 0.03, 0.22),
                    );
                }
                let moving = entity.vel.length_squared() > 0.08;
                let hooded_cloak_texture = if entity.posture.hood_up {
                    pack.hooded_cloak_layer_path
                        .as_deref()
                        .and_then(|path| loader.texture_for_sprite_by_path(path))
                } else {
                    None
                };
                let hooded_sprite_texture =
                    if entity.posture.hood_up && hooded_cloak_texture.is_none() {
                        pack.hooded_sprite_path
                            .as_deref()
                            .and_then(|path| loader.texture_for_sprite_by_path(path))
                    } else {
                        None
                    };

                let base_texture = hooded_sprite_texture
                    .or_else(|| loader.texture_for_sprite_by_path(&pack.sprite_path));
                if let Some(texture) = base_texture {
                    let source = select_sprite_frame(
                        texture,
                        pack.frame_size,
                        entity.facing,
                        entity.walk_phase,
                        moving,
                    );
                    draw_texture_ex(
                        texture,
                        position.x,
                        position.y,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(size),
                            source: Some(source),
                            ..Default::default()
                        },
                    );
                }

                if let Some(path) = &pack.kilt_layer_path {
                    if let Some(texture) = loader.texture_for_sprite_by_path(path) {
                        let source = select_sprite_frame(
                            texture,
                            pack.frame_size,
                            entity.facing,
                            entity.walk_phase,
                            moving,
                        );
                        draw_texture_ex(
                            texture,
                            position.x,
                            position.y,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(size),
                                source: Some(source),
                                ..Default::default()
                            },
                        );
                    }
                }

                let cloak_texture = if entity.posture.hood_up {
                    hooded_cloak_texture
                } else {
                    pack.cloak_layer_path
                        .as_deref()
                        .and_then(|path| loader.texture_for_sprite_by_path(path))
                };
                if let Some(texture) = cloak_texture {
                    let source = select_sprite_frame(
                        texture,
                        pack.frame_size,
                        entity.facing,
                        entity.walk_phase,
                        moving,
                    );
                    draw_texture_ex(
                        texture,
                        position.x,
                        position.y,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(size),
                            source: Some(source),
                            ..Default::default()
                        },
                    );
                }

                if debug_view {
                    debug_entity_box(position, size.x, size.y);
                }
                if entity.posture.hood_up
                    && hooded_cloak_texture.is_none()
                    && hooded_sprite_texture.is_none()
                    && matches!(entity.kind, EntityKind::Player | EntityKind::Villager)
                {
                    draw_rectangle(
                        entity.pos.x - size.x * 0.24,
                        position.y + size.y * 0.04,
                        size.x * 0.48,
                        size.y * 0.24,
                        Color::from_rgba(40, 66, 49, 235),
                    );
                }
                if entity.posture.warming_hands {
                    let pulse = (universe.time * 4.5).sin() * 0.05 + 0.22;
                    draw_circle(
                        entity.pos.x,
                        entity.pos.y - size.y * 0.45,
                        pulse,
                        Color::new(1.0, 0.78, 0.42, 0.45),
                    );
                }
            }
            RenderItem::LdtkEntity(index) => {
                let entry = &ldtk_entities[index];
                let level = &loader.project().levels[entry.level_index];
                let layer = &level.layer_instances.as_ref().unwrap()[entry.layer_index];
                let instance = &layer.entity_instances[entry.entity_index];
                let Some(pack) = loader.entity_pack(&instance.identifier) else {
                    continue;
                };
                let size = vec2(
                    instance.width / PIXELS_PER_METER,
                    instance.height / PIXELS_PER_METER,
                );
                let pivot = vec2(instance.pivot[0], instance.pivot[1]);
                let position = vec2(
                    entry.world_x - size.x * pivot.x,
                    entry.world_y - size.y * pivot.y,
                );
                draw_pack_sprite(loader, pack, position, size, debug_view);
            }
            RenderItem::Foliage(index) => {
                let tree = universe.world_stream.foliage()[index];
                let pack = if tree.chopped {
                    loader.entity_pack("Rock")
                } else {
                    loader.entity_pack("ScotsPine")
                };
                if let Some(pack) = pack {
                    let size = vec2(
                        pack.frame_size.0 as f32 / PIXELS_PER_METER,
                        pack.frame_size.1 as f32 / PIXELS_PER_METER,
                    ) * tree.scale;
                    draw_ellipse(
                        tree.position.x,
                        tree.position.y,
                        size.x * 0.35,
                        0.14,
                        0.0,
                        Color::new(0.025, 0.035, 0.03, 0.18),
                    );
                    draw_pack_sprite(
                        loader,
                        pack,
                        vec2(tree.position.x - size.x * 0.5, tree.position.y - size.y),
                        size,
                        debug_view,
                    );
                }
            }
        }
    }
}

pub fn draw_overhead_pass(loader: &LdtkLoader, z: i32) {
    draw_tiles_for_pass(loader, true, z);
}

fn draw_ui_tile(
    texture: &Texture2D,
    column: u16,
    row: u16,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    tint: Color,
) {
    draw_texture_ex(
        texture,
        x.round(),
        y.round(),
        tint,
        DrawTextureParams {
            dest_size: Some(vec2(width, height)),
            source: Some(Rect::new(
                (column * 16) as f32,
                (row * 16) as f32,
                16.0,
                16.0,
            )),
            ..Default::default()
        },
    );
}

fn draw_panel(texture: &Texture2D, x: f32, y: f32, width: f32, height: f32, tint: Color) {
    for row in 0..3 {
        for column in 0..3 {
            let dx = match column {
                0 => x,
                1 => x + 16.0,
                _ => x + width - 16.0,
            };
            let dy = match row {
                0 => y,
                1 => y + 16.0,
                _ => y + height - 16.0,
            };
            draw_ui_tile(
                texture,
                column,
                row + 1,
                dx,
                dy,
                if column == 1 {
                    (width - 32.0).max(1.0)
                } else {
                    16.0
                },
                if row == 1 {
                    (height - 32.0).max(1.0)
                } else {
                    16.0
                },
                tint,
            );
        }
    }
}

fn item_icon(item: InventoryItem) -> u16 {
    match item {
        InventoryItem::Dirk => 0,
        InventoryItem::Hatchet => 1,
        InventoryItem::Whistle => 2,
        InventoryItem::Bandage => 3,
        InventoryItem::PeatFire => 4,
    }
}

fn wrap_text(text: &str, max_width: f32, font_size: u16) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if measure_text(&candidate, None, font_size, 1.0).width > max_width && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn draw_ui_pass(universe: &Universe, assets: &AssetBank, map_open: bool) {
    draw_panel(
        &assets.ui,
        6.0,
        6.0,
        164.0,
        54.0,
        Color::new(0.92, 0.88, 0.78, 0.96),
    );
    draw_text_ivory("Oir na Giùthsaich", 14.0, 18.0, 10.0);
    draw_text_ivory("The Pine-Edge", 14.0, 29.0, 10.0);
    if let Some(player) = universe
        .player_index(PlayerId(1))
        .map(|index| &universe.entities[index])
    {
        let bars = [
            ("Health", player.health / 100.0),
            ("Stamina", player.stamina / 100.0),
        ];
        for (index, (label, amount)) in bars.iter().enumerate() {
            let y = 35.0 + index as f32 * 12.0;
            draw_text_ivory(label, 14.0, y + 7.0, 9.0);
            draw_rectangle(58.0, y, 102.0, 6.0, Color::new(0.15, 0.16, 0.15, 0.95));
            let color = match index {
                0 => Color::new(0.76, 0.22, 0.20, 1.0),
                1 => Color::new(0.32, 0.68, 0.36, 1.0),
                _ => Color::new(0.32, 0.68, 0.36, 1.0),
            };
            draw_rectangle(58.0, y, 102.0 * amount.clamp(0.0, 1.0), 6.0, color);
        }
    }

    let slot_width = 60.0;
    let slot_height = 48.0;
    let gap = 7.0;
    let bar_width = slot_width * 5.0 + gap * 4.0;
    let left = (CANVAS_WIDTH - bar_width) * 0.5;
    let top = CANVAS_HEIGHT - slot_height - 8.0;
    let inventory = universe
        .player_index(PlayerId(1))
        .and_then(|index| universe.entities[index].inventory.as_ref());
    for slot in 0..5 {
        let x = left + slot as f32 * (slot_width + gap);
        let selected = inventory
            .map(|inventory| inventory.selected_slot == slot)
            .unwrap_or(false);
        draw_panel(
            &assets.ui,
            x,
            top,
            slot_width,
            slot_height,
            if selected {
                Color::new(1.0, 0.86, 0.62, 1.0)
            } else {
                Color::new(0.72, 0.69, 0.61, 0.94)
            },
        );
        if let Some(stack) = inventory.and_then(|inventory| inventory.hotbar[slot]) {
            draw_ui_tile(
                &assets.ui,
                item_icon(stack.item),
                0,
                x + 20.0,
                top + 3.0,
                20.0,
                20.0,
                WHITE,
            );
            let label = match stack.item {
                InventoryItem::Dirk => "Dirk",
                InventoryItem::Hatchet => "Hatchet",
                InventoryItem::Whistle => "Whistle",
                InventoryItem::Bandage => "Bandage",
                InventoryItem::PeatFire => "Peat Kit",
            };
            let label_width = measure_text(label, None, 11, 1.0).width;
            draw_text_ivory(
                label,
                x + (slot_width - label_width) * 0.5,
                top + 42.0,
                11.0,
            );
        }
        draw_text_ivory(&(slot + 1).to_string(), x + 4.0, top + 13.0, 10.0);
    }

    if !universe.status_toast.0.is_empty() && universe.status_toast.1 > 0.0 {
        let message = &universe.status_toast.0;
        let width = (measure_text(message, None, 13, 1.0).width + 20.0).min(460.0);
        draw_panel(
            &assets.ui,
            (CANVAS_WIDTH - width) * 0.5,
            7.0,
            width,
            28.0,
            Color::new(0.94, 0.88, 0.72, 0.95),
        );
        draw_text_ivory(message, (CANVAS_WIDTH - width) * 0.5 + 10.0, 26.0, 13.0);
    }
    if let Some(utterance) = universe.utterances.last() {
        let box_width = 456.0;
        let box_height = 84.0;
        let box_x = (CANVAS_WIDTH - box_width) * 0.5;
        let box_y = CANVAS_HEIGHT - box_height - 62.0;
        draw_panel(
            &assets.ui,
            box_x,
            box_y,
            box_width,
            box_height,
            Color::new(0.20, 0.18, 0.15, 0.96),
        );
        draw_text(
            &utterance.speaker,
            box_x + 12.0,
            box_y + 16.0,
            12.0,
            Color::from_rgba(226, 194, 132, 255),
        );
        let mut cursor_y = box_y + 32.0;
        for line in wrap_text(&utterance.gaelic, box_width - 24.0, 11)
            .iter()
            .take(2)
        {
            draw_text(
                line,
                box_x + 12.0,
                cursor_y,
                11.0,
                Color::from_rgba(166, 214, 178, 255),
            );
            cursor_y += 12.0;
        }
        cursor_y += 4.0;
        for line in wrap_text(&utterance.text, box_width - 24.0, 11)
            .iter()
            .take(2)
        {
            draw_text_ivory(line, box_x + 12.0, cursor_y, 11.0);
            cursor_y += 12.0;
        }
    }
    if map_open {
        draw_world_map_overlay(universe);
    }
}

fn draw_world_map_overlay(universe: &Universe) {
    draw_rectangle(
        0.0,
        0.0,
        CANVAS_WIDTH,
        CANVAS_HEIGHT,
        Color::new(0.025, 0.035, 0.04, 0.9),
    );
    let map_size = 228.0;
    let map_x = (CANVAS_WIDTH - map_size) * 0.5;
    let map_y = (CANVAS_HEIGHT - map_size) * 0.5;
    draw_text_ivory("WORLD MAP  |  [M] close", map_x, map_y - 4.0, 12.0);
    draw_texture_ex(
        universe.world_stream.blueprint.map_texture(),
        map_x,
        map_y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(map_size, map_size)),
            ..Default::default()
        },
    );
    draw_rectangle_lines(map_x, map_y, map_size, map_size, 1.0, WHITE);
    if let Some(index) = universe.player_index(PlayerId(1)) {
        let chunk = universe
            .world_stream
            .blueprint
            .chunk_for_local(universe.entities[index].pos);
        let dimensions = universe.world_stream.blueprint.anchors.world_chunks;
        let marker = vec2(
            map_x + (chunk.0 as f32 + 0.5) / dimensions.0 as f32 * map_size,
            map_y + (chunk.1 as f32 + 0.5) / dimensions.1 as f32 * map_size,
        );
        draw_circle(marker.x, marker.y, 3.0, Color::new(0.95, 0.24, 0.12, 1.0));
        draw_circle_lines(marker.x, marker.y, 4.0, 1.0, WHITE);
    }
}

#[cfg(test)]
mod region_atmosphere_tests {
    use super::{ambient_for_time, RegionAtmosphere};

    #[test]
    fn starting_region_data_loads_with_bright_full_rgb_values() {
        let region: RegionAtmosphere =
            ron::from_str(include_str!("../assets/data/regions.ron")).unwrap();

        assert_eq!(region.id, "pine_edge");
        assert_eq!(region.gaelic_name, "Oir na Giùthsaich");
        assert_eq!(region.english_name, "The Pine-Edge");
        assert_eq!(region.day_ambient, (0.95, 0.97, 0.92));
        assert_eq!(region.rain_ambient, (0.74, 0.80, 0.86));
        assert_eq!(region.dusk_ambient, (0.78, 0.66, 0.58));
        assert_eq!(region.night_ambient, (0.34, 0.40, 0.52));
        assert_eq!(region.sunbeam_outer, (1.0, 0.97, 0.82, 0.24));
        assert_eq!(region.sunbeam_core, (1.0, 0.99, 0.88, 0.32));
        assert_eq!(region.clear_bg, (88, 138, 76));
        assert_eq!(ambient_for_time(&region, 0.5).r, 0.95);
    }
}

#[cfg(test)]
mod procedural_ground_tests {
    use super::{ground_tile_color, ground_tile_hash};
    use crate::world::TerrainKind;

    #[test]
    fn procedural_ground_palette_is_deterministic_and_terrain_specific() {
        let hash = ground_tile_hash(-12, 39);

        assert_eq!(hash, ground_tile_hash(-12, 39));
        assert_eq!(
            ground_tile_color(TerrainKind::Meadow, hash),
            ground_tile_color(TerrainKind::Meadow, hash)
        );
        assert_ne!(
            ground_tile_color(TerrainKind::Meadow, hash),
            ground_tile_color(TerrainKind::LochWater, hash)
        );
    }
}
