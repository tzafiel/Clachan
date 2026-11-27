use crate::config::UniverseConfig;
use chrono::{Datelike, Local, Timelike};
use macroquad::prelude::Vec2;

pub const TILE_SIZE_M: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Overworld,
    Interior(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn from_month(month: u32) -> Self {
        match month {
            3..=5 => Self::Spring,
            6..=8 => Self::Summer,
            9..=11 => Self::Autumn,
            _ => Self::Winter,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherMode {
    Natural,
    Clear,
    Mist,
    Rain,
    Snow,
}

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
    TilledSoil,
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
    pub season: Season,
    pub time_of_day: f32,
    pub time_offset_hours: f32,
    pub sun_warmth: f32,
    pub sun_dir: Vec2,
    pub wind_vec: Vec2,
    pub rain_intensity: f32,
    pub snow_intensity: f32,
    pub mist_intensity: f32,
    pub ambient_temp_k: f32,
    pub weather_mode: WeatherMode,
}

impl WeatherState {
    pub fn new(cfg: &UniverseConfig) -> Self {
        let now = Local::now();
        Self {
            day_count: now.ordinal(),
            season: Season::from_month(now.month()),
            time_of_day: local_time_of_day(0.0),
            time_offset_hours: 0.0,
            sun_warmth: 0.9,
            sun_dir: Vec2::new(0.65, 0.45),
            wind_vec: Vec2::new(cfg.base_wind_mps, 1.2),
            rain_intensity: 0.0,
            snow_intensity: 0.0,
            mist_intensity: cfg.mist_baseline,
            ambient_temp_k: cfg.base_temp_kelvin,
            weather_mode: WeatherMode::Natural,
        }
    }

    pub fn step(&mut self, dt: f32, cfg: &UniverseConfig) {
        let now = Local::now();
        self.season = Season::from_month(now.month());
        self.time_of_day = local_time_of_day(self.time_offset_hours);
        let angle = (self.time_of_day - 0.25) * std::f32::consts::TAU;
        let sun_h = angle.sin().clamp(-0.4, 1.0);
        self.sun_dir = Vec2::new(angle.cos() * 0.9, 0.35 + (1.0 - sun_h.max(0.0)) * 0.55);
        self.sun_warmth = (sun_h.max(0.0) * (1.0 - cfg.era_darkness * 0.65)).clamp(0.08, 1.0);

        let w_phase = (now.ordinal() as f32 * 2.7 + self.time_of_day * 6.283).sin() * 0.5 + 0.5;
        let natural_precipitation = if w_phase < cfg.rain_probability {
            ((cfg.rain_probability - w_phase) / cfg.rain_probability.max(0.05)).clamp(0.15, 1.0)
        } else {
            0.0
        };
        let (target_rain, target_snow, target_mist) = match self.weather_mode {
            WeatherMode::Natural => {
                if self.season == Season::Winter {
                    (
                        natural_precipitation * 0.3,
                        natural_precipitation * 0.7,
                        cfg.mist_baseline,
                    )
                } else {
                    (natural_precipitation, 0.0, cfg.mist_baseline)
                }
            }
            WeatherMode::Clear => (0.0, 0.0, 0.0),
            WeatherMode::Mist => (0.0, 0.0, 0.72),
            WeatherMode::Rain => (0.75, 0.0, cfg.mist_baseline),
            WeatherMode::Snow => (0.0, 0.75, cfg.mist_baseline),
        };
        let smoothing = |current: f32, target: f32, rate: f32| {
            current + (target - current) * (1.0 - (-rate * dt.max(0.0)).exp())
        };
        self.rain_intensity = smoothing(self.rain_intensity, target_rain, 0.32);
        self.snow_intensity = smoothing(self.snow_intensity, target_snow, 0.22);
        let night_mist = if self.weather_mode == WeatherMode::Natural {
            (1.0 - sun_h.max(0.0)) * 0.12
        } else {
            0.0
        };
        let mist_target =
            (target_mist + self.rain_intensity * 0.28 + self.snow_intensity * 0.18 + night_mist)
                .clamp(0.0, 0.85);
        self.mist_intensity = smoothing(self.mist_intensity, mist_target, 0.18);

        let wind_ang = 0.25 + (self.time_of_day * 4.0).sin() * 0.45;
        let wind_mag =
            cfg.base_wind_mps * (1.0 + self.rain_intensity * 0.6 + self.snow_intensity * 0.25);
        self.wind_vec = Vec2::new(wind_ang.cos() * wind_mag, wind_ang.sin() * wind_mag);

        let seasonal_offset = match self.season {
            Season::Spring => -2.0,
            Season::Summer => 2.0,
            Season::Autumn => -4.0,
            Season::Winter => -10.0,
        };
        self.ambient_temp_k = cfg.base_temp_kelvin + seasonal_offset + sun_h * 5.0
            - self.rain_intensity * 3.5
            - self.snow_intensity * 2.0;
    }

    pub fn cycle_weather_mode(&mut self) {
        self.weather_mode = match self.weather_mode {
            WeatherMode::Natural | WeatherMode::Snow => WeatherMode::Clear,
            WeatherMode::Clear => WeatherMode::Mist,
            WeatherMode::Mist => WeatherMode::Rain,
            WeatherMode::Rain => WeatherMode::Snow,
        };
    }

    pub fn set_time_to_hour(&mut self, target_hour: f32) {
        self.time_offset_hours = target_hour.rem_euclid(24.0) - local_hour();
        self.time_of_day = target_hour.rem_euclid(24.0) / 24.0;
    }
}

fn local_hour() -> f32 {
    let now = Local::now();
    now.hour() as f32 + now.minute() as f32 / 60.0 + now.second() as f32 / 3600.0
}

fn local_time_of_day(offset_hours: f32) -> f32 {
    (local_hour() + offset_hours).rem_euclid(24.0) / 24.0
}

#[cfg(test)]
mod season_tests {
    use super::{Season, UniverseConfig, WeatherMode, WeatherState};

    #[test]
    fn season_matches_local_calendar_months() {
        assert_eq!(Season::from_month(3), Season::Spring);
        assert_eq!(Season::from_month(6), Season::Summer);
        assert_eq!(Season::from_month(9), Season::Autumn);
        assert_eq!(Season::from_month(12), Season::Winter);
        assert_eq!(Season::from_month(0), Season::Winter);
    }

    #[test]
    fn real_time_offset_stays_in_day_and_weather_transitions_smoothly() {
        let config = UniverseConfig::pine_edge();
        let mut weather = WeatherState::new(&config);
        weather.time_offset_hours = 3.0;
        weather.weather_mode = WeatherMode::Rain;

        weather.step(1.0 / 60.0, &config);

        assert!((0.0..1.0).contains(&weather.time_of_day));
        assert!(weather.rain_intensity > 0.0 && weather.rain_intensity < 0.75);
        assert_eq!(weather.snow_intensity, 0.0);
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
    pub location: Location,
    pub pos: Vec2,
    pub kind: TraceKind,
    pub age: f32,
    pub max_age: f32,
}

#[derive(Clone, Debug)]
pub struct SoundWave {
    pub location: Location,
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
    pub location: Location,
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
    pub age: f32,
    pub max_age: f32,
    pub is_ember: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Rug,
    Bed,
    Table,
    Anvil,
    Forge,
    BarCounter,
    Barrel,
    WeaponRack,
    CellBars,
}

#[derive(Clone, Debug)]
pub struct InteriorProp {
    pub kind: PropKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub solid: bool,
    pub prompt: String,
}

impl InteriorProp {
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

#[derive(Clone, Debug)]
pub struct BuildingRecord {
    pub id: u32,
    pub name: String,
    pub role: String,
    // Overworld exterior footprint
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub door_pos: Vec2,
    pub chimney_pos: Vec2,
    pub hearth_lit: bool,
    // Separated Stardew-style interior scene dimensions & layout
    pub int_w: f32,
    pub int_h: f32,
    pub int_exit: Vec2,
    pub int_hearth: Vec2,
    pub props: Vec<InteriorProp>,
}

impl BuildingRecord {
    pub fn contains_exterior(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    pub fn resolve_exterior_collision(&self, prev: Vec2, next: Vec2, radius: f32) -> Vec2 {
        let min_x = self.x - radius;
        let max_x = self.x + self.w + radius;
        let min_y = self.y - radius;
        let max_y = self.y + self.h + radius;
        if next.x > min_x && next.x < max_x && next.y > min_y && next.y < max_y {
            let mut out = next;
            if prev.x <= min_x {
                out.x = min_x;
            } else if prev.x >= max_x {
                out.x = max_x;
            } else if prev.y <= min_y {
                out.y = min_y;
            } else {
                out.y = max_y;
            }
            return out;
        }
        next
    }

    pub fn resolve_interior_collision(&self, prev: Vec2, next: Vec2, radius: f32) -> Vec2 {
        let mut out = next;
        // North stone wall takes up top 1.55m for authentic 3/4 depth
        out.x = out.x.clamp(radius + 0.35, self.int_w - radius - 0.35);
        out.y = out.y.clamp(1.65 + radius, self.int_h - radius - 0.30);

        for prop in &self.props {
            if !prop.solid {
                continue;
            }
            let min_x = prop.x - radius;
            let max_x = prop.x + prop.w + radius;
            let min_y = prop.y - radius;
            let max_y = prop.y + prop.h + radius;
            if out.x > min_x && out.x < max_x && out.y > min_y && out.y < max_y {
                if prev.x <= min_x {
                    out.x = min_x;
                } else if prev.x >= max_x {
                    out.x = max_x;
                } else if prev.y <= min_y {
                    out.y = min_y;
                } else {
                    out.y = max_y;
                }
            }
        }
        out
    }

    pub fn blocks_segment(&self, a: Vec2, b: Vec2) -> bool {
        for i in 1..8 {
            let t = i as f32 / 8.0;
            if self.contains_exterior(a.lerp(b, t)) {
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
            chimney_pos: Vec2::new(-11.3, -6.5),
            hearth_lit: true,
            int_w: 11.0,
            int_h: 8.0,
            int_exit: Vec2::new(5.5, 7.35),
            int_hearth: Vec2::new(5.5, 1.55),
            props: vec![
                InteriorProp {
                    kind: PropKind::Rug,
                    x: 3.7,
                    y: 3.3,
                    w: 3.6,
                    h: 2.3,
                    solid: false,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Bed,
                    x: 1.0,
                    y: 2.1,
                    w: 1.8,
                    h: 2.6,
                    solid: true,
                    prompt: "[E] Sleep until Dawn".to_string(),
                },
                InteriorProp {
                    kind: PropKind::Table,
                    x: 7.4,
                    y: 2.8,
                    w: 2.1,
                    h: 1.4,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::WeaponRack,
                    x: 8.4,
                    y: 1.35,
                    w: 1.6,
                    h: 0.65,
                    solid: true,
                    prompt: "[E] Check Gear".to_string(),
                },
                InteriorProp {
                    kind: PropKind::Barrel,
                    x: 1.0,
                    y: 5.5,
                    w: 0.9,
                    h: 0.9,
                    solid: true,
                    prompt: String::new(),
                },
            ],
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
            chimney_pos: Vec2::new(-12.2, 9.6),
            hearth_lit: true,
            int_w: 12.0,
            int_h: 8.5,
            int_exit: Vec2::new(6.0, 7.85),
            int_hearth: Vec2::new(2.6, 1.65),
            props: vec![
                InteriorProp {
                    kind: PropKind::Forge,
                    x: 1.4,
                    y: 1.35,
                    w: 2.4,
                    h: 1.2,
                    solid: true,
                    prompt: "[E] Stoke Forge".to_string(),
                },
                InteriorProp {
                    kind: PropKind::Anvil,
                    x: 4.5,
                    y: 3.4,
                    w: 1.3,
                    h: 0.85,
                    solid: true,
                    prompt: "[E] Hone Dirk at Anvil".to_string(),
                },
                InteriorProp {
                    kind: PropKind::WeaponRack,
                    x: 8.5,
                    y: 1.4,
                    w: 2.4,
                    h: 0.75,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Table,
                    x: 8.4,
                    y: 4.4,
                    w: 2.2,
                    h: 1.3,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Barrel,
                    x: 1.3,
                    y: 5.6,
                    w: 0.95,
                    h: 0.95,
                    solid: true,
                    prompt: String::new(),
                },
            ],
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
            chimney_pos: Vec2::new(-14.2, -20.2),
            hearth_lit: true,
            int_w: 14.0,
            int_h: 9.5,
            int_exit: Vec2::new(7.0, 8.85),
            int_hearth: Vec2::new(2.8, 1.6),
            props: vec![
                InteriorProp {
                    kind: PropKind::Rug,
                    x: 1.8,
                    y: 2.8,
                    w: 3.4,
                    h: 2.4,
                    solid: false,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::BarCounter,
                    x: 7.6,
                    y: 2.7,
                    w: 4.8,
                    h: 1.15,
                    solid: true,
                    prompt: "[E] Drink Warm Ale".to_string(),
                },
                InteriorProp {
                    kind: PropKind::Barrel,
                    x: 11.6,
                    y: 1.45,
                    w: 0.95,
                    h: 0.95,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Barrel,
                    x: 10.4,
                    y: 1.45,
                    w: 0.95,
                    h: 0.95,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Table,
                    x: 2.2,
                    y: 5.6,
                    w: 2.3,
                    h: 1.3,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Table,
                    x: 9.2,
                    y: 5.8,
                    w: 2.3,
                    h: 1.3,
                    solid: true,
                    prompt: String::new(),
                },
            ],
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
            chimney_pos: Vec2::new(25.8, -12.2),
            hearth_lit: true,
            int_w: 12.0,
            int_h: 8.5,
            int_exit: Vec2::new(6.0, 7.85),
            int_hearth: Vec2::new(9.6, 1.6),
            props: vec![
                InteriorProp {
                    kind: PropKind::CellBars,
                    x: 0.8,
                    y: 1.6,
                    w: 3.8,
                    h: 3.1,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::Table,
                    x: 6.2,
                    y: 3.6,
                    w: 2.4,
                    h: 1.3,
                    solid: true,
                    prompt: String::new(),
                },
                InteriorProp {
                    kind: PropKind::WeaponRack,
                    x: 5.6,
                    y: 1.4,
                    w: 2.5,
                    h: 0.7,
                    solid: true,
                    prompt: String::new(),
                },
            ],
        },
    ]
}
