use crate::config::UniverseConfig;
use crate::entities::{AiGoal, Entity, EntityKind, PlayerId};
use crate::sim::Universe;
use crate::world::{
    fbm2d, hash2d, road_center_x, sample_terrain, value_noise, BuildingRecord, Location, PropKind,
    Rng, TerrainKind, TraceKind,
};
use macroquad::prelude::*;

const TERRAIN_MIN_X: f32 = -48.0;
const TERRAIN_MIN_Y: f32 = -48.0;
const TERRAIN_W: f32 = 100.0;
const TERRAIN_H: f32 = 96.0;

pub const VERT_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    color = color0 / 255.0;
    uv = texcoord;
}
"#;

pub const FRAG_SHADER: &str = r#"#version 100
precision mediump float;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
uniform float u_time;
uniform float u_warmth;
uniform float u_darkness;
uniform float u_mist;
uniform float u_res_x;
uniform float u_res_y;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

void main() {
    vec2 px = vec2(1.0 / max(u_res_x, 1.0), 1.0 / max(u_res_y, 1.0));

    vec4 c0 = texture2D(Texture, uv);
    vec4 c1 = texture2D(Texture, uv + vec2(px.x * 1.1, px.y * 0.4));
    vec4 c2 = texture2D(Texture, uv - vec2(px.x * 0.7, px.y * 0.9));
    vec3 col = (c0.rgb * 0.72 + c1.rgb * 0.15 + c2.rgb * 0.13);

    float weave = sin(uv.x * u_res_x * 0.75) * sin(uv.y * u_res_y * 0.75) * 0.011;
    float grain = (hash(floor(uv * vec2(u_res_x, u_res_y) * 0.5)) - 0.5) * 0.016;
    col += vec3(weave + grain);

    vec3 warm_tint = vec3(1.06, 1.01, 0.91);
    vec3 bleak_tint = vec3(0.68, 0.75, 0.85);
    float luma = dot(col, vec3(0.299, 0.587, 0.114));
    vec3 desat = mix(col, vec3(luma), u_darkness * 0.50);
    vec3 grade = mix(warm_tint, bleak_tint, u_darkness);
    col = desat * grade * (0.60 + u_warmth * 0.48);

    float mist_wave = 0.5 + 0.5 * sin(uv.x * 6.5 + u_time * 0.30 + sin(uv.y * 4.5));
    vec3 mist_col = mix(vec3(0.80, 0.84, 0.78), vec3(0.42, 0.48, 0.54), u_darkness);
    col = mix(col, mist_col, u_mist * 0.24 * (0.5 + 0.5 * mist_wave));

    vec2 dv = uv - vec2(0.5);
    float vig = 1.0 - dot(dv, dv) * (0.38 + u_darkness * 0.30);
    col *= clamp(vig, 0.38, 1.04);

    gl_FragColor = vec4(col, 1.0) * color;
}
"#;

pub struct PaintedAtlas {
    pub terrain_map: Texture2D,
    pub roof: Texture2D,
    pub floor: Texture2D,
    pub stone_wall: Texture2D,
    pub pine: Texture2D,
    pub soft_glow: Texture2D,
    pub horse: Texture2D,
    pub deer: Texture2D,
    pub spr_player: Texture2D,
    pub spr_villager: Texture2D,
    pub spr_redcoat: Texture2D,
    pub post_mat: Material,
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn put_px(img: &mut Image, x: i32, y: i32, col: Color) {
    if x >= 0 && y >= 0 && x < img.width as i32 && y < img.height as i32 {
        img.set_pixel(x as u32, y as u32, col);
    }
}

fn fill_ellipse(img: &mut Image, cx: f32, cy: f32, rx: f32, ry: f32, col: Color) {
    let x0 = (cx - rx - 1.0).floor() as i32;
    let x1 = (cx + rx + 1.0).ceil() as i32;
    let y0 = (cy - ry - 1.0).floor() as i32;
    let y1 = (cy + ry + 1.0).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = (x as f32 - cx) / rx.max(0.1);
            let dy = (y as f32 - cy) / ry.max(0.1);
            if dx * dx + dy * dy <= 1.0 {
                put_px(img, x, y, col);
            }
        }
    }
}

fn fill_rect(img: &mut Image, x0: i32, y0: i32, w: i32, h: i32, col: Color) {
    for y in y0..(y0 + h) {
        for x in x0..(x0 + w) {
            put_px(img, x, y, col);
        }
    }
}

fn add_outline(img: &mut Image, outline_col: Color) {
    let w = img.width as i32;
    let h = img.height as i32;
    let mut to_outline = Vec::new();
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            if img.get_pixel(x as u32, y as u32).a < 0.1 {
                let near_solid = img.get_pixel((x - 1) as u32, y as u32).a > 0.5
                    || img.get_pixel((x + 1) as u32, y as u32).a > 0.5
                    || img.get_pixel(x as u32, (y - 1) as u32).a > 0.5
                    || img.get_pixel(x as u32, (y + 1) as u32).a > 0.5;
                if near_solid {
                    to_outline.push((x, y));
                }
            }
        }
    }
    for (x, y) in to_outline {
        put_px(img, x, y, outline_col);
    }
}

fn paint_continuous_terrain(seed: u64) -> Texture2D {
    let w: u16 = 500;
    let h: u16 = 480;
    let mut img = Image::gen_image_color(w, h, BLACK);

    for py in 0..h {
        let wy = TERRAIN_MIN_Y + (py as f32 / h as f32) * TERRAIN_H;
        let rx = road_center_x(wy);
        for px in 0..w {
            let wx = TERRAIN_MIN_X + (px as f32 / w as f32) * TERRAIN_W;

            let n = fbm2d(seed, wx * 0.025, wy * 0.025, 3);
            let n_e = fbm2d(seed, (wx + 0.8) * 0.025, wy * 0.025, 2);
            let n_s = fbm2d(seed, wx * 0.025, (wy + 0.8) * 0.025, 2);
            let detail = fbm2d(seed.wrapping_add(77), wx * 0.09, wy * 0.09, 2);
            let micro = value_noise(seed.wrapping_add(313), wx * 0.65, wy * 0.65);

            let loch_edge = -28.0 + (wy * 0.045).sin() * 6.5 + (n - 0.5) * 8.0;
            let east_slope = ((wx + 25.0) * 0.14).max(0.0);
            let elev = east_slope + (n - 0.45) * 8.0;

            // Sunlit NW hillshade across Highland contours
            let slope_x = (n_e - n) * 4.5 + 0.06;
            let slope_y = (n_s - n) * 4.5;
            let hillshade = (0.96 - (slope_x * 0.7 + slope_y * 0.7)).clamp(0.76, 1.16);

            // 1. Lush Highland Meadow base
            let meadow_a = [0.27, 0.44, 0.21];
            let meadow_b = [0.39, 0.53, 0.25];
            let mut rgb = lerp_rgb(meadow_a, meadow_b, (micro * 0.7 + detail * 0.3).clamp(0.0, 1.0));

            // 2. Smooth Purple Heather patches
            let heather_w = smoothstep(0.47, 0.58, detail);
            let heather_col = lerp_rgb([0.36, 0.33, 0.29], [0.55, 0.34, 0.52], micro);
            rgb = lerp_rgb(rgb, heather_col, heather_w);

            // 3. Damp Mossy Peat Bog hollows
            let bog_w = smoothstep(0.39, 0.27, n) * smoothstep(5.5, -1.5, wx);
            let bog_col = lerp_rgb([0.19, 0.25, 0.16], [0.27, 0.30, 0.19], micro);
            rgb = lerp_rgb(rgb, bog_col, bog_w);

            // 4. Eastern Granite Crags
            let crag_w = smoothstep(9.5, 12.2, elev);
            let crag_col = lerp_rgb([0.41, 0.42, 0.40], [0.58, 0.57, 0.53], micro);
            rgb = lerp_rgb(rgb, crag_col, crag_w);

            // 5. Wade's Military Road with packed earth edges & twin cart-wheel ruts
            let road_d = (wx - rx).abs();
            let road_edge = smoothstep(3.15, 1.85, road_d + (micro - 0.5) * 0.7);
            let rut = smoothstep(0.38, 0.05, (road_d - 0.85).abs());
            let road_col = lerp_rgb([0.45, 0.36, 0.26], [0.31, 0.24, 0.17], rut * 0.75 + micro * 0.25);
            rgb = lerp_rgb(rgb, road_col, road_edge);

            // 6. Western Sea Loch + Pebble Shoreline + Foam Edge
            let shore_dist = wx - loch_edge;
            let bank_w = smoothstep(2.4, 0.1, shore_dist);
            let bank_col = lerp_rgb([0.48, 0.45, 0.38], [0.36, 0.35, 0.31], micro);
            rgb = lerp_rgb(rgb, bank_col, bank_w);

            let water_w = smoothstep(0.35, -0.55, shore_dist);
            if water_w > 0.01 {
                let depth = (-shore_dist * 0.12).clamp(0.0, 1.0);
                let w_col = lerp_rgb([0.22, 0.42, 0.49], [0.11, 0.23, 0.34], depth);
                let foam = smoothstep(0.45, 0.0, (shore_dist + 0.1).abs()) * 0.35;
                let w_final = lerp_rgb(w_col, [0.72, 0.80, 0.82], foam);
                rgb = lerp_rgb(rgb, w_final, water_w);
            }

            let shade = if water_w > 0.5 { 1.0 } else { hillshade };
            img.set_pixel(
                px as u32,
                py as u32,
                Color::new(
                    (rgb[0] * shade).clamp(0.0, 1.0),
                    (rgb[1] * shade).clamp(0.0, 1.0),
                    (rgb[2] * shade).clamp(0.0, 1.0),
                    1.0,
                ),
            );
        }
    }

    // Painterly bristle strokes directly across the continuous valley canvas
    let mut rng = Rng::new(seed.wrapping_add(9001));
    for _ in 0..4200 {
        let sx = rng.range_f32(2.0, (w - 3) as f32);
        let sy = rng.range_f32(2.0, (h - 3) as f32);
        let base = img.get_pixel(sx as u32, sy as u32);
        let lift = rng.range_f32(0.89, 1.13);
        let c = Color::new(
            (base.r * lift).clamp(0.0, 1.0),
            (base.g * lift).clamp(0.0, 1.0),
            (base.b * lift).clamp(0.0, 1.0),
            1.0,
        );
        let ang = rng.range_f32(-0.55, 0.55);
        let len = rng.range_f32(2.5, 5.5) as i32;
        for s in 0..len {
            let px = (sx + ang.cos() * s as f32) as i32;
            let py = (sy + ang.sin() * s as f32) as i32;
            put_px(&mut img, px, py, c);
        }
    }

    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_radial_glow(size: u16) -> Texture2D {
    let mut img = Image::gen_image_color(size, size, Color::new(1.0, 1.0, 1.0, 0.0));
    let half = size as f32 * 0.5;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5 - half) / half;
            let dy = (y as f32 + 0.5 - half) / half;
            let r = (dx * dx + dy * dy).sqrt();
            let a = (1.0 - r).max(0.0).powf(2.2);
            img.set_pixel(x as u32, y as u32, Color::new(1.0, 1.0, 1.0, a));
        }
    }
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_tile(
    size: u16,
    seed: u64,
    base: [f32; 3],
    accent: [f32; 3],
    highlight: [f32; 3],
    stroke_count: usize,
    plank_lines: bool,
) -> Texture2D {
    let mut img = Image::gen_image_color(size, size, Color::new(base[0], base[1], base[2], 1.0));
    let mut rng = Rng::new(seed);

    for y in 0..size {
        for x in 0..size {
            let n = fbm2d(seed, x as f32 * 0.08, y as f32 * 0.08, 3);
            let r = (base[0] * (0.86 + n * 0.28) + accent[0] * n * 0.16).clamp(0.0, 1.0);
            let g = (base[1] * (0.86 + n * 0.28) + accent[1] * n * 0.16).clamp(0.0, 1.0);
            let b = (base[2] * (0.86 + n * 0.28) + accent[2] * n * 0.16).clamp(0.0, 1.0);
            img.set_pixel(x as u32, y as u32, Color::new(r, g, b, 1.0));
        }
    }

    for _ in 0..stroke_count {
        let cx = rng.range_f32(0.0, size as f32);
        let cy = rng.range_f32(0.0, size as f32);
        let len = rng.range_f32(8.0, 18.0);
        let ang = rng.range_f32(-0.14, 0.14);
        let pick = rng.next_f32();
        let c = if pick < 0.5 { accent } else if pick < 0.85 { base } else { highlight };
        let bright = rng.range_f32(0.88, 1.12);
        for s in 0..(len as i32) {
            let px = (cx + ang.cos() * s as f32).rem_euclid(size as f32) as u32;
            let py = (cy + ang.sin() * s as f32).rem_euclid(size as f32) as u32;
            img.set_pixel(
                px,
                py,
                Color::new(
                    (c[0] * bright).clamp(0.0, 1.0),
                    (c[1] * bright).clamp(0.0, 1.0),
                    (c[2] * bright).clamp(0.0, 1.0),
                    1.0,
                ),
            );
        }
    }

    if plank_lines {
        let seam = Color::new(base[0] * 0.45, base[1] * 0.42, base[2] * 0.40, 1.0);
        for row in 0..4u32 {
            let y = row * (size as u32 / 4);
            for x in 0..size as u32 {
                img.set_pixel(x, y, seam);
            }
            let joint_x = ((row * 23 + 11) % size as u32) as u32;
            for py in y..(y + size as u32 / 4).min(size as u32) {
                img.set_pixel(joint_x, py, seam);
            }
        }
    }

    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_masonry(size: u16) -> Texture2D {
    let mut img = Image::gen_image_color(size, size, Color::new(0.22, 0.21, 0.19, 1.0));
    let mut rng = Rng::new(4109);
    let rows = 5;
    let rh = size / rows;
    for r in 0..rows {
        let mut x = if r % 2 == 0 { 0 } else { 6 };
        while x < size {
            let bw = (10 + (rng.next_u64() % 9) as u16).min(size - x);
            let shade = rng.range_f32(0.38, 0.54);
            let warm = rng.range_f32(-0.02, 0.03);
            let col = Color::new(shade + warm, shade, shade - warm * 0.4, 1.0);
            let hi = Color::new((col.r + 0.07).min(1.0), (col.g + 0.07).min(1.0), (col.b + 0.06).min(1.0), 1.0);
            for py in (r * rh + 1)..((r + 1) * rh).saturating_sub(1).min(size) {
                for px in (x + 1)..(x + bw).saturating_sub(1).min(size) {
                    img.set_pixel(px as u32, py as u32, if py == r * rh + 1 { hi } else { col });
                }
            }
            x += bw;
        }
    }
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_pine_34(size: u16) -> Texture2D {
    let mut img = Image::gen_image_color(size, size, Color::new(0.0, 0.0, 0.0, 0.0));
    let mut rng = Rng::new(7741);
    let s = size as f32;

    // Visible Scottish pine reddish-brown trunk at bottom center
    fill_rect(&mut img, (s * 0.45) as i32, (s * 0.62) as i32, (s * 0.11) as i32, (s * 0.30) as i32, Color::new(0.32, 0.20, 0.13, 1.0));
    fill_rect(&mut img, (s * 0.46) as i32, (s * 0.62) as i32, (s * 0.04) as i32, (s * 0.28) as i32, Color::new(0.44, 0.28, 0.18, 1.0));

    // Three overlapping foliage tiers with sunlit NW highlight & shaded SE underside
    let tiers = [
        (s * 0.50, s * 0.58, s * 0.37, s * 0.24, 0.0),
        (s * 0.50, s * 0.42, s * 0.31, s * 0.22, 0.05),
        (s * 0.50, s * 0.27, s * 0.21, s * 0.18, 0.11),
    ];
    for (cx, cy, rx, ry, lift) in tiers {
        for _ in 0..240 {
            let r = rng.next_f32().sqrt();
            let a = rng.range_f32(0.0, std::f32::consts::TAU);
            let px = cx + a.cos() * rx * r;
            let py = cy + a.sin() * ry * r;
            let sun = (((cx - px) / rx) * 0.5 + ((cy - py) / ry) * 0.5).clamp(-0.4, 1.0);
            let col = Color::new(
                (0.09 + lift + sun.max(0.0) * 0.11).clamp(0.05, 0.36),
                (0.22 + lift + sun.max(0.0) * 0.22 + rng.range_f32(-0.03, 0.04)).clamp(0.12, 0.56),
                (0.13 + lift * 0.6 + sun.max(0.0) * 0.07).clamp(0.06, 0.32),
                0.98,
            );
            fill_ellipse(&mut img, px, py, 2.2, 1.8, col);
        }
    }

    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_character(coat: Color, accent: Color, hat: Color, redcoat_belts: bool) -> Texture2D {
    let mut img = Image::gen_image_color(48, 56, Color::new(0.0, 0.0, 0.0, 0.0));
    let boot = Color::new(0.17, 0.11, 0.08, 1.0);
    let skin = Color::new(0.86, 0.71, 0.58, 1.0);
    let hair = Color::new(0.24, 0.16, 0.11, 1.0);

    // Boots & legs
    fill_rect(&mut img, 17, 44, 5, 8, boot);
    fill_rect(&mut img, 26, 44, 5, 8, boot);

    // Belted kilt / breeches + coat torso
    fill_ellipse(&mut img, 24.0, 35.0, 10.5, 11.5, coat);
    fill_rect(&mut img, 14, 24, 20, 18, coat);

    // Belt
    fill_rect(&mut img, 14, 34, 20, 3, Color::new(0.20, 0.13, 0.09, 1.0));
    fill_rect(&mut img, 22, 34, 4, 3, Color::new(0.76, 0.65, 0.36, 1.0));

    // Tartan sash OR Redcoat white crossbelts
    if redcoat_belts {
        for i in 0..16 {
            put_px(&mut img, 15 + i, 23 + i, accent);
            put_px(&mut img, 16 + i, 23 + i, accent);
            put_px(&mut img, 32 - i, 23 + i, accent);
            put_px(&mut img, 31 - i, 23 + i, accent);
        }
    } else {
        for i in 0..18 {
            put_px(&mut img, 14 + i, 23 + i, accent);
            put_px(&mut img, 15 + i, 23 + i, accent);
            put_px(&mut img, 16 + i, 23 + i, accent);
        }
        // Dark leather sporran
        fill_ellipse(&mut img, 24.0, 41.0, 3.2, 3.8, Color::new(0.24, 0.17, 0.12, 1.0));
    }

    // Arms & hands
    fill_ellipse(&mut img, 12.0, 31.0, 3.2, 6.5, coat);
    fill_ellipse(&mut img, 36.0, 31.0, 3.2, 6.5, coat);
    fill_ellipse(&mut img, 12.0, 37.0, 2.2, 2.2, skin);
    fill_ellipse(&mut img, 36.0, 37.0, 2.2, 2.2, skin);

    // Head, hair & bonnet / tricorne hat
    fill_ellipse(&mut img, 24.0, 15.5, 7.2, 7.5, skin);
    fill_ellipse(&mut img, 24.0, 12.0, 7.5, 4.0, hair);
    if redcoat_belts {
        // Tricorne hat
        fill_ellipse(&mut img, 24.0, 9.0, 10.5, 4.2, hat);
        fill_ellipse(&mut img, 24.0, 7.0, 6.5, 3.8, hat);
    } else {
        // Highland blue bonnet tilted slightly
        fill_ellipse(&mut img, 25.0, 8.5, 9.2, 4.6, hat);
        fill_ellipse(&mut img, 24.0, 7.0, 2.0, 1.5, Color::new(0.75, 0.18, 0.18, 1.0));
    }

    // Eyes
    put_px(&mut img, 21, 16, Color::new(0.12, 0.10, 0.10, 1.0));
    put_px(&mut img, 27, 16, Color::new(0.12, 0.10, 0.10, 1.0));

    add_outline(&mut img, Color::new(0.10, 0.09, 0.08, 0.92));
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_horse(coat: Color, mane: Color, with_saddle: bool) -> Texture2D {
    let mut img = Image::gen_image_color(64, 52, Color::new(0.0, 0.0, 0.0, 0.0));
    let dark_coat = Color::new(coat.r * 0.78, coat.g * 0.78, coat.b * 0.78, 1.0);
    let hoof = Color::new(0.14, 0.11, 0.09, 1.0);

    // Back legs (darker)
    fill_rect(&mut img, 17, 32, 4, 15, dark_coat);
    fill_rect(&mut img, 39, 32, 4, 15, dark_coat);
    fill_rect(&mut img, 17, 45, 4, 3, hoof);
    fill_rect(&mut img, 39, 45, 4, 3, hoof);

    // Flowing dark tail
    fill_ellipse(&mut img, 10.0, 27.0, 4.5, 8.5, mane);

    // Barrel body
    fill_ellipse(&mut img, 29.0, 26.0, 16.5, 9.5, coat);

    // Front legs
    fill_rect(&mut img, 21, 33, 4, 15, coat);
    fill_rect(&mut img, 42, 33, 4, 15, coat);
    fill_rect(&mut img, 21, 46, 4, 3, hoof);
    fill_rect(&mut img, 42, 46, 4, 3, hoof);

    // Arched neck & head facing right
    fill_ellipse(&mut img, 45.0, 19.0, 6.0, 9.0, coat);
    fill_ellipse(&mut img, 52.0, 13.5, 7.0, 4.5, coat);
    // Ears
    fill_ellipse(&mut img, 47.0, 7.5, 1.8, 3.5, dark_coat);
    // Mane along neck
    fill_ellipse(&mut img, 41.5, 15.0, 3.2, 7.5, mane);
    // Eye & muzzle
    put_px(&mut img, 52, 12, Color::new(0.08, 0.07, 0.07, 1.0));
    put_px(&mut img, 53, 12, Color::new(0.08, 0.07, 0.07, 1.0));

    if with_saddle {
        // Tartan saddleblanket + leather saddle + saddlebag
        fill_rect(&mut img, 22, 17, 14, 11, Color::new(0.18, 0.36, 0.24, 1.0));
        fill_rect(&mut img, 22, 22, 14, 2, Color::new(0.68, 0.18, 0.18, 1.0));
        fill_ellipse(&mut img, 29.0, 19.0, 6.5, 4.2, Color::new(0.28, 0.17, 0.10, 1.0));
        fill_rect(&mut img, 19, 21, 6, 7, Color::new(0.32, 0.20, 0.12, 1.0));
    }

    add_outline(&mut img, Color::new(0.11, 0.09, 0.07, 0.90));
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

impl PaintedAtlas {
    pub fn build() -> Self {
        let post_mat = load_material(
            ShaderSource::Glsl {
                vertex: VERT_SHADER,
                fragment: FRAG_SHADER,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("u_time", UniformType::Float1),
                    UniformDesc::new("u_warmth", UniformType::Float1),
                    UniformDesc::new("u_darkness", UniformType::Float1),
                    UniformDesc::new("u_mist", UniformType::Float1),
                    UniformDesc::new("u_res_x", UniformType::Float1),
                    UniformDesc::new("u_res_y", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .expect("Failed to compile Clachan painterly shader");

        Self {
            terrain_map: paint_continuous_terrain(1744_0519),
            roof: paint_tile(64, 707, [0.47, 0.37, 0.22], [0.35, 0.27, 0.15], [0.64, 0.52, 0.33], 360, false),
            floor: paint_tile(64, 808, [0.40, 0.29, 0.19], [0.30, 0.21, 0.14], [0.51, 0.39, 0.27], 260, true),
            stone_wall: paint_masonry(64),
            pine: paint_pine_34(88),
            soft_glow: paint_radial_glow(128),
            horse: paint_horse(Color::new(0.50, 0.35, 0.22, 1.0), Color::new(0.18, 0.12, 0.09, 1.0), true),
            deer: paint_horse(Color::new(0.60, 0.41, 0.26, 1.0), Color::new(0.42, 0.28, 0.18, 1.0), false),
            spr_player: paint_character(
                Color::new(0.18, 0.37, 0.24, 1.0),
                Color::new(0.72, 0.18, 0.18, 1.0),
                Color::new(0.16, 0.23, 0.35, 1.0),
                false,
            ),
            spr_villager: paint_character(
                Color::new(0.42, 0.36, 0.28, 1.0),
                Color::new(0.24, 0.38, 0.34, 1.0),
                Color::new(0.28, 0.24, 0.19, 1.0),
                false,
            ),
            spr_redcoat: paint_character(
                Color::new(0.76, 0.15, 0.14, 1.0),
                Color::new(0.94, 0.90, 0.80, 1.0),
                Color::new(0.12, 0.12, 0.14, 1.0),
                true,
            ),
            post_mat,
        }
    }
}

fn draw_glow(atlas: &PaintedAtlas, pos: Vec2, radius: f32, col: Color) {
    draw_texture_ex(
        &atlas.soft_glow,
        pos.x - radius,
        pos.y - radius,
        col,
        DrawTextureParams {
            dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
            ..Default::default()
        },
    );
}

fn draw_entity(e: &Entity, u: &Universe, atlas: &PaintedAtlas, shadow_off: Vec2) {
    if e.buried {
        return;
    }
    let p = e.pos;
    let sh_rad = match e.kind {
        EntityKind::Mount => 0.92,
        EntityKind::Cairn => 0.80,
        _ => 0.48,
    };
    draw_circle(
        p.x + shadow_off.x * 0.25,
        p.y + 0.36 + shadow_off.y * 0.18,
        sh_rad,
        Color::new(0.03, 0.04, 0.05, 0.32),
    );

    let bob = if e.vel.length() > 0.35 {
        (u.time * 12.0 + e.id as f32).sin().abs() * -0.10
    } else {
        0.0
    };

    match e.kind {
        EntityKind::Cairn => {
            draw_circle(p.x - 0.30, p.y + 0.12, 0.35, Color::new(0.45, 0.46, 0.44, 1.0));
            draw_circle(p.x + 0.28, p.y + 0.14, 0.33, Color::new(0.39, 0.41, 0.40, 1.0));
            draw_circle(p.x, p.y - 0.06, 0.38, Color::new(0.55, 0.56, 0.52, 1.0));
            draw_circle(p.x, p.y - 0.34, 0.22, Color::new(0.65, 0.65, 0.61, 1.0));
        }
        EntityKind::Campfire => {
            let fl = 0.88 + (u.time * 16.0).sin() * 0.12;
            draw_glow(atlas, p, 3.2 * fl, Color::new(1.0, 0.56, 0.16, 0.34));
            draw_circle(p.x, p.y, 0.44, Color::new(0.22, 0.16, 0.12, 1.0));
            draw_circle(p.x, p.y - 0.06, 0.30 * fl, Color::new(1.0, 0.64, 0.16, 0.95));
            draw_circle(p.x, p.y - 0.10, 0.16 * fl, Color::new(1.0, 0.90, 0.45, 0.95));
        }
        EntityKind::Mount => {
            let w = 2.45;
            let h = 2.0;
            draw_texture_ex(
                &atlas.horse,
                p.x - w * 0.5,
                p.y - h * 0.62 + bob,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(w, h)),
                    flip_x: e.facing.x < -0.05,
                    ..Default::default()
                },
            );
        }
        EntityKind::Deer => {
            let w = 1.65;
            let h = 1.35;
            draw_texture_ex(
                &atlas.deer,
                p.x - w * 0.5,
                p.y - h * 0.62 + bob,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(w, h)),
                    flip_x: e.facing.x < -0.05,
                    ..Default::default()
                },
            );
        }
        EntityKind::Corpse => {
            let y_off = -e.z * 0.55;
            draw_rectangle(p.x - 0.30, p.y - 0.18 + y_off, 0.60, 0.80, Color::new(0.24, 0.31, 0.24, 1.0));
            draw_circle(p.x, p.y - 0.40 + y_off, 0.24, Color::new(0.76, 0.65, 0.56, 1.0));
        }
        EntityKind::Player | EntityKind::Villager | EntityKind::Redcoat => {
            let y_lift = if e.mounted_id.is_some() { -0.52 } else { 0.0 } + bob;
            let tex = match e.kind {
                EntityKind::Player => &atlas.spr_player,
                EntityKind::Redcoat => &atlas.spr_redcoat,
                _ => &atlas.spr_villager,
            };
            let w = 1.48;
            let h = 1.72;
            draw_texture_ex(
                tex,
                p.x - w * 0.5,
                p.y - h * 0.72 + y_lift,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(w, h)),
                    flip_x: e.facing.x < -0.1,
                    ..Default::default()
                },
            );

            if e.blood_stain > 0.12 {
                draw_circle(
                    p.x + 0.12,
                    p.y - 0.05 + y_lift,
                    0.18,
                    Color::new(0.58, 0.05, 0.05, e.blood_stain * 0.75),
                );
            }

            if e.swing_timer > 0.0 {
                let tip = p + e.facing * 1.65;
                draw_line(
                    p.x,
                    p.y - 0.15 + y_lift,
                    tip.x,
                    tip.y - 0.15 + y_lift,
                    0.11,
                    Color::new(0.90, 0.94, 0.98, 0.95),
                );
            }

            if e.kind == EntityKind::Redcoat {
                if let Some(m) = &e.mind {
                    if m.awareness > 0.25 {
                        let col = if m.goal == AiGoal::Engage { RED } else { YELLOW };
                        draw_circle(p.x, p.y - 1.38, 0.15, col);
                    }
                }
            }
        }
    }
}

fn draw_cottage_exterior(b: &BuildingRecord, u: &Universe, atlas: &PaintedAtlas, shadow_off: Vec2) {
    draw_rectangle(
        b.x + shadow_off.x * 0.85,
        b.y + shadow_off.y * 0.85,
        b.w,
        b.h,
        Color::new(0.04, 0.05, 0.06, 0.34 * u.weather.sun_warmth),
    );

    let roof_h = b.h * 0.58;
    let wall_y = b.y + roof_h;
    let wall_h = b.h - roof_h;

    draw_texture_ex(
        &atlas.stone_wall,
        b.x,
        wall_y,
        Color::new(0.95, 0.93, 0.88, 1.0),
        DrawTextureParams {
            dest_size: Some(vec2(b.w, wall_h)),
            ..Default::default()
        },
    );
    draw_rectangle_lines(b.x, wall_y, b.w, wall_h, 0.14, Color::new(0.18, 0.17, 0.16, 1.0));

    let win_glow = if b.hearth_lit {
        Color::new(0.96, 0.72, 0.30, 0.92)
    } else {
        Color::new(0.22, 0.26, 0.30, 0.90)
    };
    let win_y = wall_y + wall_h * 0.22;
    for wx in [b.x + 1.1, b.x + b.w - 2.3] {
        if b.hearth_lit {
            draw_glow(atlas, Vec2::new(wx + 0.6, win_y + 0.5), 1.6, Color::new(1.0, 0.68, 0.25, 0.22));
        }
        draw_rectangle(wx, win_y, 1.2, 1.0, win_glow);
        draw_rectangle_lines(wx, win_y, 1.2, 1.0, 0.12, Color::new(0.24, 0.17, 0.11, 1.0));
        draw_line(wx + 0.6, win_y, wx + 0.6, win_y + 1.0, 0.08, Color::new(0.24, 0.17, 0.11, 1.0));
        draw_line(wx, win_y + 0.5, wx + 1.2, win_y + 0.5, 0.08, Color::new(0.24, 0.17, 0.11, 1.0));
    }

    let dx = b.door_pos.x - 0.7;
    let dy = wall_y + wall_h - 1.75;
    draw_rectangle(dx - 0.15, b.y + b.h - 0.05, 1.7, 0.42, Color::new(0.45, 0.44, 0.40, 1.0));
    draw_rectangle(dx, dy, 1.4, 1.75, Color::new(0.30, 0.20, 0.12, 1.0));
    draw_rectangle_lines(dx, dy, 1.4, 1.75, 0.12, Color::new(0.18, 0.12, 0.08, 1.0));
    draw_circle(dx + 1.12, dy + 0.92, 0.09, Color::new(0.75, 0.68, 0.45, 1.0));

    draw_texture_ex(
        &atlas.roof,
        b.x - 0.35,
        b.y - 0.35,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(b.w + 0.7, roof_h + 0.55)),
            ..Default::default()
        },
    );
    draw_rectangle(b.x, wall_y + 0.15, b.w, 0.32, Color::new(0.05, 0.04, 0.04, 0.38));
    draw_line(
        b.x - 0.3,
        b.y + roof_h * 0.36,
        b.x + b.w + 0.3,
        b.y + roof_h * 0.36,
        0.22,
        Color::new(0.28, 0.21, 0.13, 1.0),
    );
    draw_rectangle(
        b.chimney_pos.x - 0.48,
        b.chimney_pos.y - 0.48,
        0.96,
        0.96,
        Color::new(0.36, 0.35, 0.33, 1.0),
    );
    draw_rectangle_lines(
        b.chimney_pos.x - 0.48,
        b.chimney_pos.y - 0.48,
        0.96,
        0.96,
        0.12,
        Color::new(0.20, 0.19, 0.18, 1.0),
    );
    draw_rectangle(
        b.chimney_pos.x - 0.26,
        b.chimney_pos.y - 0.26,
        0.52,
        0.52,
        Color::new(0.10, 0.09, 0.09, 1.0),
    );
}

fn draw_overworld(u: &Universe, atlas: &PaintedAtlas, cam: Vec2, view_w_m: f32, view_h_m: f32) {
    // 1. Single seamless oil-painted terrain canvas (zero grid seams!)
    draw_texture_ex(
        &atlas.terrain_map,
        TERRAIN_MIN_X,
        TERRAIN_MIN_Y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(TERRAIN_W, TERRAIN_H)),
            ..Default::default()
        },
    );

    // 2. Subtle animated Highland grass tufts, heather sprigs, wildflowers & loch shimmer
    let step_m = 2.5f32;
    let min_gx = ((cam.x - view_w_m * 0.55) / step_m).floor() as i32;
    let max_gx = ((cam.x + view_w_m * 0.55) / step_m).ceil() as i32;
    let min_gy = ((cam.y - view_h_m * 0.55) / step_m).floor() as i32;
    let max_gy = ((cam.y + view_h_m * 0.55) / step_m).ceil() as i32;
    let wind_sway = u.weather.wind_vec.x.clamp(-6.0, 6.0) * 0.025;

    for gy in min_gy..=max_gy {
        for gx in min_gx..=max_gx {
            let jx = (hash2d(1744_11, gx, gy) - 0.5) * 2.1;
            let jy = (hash2d(1744_29, gx, gy) - 0.5) * 2.1;
            let wx = gx as f32 * step_m + jx;
            let wy = gy as f32 * step_m + jy;
            let (_, kind) = sample_terrain(u.cfg.world_seed, wx, wy);
            let sway = (u.time * 2.4 + wx * 0.6 + wy * 0.4).sin() * (0.06 + wind_sway.abs());

            match kind {
                TerrainKind::Meadow => {
                    draw_line(wx, wy, wx + sway - 0.08, wy - 0.28, 0.05, Color::new(0.42, 0.58, 0.26, 0.72));
                    draw_line(wx + 0.12, wy + 0.04, wx + 0.18 + sway, wy - 0.24, 0.05, Color::new(0.50, 0.64, 0.30, 0.68));
                    if hash2d(1744_73, gx, gy) > 0.78 {
                        // Tiny golden Highland gorse blossom
                        draw_circle(wx + sway, wy - 0.26, 0.09, Color::new(0.92, 0.78, 0.26, 0.85));
                    }
                }
                TerrainKind::Heather => {
                    draw_line(wx, wy, wx + sway, wy - 0.26, 0.06, Color::new(0.32, 0.36, 0.22, 0.7));
                    draw_circle(wx + sway, wy - 0.26, 0.11, Color::new(0.66, 0.42, 0.64, 0.78));
                    draw_circle(wx + 0.14 + sway, wy - 0.18, 0.09, Color::new(0.54, 0.31, 0.54, 0.75));
                }
                TerrainKind::LochWater => {
                    let shimmer = (u.time * 2.0 + wx * 0.5 + wy * 0.4).sin();
                    if shimmer > 0.35 {
                        draw_line(
                            wx - 0.35,
                            wy,
                            wx + 0.35,
                            wy,
                            0.05,
                            Color::new(0.70, 0.82, 0.86, (shimmer - 0.35) * 0.45),
                        );
                    }
                }
                TerrainKind::GraniteCrag => {
                    if hash2d(1744_88, gx, gy) > 0.65 {
                        draw_circle(wx, wy, 0.24, Color::new(0.50, 0.50, 0.47, 0.75));
                    }
                }
                _ => {}
            }
        }
    }

    // 3. Traces & sound waves
    for tr in &u.traces {
        if tr.location != Location::Overworld {
            continue;
        }
        let a = (1.0 - tr.age / tr.max_age).clamp(0.0, 1.0) * 0.62;
        let (col, r) = match tr.kind {
            TraceKind::BootPrint => (Color::new(0.18, 0.13, 0.09, a), 0.15),
            TraceKind::HoofPrint => (Color::new(0.16, 0.11, 0.07, a), 0.22),
            TraceKind::BloodDrop => (Color::new(0.55, 0.05, 0.05, a * 1.2), 0.20),
            TraceKind::Scorch => (Color::new(0.08, 0.08, 0.08, a), 0.52),
        };
        draw_circle(tr.pos.x, tr.pos.y, r, col);
    }

    for sw in &u.sound_waves {
        if sw.location != Location::Overworld {
            continue;
        }
        let a = (1.0 - sw.age / 1.1).clamp(0.0, 1.0) * 0.45;
        let col = if sw.whistle {
            Color::new(0.95, 0.86, 0.45, a)
        } else if sw.hostiles_care {
            Color::new(0.95, 0.42, 0.28, a)
        } else {
            Color::new(0.85, 0.88, 0.85, a * 0.5)
        };
        draw_circle_lines(sw.pos.x, sw.pos.y, sw.radius, 0.08, col);
    }

    let shadow_off = u.weather.sun_dir * 0.75;

    // 4. 3/4 Cottages
    for b in &u.buildings {
        draw_cottage_exterior(b, u, atlas, shadow_off);
    }

    // 5. Gibbet on Wade's Road
    let gx = 12.0;
    let gy = -1.5;
    draw_rectangle(gx - 2.2 + shadow_off.x, gy - 0.3 + shadow_off.y, 4.4, 0.45, Color::new(0.05, 0.05, 0.07, 0.35));
    draw_rectangle(gx - 2.1, gy - 2.2, 0.35, 2.5, Color::new(0.30, 0.21, 0.14, 1.0));
    draw_rectangle(gx + 1.75, gy - 2.2, 0.35, 2.5, Color::new(0.30, 0.21, 0.14, 1.0));
    draw_rectangle(gx - 2.25, gy - 2.25, 4.5, 0.38, Color::new(0.34, 0.24, 0.16, 1.0));
    for c in &u.constraints {
        if c.intact {
            draw_line(c.anchor.x, c.anchor.y - 2.0, c.anchor.x, c.anchor.y + 0.1, 0.09, Color::new(0.76, 0.66, 0.44, 1.0));
        } else {
            draw_line(c.anchor.x, c.anchor.y - 2.0, c.anchor.x + 0.08, c.anchor.y - 1.35, 0.09, Color::new(0.76, 0.66, 0.44, 0.9));
        }
    }

    // 6. Depth-sorted Entities & Scots Pines together by Y so Callum walks cleanly behind/in front of trees!
    enum DrawItem {
        Ent(usize),
        Tree(usize),
    }
    let mut items: Vec<(f32, DrawItem)> = Vec::with_capacity(u.entities.len() + u.trees.len());
    for i in 0..u.entities.len() {
        if u.entities[i].location == Location::Overworld {
            items.push((u.entities[i].pos.y, DrawItem::Ent(i)));
        }
    }
    for (ti, (tp, _)) in u.trees.iter().enumerate() {
        items.push((tp.y + 0.6, DrawItem::Tree(ti)));
    }
    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (_, item) in items {
        match item {
            DrawItem::Ent(idx) => draw_entity(&u.entities[idx], u, atlas, shadow_off),
            DrawItem::Tree(ti) => {
                let (tp, rad) = u.trees[ti];
                draw_circle(
                    tp.x + shadow_off.x * 0.65,
                    tp.y + 0.55 + shadow_off.y * 0.55,
                    rad * 0.78,
                    Color::new(0.04, 0.06, 0.05, 0.28 * u.weather.sun_warmth),
                );
                let sway = (u.time * 1.7 + tp.x * 0.4).sin() * 0.06 * (u.weather.wind_vec.length() * 0.18);
                let sz = rad * 2.25;
                draw_texture_ex(
                    &atlas.pine,
                    tp.x - sz * 0.5 + sway,
                    tp.y - sz * 0.62,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(sz, sz)),
                        ..Default::default()
                    },
                );
            }
        }
    }

    // 7. Soft curling chimney smoke & tiny glowing sparks
    for sp in &u.smoke {
        if sp.location != Location::Overworld {
            continue;
        }
        let a = (1.0 - sp.age / sp.max_age).clamp(0.0, 1.0);
        if sp.is_ember {
            draw_glow(atlas, sp.pos, 0.26, Color::new(1.0, 0.62, 0.18, a * 0.55));
            draw_circle(sp.pos.x, sp.pos.y, 0.07, Color::new(1.0, 0.85, 0.42, a * 0.9));
        } else {
            draw_glow(atlas, sp.pos, sp.radius * 1.35, Color::new(0.84, 0.86, 0.86, a * 0.26));
        }
    }

    // 8. Driving Highland rain streaks
    if u.weather.rain_intensity > 0.06 {
        let drops = (u.weather.rain_intensity * 90.0) as usize;
        for i in 0..drops {
            let rx = cam.x + (((i as f32 * 37.1 + u.time * 12.0) % view_w_m) - view_w_m * 0.5);
            let ry = cam.y + (((i as f32 * 19.7 + u.time * 24.0) % view_h_m) - view_h_m * 0.5);
            draw_line(
                rx,
                ry,
                rx + u.weather.wind_vec.x * 0.05,
                ry + 0.65,
                0.04,
                Color::new(0.74, 0.82, 0.88, 0.35 * u.weather.rain_intensity),
            );
        }
    }
}

fn draw_interior(u: &Universe, bid: u32, atlas: &PaintedAtlas) {
    let Some(b) = u.building_by_id(bid) else { return; };

    // Warm timber floorboards with plank seams
    let tile = 2.0f32;
    let cols = (b.int_w / tile).ceil() as i32;
    let rows = (b.int_h / tile).ceil() as i32;
    for r in 0..rows {
        for c in 0..cols {
            let x = c as f32 * tile;
            let y = r as f32 * tile;
            let w = (b.int_w - x).min(tile);
            let h = (b.int_h - y).min(tile);
            draw_texture_ex(
                &atlas.floor,
                x,
                y,
                Color::new(0.95, 0.92, 0.86, 1.0),
                DrawTextureParams {
                    dest_size: Some(vec2(w + 0.03, h + 0.03)),
                    ..Default::default()
                },
            );
        }
    }

    // 3/4 north stone wall with heavy timber beam & wainscoting
    draw_texture_ex(
        &atlas.stone_wall,
        0.0,
        0.0,
        Color::new(0.86, 0.83, 0.78, 1.0),
        DrawTextureParams {
            dest_size: Some(vec2(b.int_w, 1.55)),
            ..Default::default()
        },
    );
    draw_rectangle(0.0, 1.44, b.int_w, 0.22, Color::new(0.24, 0.17, 0.12, 1.0));
    draw_rectangle(0.0, 1.66, b.int_w, 0.24, Color::new(0.04, 0.03, 0.03, 0.26));

    // Paned windows tinted by real outdoor weather
    let sky_col = if u.weather.rain_intensity > 0.3 {
        Color::new(0.32, 0.40, 0.48, 0.95)
    } else {
        Color::new(
            0.48 + u.weather.sun_warmth * 0.40,
            0.58 + u.weather.sun_warmth * 0.32,
            0.66 + u.weather.sun_warmth * 0.20,
            0.95,
        )
    };
    for wx in [b.int_w * 0.26, b.int_w * 0.72] {
        if (wx - b.int_hearth.x).abs() > 1.8 {
            draw_rectangle(wx - 0.65, 0.28, 1.3, 0.95, sky_col);
            draw_rectangle_lines(wx - 0.65, 0.28, 1.3, 0.95, 0.10, Color::new(0.25, 0.18, 0.12, 1.0));
            draw_line(wx, 0.28, wx, 1.23, 0.07, Color::new(0.25, 0.18, 0.12, 1.0));
            draw_line(wx - 0.65, 0.75, wx + 0.65, 0.75, 0.07, Color::new(0.25, 0.18, 0.12, 1.0));
            if u.weather.sun_warmth > 0.35 && u.weather.rain_intensity < 0.3 {
                draw_glow(atlas, Vec2::new(wx, 2.35), 1.4, Color::new(1.0, 0.92, 0.72, 0.14));
            }
        }
    }

    // Detailed stone hearth fireplace with timber mantel, peat logs & iron kettle
    let hx = b.int_hearth.x;
    let hy = b.int_hearth.y;
    draw_rectangle(hx - 1.20, 0.10, 2.40, 1.50, Color::new(0.32, 0.30, 0.29, 1.0));
    draw_rectangle(hx - 1.32, 0.44, 2.64, 0.22, Color::new(0.28, 0.19, 0.12, 1.0)); // Timber mantelpiece
    draw_rectangle(hx - 0.72, 0.78, 1.44, 0.85, Color::new(0.08, 0.07, 0.07, 1.0));
    draw_rectangle(hx - 1.00, 1.56, 2.00, 0.46, Color::new(0.42, 0.40, 0.37, 1.0)); // Stone hearth slab
    // Stacked peat blocks
    draw_rectangle(hx - 0.36, 1.28, 0.72, 0.18, Color::new(0.20, 0.14, 0.10, 1.0));
    if b.hearth_lit {
        let flick = 0.90 + (u.time * 15.0 + b.id as f32).sin() * 0.10;
        draw_circle(hx, hy - 0.26, 0.36 * flick, Color::new(1.0, 0.52, 0.12, 0.95));
        draw_circle(hx, hy - 0.30, 0.20 * flick, Color::new(1.0, 0.88, 0.40, 0.95));
    }

    // South exit doormat
    draw_rectangle(b.int_exit.x - 1.0, b.int_h - 0.42, 2.0, 0.48, Color::new(0.42, 0.30, 0.18, 1.0));
    draw_rectangle(b.int_exit.x - 0.85, b.int_h - 0.90, 1.7, 0.46, Color::new(0.50, 0.36, 0.22, 0.95));
    draw_rectangle_lines(b.int_exit.x - 0.85, b.int_h - 0.90, 1.7, 0.46, 0.06, Color::new(0.30, 0.20, 0.12, 1.0));

    // Woven tartan rugs
    for prop in &b.props {
        if prop.kind == PropKind::Rug {
            draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.20, 0.34, 0.23, 0.94));
            draw_rectangle_lines(prop.x + 0.14, prop.y + 0.14, prop.w - 0.28, prop.h - 0.28, 0.10, Color::new(0.68, 0.20, 0.18, 0.90));
            draw_line(prop.x, prop.y + prop.h * 0.5, prop.x + prop.w, prop.y + prop.h * 0.5, 0.09, Color::new(0.68, 0.20, 0.18, 0.55));
            draw_line(prop.x + prop.w * 0.5, prop.y, prop.x + prop.w * 0.5, prop.y + prop.h, 0.09, Color::new(0.68, 0.20, 0.18, 0.55));
        }
    }

    // Furnishings
    for prop in &b.props {
        match prop.kind {
            PropKind::Rug => {}
            PropKind::Bed => {
                draw_rectangle(prop.x + 0.10, prop.y + 0.14, prop.w, prop.h, Color::new(0.04, 0.03, 0.03, 0.34));
                draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.30, 0.20, 0.12, 1.0));
                draw_rectangle(prop.x + 0.14, prop.y + 0.20, prop.w - 0.28, 0.55, Color::new(0.88, 0.85, 0.78, 1.0));
                draw_rectangle(prop.x + 0.14, prop.y + 0.80, prop.w - 0.28, prop.h - 1.02, Color::new(0.18, 0.36, 0.24, 1.0));
                draw_line(prop.x + 0.14, prop.y + 1.45, prop.x + prop.w - 0.14, prop.y + 1.45, 0.12, Color::new(0.68, 0.18, 0.18, 0.90));
                draw_rectangle(prop.x, prop.y + prop.h - 0.18, prop.w, 0.18, Color::new(0.24, 0.15, 0.09, 1.0));
            }
            PropKind::Table => {
                draw_rectangle(prop.x + 0.12, prop.y + 0.18, prop.w, prop.h, Color::new(0.04, 0.03, 0.03, 0.34));
                draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.42, 0.28, 0.17, 1.0));
                draw_rectangle_lines(prop.x, prop.y, prop.w, prop.h, 0.08, Color::new(0.24, 0.15, 0.09, 1.0));
                draw_circle(prop.x + prop.w * 0.5, prop.y + prop.h * 0.45, 0.10, Color::new(1.0, 0.90, 0.52, 1.0));
            }
            PropKind::Anvil => {
                draw_rectangle(prop.x + 0.10, prop.y + 0.14, prop.w, prop.h, Color::new(0.04, 0.04, 0.05, 0.42));
                draw_rectangle(prop.x, prop.y + 0.18, prop.w * 0.3, prop.h - 0.36, Color::new(0.32, 0.34, 0.38, 1.0));
                draw_rectangle(prop.x + 0.2, prop.y, prop.w - 0.2, prop.h, Color::new(0.26, 0.28, 0.32, 1.0));
                draw_rectangle(prop.x + 0.32, prop.y + 0.12, prop.w - 0.46, prop.h - 0.24, Color::new(0.46, 0.50, 0.54, 1.0));
            }
            PropKind::Forge => {
                draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.28, 0.26, 0.25, 1.0));
                let fl = 0.88 + (u.time * 18.0).sin() * 0.12;
                draw_circle(prop.x + prop.w * 0.5, prop.y + prop.h * 0.55, 0.46 * fl, Color::new(1.0, 0.42, 0.10, 0.95));
            }
            PropKind::BarCounter => {
                draw_rectangle(prop.x + 0.08, prop.y + 0.14, prop.w, prop.h, Color::new(0.04, 0.03, 0.03, 0.35));
                draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.38, 0.24, 0.14, 1.0));
                draw_rectangle_lines(prop.x, prop.y, prop.w, prop.h, 0.10, Color::new(0.20, 0.12, 0.07, 1.0));
                for mx in [prop.x + 0.8, prop.x + 2.2, prop.x + 3.7] {
                    draw_circle(mx, prop.y + 0.45, 0.12, Color::new(0.70, 0.72, 0.74, 1.0));
                }
            }
            PropKind::Barrel => {
                let c = prop.center();
                draw_circle(c.x + 0.08, c.y + 0.12, prop.w * 0.48, Color::new(0.04, 0.03, 0.03, 0.32));
                draw_circle(c.x, c.y, prop.w * 0.48, Color::new(0.38, 0.25, 0.15, 1.0));
                draw_circle_lines(c.x, c.y, prop.w * 0.48, 0.08, Color::new(0.20, 0.20, 0.22, 1.0));
            }
            PropKind::WeaponRack => {
                draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.30, 0.20, 0.12, 1.0));
                draw_line(prop.x + 0.2, prop.y + 0.25, prop.x + prop.w - 0.2, prop.y + 0.25, 0.08, Color::new(0.74, 0.78, 0.82, 1.0));
                draw_line(prop.x + 0.2, prop.y + 0.48, prop.x + prop.w - 0.2, prop.y + 0.48, 0.08, Color::new(0.74, 0.78, 0.82, 1.0));
            }
            PropKind::CellBars => {
                draw_rectangle(prop.x, prop.y, prop.w, prop.h, Color::new(0.14, 0.14, 0.16, 0.65));
                draw_rectangle_lines(prop.x, prop.y, prop.w, prop.h, 0.14, Color::new(0.22, 0.23, 0.26, 1.0));
                for i in 1..6 {
                    let bx = prop.x + (prop.w * i as f32 / 6.0);
                    draw_line(bx, prop.y, bx, prop.y + prop.h, 0.08, Color::new(0.32, 0.34, 0.38, 1.0));
                }
            }
        }
    }

    // Entities inside this room sorted by Y
    let loc = Location::Interior(bid);
    let mut order: Vec<usize> = (0..u.entities.len())
        .filter(|&i| u.entities[i].location == loc)
        .collect();
    order.sort_by(|&a, &b| {
        u.entities[a]
            .pos
            .y
            .partial_cmp(&u.entities[b].pos.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for idx in order {
        draw_entity(&u.entities[idx], u, atlas, Vec2::new(0.25, 0.35));
    }

    // Smooth radial interior warmth (hearth + table candles + forge)
    if b.hearth_lit {
        let flick = 0.92 + (u.time * 14.0 + b.id as f32).sin() * 0.08;
        draw_glow(atlas, Vec2::new(hx, hy + 0.8), 5.2 * flick, Color::new(1.0, 0.58, 0.20, 0.28));
    }
    for prop in &b.props {
        if prop.kind == PropKind::Table {
            draw_glow(atlas, Vec2::new(prop.x + prop.w * 0.5, prop.y + prop.h * 0.45), 1.7, Color::new(1.0, 0.74, 0.30, 0.22));
        } else if prop.kind == PropKind::Forge {
            draw_glow(atlas, prop.center(), 3.0, Color::new(1.0, 0.45, 0.14, 0.26));
        }
    }

    // Tiny hearth embers
    for sp in &u.smoke {
        if sp.location != loc {
            continue;
        }
        let a = (1.0 - sp.age / sp.max_age).clamp(0.0, 1.0);
        draw_circle(sp.pos.x, sp.pos.y, 0.06, Color::new(1.0, 0.74, 0.26, a * 0.90));
    }

    // Crisp outer dark-void clipping mask so hearth glow NEVER bleeds outside the stone room walls!
    let void = Color::new(0.04, 0.04, 0.05, 1.0);
    let pad = 0.42;
    draw_rectangle(-25.0, -25.0, b.int_w + 50.0, 25.0 - pad, void);
    draw_rectangle(-25.0, b.int_h + pad, b.int_w + 50.0, 25.0, void);
    draw_rectangle(-25.0, -25.0, 25.0 - pad, b.int_h + 50.0, void);
    draw_rectangle(b.int_w + pad, -25.0, 25.0, b.int_h + 50.0, void);
    draw_rectangle_lines(-pad, -pad, b.int_w + pad * 2.0, b.int_h + pad * 2.0, 0.42, Color::new(0.16, 0.15, 0.14, 1.0));
}

pub fn draw_world(u: &Universe, atlas: &PaintedAtlas, cam: Vec2, view_w_m: f32, view_h_m: f32) {
    match u.player_location(PlayerId(1)) {
        Location::Overworld => draw_overworld(u, atlas, cam, view_w_m, view_h_m),
        Location::Interior(bid) => draw_interior(u, bid, atlas),
    }
}

pub fn draw_hud_and_forge(u: &mut Universe, forge_open: &mut bool) {
    let sw = screen_width();
    let sh = screen_height();

    if u.fade_alpha > 0.01 {
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.04, 0.04, 0.05, u.fade_alpha));
    }

    let loc = u.player_location(PlayerId(1));
    let loc_label = match loc {
        Location::Overworld => "Lochaber Glen".to_string(),
        Location::Interior(bid) => u
            .building_by_id(bid)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "Indoors".to_string()),
    };

    draw_rectangle(14.0, 14.0, 360.0, 136.0, Color::new(0.09, 0.10, 0.09, 0.80));
    draw_rectangle_lines(14.0, 14.0, 360.0, 136.0, 1.5, Color::new(0.62, 0.54, 0.36, 0.8));

    let hour = (u.weather.time_of_day * 24.0) as u32;
    let min = ((u.weather.time_of_day * 24.0 - hour as f32) * 60.0) as u32;
    draw_text(
        &format!("CLACHAN  |  {}  |  Day {} {:02}:{:02}", u.cfg.era_year, u.weather.day_count, hour, min),
        24.0,
        35.0,
        18.0,
        Color::new(0.95, 0.88, 0.72, 1.0),
    );
    draw_text(&loc_label, 24.0, 54.0, 15.0, Color::new(0.75, 0.82, 0.68, 0.95));

    if let Some(pi) = u.player_index(PlayerId(1)) {
        let p = &u.entities[pi];
        let bars = [
            ("Health", p.health / 100.0, Color::new(0.76, 0.22, 0.20, 1.0)),
            ("Stamina", p.stamina / 100.0, Color::new(0.32, 0.68, 0.36, 1.0)),
            ("Wool Dryness", 1.0 - p.wetness, Color::new(0.38, 0.64, 0.84, 1.0)),
            ("Powder Dry", 1.0 - p.powder_wetness, Color::new(0.86, 0.68, 0.30, 1.0)),
        ];
        for (idx, (label, val, col)) in bars.iter().enumerate() {
            let by = 64.0 + idx as f32 * 19.0;
            draw_text(label, 24.0, by + 11.0, 15.0, Color::new(0.85, 0.84, 0.80, 0.95));
            draw_rectangle(142.0, by, 214.0, 12.0, Color::new(0.18, 0.19, 0.18, 0.9));
            draw_rectangle(142.0, by, 214.0 * val.clamp(0.0, 1.0), 12.0, *col);
        }

        let mut prompt_text: Option<String> = None;
        match loc {
            Location::Overworld => {
                if let Some(b) = u.buildings.iter().find(|b| b.door_pos.distance(p.pos) < 2.2) {
                    prompt_text = Some(format!("[Walk Up / E] Enter {}", b.name));
                }
            }
            Location::Interior(bid) => {
                if let Some(b) = u.building_by_id(bid) {
                    if b.int_exit.distance(p.pos) < 1.65 {
                        prompt_text = Some("[Walk Down / E] Step Outside".to_string());
                    } else if b.int_hearth.distance(p.pos) < 2.2 {
                        prompt_text = Some("[E] Stoke / Bank Hearth".to_string());
                    } else {
                        for prop in &b.props {
                            if !prop.prompt.is_empty() && prop.center().distance(p.pos) < 2.25 {
                                prompt_text = Some(prop.prompt.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
        if let Some(pt) = prompt_text {
            let pw = measure_text(&pt, None, 18, 1.0).width + 26.0;
            let px = (sw - pw) * 0.5;
            let py = sh - 82.0;
            draw_rectangle(px, py, pw, 30.0, Color::new(0.09, 0.10, 0.09, 0.86));
            draw_rectangle_lines(px, py, pw, 30.0, 1.2, Color::new(0.80, 0.68, 0.42, 0.9));
            draw_text(&pt, px + 13.0, py + 20.0, 18.0, Color::new(0.96, 0.90, 0.70, 1.0));
        }
    }

    draw_rectangle(14.0, sh - 38.0, sw - 28.0, 26.0, Color::new(0.08, 0.09, 0.08, 0.78));
    draw_text(
        "[WASD] Move  [Click/Space] Steel  [F] Flintlock  [E] Interact/Door/Bed  [B] Bury Cairn  [H] Whistle Bramble  [C] Fire  [R] Bandage  [TAB] Universe Forge",
        22.0,
        sh - 20.0,
        15.0,
        Color::new(0.84, 0.82, 0.76, 0.92),
    );

    if let Some(utt) = u.utterances.last() {
        let bw = (sw - 120.0).min(860.0);
        let bx = (sw - bw) * 0.5;
        let by = sh - 160.0;
        draw_rectangle(bx, by, bw, 76.0, Color::new(0.10, 0.09, 0.08, 0.88));
        draw_rectangle_lines(bx, by, bw, 76.0, 1.5, Color::new(0.75, 0.64, 0.40, 0.9));
        draw_text(&utt.speaker, bx + 14.0, by + 22.0, 19.0, Color::new(0.96, 0.84, 0.52, 1.0));
        draw_text(&utt.text, bx + 14.0, by + 48.0, 17.0, Color::new(0.93, 0.92, 0.88, 1.0));
    }

    if u.status_toast.1 > 0.0 {
        let tw = measure_text(&u.status_toast.0, None, 18, 1.0).width + 28.0;
        let tx = (sw - tw) * 0.5;
        draw_rectangle(tx, 18.0, tw, 30.0, Color::new(0.10, 0.12, 0.10, 0.86));
        draw_text(&u.status_toast.0, tx + 14.0, 38.0, 18.0, Color::new(0.95, 0.90, 0.72, 1.0));
    }

    if *forge_open {
        let px = sw - 330.0;
        let py = 14.0;
        draw_rectangle(px, py, 316.0, 370.0, Color::new(0.08, 0.09, 0.11, 0.92));
        draw_rectangle_lines(px, py, 316.0, 370.0, 2.0, Color::new(0.82, 0.68, 0.36, 0.95));
        draw_text("UNIVERSE FORGE (Live Laws)", px + 14.0, py + 28.0, 20.0, Color::new(0.96, 0.86, 0.56, 1.0));
        draw_text(&format!("Preset: {}", u.cfg.preset_name), px + 14.0, py + 50.0, 15.0, WHITE);

        let (mx, my) = mouse_position();
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        let btn = |label: &str, x: f32, y: f32, w: f32, h: f32| -> bool {
            let hov = mx >= x && mx <= x + w && my >= y && my <= y + h;
            draw_rectangle(x, y, w, h, if hov { Color::new(0.28, 0.34, 0.26, 1.0) } else { Color::new(0.18, 0.20, 0.22, 1.0) });
            draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.65, 0.60, 0.45, 0.8));
            draw_text(label, x + 8.0, y + h * 0.68, 15.0, WHITE);
            hov && clicked
        };

        if btn("1744 Lochaber (Bright)", px + 14.0, py + 64.0, 288.0, 26.0) {
            *u = Universe::new(UniverseConfig::preset_1744_lochaber());
        }
        if btn("1746 Bleak Rising (Dark)", px + 14.0, py + 96.0, 288.0, 26.0) {
            *u = Universe::new(UniverseConfig::preset_1746_bleak_rising());
        }
        if btn("Party Chaos (Bounce Arena)", px + 14.0, py + 128.0, 288.0, 26.0) {
            *u = Universe::new(UniverseConfig::preset_party_chaos());
        }

        let rows: [(&str, f32); 5] = [
            ("Era Darkness", u.cfg.era_darkness),
            ("Rain Probability", u.cfg.rain_probability),
            ("Lethality Mult", u.cfg.lethality_mult),
            ("Knockback Mult", u.cfg.knockback_mult),
            ("Wind Speed (m/s)", u.cfg.base_wind_mps),
        ];
        for (i, (label, val)) in rows.iter().enumerate() {
            let ry = py + 174.0 + i as f32 * 34.0;
            draw_text(&format!("{}: {:.2}", label, val), px + 14.0, ry + 18.0, 15.0, LIGHTGRAY);
            if btn("-", px + 230.0, ry, 32.0, 24.0) {
                match i {
                    0 => u.cfg.era_darkness = (u.cfg.era_darkness - 0.1).clamp(0.0, 1.0),
                    1 => u.cfg.rain_probability = (u.cfg.rain_probability - 0.15).clamp(0.0, 1.0),
                    2 => u.cfg.lethality_mult = (u.cfg.lethality_mult - 0.25).clamp(0.0, 2.0),
                    3 => u.cfg.knockback_mult = (u.cfg.knockback_mult - 0.4).clamp(0.2, 5.0),
                    4 => u.cfg.base_wind_mps = (u.cfg.base_wind_mps - 1.5).clamp(0.0, 18.0),
                    _ => {}
                }
            }
            if btn("+", px + 268.0, ry, 32.0, 24.0) {
                match i {
                    0 => u.cfg.era_darkness = (u.cfg.era_darkness + 0.1).clamp(0.0, 1.0),
                    1 => u.cfg.rain_probability = (u.cfg.rain_probability + 0.15).clamp(0.0, 1.0),
                    2 => u.cfg.lethality_mult = (u.cfg.lethality_mult + 0.25).clamp(0.0, 2.0),
                    3 => u.cfg.knockback_mult = (u.cfg.knockback_mult + 0.4).clamp(0.2, 5.0),
                    4 => u.cfg.base_wind_mps = (u.cfg.base_wind_mps + 1.5).clamp(0.0, 18.0),
                    _ => {}
                }
            }
        }

        if btn("Close Forge [TAB]", px + 14.0, py + 340.0, 288.0, 22.0) {
            *forge_open = false;
        }
    }
}
