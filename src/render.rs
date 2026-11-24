use crate::config::UniverseConfig;
use crate::entities::{AiGoal, EntityKind, PlayerId};
use crate::sim::Universe;
use crate::world::{fbm2d, sample_terrain, Rng, TerrainKind, TraceKind};
use macroquad::prelude::*;

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
    vec4 c1 = texture2D(Texture, uv + vec2(px.x * 1.4, px.y * 0.6));
    vec4 c2 = texture2D(Texture, uv - vec2(px.x * 0.9, px.y * 1.2));
    vec3 col = (c0.rgb * 0.62 + c1.rgb * 0.20 + c2.rgb * 0.18);

    float weave = sin(uv.x * u_res_x * 0.75) * sin(uv.y * u_res_y * 0.75) * 0.018;
    float grain = (hash(floor(uv * vec2(u_res_x, u_res_y) * 0.5)) - 0.5) * 0.025;
    col += vec3(weave + grain);

    vec3 warm_tint = vec3(1.08, 1.02, 0.90);
    vec3 bleak_tint = vec3(0.66, 0.74, 0.84);
    float luma = dot(col, vec3(0.299, 0.587, 0.114));
    vec3 desat = mix(col, vec3(luma), u_darkness * 0.52);
    vec3 grade = mix(warm_tint, bleak_tint, u_darkness);
    col = desat * grade * (0.55 + u_warmth * 0.55);

    float mist_wave = 0.5 + 0.5 * sin(uv.x * 7.0 + u_time * 0.35 + sin(uv.y * 5.0));
    vec3 mist_col = mix(vec3(0.78, 0.82, 0.76), vec3(0.42, 0.48, 0.54), u_darkness);
    col = mix(col, mist_col, u_mist * 0.28 * (0.5 + 0.5 * mist_wave));

    vec2 dv = uv - vec2(0.5);
    float vig = 1.0 - dot(dv, dv) * (0.45 + u_darkness * 0.35);
    col *= clamp(vig, 0.35, 1.05);

    gl_FragColor = vec4(col, 1.0) * color;
}
"#;

pub struct PaintedAtlas {
    pub meadow: Texture2D,
    pub heather: Texture2D,
    pub road: Texture2D,
    pub bog: Texture2D,
    pub granite: Texture2D,
    pub water: Texture2D,
    pub roof: Texture2D,
    pub floor: Texture2D,
    pub pine: Texture2D,
    pub post_mat: Material,
}

fn paint_tile(
    size: u16,
    seed: u64,
    base: [f32; 3],
    accent: [f32; 3],
    highlight: [f32; 3],
    stroke_count: usize,
    streaky: bool,
) -> Texture2D {
    let mut img = Image::gen_image_color(size, size, Color::new(base[0], base[1], base[2], 1.0));
    let mut rng = Rng::new(seed);

    for y in 0..size {
        for x in 0..size {
            let n = fbm2d(seed, x as f32 * 0.08, y as f32 * 0.08, 3);
            let r = (base[0] * (0.85 + n * 0.3) + accent[0] * n * 0.18).clamp(0.0, 1.0);
            let g = (base[1] * (0.85 + n * 0.3) + accent[1] * n * 0.18).clamp(0.0, 1.0);
            let b = (base[2] * (0.85 + n * 0.3) + accent[2] * n * 0.18).clamp(0.0, 1.0);
            img.set_pixel(x as u32, y as u32, Color::new(r, g, b, 1.0));
        }
    }

    for _ in 0..stroke_count {
        let cx = rng.range_f32(0.0, size as f32);
        let cy = rng.range_f32(0.0, size as f32);
        let len = if streaky { rng.range_f32(7.0, 16.0) } else { rng.range_f32(4.0, 10.0) };
        let ang = if streaky { rng.range_f32(-0.25, 0.25) } else { rng.range_f32(-0.9, 0.9) };
        let pick = rng.next_f32();
        let c = if pick < 0.5 { accent } else if pick < 0.85 { base } else { highlight };
        let bright = rng.range_f32(0.86, 1.15);

        for s in 0..(len as i32) {
            let px = (cx + ang.cos() * s as f32).rem_euclid(size as f32) as u32;
            let py = (cy + ang.sin() * s as f32).rem_euclid(size as f32) as u32;
            let col = Color::new(
                (c[0] * bright).clamp(0.0, 1.0),
                (c[1] * bright).clamp(0.0, 1.0),
                (c[2] * bright).clamp(0.0, 1.0),
                1.0,
            );
            img.set_pixel(px, py, col);
            if py + 1 < size as u32 {
                img.set_pixel(px, py + 1, Color::new(col.r * 0.92, col.g * 0.92, col.b * 0.92, 1.0));
            }
        }
    }

    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn paint_pine(size: u16) -> Texture2D {
    let mut img = Image::gen_image_color(size, size, Color::new(0.0, 0.0, 0.0, 0.0));
    let mut rng = Rng::new(7741);
    let cx = size as f32 * 0.5;
    let cy = size as f32 * 0.5;
    for _ in 0..420 {
        let r = rng.next_f32().sqrt() * (size as f32 * 0.44);
        let a = rng.range_f32(0.0, std::f32::consts::TAU);
        let px = cx + a.cos() * r;
        let py = cy + a.sin() * r;
        let sun = ((px - cx) * -0.5 + (py - cy) * -0.5) / (size as f32 * 0.5);
        let col = Color::new(
            (0.11 + sun.max(0.0) * 0.12).clamp(0.05, 0.35),
            (0.24 + sun.max(0.0) * 0.20 + rng.range_f32(-0.04, 0.05)).clamp(0.12, 0.52),
            (0.14 + sun.max(0.0) * 0.08).clamp(0.06, 0.30),
            0.96,
        );
        for dx in -1..=1 {
            for dy in -1..=1 {
                let ix = px as i32 + dx;
                let iy = py as i32 + dy;
                if ix >= 0 && iy >= 0 && ix < size as i32 && iy < size as i32 {
                    img.set_pixel(ix as u32, iy as u32, col);
                }
            }
        }
    }
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
            meadow: paint_tile(64, 101, [0.27, 0.43, 0.21], [0.35, 0.52, 0.24], [0.55, 0.63, 0.30], 260, false),
            heather: paint_tile(64, 202, [0.31, 0.36, 0.23], [0.49, 0.31, 0.48], [0.68, 0.46, 0.64], 340, false),
            road: paint_tile(64, 303, [0.41, 0.33, 0.24], [0.32, 0.25, 0.18], [0.53, 0.46, 0.35], 280, true),
            bog: paint_tile(64, 404, [0.20, 0.23, 0.16], [0.14, 0.18, 0.14], [0.32, 0.35, 0.24], 240, false),
            granite: paint_tile(64, 505, [0.43, 0.44, 0.42], [0.32, 0.34, 0.35], [0.62, 0.62, 0.58], 300, false),
            water: paint_tile(64, 606, [0.15, 0.30, 0.38], [0.20, 0.39, 0.47], [0.46, 0.64, 0.68], 300, true),
            roof: paint_tile(64, 707, [0.48, 0.38, 0.23], [0.36, 0.28, 0.16], [0.64, 0.53, 0.34], 380, true),
            floor: paint_tile(64, 808, [0.33, 0.26, 0.19], [0.25, 0.19, 0.14], [0.43, 0.35, 0.26], 240, true),
            pine: paint_pine(64),
            post_mat,
        }
    }
}

pub fn draw_world(u: &Universe, atlas: &PaintedAtlas, cam: Vec2, view_w_m: f32, view_h_m: f32) {
    let tile_m = 2.0f32;
    let min_tx = ((cam.x - view_w_m * 0.6) / tile_m).floor() as i32;
    let max_tx = ((cam.x + view_w_m * 0.6) / tile_m).ceil() as i32;
    let min_ty = ((cam.y - view_h_m * 0.6) / tile_m).floor() as i32;
    let max_ty = ((cam.y + view_h_m * 0.6) / tile_m).ceil() as i32;

    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            let wx = tx as f32 * tile_m;
            let wy = ty as f32 * tile_m;
            let (elev, kind) = sample_terrain(u.cfg.world_seed, wx + 1.0, wy + 1.0);
            let tex = match kind {
                TerrainKind::LochWater => &atlas.water,
                TerrainKind::MudRoad => &atlas.road,
                TerrainKind::PeatBog => &atlas.bog,
                TerrainKind::Heather => &atlas.heather,
                TerrainKind::Meadow => &atlas.meadow,
                TerrainKind::GraniteCrag => &atlas.granite,
            };
            let shade = (0.86 + (elev * 0.018).clamp(-0.12, 0.16)).clamp(0.7, 1.05);
            let water_shimmer = if kind == TerrainKind::LochWater {
                (u.time * 2.2 + wx * 0.5 + wy * 0.3).sin() * 0.06
            } else {
                0.0
            };
            let tint = Color::new(
                (shade + water_shimmer).clamp(0.0, 1.0),
                (shade + water_shimmer).clamp(0.0, 1.0),
                (shade + water_shimmer * 1.4).clamp(0.0, 1.0),
                1.0,
            );
            draw_texture_ex(
                tex,
                wx,
                wy,
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(tile_m + 0.06, tile_m + 0.06)),
                    ..Default::default()
                },
            );
        }
    }

    for tr in &u.traces {
        let a = (1.0 - tr.age / tr.max_age).clamp(0.0, 1.0) * 0.65;
        let (col, r) = match tr.kind {
            TraceKind::BootPrint => (Color::new(0.18, 0.13, 0.09, a), 0.16),
            TraceKind::HoofPrint => (Color::new(0.16, 0.11, 0.07, a), 0.24),
            TraceKind::BloodDrop => (Color::new(0.55, 0.05, 0.05, a * 1.2), 0.22),
            TraceKind::Scorch => (Color::new(0.08, 0.08, 0.08, a), 0.55),
        };
        draw_circle(tr.pos.x, tr.pos.y, r, col);
    }

    for sw in &u.sound_waves {
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
    for b in &u.buildings {
        draw_rectangle(
            b.x + shadow_off.x,
            b.y + shadow_off.y,
            b.w,
            b.h,
            Color::new(0.05, 0.06, 0.08, 0.34 * u.weather.sun_warmth),
        );
        draw_texture_ex(
            &atlas.floor,
            b.x,
            b.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(b.w, b.h)),
                ..Default::default()
            },
        );
        draw_rectangle_lines(b.x, b.y, b.w, b.h, 0.45, Color::new(0.36, 0.35, 0.33, 1.0));
        draw_rectangle(b.door_pos.x - 0.8, b.y + b.h - 0.3, 1.6, 0.55, Color::new(0.45, 0.32, 0.18, 1.0));
        draw_circle(b.hearth_pos.x, b.hearth_pos.y, 0.72, Color::new(0.24, 0.22, 0.21, 1.0));
        if b.hearth_lit {
            let flick = 0.85 + (u.time * 14.0 + b.id as f32).sin() * 0.15;
            draw_circle(b.hearth_pos.x, b.hearth_pos.y, 2.4 * flick, Color::new(1.0, 0.56, 0.18, 0.16));
            draw_circle(b.hearth_pos.x, b.hearth_pos.y, 0.42 * flick, Color::new(1.0, 0.65, 0.18, 0.95));
        }
    }

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

    let mut order: Vec<usize> = (0..u.entities.len()).collect();
    order.sort_by(|&a, &b| u.entities[a].pos.y.partial_cmp(&u.entities[b].pos.y).unwrap_or(std::cmp::Ordering::Equal));

    for idx in order {
        let e = &u.entities[idx];
        if e.buried {
            continue;
        }
        let p = e.pos;
        let sh_rad = match e.kind {
            EntityKind::Mount => 0.95,
            EntityKind::Cairn => 0.85,
            _ => 0.55,
        };
        draw_circle(
            p.x + shadow_off.x * 0.45,
            p.y + 0.35 + shadow_off.y * 0.35,
            sh_rad,
            Color::new(0.04, 0.05, 0.06, 0.34),
        );

        match e.kind {
            EntityKind::Cairn => {
                draw_circle(p.x - 0.28, p.y + 0.12, 0.36, Color::new(0.46, 0.47, 0.45, 1.0));
                draw_circle(p.x + 0.26, p.y + 0.14, 0.34, Color::new(0.40, 0.42, 0.41, 1.0));
                draw_circle(p.x, p.y - 0.05, 0.38, Color::new(0.55, 0.56, 0.52, 1.0));
                draw_circle(p.x, p.y - 0.32, 0.22, Color::new(0.64, 0.64, 0.60, 1.0));
            }
            EntityKind::Campfire => {
                let fl = 0.85 + (u.time * 16.0).sin() * 0.15;
                draw_circle(p.x, p.y, 2.5 * fl, Color::new(1.0, 0.55, 0.16, 0.18));
                draw_circle(p.x, p.y, 0.42, Color::new(0.22, 0.16, 0.12, 1.0));
                draw_circle(p.x, p.y - 0.05, 0.32 * fl, Color::new(1.0, 0.68, 0.20, 0.95));
            }
            EntityKind::Mount => {
                let dir = e.facing.normalize_or_zero();
                draw_circle(p.x - dir.x * 0.35, p.y - dir.y * 0.2, 0.68, Color::new(0.47, 0.34, 0.22, 1.0));
                draw_circle(p.x + dir.x * 0.35, p.y + dir.y * 0.2, 0.62, Color::new(0.52, 0.38, 0.24, 1.0));
                draw_circle(p.x + dir.x * 0.85, p.y + dir.y * 0.55 - 0.25, 0.34, Color::new(0.38, 0.26, 0.17, 1.0));
                draw_rectangle(p.x - 0.32, p.y - 0.28, 0.64, 0.42, Color::new(0.28, 0.18, 0.11, 1.0));
            }
            EntityKind::Deer => {
                draw_circle(p.x, p.y, 0.46, Color::new(0.58, 0.39, 0.24, 1.0));
                draw_circle(p.x + e.facing.x * 0.4, p.y - 0.25, 0.22, Color::new(0.52, 0.34, 0.20, 1.0));
            }
            EntityKind::Corpse => {
                let y_off = -e.z * 0.55;
                draw_rectangle(p.x - 0.32, p.y - 0.2 + y_off, 0.64, 0.85, Color::new(0.24, 0.31, 0.24, 1.0));
                draw_circle(p.x, p.y - 0.42 + y_off, 0.26, Color::new(0.76, 0.65, 0.56, 1.0));
            }
            EntityKind::Player | EntityKind::Villager | EntityKind::Redcoat => {
                let y_lift = if e.mounted_id.is_some() { -0.45 } else { 0.0 };
                let (coat, accent) = match e.kind {
                    EntityKind::Player => (Color::new(0.18, 0.36, 0.24, 1.0), Color::new(0.68, 0.18, 0.18, 1.0)),
                    EntityKind::Redcoat => (Color::new(0.74, 0.15, 0.14, 1.0), Color::new(0.92, 0.88, 0.78, 1.0)),
                    _ => (Color::new(0.38, 0.34, 0.27, 1.0), Color::new(0.24, 0.38, 0.34, 1.0)),
                };
                draw_circle(p.x, p.y + y_lift, 0.48, coat);
                draw_line(p.x - 0.3, p.y - 0.25 + y_lift, p.x + 0.3, p.y + 0.25 + y_lift, 0.16, accent);
                if e.blood_stain > 0.1 {
                    draw_circle(p.x + 0.12, p.y + 0.1 + y_lift, 0.22, Color::new(0.58, 0.05, 0.05, e.blood_stain * 0.8));
                }
                draw_circle(p.x, p.y - 0.46 + y_lift, 0.28, Color::new(0.84, 0.70, 0.58, 1.0));
                let hat = if e.kind == EntityKind::Redcoat {
                    Color::new(0.12, 0.12, 0.14, 1.0)
                } else {
                    Color::new(0.16, 0.22, 0.32, 1.0)
                };
                draw_circle(p.x, p.y - 0.62 + y_lift, 0.24, hat);

                if e.swing_timer > 0.0 {
                    let tip = p + e.facing * 1.65;
                    draw_line(p.x, p.y + y_lift, tip.x, tip.y + y_lift, 0.12, Color::new(0.88, 0.92, 0.96, 0.95));
                }

                if e.kind == EntityKind::Redcoat {
                    if let Some(m) = &e.mind {
                        if m.awareness > 0.25 {
                            let col = if m.goal == AiGoal::Engage { RED } else { YELLOW };
                            draw_circle(p.x, p.y - 1.15, 0.16, col);
                        }
                    }
                }
            }
        }
    }

    for b in &u.buildings {
        if b.roof_alpha > 0.02 {
            let tint = Color::new(1.0, 1.0, 1.0, b.roof_alpha);
            draw_texture_ex(
                &atlas.roof,
                b.x - 0.25,
                b.y - 0.35,
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(b.w + 0.5, b.h + 0.5)),
                    ..Default::default()
                },
            );
            draw_line(
                b.x - 0.2,
                b.y + b.h * 0.45,
                b.x + b.w + 0.2,
                b.y + b.h * 0.45,
                0.22,
                Color::new(0.28, 0.21, 0.13, b.roof_alpha),
            );
            draw_rectangle(
                b.hearth_pos.x - 0.45,
                b.hearth_pos.y - 0.45,
                0.9,
                0.9,
                Color::new(0.38, 0.37, 0.35, b.roof_alpha),
            );
        }
    }

    for (tp, rad) in &u.trees {
        draw_circle(
            tp.x + shadow_off.x * 0.85,
            tp.y + shadow_off.y * 0.85,
            rad * 0.9,
            Color::new(0.04, 0.06, 0.06, 0.30 * u.weather.sun_warmth),
        );
        let sway = (u.time * 1.8 + tp.x * 0.4).sin() * 0.07 * (u.weather.wind_vec.length() * 0.2);
        draw_texture_ex(
            &atlas.pine,
            tp.x - *rad + sway,
            tp.y - *rad,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(*rad * 2.0, *rad * 2.0)),
                ..Default::default()
            },
        );
    }

    for sp in &u.smoke {
        let a = (1.0 - sp.age / sp.max_age).clamp(0.0, 1.0);
        let col = if sp.is_ember {
            Color::new(1.0, 0.62, 0.18, a)
        } else {
            Color::new(0.76, 0.78, 0.78, a * 0.38)
        };
        draw_circle(sp.pos.x, sp.pos.y, sp.radius, col);
    }

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

pub fn draw_hud_and_forge(u: &mut Universe, forge_open: &mut bool) {
    let sw = screen_width();
    let sh = screen_height();

    draw_rectangle(14.0, 14.0, 340.0, 118.0, Color::new(0.09, 0.10, 0.09, 0.78));
    draw_rectangle_lines(14.0, 14.0, 340.0, 118.0, 1.5, Color::new(0.62, 0.54, 0.36, 0.8));

    let hour = (u.weather.time_of_day * 24.0) as u32;
    let min = ((u.weather.time_of_day * 24.0 - hour as f32) * 60.0) as u32;
    draw_text(
        &format!("CLACHAN  |  {}  |  Day {} {:02}:{:02}", u.cfg.era_year, u.weather.day_count, hour, min),
        24.0,
        36.0,
        18.0,
        Color::new(0.95, 0.88, 0.72, 1.0),
    );

    if let Some(pi) = u.player_index(PlayerId(1)) {
        let p = &u.entities[pi];
        let bars = [
            ("Health", p.health / 100.0, Color::new(0.76, 0.22, 0.20, 1.0)),
            ("Stamina", p.stamina / 100.0, Color::new(0.32, 0.68, 0.36, 1.0)),
            ("Wool Dryness", 1.0 - p.wetness, Color::new(0.38, 0.64, 0.84, 1.0)),
            ("Powder Dry", 1.0 - p.powder_wetness, Color::new(0.86, 0.68, 0.30, 1.0)),
        ];
        for (idx, (label, val, col)) in bars.iter().enumerate() {
            let by = 48.0 + idx as f32 * 19.0;
            draw_text(label, 24.0, by + 11.0, 15.0, Color::new(0.85, 0.84, 0.80, 0.95));
            draw_rectangle(135.0, by, 204.0, 12.0, Color::new(0.18, 0.19, 0.18, 0.9));
            draw_rectangle(135.0, by, 204.0 * val.clamp(0.0, 1.0), 12.0, *col);
        }
    }

    draw_rectangle(14.0, sh - 38.0, sw - 28.0, 26.0, Color::new(0.08, 0.09, 0.08, 0.75));
    draw_text(
        "[WASD] Move  [Click/Space] Steel  [F] Flintlock  [E] Talk/Mount/Carry  [B] Bury Cairn  [H] Whistle Bramble  [C] Fire  [R] Bandage  [V] Whet  [TAB] Universe Forge  [F5/F9] Save/Load",
        22.0,
        sh - 20.0,
        15.0,
        Color::new(0.84, 0.82, 0.76, 0.92),
    );

    if let Some(utt) = u.utterances.last() {
        let bw = (sw - 120.0).min(860.0);
        let bx = (sw - bw) * 0.5;
        let by = sh - 126.0;
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
