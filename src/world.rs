use crate::config::UniverseConfig;
use macroquad::prelude::Vec2;

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32)
    }

    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }
}

pub fn hash2d(seed: u64, x: i32, y: i32) -> f32 {
    let mut h = seed
        ^ ((x as i64 as u64).wrapping_mul(0x517C_C1B7_2722_0A95))
        ^ ((y as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = h ^ (h >> 27);
    ((h >> 40) as f32) / ((1u64 << 24) as f32)
}

pub fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let a = hash2d(seed, xi, yi);
    let b = hash2d(seed, xi + 1, yi);
    let c = hash2d(seed, xi, yi + 1);
    let d = hash2d(seed, xi + 1, yi + 1);
    let ab = a + (b - a) * u;
    let cd = c + (d - c) * u;
    ab + (cd - ab) * v
}

pub fn fbm2d(seed: u64, x: f32, y: f32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += value_noise(seed.wrapping_add(i as u64 * 1013), x * freq, y * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm.max(0.0001)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainKind {
    LochWater,
    MudRoad,
    PeatBog,
    Heather,
    Meadow,
    GraniteCrag,
}

pub fn road_center_x(y: f32) -> f32 {
    6.0 + (y * 0.055).sin() * 7.5 + (y * 0.02).cos() * 3.0
}

pub fn sample_terrain(seed: u64, x: f32, y: f32) -> (f32, TerrainKind) {
    let n = fbm2d(seed, x * 0.025, y * 0.025, 4);
    let detail = fbm2d(seed.wrapping_add(77), x * 0.09, y * 0.09, 2);

    let loch_edge = -28.0 + (y * 0.045).sin() * 6.5 + (n - 0.5) * 8.0;
    if x < loch_edge {
        return (-2.0, TerrainKind::LochWater);
    }

    let east_slope = ((x + 25.0) * 0.14).max(0.0);
    let elev = east_slope + (n - 0.45) * 8.0;

    let rx = road_center_x(y);
    if (x - rx).abs() < 2.25 {
        return (elev.max(0.5), TerrainKind::MudRoad);
    }

    if elev > 11.0 {
        return (elev, TerrainKind::GraniteCrag);
    }
    if n < 0.34 && x < 2.0 {
        return (elev.max(0.2), TerrainKind::PeatBog);
    }
    if detail > 0.53 {
        return (elev, TerrainKind::Heather);
    }
    (elev, TerrainKind::Meadow)
}

#[derive(Clone, Debug)]
pub struct WeatherState {
    pub day_count: u32,
    pub time_of_day: f32,
    pub sun_warmth: f32,
    pub sun_dir: Vec2,
    pub wind_vec: Vec2,
    pub rain_intensity: f32,
    pub mist_intensity: f32,
    pub ambient_temp_k: f32,
}

impl WeatherState {
    pub fn new(cfg: &UniverseConfig) -> Self {
        Self {
            day_count: 1,
            time_of_day: 0.36,
            sun_warmth: 0.9,
            sun_dir: Vec2::new(0.65, 0.45),
            wind_vec: Vec2::new(cfg.base_wind_mps, 1.2),
            rain_intensity: if cfg.rain_probability > 0.5 { 0.55 } else { 0.0 },
            mist_intensity: cfg.mist_baseline,
            ambient_temp_k: cfg.base_temp_kelvin,
        }
    }

    pub fn step(&mut self, dt: f32, cfg: &UniverseConfig) {
        self.time_of_day += dt / cfg.day_length_sec.max(30.0);
        if self.time_of_day >= 1.0 {
            self.time_of_day -= 1.0;
            self.day_count += 1;
        }
        let angle = (self.time_of_day - 0.25) * std::f32::consts::TAU;
        let sun_h = angle.sin().clamp(-0.4, 1.0);
        self.sun_dir = Vec2::new(angle.cos() * 0.9, 0.35 + (1.0 - sun_h.max(0.0)) * 0.55);
        self.sun_warmth = (sun_h.max(0.0) * (1.0 - cfg.era_darkness * 0.65)).clamp(0.08, 1.0);

        let w_phase = (self.day_count as f32 * 2.7 + self.time_of_day * 6.283).sin() * 0.5 + 0.5;
        let target_rain = if w_phase < cfg.rain_probability {
            ((cfg.rain_probability - w_phase) / cfg.rain_probability.max(0.05)).clamp(0.15, 1.0)
        } else {
            0.0
        };
        self.rain_intensity += (target_rain - self.rain_intensity) * (dt * 0.35).min(1.0);
        self.mist_intensity = (cfg.mist_baseline + self.rain_intensity * 0.28 + (1.0 - sun_h.max(0.0)) * 0.15).clamp(0.0, 0.85);

        let wind_ang = 0.25 + (self.time_of_day * 4.0).sin() * 0.45;
        let wind_mag = cfg.base_wind_mps * (1.0 + self.rain_intensity * 0.6);
        self.wind_vec = Vec2::new(wind_ang.cos() * wind_mag, wind_ang.sin() * wind_mag);

        self.ambient_temp_k = cfg.base_temp_kelvin + sun_h * 5.0 - self.rain_intensity * 3.5;
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TraceKind {
    BootPrint,
    HoofPrint,
    BloodDrop,
    Scorch,
}

#[derive(Clone, Copy, Debug)]
pub struct TraceMark {
    pub pos: Vec2,
    pub kind: TraceKind,
    pub age: f32,
    pub max_age: f32,
}

#[derive(Clone, Debug)]
pub struct SoundWave {
    pub pos: Vec2,
    pub radius: f32,
    pub max_radius: f32,
    pub decibels: f32,
    pub age: f32,
    pub hostiles_care: bool,
    pub whistle: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SmokePuff {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
    pub age: f32,
    pub max_age: f32,
    pub is_ember: bool,
}

#[derive(Clone, Debug)]
pub struct BuildingRecord {
    pub id: u32,
    pub name: String,
    pub role: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub door_pos: Vec2,
    pub hearth_pos: Vec2,
    pub hearth_lit: bool,
    pub roof_alpha: f32,
}

impl BuildingRecord {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    pub fn near_door(&self, p: Vec2) -> bool {
        p.distance(self.door_pos) < 1.85
    }

    pub fn resolve_wall_collision(&self, prev: Vec2, next: Vec2, radius: f32) -> Vec2 {
        let was_in = self.contains(prev);
        let now_in = self.contains(next);
        if was_in == now_in || self.near_door(prev) || self.near_door(next) {
            return next;
        }
        let mut out = next;
        if !was_in && now_in {
            let dl = (next.x - self.x).abs();
            let dr = (self.x + self.w - next.x).abs();
            let dt = (next.y - self.y).abs();
            let db = (self.y + self.h - next.y).abs();
            let m = dl.min(dr).min(dt).min(db);
            if m == dl { out.x = self.x - radius; }
            else if m == dr { out.x = self.x + self.w + radius; }
            else if m == dt { out.y = self.y - radius; }
            else { out.y = self.y + self.h + radius; }
        } else if was_in && !now_in {
            out.x = out.x.clamp(self.x + radius, self.x + self.w - radius);
            out.y = out.y.clamp(self.y + radius, self.y + self.h - radius);
        }
        out
    }

    pub fn blocks_segment(&self, a: Vec2, b: Vec2) -> bool {
        for i in 1..8 {
            let t = i as f32 / 8.0;
            let p = a.lerp(b, t);
            if self.contains(p) && !self.near_door(p) {
                return true;
            }
        }
        false
    }
}

pub fn generate_clachan_buildings() -> Vec<BuildingRecord> {
    vec![
        BuildingRecord {
            id: 1,
            name: "Callum's Stone Bothy".to_string(),
            role: "Homestead".to_string(),
            x: -13.0,
            y: -8.0,
            w: 8.0,
            h: 6.0,
            door_pos: Vec2::new(-9.0, -2.0),
            hearth_pos: Vec2::new(-11.4, -6.4),
            hearth_lit: true,
            roof_alpha: 1.0,
        },
        BuildingRecord {
            id: 2,
            name: "MacRae Smithy".to_string(),
            role: "Blacksmith".to_string(),
            x: -14.0,
            y: 8.0,
            w: 9.0,
            h: 6.5,
            door_pos: Vec2::new(-9.5, 14.5),
            hearth_pos: Vec2::new(-12.2, 9.6),
            hearth_lit: true,
            roof_alpha: 1.0,
        },
        BuildingRecord {
            id: 3,
            name: "The Heather Alehouse".to_string(),
            role: "Tavern".to_string(),
            x: -16.0,
            y: -22.0,
            w: 10.5,
            h: 7.0,
            door_pos: Vec2::new(-10.8, -15.0),
            hearth_pos: Vec2::new(-14.2, -20.2),
            hearth_lit: true,
            roof_alpha: 1.0,
        },
        BuildingRecord {
            id: 4,
            name: "Wade's Toll Watch-Post".to_string(),
            role: "Garrison".to_string(),
            x: 19.0,
            y: -14.0,
            w: 8.5,
            h: 6.5,
            door_pos: Vec2::new(23.2, -7.5),
            hearth_pos: Vec2::new(25.8, -12.2),
            hearth_lit: true,
            roof_alpha: 1.0,
        },
    ]
}
