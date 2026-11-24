#![allow(dead_code)]

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

// ============================================================================
// MODULE 1: DETERMINISTIC PRNG & PROCEDURAL NOISE
// ============================================================================
mod prng {
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

        pub fn usize_range(&mut self, lo: usize, hi: usize) -> usize {
            if hi <= lo {
                return lo;
            }
            lo + (self.next_u64() as usize) % (hi - lo)
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
}

// ============================================================================
// MODULE 2: UNIVERSAL CONSTANTS, MATERIAL MATRIX & SWAPPABLE PRESETS
// ============================================================================
mod constants {
    #[repr(u8)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MaterialId {
        Air = 0,
        Water = 1,
        Soil = 2,
        Peat = 3,
        Granite = 4,
        Oak = 5,
        Pine = 6,
        Thatch = 7,
        HempRope = 8,
        Wool = 9,
        Leather = 10,
        Flesh = 11,
        Bone = 12,
        Iron = 13,
        Steel = 14,
        BlackPowder = 15,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct MaterialSpec {
        pub density_kgm3: f32,
        pub tensile_pa: f32,
        pub shear_pa: f32,
        pub friction_mu: f32,
        pub restitution: f32,
        pub porosity: f32,
        pub ignition_k: f32,
        pub sound_absorb: f32,
    }

    pub const MATERIALS: [MaterialSpec; 16] = [
        // 0: Air
        MaterialSpec { density_kgm3: 1.225, tensile_pa: 0.0, shear_pa: 0.0, friction_mu: 0.0, restitution: 0.0, porosity: 1.0, ignition_k: 9999.0, sound_absorb: 0.02 },
        // 1: Water
        MaterialSpec { density_kgm3: 1000.0, tensile_pa: 0.0, shear_pa: 0.0, friction_mu: 0.05, restitution: 0.02, porosity: 1.0, ignition_k: 9999.0, sound_absorb: 0.20 },
        // 2: Soil
        MaterialSpec { density_kgm3: 1500.0, tensile_pa: 1.0e4, shear_pa: 2.0e4, friction_mu: 0.65, restitution: 0.05, porosity: 0.45, ignition_k: 9999.0, sound_absorb: 0.55 },
        // 3: Peat
        MaterialSpec { density_kgm3: 900.0, tensile_pa: 5.0e3, shear_pa: 8.0e3, friction_mu: 0.45, restitution: 0.03, porosity: 0.80, ignition_k: 520.0, sound_absorb: 0.70 },
        // 4: Granite
        MaterialSpec { density_kgm3: 2700.0, tensile_pa: 1.5e7, shear_pa: 3.0e7, friction_mu: 0.75, restitution: 0.25, porosity: 0.02, ignition_k: 9999.0, sound_absorb: 0.92 },
        // 5: Oak
        MaterialSpec { density_kgm3: 750.0, tensile_pa: 9.0e7, shear_pa: 1.2e7, friction_mu: 0.55, restitution: 0.20, porosity: 0.25, ignition_k: 573.0, sound_absorb: 0.65 },
        // 6: Pine
        MaterialSpec { density_kgm3: 520.0, tensile_pa: 6.0e7, shear_pa: 7.0e6, friction_mu: 0.50, restitution: 0.18, porosity: 0.35, ignition_k: 530.0, sound_absorb: 0.55 },
        // 7: Thatch
        MaterialSpec { density_kgm3: 180.0, tensile_pa: 2.0e5, shear_pa: 1.5e5, friction_mu: 0.60, restitution: 0.05, porosity: 0.90, ignition_k: 495.0, sound_absorb: 0.75 },
        // 8: HempRope
        MaterialSpec { density_kgm3: 850.0, tensile_pa: 6.5e7, shear_pa: 180.0, friction_mu: 0.62, restitution: 0.06, porosity: 0.65, ignition_k: 510.0, sound_absorb: 0.40 },
        // 9: Wool
        MaterialSpec { density_kgm3: 320.0, tensile_pa: 1.5e6, shear_pa: 2.5e5, friction_mu: 0.65, restitution: 0.04, porosity: 0.85, ignition_k: 620.0, sound_absorb: 0.80 },
        // 10: Leather
        MaterialSpec { density_kgm3: 940.0, tensile_pa: 2.5e7, shear_pa: 8.0e6, friction_mu: 0.60, restitution: 0.10, porosity: 0.20, ignition_k: 640.0, sound_absorb: 0.50 },
        // 11: Flesh
        MaterialSpec { density_kgm3: 1060.0, tensile_pa: 1.5e6, shear_pa: 4.5e5, friction_mu: 0.45, restitution: 0.05, porosity: 0.10, ignition_k: 750.0, sound_absorb: 0.45 },
        // 12: Bone
        MaterialSpec { density_kgm3: 1900.0, tensile_pa: 1.2e8, shear_pa: 5.0e7, friction_mu: 0.40, restitution: 0.15, porosity: 0.05, ignition_k: 900.0, sound_absorb: 0.60 },
        // 13: Iron
        MaterialSpec { density_kgm3: 7870.0, tensile_pa: 3.5e8, shear_pa: 2.2e8, friction_mu: 0.35, restitution: 0.35, porosity: 0.0, ignition_k: 1800.0, sound_absorb: 0.85 },
        // 14: Steel
        MaterialSpec { density_kgm3: 7850.0, tensile_pa: 6.0e8, shear_pa: 4.2e8, friction_mu: 0.30, restitution: 0.45, porosity: 0.0, ignition_k: 1850.0, sound_absorb: 0.88 },
        // 15: BlackPowder
        MaterialSpec { density_kgm3: 1700.0, tensile_pa: 0.0, shear_pa: 0.0, friction_mu: 0.50, restitution: 0.0, porosity: 0.95, ignition_k: 450.0, sound_absorb: 0.25 },
    ];

    pub fn spec(id: MaterialId) -> MaterialSpec {
        MATERIALS[id as usize]
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DefeatMode {
        CaptivityTimeSkip,
        KnockoutBounce,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DialectId {
        Highland1744,
        RedcoatMilitary,
        PartyAnnouncer,
    }

    #[derive(Clone, Debug)]
    pub struct UniverseConfig {
        pub preset_name: String,
        pub world_seed: u64,
        pub era_year: i32,
        // 1. Spacetime & Kinematics
        pub gravity_mps2: f32,
        pub air_density_kgm3: f32,
        pub day_length_sec: f32,
        pub global_friction_mult: f32,
        pub global_restitution_mult: f32,
        pub knockback_mult: f32,
        // 2. Climate & Atmosphere
        pub base_temp_kelvin: f32,
        pub rain_probability: f32,
        pub base_wind_mps: f32,
        pub mist_baseline: f32,
        pub era_darkness: f32, // 0.0 = bright golden 1744, 1.0 = bleak ash 1746
        // 3. Thermodynamics & Chemistry
        pub wetness_soak_rate: f32,
        pub wetness_dry_rate: f32,
        pub ignition_humidity_penalty: f32,
        // 4. Biology & Consequence
        pub lethality_mult: f32,
        pub stamina_drain_mult: f32,
        pub bleed_rate_mult: f32,
        pub hypothermia_rate: f32,
        pub defeat_mode: DefeatMode,
        pub defeat_time_skip_days: u32,
        // 5. Sensory & Social
        pub vision_range_m: f32,
        pub hearing_range_m: f32,
        pub gossip_spread_radius_m: f32,
        pub mount_bond_rate: f32,
        pub default_dialect: DialectId,
    }

    impl UniverseConfig {
        pub fn preset_1744_lochaber() -> Self {
            Self {
                preset_name: "1744 Lochaber (Golden Calm)".to_string(),
                world_seed: 1744_0519,
                era_year: 1744,
                gravity_mps2: 9.81,
                air_density_kgm3: 1.225,
                day_length_sec: 300.0,
                global_friction_mult: 1.0,
                global_restitution_mult: 0.06,
                knockback_mult: 1.0,
                base_temp_kelvin: 285.0,
                rain_probability: 0.25,
                base_wind_mps: 4.5,
                mist_baseline: 0.14,
                era_darkness: 0.08,
                wetness_soak_rate: 0.09,
                wetness_dry_rate: 0.16,
                ignition_humidity_penalty: 0.72,
                lethality_mult: 1.0,
                stamina_drain_mult: 1.0,
                bleed_rate_mult: 1.0,
                hypothermia_rate: 1.0,
                defeat_mode: DefeatMode::CaptivityTimeSkip,
                defeat_time_skip_days: 3,
                vision_range_m: 26.0,
                hearing_range_m: 44.0,
                gossip_spread_radius_m: 38.0,
                mount_bond_rate: 0.08,
                default_dialect: DialectId::Highland1744,
            }
        }

        pub fn preset_1746_bleak_rising() -> Self {
            let mut c = Self::preset_1744_lochaber();
            c.preset_name = "1746 Culloden Storm (Bleak)".to_string();
            c.era_year = 1746;
            c.base_temp_kelvin = 277.0;
            c.rain_probability = 0.78;
            c.base_wind_mps = 10.5;
            c.mist_baseline = 0.42;
            c.era_darkness = 0.78;
            c.lethality_mult = 1.35;
            c
        }

        pub fn preset_party_chaos() -> Self {
            Self {
                preset_name: "Party Chaos (Bounce Arena)".to_string(),
                world_seed: 2026_9999,
                era_year: 2026,
                gravity_mps2: 22.0,
                air_density_kgm3: 1.0,
                day_length_sec: 180.0,
                global_friction_mult: 0.35,
                global_restitution_mult: 0.88,
                knockback_mult: 3.4,
                base_temp_kelvin: 294.0,
                rain_probability: 0.10,
                base_wind_mps: 2.0,
                mist_baseline: 0.02,
                era_darkness: 0.0,
                wetness_soak_rate: 0.04,
                wetness_dry_rate: 0.30,
                ignition_humidity_penalty: 0.0,
                lethality_mult: 0.0,
                stamina_drain_mult: 0.3,
                bleed_rate_mult: 0.0,
                hypothermia_rate: 0.0,
                defeat_mode: DefeatMode::KnockoutBounce,
                defeat_time_skip_days: 0,
                vision_range_m: 22.0,
                hearing_range_m: 30.0,
                gossip_spread_radius_m: 20.0,
                mount_bond_rate: 0.30,
                default_dialect: DialectId::PartyAnnouncer,
            }
        }
    }
}

// ============================================================================
// MODULE 3: CONTINUOUS DETERMINISTIC WORLD FIELD, WEATHER & LOCAL TRACES
// ============================================================================
mod field {
    use crate::constants::UniverseConfig;
    use crate::prng::fbm2d;
    use macroquad::prelude::Vec2;

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

        // Western sea loch shoreline
        let loch_edge = -28.0 + (y * 0.045).sin() * 6.5 + (n - 0.5) * 8.0;
        if x < loch_edge {
            return (-2.0, TerrainKind::LochWater);
        }

        // General elevation rises gently eastward toward Highland crags
        let east_slope = ((x + 25.0) * 0.14).max(0.0);
        let elev = east_slope + (n - 0.45) * 8.0;

        // Wade's Military Road running north-south through the glen
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
        pub time_of_day: f32, // 0.0..1.0 (0.25 dawn, 0.5 noon, 0.78 dusk)
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
                time_of_day: 0.36, // Bright golden late-morning start
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
            self.sun_dir = Vec2::new(angle.cos() * 0.9, (0.35 + (1.0 - sun_h.max(0.0)) * 0.55));
            self.sun_warmth = (sun_h.max(0.0) * (1.0 - cfg.era_darkness * 0.65)).clamp(0.08, 1.0);

            // Smooth deterministic weather wave
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

            let diurnal = sun_h * 5.0;
            self.ambient_temp_k = cfg.base_temp_kelvin + diurnal - self.rain_intensity * 3.5;
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
}

// ============================================================================
// MODULE 4: DETERMINISTIC ENTERABLE ARCHITECTURE & THERMAL SHELTER
// ============================================================================
mod architecture {
    use macroquad::prelude::Vec2;

    #[derive(Clone, Debug)]
    pub struct BuildingRecord {
        pub id: u32,
        pub name: String,
        pub role: String,
        pub x: f32, // Top-left in meters
        pub y: f32,
        pub w: f32,
        pub h: f32,
        pub door_pos: Vec2,
        pub hearth_pos: Vec2,
        pub hearth_lit: bool,
        pub roof_alpha: f32, // Smoothly fades to 0 when player steps inside
    }

    impl BuildingRecord {
        pub fn contains(&self, p: Vec2) -> bool {
            p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
        }

        pub fn near_door(&self, p: Vec2) -> bool {
            p.distance(self.door_pos) < 1.85
        }

        /// Wall collision with real open doorway portal on the south facade
        pub fn resolve_wall_collision(&self, prev: Vec2, next: Vec2, radius: f32) -> Vec2 {
            let was_in = self.contains(prev);
            let now_in = self.contains(next);
            if was_in == now_in {
                return next;
            }
            // Allow smooth transit through the doorway opening
            if self.near_door(prev) || self.near_door(next) {
                return next;
            }
            // Otherwise block solid stone masonry walls
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
            // Sample 8 interior points along ray for fast, deterministic occlusion
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
}

// ============================================================================
// MODULE 5: ZERO-LLM LINGUISTICS, MEMORY RINGS & DIALECT REWRITE ENGINE
// ============================================================================
mod linguistics {
    use crate::constants::DialectId;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MemoryKind {
        SawBurial,
        SawCutRope,
        FoundEmptyGibbet,
        SawBloodyPlayer,
        HeardGunfire,
        SharedHearth,
        GossipHonored,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct MemoryEvent {
        pub kind: MemoryKind,
        pub timestamp: f32,
        pub weight: f32, // positive = respect/trust, negative = fear/hostility
    }

    #[derive(Clone, Debug)]
    pub struct MemoryRing {
        pub slots: Vec<MemoryEvent>,
        pub capacity: usize,
    }

    impl MemoryRing {
        pub fn new() -> Self {
            Self {
                slots: Vec::with_capacity(16),
                capacity: 16,
            }
        }

        pub fn push(&mut self, kind: MemoryKind, timestamp: f32, weight: f32) {
            if self.slots.len() >= self.capacity {
                self.slots.remove(0);
            }
            self.slots.push(MemoryEvent {
                kind,
                timestamp,
                weight,
            });
        }

        pub fn has(&self, kind: MemoryKind) -> bool {
            self.slots.iter().any(|m| m.kind == kind)
        }

        pub fn net_trust(&self) -> f32 {
            self.slots.iter().map(|m| m.weight).sum::<f32>().clamp(-1.0, 1.0)
        }
    }

    #[derive(Clone, Debug)]
    pub struct UtteranceOutput {
        pub speaker: String,
        pub text: String,
        pub volume_db: f32,
        pub formant_f0: f32,
        pub formant_f1: f32,
        pub formant_f2: f32,
        pub ttl: f32,
    }

    /// Deterministic phonological & lexical rewrite rules (zero LLM)
    pub fn apply_dialect(base: &str, dialect: DialectId) -> String {
        match dialect {
            DialectId::Highland1744 => {
                let mut s = base.to_string();
                let swaps = [
                    ("cannot", "canna"),
                    ("do not", "dinna"),
                    ("nothing", "naething"),
                    ("know", "ken"),
                    ("from", "frae"),
                    ("cold", "cauld"),
                    ("old", "auld"),
                    ("child", "bairn"),
                    ("small", "wee"),
                    ("English", "Sassenach"),
                    ("home", "hame"),
                    ("stone", "stane"),
                    ("all", "a'"),
                ];
                for (a, b) in swaps {
                    s = s.replace(a, b);
                }
                s
            }
            DialectId::RedcoatMilitary => format!("Halt! {}", base),
            DialectId::PartyAnnouncer => format!("WHOA! {}!", base.to_uppercase()),
        }
    }

    pub fn compose_villager_line(
        speaker: &str,
        dialect: DialectId,
        memories: &MemoryRing,
        rain: f32,
        player_blood: f32,
        era_darkness: f32,
    ) -> UtteranceOutput {
        let greeting = if rain > 0.35 {
            "A bitter wet squall from the loch."
        } else if era_darkness > 0.45 {
            "Dark clouds gather over Lochaber."
        } else {
            "Fair morning across the glen."
        };

        let middle = if player_blood > 0.35 {
            "You carry fresh blood on your wool — I do not know what trouble followed you home."
        } else if memories.has(MemoryKind::SawBurial) || memories.has(MemoryKind::GossipHonored) {
            "Word reached the clachan that you laid the hanged lad beneath a proper stone cairn. Take warmth by our hearth whenever you need."
        } else if memories.has(MemoryKind::HeardGunfire) {
            "Black powder echoed down Wade's Road — keep your dirk close."
        } else {
            "Two Highland kin still hang on the King's road east of here. Nobody dares cut the hemp while the Redcoats march."
        };

        let raw = format!("{} {}", greeting, middle);
        let text = apply_dialect(&raw, dialect);
        let f0 = if dialect == DialectId::PartyAnnouncer { 230.0 } else { 135.0 };
        UtteranceOutput {
            speaker: speaker.to_string(),
            text,
            volume_db: 58.0,
            formant_f0: f0,
            formant_f1: 540.0,
            formant_f2: 1480.0,
            ttl: 6.5,
        }
    }

    pub fn compose_redcoat_alert(speaker: &str, found_rope: bool, dialect: DialectId) -> UtteranceOutput {
        let base = if found_rope {
            "Someone severed the gibbet rope with clean steel! Put the ridge on alert!"
        } else {
            "Keep your steel sheathed, Highlander, or answer to Fort William!"
        };
        UtteranceOutput {
            speaker: speaker.to_string(),
            text: apply_dialect(base, dialect),
            volume_db: 86.0,
            formant_f0: 118.0,
            formant_f1: 620.0,
            formant_f2: 1320.0,
            ttl: 5.5,
        }
    }
}

// ============================================================================
// MODULE 6: UNIFIED PHYSICAL ENTITIES, CONSTRAINTS, CLANS & COMMAND QUEUE
// ============================================================================
mod entities {
    use crate::constants::{DialectId, MaterialId};
    use crate::linguistics::MemoryRing;
    use macroquad::prelude::{Color, Vec2};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PlayerId(pub u32);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ClanId(pub u8);

    #[derive(Clone, Debug)]
    pub struct ClanRecord {
        pub id: ClanId,
        pub name: String,
        pub tartan_primary: Color,
        pub tartan_accent: Color,
        pub alert_level: f32,
        pub territory_count: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EntityKind {
        Player,
        Mount,
        Villager,
        Redcoat,
        Corpse,
        Cairn,
        Deer,
        Campfire,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum AiGoal {
        Idle,
        Patrol,
        Investigate(Vec2),
        WarmAtHearth,
        Flee,
        Engage,
        FollowOwner,
        Graze,
    }

    #[derive(Clone, Debug)]
    pub struct AgentMind {
        pub courage: f32,
        pub loyalty: f32,
        pub suspicion: f32,
        pub bond_to_player: f32,
        pub awareness: f32,
        pub goal: AiGoal,
        pub dialect: DialectId,
        pub memories: MemoryRing,
        pub home_pos: Vec2,
        pub patrol_a: Vec2,
        pub patrol_b: Vec2,
        pub patrol_to_b: bool,
        pub speak_cooldown: f32,
        pub attack_cooldown: f32,
    }

    #[derive(Clone, Debug)]
    pub struct Entity {
        pub id: u32,
        pub owner_player: Option<PlayerId>,
        pub name: String,
        pub kind: EntityKind,
        pub clan: ClanId,
        pub pos: Vec2,
        pub z: f32,
        pub vel: Vec2,
        pub facing: Vec2,
        pub mass_kg: f32,
        pub material: MaterialId,
        pub wetness: f32,         // 0..1 soaked wool/coat
        pub temp_k: f32,          // core body temp (~310K healthy)
        pub health: f32,          // 0..100
        pub stamina: f32,         // 0..100
        pub bleed_rate: f32,      // hp/sec
        pub blood_stain: f32,     // 0..1 visual & social contamination
        pub blade_sharpness: f32, // 0..1
        pub powder_wetness: f32,  // 0..1 black powder fouling
        pub carrying_id: Option<u32>,
        pub mounted_id: Option<u32>,
        pub buried: bool,
        pub alive: bool,
        pub swing_timer: f32,
        pub recoil_timer: f32,
        pub trace_accum: f32,
        pub mind: Option<AgentMind>,
    }

    #[derive(Clone, Debug)]
    pub struct Constraint {
        pub id: u32,
        pub entity_id: u32,
        pub anchor: Vec2,
        pub material: MaterialId,
        pub max_shear_n: f32,
        pub intact: bool,
        pub discovered_by_patrol: bool,
    }

    /// Multiplayer-ready command packet — local & networked actions share one pipe
    #[derive(Clone, Debug)]
    pub enum CommandPacket {
        Move { player: PlayerId, dir: Vec2, sprint: bool },
        SwingSteel { player: PlayerId, aim: Vec2 },
        FireFlintlock { player: PlayerId, aim: Vec2 },
        Interact { player: PlayerId },
        BuryCarried { player: PlayerId },
        WhistleMount { player: PlayerId },
        BindWounds { player: PlayerId },
        WhetBlade { player: PlayerId },
        BuildCampfire { player: PlayerId },
    }
}

// ============================================================================
// MODULE 7: DELTA PERSISTENCE LEDGER (ZERO-DEPENDENCY COMPACT SAVE)
// ============================================================================
mod ledger {
    #[derive(Clone, Debug)]
    pub enum DeltaEntry {
        RopeSevered(u32),
        CorpseBuried(u32, f32, f32),
        GarrisonAlert(f32),
        ScarEarned(String),
    }

    #[derive(Clone, Debug, Default)]
    pub struct DeltaLedger {
        pub entries: Vec<DeltaEntry>,
    }

    impl DeltaLedger {
        pub fn push(&mut self, entry: DeltaEntry) {
            self.entries.push(entry);
        }

        pub fn serialize_text(&self) -> String {
            let mut out = String::from("CLACHAN_LEDGER_V1\n");
            for e in &self.entries {
                match e {
                    DeltaEntry::RopeSevered(id) => out.push_str(&format!("ROPE_CUT {}\n", id)),
                    DeltaEntry::CorpseBuried(id, x, y) => {
                        out.push_str(&format!("BURIED {} {:.2} {:.2}\n", id, x, y))
                    }
                    DeltaEntry::GarrisonAlert(v) => out.push_str(&format!("ALERT {:.2}\n", v)),
                    DeltaEntry::ScarEarned(s) => out.push_str(&format!("SCAR {}\n", s)),
                }
            }
            out
        }

        pub fn deserialize_text(data: &str) -> Self {
            let mut ledger = Self::default();
            for line in data.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                match parts[0] {
                    "ROPE_CUT" if parts.len() >= 2 => {
                        if let Ok(id) = parts[1].parse::<u32>() {
                            ledger.push(DeltaEntry::RopeSevered(id));
                        }
                    }
                    "BURIED" if parts.len() >= 4 => {
                        if let (Ok(id), Ok(x), Ok(y)) = (
                            parts[1].parse::<u32>(),
                            parts[2].parse::<f32>(),
                            parts[3].parse::<f32>(),
                        ) {
                            ledger.push(DeltaEntry::CorpseBuried(id, x, y));
                        }
                    }
                    "ALERT" if parts.len() >= 2 => {
                        if let Ok(v) = parts[1].parse::<f32>() {
                            ledger.push(DeltaEntry::GarrisonAlert(v));
                        }
                    }
                    "SCAR" if parts.len() >= 2 => {
                        ledger.push(DeltaEntry::ScarEarned(parts[1..].join(" ")));
                    }
                    _ => {}
                }
            }
            ledger
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub fn save_to_disk(&self) -> Result<(), String> {
            std::fs::write("clachan_save.ledger", self.serialize_text())
                .map_err(|e| e.to_string())
        }

        #[cfg(target_arch = "wasm32")]
        pub fn save_to_disk(&self) -> Result<(), String> {
            Ok(())
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub fn load_from_disk() -> Result<Self, String> {
            let txt = std::fs::read_to_string("clachan_save.ledger")
                .map_err(|e| e.to_string())?;
            Ok(Self::deserialize_text(&txt))
        }

        #[cfg(target_arch = "wasm32")]
        pub fn load_from_disk() -> Result<Self, String> {
            Err("Disk load unavailable in WASM".to_string())
        }
    }
}

// ============================================================================
// MODULE 8: PROCEDURAL AUDIO SYNTHESIZER (IN-MEMORY WAV PCM, ZERO ASSETS)
// ============================================================================
mod audio {
    use crate::prng::Rng;
    use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};

    pub struct AudioBank {
        pub steel: Option<Sound>,
        pub rope_snap: Option<Sound>,
        pub gunshot: Option<Sound>,
        pub misfire: Option<Sound>,
        pub whistle: Option<Sound>,
        pub voice_bark: Option<Sound>,
    }

    fn build_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut buf = Vec::with_capacity(44 + data_len as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&(36 + data_len).to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
        buf.extend_from_slice(&1u16.to_le_bytes()); // Mono
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&16u16.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_len.to_le_bytes());
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * 32760.0) as i16;
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    impl AudioBank {
        pub async fn synthesize() -> Self {
            let sr = 22050u32;
            let mut rng = Rng::new(1744_888);

            // 1. Steel slash
            let steel_s: Vec<f32> = (0..(sr / 5))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 18.0).exp();
                    let ring = (t * 2400.0 * std::f32::consts::TAU).sin() * 0.25
                        + (t * 3650.0 * std::f32::consts::TAU).sin() * 0.15;
                    let air = (rng.next_f32() * 2.0 - 1.0) * 0.45;
                    (ring + air) * env * 0.5
                })
                .collect();

            // 2. Hemp rope snap
            let rope_s: Vec<f32> = (0..(sr / 6))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 34.0).exp();
                    let crack = (rng.next_f32() * 2.0 - 1.0) * 0.7;
                    let twang = (t * (420.0 - t * 1400.0).max(80.0) * std::f32::consts::TAU).sin() * 0.5;
                    (crack + twang) * env * 0.6
                })
                .collect();

            // 3. Black-powder flintlock boom
            let mut lp = 0.0f32;
            let gun_s: Vec<f32> = (0..(sr / 2))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 6.5).exp();
                    let white = rng.next_f32() * 2.0 - 1.0;
                    lp = lp * 0.78 + white * 0.22;
                    let boom = (t * (110.0 * (-t * 4.0).exp()) * std::f32::consts::TAU).sin() * 0.7;
                    (lp * 1.4 + boom) * env * 0.75
                })
                .collect();

            // 4. Wet powder misfire (dry flint click + faint fizzle)
            let misfire_s: Vec<f32> = (0..(sr / 5))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let click = if t < 0.012 { (t * 4800.0 * std::f32::consts::TAU).sin() * 0.6 } else { 0.0 };
                    let fizz = (rng.next_f32() * 2.0 - 1.0) * (-t * 14.0).exp() * 0.22;
                    click + fizz
                })
                .collect();

            // 5. Highland mount whistle
            let whistle_s: Vec<f32> = (0..(sr / 3))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (t * 15.0).min(1.0) * (-t * 4.5).exp();
                    let freq = 1150.0 + (t * 16.0).sin() * 140.0 + t * 480.0;
                    (t * freq * std::f32::consts::TAU).sin() * env * 0.35
                })
                .collect();

            // 6. Formant vocal bark (glottal pulse + F1/F2 resonance)
            let bark_s: Vec<f32> = (0..(sr / 5))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (t * 25.0).min(1.0) * (-t * 11.0).exp();
                    let f0 = 132.0 - t * 22.0;
                    let glottal = ((t * f0) % 1.0) * 2.0 - 1.0;
                    let f1 = (t * 560.0 * std::f32::consts::TAU).sin();
                    let f2 = (t * 1420.0 * std::f32::consts::TAU).sin() * 0.5;
                    (glottal * 0.35 + f1 * 0.4 + f2 * 0.25) * env * 0.4
                })
                .collect();

            Self {
                steel: load_sound_from_bytes(&build_wav(&steel_s, sr)).await.ok(),
                rope_snap: load_sound_from_bytes(&build_wav(&rope_s, sr)).await.ok(),
                gunshot: load_sound_from_bytes(&build_wav(&gun_s, sr)).await.ok(),
                misfire: load_sound_from_bytes(&build_wav(&misfire_s, sr)).await.ok(),
                whistle: load_sound_from_bytes(&build_wav(&whistle_s, sr)).await.ok(),
                voice_bark: load_sound_from_bytes(&build_wav(&bark_s, sr)).await.ok(),
            }
        }

        pub fn play(sound: &Option<Sound>, vol: f32) {
            if let Some(s) = sound {
                play_sound(s, PlaySoundParams { looped: false, volume: vol });
            }
        }
    }
}

// ============================================================================
// MODULE 9: PROCEDURAL OIL-PAINTED TEXTURE SYNTHESIS & GLSL LIGHTING SHADER
// ============================================================================
mod painter {
    use crate::prng::{fbm2d, Rng};
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

        // Subtle impasto oil-brush sample blend
        vec4 c0 = texture2D(Texture, uv);
        vec4 c1 = texture2D(Texture, uv + vec2(px.x * 1.4, px.y * 0.6));
        vec4 c2 = texture2D(Texture, uv - vec2(px.x * 0.9, px.y * 1.2));
        vec3 col = (c0.rgb * 0.62 + c1.rgb * 0.20 + c2.rgb * 0.18);

        // Woven linen canvas grain
        float weave = sin(uv.x * u_res_x * 0.75) * sin(uv.y * u_res_y * 0.75) * 0.018;
        float grain = (hash(floor(uv * vec2(u_res_x, u_res_y) * 0.5)) - 0.5) * 0.025;
        col += vec3(weave + grain);

        // Bright 1744 golden sunlit warmth vs Bleak 1746 ash/slate grade
        vec3 warm_tint = vec3(1.08, 1.02, 0.90);
        vec3 bleak_tint = vec3(0.66, 0.74, 0.84);
        float luma = dot(col, vec3(0.299, 0.587, 0.114));
        vec3 desat = mix(col, vec3(luma), u_darkness * 0.52);
        vec3 grade = mix(warm_tint, bleak_tint, u_darkness);
        col = desat * grade * (0.55 + u_warmth * 0.55);

        // Drifting Highland atmospheric mist
        float mist_wave = 0.5 + 0.5 * sin(uv.x * 7.0 + u_time * 0.35 + sin(uv.y * 5.0));
        vec3 mist_col = mix(vec3(0.78, 0.82, 0.76), vec3(0.42, 0.48, 0.54), u_darkness);
        col = mix(col, mist_col, u_mist * 0.28 * (0.5 + 0.5 * mist_wave));

        // Soft painterly vignette
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

        // Base pigment wash via fBm
        for y in 0..size {
            for x in 0..size {
                let n = fbm2d(seed, x as f32 * 0.08, y as f32 * 0.08, 3);
                let r = (base[0] * (0.85 + n * 0.3) + accent[0] * n * 0.18).clamp(0.0, 1.0);
                let g = (base[1] * (0.85 + n * 0.3) + accent[1] * n * 0.18).clamp(0.0, 1.0);
                let b = (base[2] * (0.85 + n * 0.3) + accent[2] * n * 0.18).clamp(0.0, 1.0);
                img.set_pixel(x as u32, y as u32, Color::new(r, g, b, 1.0));
            }
        }

        // Layered impasto oil brushstrokes
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
}

// ============================================================================
// MODULE 10: THE 7-LOOP DETERMINISTIC UNIVERSE SOLVER
// ============================================================================
use architecture::{generate_clachan_buildings, BuildingRecord};
use audio::AudioBank;
use constants::{spec, DefeatMode, DialectId, MaterialId, UniverseConfig};
use entities::{
    AgentMind, AiGoal, ClanId, ClanRecord, CommandPacket, Constraint, Entity, EntityKind, PlayerId,
};
use field::{sample_terrain, SmokePuff, SoundWave, TerrainKind, TraceKind, TraceMark, WeatherState};
use ledger::{DeltaEntry, DeltaLedger};
use linguistics::{
    compose_redcoat_alert, compose_villager_line, MemoryKind, MemoryRing, UtteranceOutput,
};
use painter::PaintedAtlas;
use prng::Rng;

struct Universe {
    cfg: UniverseConfig,
    rng: Rng,
    time: f32,
    weather: WeatherState,
    buildings: Vec<BuildingRecord>,
    clans: Vec<ClanRecord>,
    entities: Vec<Entity>,
    constraints: Vec<Constraint>,
    trees: Vec<(Vec2, f32)>,
    traces: Vec<TraceMark>,
    sound_waves: Vec<SoundWave>,
    smoke: Vec<SmokePuff>,
    utterances: Vec<UtteranceOutput>,
    ledger: DeltaLedger,
    status_toast: (String, f32),
    scars: Vec<String>,
    next_id: u32,
}

impl Universe {
    fn new(cfg: UniverseConfig) -> Self {
        let mut rng = Rng::new(cfg.world_seed);
        let weather = WeatherState::new(&cfg);
        let buildings = generate_clachan_buildings();

        let clans = vec![
            ClanRecord {
                id: ClanId(0),
                name: "Clan MacRae".to_string(),
                tartan_primary: Color::new(0.18, 0.36, 0.24, 1.0),
                tartan_accent: Color::new(0.68, 0.18, 0.18, 1.0),
                alert_level: 0.0,
                territory_count: 3,
            },
            ClanRecord {
                id: ClanId(1),
                name: "Fort William Garrison".to_string(),
                tartan_primary: Color::new(0.72, 0.16, 0.15, 1.0),
                tartan_accent: Color::new(0.92, 0.88, 0.78, 1.0),
                alert_level: 0.0,
                territory_count: 1,
            },
            ClanRecord {
                id: ClanId(2),
                name: "Wilderness".to_string(),
                tartan_primary: Color::new(0.45, 0.35, 0.25, 1.0),
                tartan_accent: Color::new(0.7, 0.65, 0.55, 1.0),
                alert_level: 0.0,
                territory_count: 0,
            },
        ];

        let mut u = Self {
            cfg: cfg.clone(),
            rng: Rng::new(cfg.world_seed),
            time: 0.0,
            weather,
            buildings,
            clans,
            entities: Vec::new(),
            constraints: Vec::new(),
            trees: Vec::new(),
            traces: Vec::new(),
            sound_waves: Vec::new(),
            smoke: Vec::new(),
            utterances: Vec::new(),
            ledger: DeltaLedger::default(),
            status_toast: (
                "Welcome to Clachan (1744). Press TAB for Universe Forge.".to_string(),
                6.0,
            ),
            scars: Vec::new(),
            next_id: 100,
        };

        // 1. Player: Callum MacRae (ID 1)
        u.entities.push(Entity {
            id: 1,
            owner_player: Some(PlayerId(1)),
            name: "Callum MacRae".to_string(),
            kind: EntityKind::Player,
            clan: ClanId(0),
            pos: Vec2::new(-8.0, 0.5),
            z: 0.0,
            vel: Vec2::ZERO,
            facing: Vec2::new(1.0, 0.0),
            mass_kg: 82.0,
            material: MaterialId::Wool,
            wetness: 0.05,
            temp_k: 310.1,
            health: 100.0,
            max_health: 100.0,
            stamina: 100.0,
            bleed_rate: 0.0,
            blood_stain: 0.0,
            blade_sharpness: 1.0,
            powder_wetness: 0.08,
            carrying_id: None,
            mounted_id: None,
            buried: false,
            alive: true,
            swing_timer: 0.0,
            recoil_timer: 0.0,
            trace_accum: 0.0,
            mind: None,
        });

        // 2. Companion Horse: Bramble (ID 2)
        u.entities.push(Entity {
            id: 2,
            owner_player: Some(PlayerId(1)),
            name: "Bramble".to_string(),
            kind: EntityKind::Mount,
            clan: ClanId(0),
            pos: Vec2::new(-4.5, 2.2),
            z: 0.0,
            vel: Vec2::ZERO,
            facing: Vec2::new(1.0, 0.2),
            mass_kg: 460.0,
            material: MaterialId::Leather,
            wetness: 0.05,
            temp_k: 311.0,
            health: 140.0,
            max_health: 140.0,
            stamina: 100.0,
            bleed_rate: 0.0,
            blood_stain: 0.0,
            blade_sharpness: 1.0,
            powder_wetness: 0.0,
            carrying_id: None,
            mounted_id: None,
            buried: false,
            alive: true,
            swing_timer: 0.0,
            recoil_timer: 0.0,
            trace_accum: 0.0,
            mind: Some(AgentMind {
                courage: 0.72,
                loyalty: 0.92,
                suspicion: 0.0,
                bond_to_player: 0.75,
                awareness: 0.0,
                goal: AiGoal::Graze,
                dialect: cfg.default_dialect,
                memories: MemoryRing::new(),
                home_pos: Vec2::new(-4.5, 2.2),
                patrol_a: Vec2::ZERO,
                patrol_b: Vec2::ZERO,
                patrol_to_b: false,
                speak_cooldown: 0.0,
                attack_cooldown: 0.0,
            }),
        });

        // 3. Gibbet on Wade's Road with two hanged Highlanders suspended on live physics hemp ropes!
        let gibbet_x = 12.0;
        let gibbet_y = -1.5;
        for (idx, (cname, ox)) in [("Ewan of Glen Nevis", -0.85), ("Hamish MacDonald", 0.85)]
            .iter()
            .enumerate()
        {
            let cid = 10 + idx as u32;
            let anchor = Vec2::new(gibbet_x + *ox, gibbet_y);
            u.entities.push(Entity {
                id: cid,
                owner_player: None,
                name: cname.to_string(),
                kind: EntityKind::Corpse,
                clan: ClanId(0),
                pos: anchor + Vec2::new(0.0, 0.35),
                z: 1.4,
                vel: Vec2::ZERO,
                facing: Vec2::new(0.0, 1.0),
                mass_kg: 76.0,
                material: MaterialId::Flesh,
                wetness: 0.3,
                temp_k: 284.0,
                health: 0.0,
                max_health: 100.0,
                stamina: 0.0,
                bleed_rate: 0.0,
                blood_stain: 0.25,
                blade_sharpness: 0.0,
                powder_wetness: 0.0,
                carrying_id: None,
                mounted_id: None,
                buried: false,
                alive: false,
                swing_timer: 0.0,
                recoil_timer: 0.0,
                trace_accum: 0.0,
                mind: None,
            });
            u.constraints.push(Constraint {
                id: 500 + idx as u32,
                entity_id: cid,
                anchor,
                material: MaterialId::HempRope,
                max_shear_n: spec(MaterialId::HempRope).shear_pa,
                intact: true,
                discovered_by_patrol: false,
            });
        }

        // 4. Clachan Villagers with real schedules, hearths, and memory rings
        let villagers = [
            ("Mairi the Weaver", Vec2::new(-8.0, 15.8), Vec2::new(-10.5, 11.2)),
            ("Duncan the Smith", Vec2::new(-6.0, 11.0), Vec2::new(-10.0, 10.5)),
            ("Alistair of the Alehouse", Vec2::new(-9.5, -13.0), Vec2::new(-12.5, -18.5)),
            ("Morag the Crofter", Vec2::new(-2.5, -6.0), Vec2::new(-9.5, -5.0)),
        ];
        for (i, (vname, pos, home)) in villagers.iter().enumerate() {
            u.entities.push(Entity {
                id: 20 + i as u32,
                owner_player: None,
                name: vname.to_string(),
                kind: EntityKind::Villager,
                clan: ClanId(0),
                pos: *pos,
                z: 0.0,
                vel: Vec2::ZERO,
                facing: Vec2::new(1.0, 0.0),
                mass_kg: 72.0,
                material: MaterialId::Wool,
                wetness: 0.1,
                temp_k: 310.0,
                health: 80.0,
                max_health: 80.0,
                stamina: 100.0,
                bleed_rate: 0.0,
                blood_stain: 0.0,
                blade_sharpness: 0.5,
                powder_wetness: 0.0,
                carrying_id: None,
                mounted_id: None,
                buried: false,
                alive: true,
                swing_timer: 0.0,
                recoil_timer: 0.0,
                trace_accum: 0.0,
                mind: Some(AgentMind {
                    courage: 0.45,
                    loyalty: 0.85,
                    suspicion: 0.1,
                    bond_to_player: 0.25,
                    awareness: 0.0,
                    goal: AiGoal::Idle,
                    dialect: cfg.default_dialect,
                    memories: MemoryRing::new(),
                    home_pos: *home,
                    patrol_a: *pos,
                    patrol_b: *pos + Vec2::new(4.0, 2.0),
                    patrol_to_b: true,
                    speak_cooldown: 0.0,
                    attack_cooldown: 0.0,
                }),
            });
        }

        // 5. Fort William Redcoat Patrols marching Wade's Road
        let patrols = [
            ("Corporal Hargreaves", Vec2::new(11.0, -16.0), Vec2::new(11.0, 14.0)),
            ("Private Finch", Vec2::new(13.2, -17.5), Vec2::new(13.2, 12.5)),
            ("Sergeant Mercer", Vec2::new(21.5, -5.0), Vec2::new(16.0, 8.0)),
        ];
        for (i, (rname, pa, pb)) in patrols.iter().enumerate() {
            u.entities.push(Entity {
                id: 40 + i as u32,
                owner_player: None,
                name: rname.to_string(),
                kind: EntityKind::Redcoat,
                clan: ClanId(1),
                pos: *pa,
                z: 0.0,
                vel: Vec2::ZERO,
                facing: Vec2::new(0.0, 1.0),
                mass_kg: 86.0,
                material: MaterialId::Wool,
                wetness: 0.15,
                temp_k: 310.0,
                health: 95.0,
                max_health: 95.0,
                stamina: 100.0,
                bleed_rate: 0.0,
                blood_stain: 0.0,
                blade_sharpness: 0.9,
                powder_wetness: 0.1,
                carrying_id: None,
                mounted_id: None,
                buried: false,
                alive: true,
                swing_timer: 0.0,
                recoil_timer: 0.0,
                trace_accum: 0.0,
                mind: Some(AgentMind {
                    courage: 0.85,
                    loyalty: 0.9,
                    suspicion: 0.35,
                    bond_to_player: -0.25,
                    awareness: 0.0,
                    goal: AiGoal::Patrol,
                    dialect: if cfg.default_dialect == DialectId::PartyAnnouncer {
                        DialectId::PartyAnnouncer
                    } else {
                        DialectId::RedcoatMilitary
                    },
                    memories: MemoryRing::new(),
                    home_pos: Vec2::new(23.0, -10.0),
                    patrol_a: *pa,
                    patrol_b: *pb,
                    patrol_to_b: true,
                    speak_cooldown: 0.0,
                    attack_cooldown: 0.0,
                }),
            });
        }

        // 6. Wild Highland Red Deer
        for i in 0..3 {
            let dp = Vec2::new(-18.0 + i as f32 * 4.5, 22.0 + (i as f32 * 2.1).sin() * 3.0);
            u.entities.push(Entity {
                id: 60 + i as u32,
                owner_player: None,
                name: "Highland Red Deer".to_string(),
                kind: EntityKind::Deer,
                clan: ClanId(2),
                pos: dp,
                z: 0.0,
                vel: Vec2::ZERO,
                facing: Vec2::new(1.0, 0.0),
                mass_kg: 110.0,
                material: MaterialId::Flesh,
                wetness: 0.0,
                temp_k: 311.0,
                health: 45.0,
                max_health: 45.0,
                stamina: 100.0,
                bleed_rate: 0.0,
                blood_stain: 0.0,
                blade_sharpness: 0.0,
                powder_wetness: 0.0,
                carrying_id: None,
                mounted_id: None,
                buried: false,
                alive: true,
                swing_timer: 0.0,
                recoil_timer: 0.0,
                trace_accum: 0.0,
                mind: Some(AgentMind {
                    courage: 0.1,
                    loyalty: 0.0,
                    suspicion: 0.5,
                    bond_to_player: 0.0,
                    awareness: 0.0,
                    goal: AiGoal::Graze,
                    dialect: DialectId::Highland1744,
                    memories: MemoryRing::new(),
                    home_pos: dp,
                    patrol_a: dp,
                    patrol_b: dp,
                    patrol_to_b: false,
                    speak_cooldown: 0.0,
                    attack_cooldown: 0.0,
                }),
            });
        }

        // 7. Deterministic Scots Pine groves avoiding buildings, water & road
        for _ in 0..115 {
            let tx = rng.range_f32(-36.0, 42.0);
            let ty = rng.range_f32(-36.0, 36.0);
            let p = Vec2::new(tx, ty);
            let (_, tk) = sample_terrain(cfg.world_seed, tx, ty);
            if tk == TerrainKind::LochWater || tk == TerrainKind::MudRoad {
                continue;
            }
            if p.distance(Vec2::new(gibbet_x, gibbet_y)) < 4.5 {
                continue;
            }
            if u.buildings.iter().any(|b| {
                tx > b.x - 2.2 && tx < b.x + b.w + 2.2 && ty > b.y - 2.2 && ty < b.y + b.h + 2.5
            }) {
                continue;
            }
            u.trees.push((p, rng.range_f32(1.4, 2.4)));
        }

        u
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.status_toast = (msg.into(), 4.5);
    }

    fn player_index(&self, pid: PlayerId) -> Option<usize> {
        self.entities
            .iter()
            .position(|e| e.kind == EntityKind::Player && e.owner_player == Some(pid))
    }

    fn entity_index(&self, id: u32) -> Option<usize> {
        self.entities.iter().position(|e| e.id == id)
    }

    fn is_sheltered(&self, pos: Vec2) -> bool {
        self.buildings.iter().any(|b| b.contains(pos))
    }

    fn has_line_of_sight(&self, a: Vec2, b: Vec2) -> bool {
        !self.buildings.iter().any(|bld| bld.blocks_segment(a, b))
    }

    fn emit_sound(&mut self, pos: Vec2, decibels: f32, hostiles_care: bool, whistle: bool) {
        let radius = (decibels / 95.0) * self.cfg.hearing_range_m;
        self.sound_waves.push(SoundWave {
            pos,
            radius: 0.4,
            max_radius: radius.max(4.0),
            decibels,
            age: 0.0,
            hostiles_care,
            whistle,
        });
    }

    fn apply_command(&mut self, cmd: CommandPacket, sfx: &AudioBank) {
        match cmd {
            CommandPacket::Move { player, dir, sprint } => {
                if let Some(pi) = self.player_index(player) {
                    let carrying_mass = if let Some(cid) = self.entities[pi].carrying_id {
                        self.entity_index(cid)
                            .map(|idx| self.entities[idx].mass_kg)
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let mounted = self.entities[pi].mounted_id.is_some();
                    let (_, tk) = sample_terrain(self.cfg.world_seed, self.entities[pi].pos.x, self.entities[pi].pos.y);
                    let mud_drag = match tk {
                        TerrainKind::PeatBog => 0.58,
                        TerrainKind::MudRoad => 1.0 - self.weather.rain_intensity * 0.22,
                        _ => 1.0,
                    };
                    let burden = 82.0 / (82.0 + carrying_mass * 0.65);
                    let base_speed = if mounted { 9.2 } else { 4.8 };
                    let want_sprint = sprint && self.entities[pi].stamina > 6.0 && carrying_mass < 1.0;
                    let mult = if want_sprint { 1.55 } else { 1.0 };
                    let target_v = dir.normalize_or_zero() * base_speed * mult * burden * mud_drag;
                    let accel = (9.0 * self.cfg.global_friction_mult).clamp(2.5, 14.0);
                    let dt_approx = 1.0 / 60.0;
                    let cur_v = self.entities[pi].vel;
                    self.entities[pi].vel = cur_v.lerp(target_v, (accel * dt_approx).min(1.0));
                    if dir.length_squared() > 0.01 {
                        self.entities[pi].facing = dir.normalize();
                    }
                    if want_sprint && dir.length_squared() > 0.01 {
                        self.entities[pi].stamina =
                            (self.entities[pi].stamina - 16.0 * dt_approx * self.cfg.stamina_drain_mult).max(0.0);
                    }
                }
            }

            CommandPacket::SwingSteel { player, aim } => {
                let Some(pi) = self.player_index(player) else { return; };
                if self.entities[pi].swing_timer > 0.0 || self.entities[pi].stamina < 8.0 {
                    return;
                }
                self.entities[pi].facing = aim.normalize_or_zero();
                self.entities[pi].swing_timer = 0.28;
                self.entities[pi].stamina = (self.entities[pi].stamina - 10.0 * self.cfg.stamina_drain_mult).max(0.0);
                let p_pos = self.entities[pi].pos;
                let p_face = self.entities[pi].facing;
                let sharp = self.entities[pi].blade_sharpness;
                AudioBank::play(&sfx.steel, 0.35);
                self.emit_sound(p_pos, 52.0, false, false);

                // LOOP 1: Check universal physics constraints (hemp gibbet ropes!)
                let mut snapped_indices = Vec::new();
                for (ci, c) in self.constraints.iter().enumerate() {
                    if !c.intact {
                        continue;
                    }
                    let d = p_pos.distance(c.anchor);
                    if d < 2.6 {
                        // Steel shear impulse scales with sharpness
                        let steel_shear_n = 260.0 * (0.4 + sharp * 0.8);
                        if steel_shear_n > c.max_shear_n {
                            snapped_indices.push(ci);
                        }
                    }
                }
                for ci in snapped_indices {
                    self.constraints[ci].intact = false;
                    let cid = self.constraints[ci].id;
                    let eid = self.constraints[ci].entity_id;
                    self.ledger.push(DeltaEntry::RopeSevered(cid));
                    AudioBank::play(&sfx.rope_snap, 0.65);
                    if let Some(ei) = self.entity_index(eid) {
                        self.entities[ei].z = 0.0;
                        self.entities[ei].vel = Vec2::new(self.rng.range_f32(-0.8, 0.8), 1.4);
                        self.toast(format!(
                            "Severed the hemp rope — {} dropped heavily into the wet grass. [E] Carry, [B] Bury.",
                            self.entities[ei].name
                        ));
                    }
                    self.entities[pi].blade_sharpness = (self.entities[pi].blade_sharpness - 0.02).max(0.1);
                }

                // Check melee strike against living hostiles / targets
                let mut hits = Vec::new();
                for i in 0..self.entities.len() {
                    if i == pi || !self.entities[i].alive || self.entities[i].kind == EntityKind::Mount {
                        continue;
                    }
                    let to_e = self.entities[i].pos - p_pos;
                    let dist = to_e.length();
                    if dist < 2.35 && to_e.normalize_or_zero().dot(p_face) > 0.2 {
                        hits.push((i, to_e.normalize_or_zero()));
                    }
                }
                for (ti, dir) in hits {
                    let dmg = (34.0 * (0.45 + sharp * 0.65)) * self.cfg.lethality_mult;
                    self.entities[ti].health -= dmg;
                    self.entities[ti].vel += dir * (6.5 * self.cfg.knockback_mult);
                    if self.cfg.lethality_mult > 0.05 {
                        self.entities[ti].bleed_rate += 1.6 * self.cfg.bleed_rate_mult;
                        self.entities[ti].blood_stain = (self.entities[ti].blood_stain + 0.35).min(1.0);
                        self.entities[pi].blood_stain = (self.entities[pi].blood_stain + 0.22).min(1.0);
                        self.entities[pi].blade_sharpness = (self.entities[pi].blade_sharpness - 0.05).max(0.15);
                    }
                    if self.entities[ti].health <= 0.0 && self.cfg.lethality_mult > 0.0 {
                        self.entities[ti].alive = false;
                        self.entities[ti].kind = EntityKind::Corpse;
                        self.toast(format!("{} fell.", self.entities[ti].name));
                    } else if let Some(mind) = &mut self.entities[ti].mind {
                        if self.entities[ti].kind == EntityKind::Redcoat {
                            mind.goal = AiGoal::Engage;
                            mind.awareness = 1.0;
                        } else {
                            mind.goal = AiGoal::Flee;
                        }
                    }
                }
            }

            CommandPacket::FireFlintlock { player, aim } => {
                let Some(pi) = self.player_index(player) else { return; };
                if self.entities[pi].recoil_timer > 0.0 {
                    return;
                }
                self.entities[pi].recoil_timer = 1.1;
                let dir = aim.normalize_or_zero();
                self.entities[pi].facing = dir;
                let p_pos = self.entities[pi].pos;

                // LOOP 2: Real black-powder humidity misfire physics!
                let misfire_chance =
                    (self.entities[pi].powder_wetness * self.cfg.ignition_humidity_penalty).clamp(0.0, 0.92);
                if self.rng.next_f32() < misfire_chance {
                    AudioBank::play(&sfx.misfire, 0.55);
                    self.toast("CLICK-FIZZLE! Rain fouled your black powder — dry off by a hearth!");
                    return;
                }

                AudioBank::play(&sfx.gunshot, 0.65);
                self.emit_sound(p_pos, 98.0, true, false);
                // Thick black-powder smoke cloud pushed by wind
                for _ in 0..8 {
                    self.smoke.push(SmokePuff {
                        pos: p_pos + dir * 0.9,
                        vel: dir * self.rng.range_f32(1.2, 2.8) + self.weather.wind_vec * 0.35,
                        radius: self.rng.range_f32(0.5, 1.1),
                        age: 0.0,
                        max_age: 4.5,
                        is_ember: false,
                    });
                }

                // Ballistic raycast with wind drift
                let wind_deflect = Vec2::new(-dir.y, dir.x) * (self.weather.wind_vec.length() * 0.008);
                let shot_dir = (dir + wind_deflect).normalize_or_zero();
                let mut hit_target = None;
                for i in 0..self.entities.len() {
                    if i == pi || !self.entities[i].alive || self.entities[i].kind == EntityKind::Mount {
                        continue;
                    }
                    let rel = self.entities[i].pos - p_pos;
                    let proj = rel.dot(shot_dir);
                    if proj > 0.5 && proj < 26.0 {
                        let perp = (rel - shot_dir * proj).length();
                        if perp < 0.95 && self.has_line_of_sight(p_pos, self.entities[i].pos) {
                            hit_target = Some(i);
                            break;
                        }
                    }
                }
                if let Some(ti) = hit_target {
                    let dmg = 78.0 * self.cfg.lethality_mult;
                    self.entities[ti].health -= dmg;
                    self.entities[ti].vel += shot_dir * (11.0 * self.cfg.knockback_mult);
                    if self.cfg.lethality_mult > 0.0 {
                        self.entities[ti].bleed_rate += 3.0 * self.cfg.bleed_rate_mult;
                        if self.entities[ti].health <= 0.0 {
                            self.entities[ti].alive = false;
                            self.entities[ti].kind = EntityKind::Corpse;
                            self.toast(format!("Musket ball dropped {}.", self.entities[ti].name));
                        }
                    }
                }
            }

            CommandPacket::Interact { player } => {
                let Some(pi) = self.player_index(player) else { return; };
                let p_pos = self.entities[pi].pos;

                // 1. Dismount if mounted
                if self.entities[pi].mounted_id.is_some() {
                    self.entities[pi].mounted_id = None;
                    self.toast("Dismounted Bramble.");
                    return;
                }

                // 2. Drop carried corpse if carrying one
                if let Some(cid) = self.entities[pi].carrying_id.take() {
                    if let Some(ci) = self.entity_index(cid) {
                        self.entities[ci].pos = p_pos + self.entities[pi].facing * 1.1;
                        self.toast(format!("Laid {} gently on the ground.", self.entities[ci].name));
                    }
                    return;
                }

                // 3. Find closest interactive entity (Mount, Villager, Unburied Freed Corpse)
                let mut best: Option<(usize, f32)> = None;
                for i in 0..self.entities.len() {
                    if i == pi || self.entities[i].buried {
                        continue;
                    }
                    let d = self.entities[i].pos.distance(p_pos);
                    if d < 2.6 {
                        let valid = matches!(
                            self.entities[i].kind,
                            EntityKind::Mount | EntityKind::Villager | EntityKind::Corpse
                        );
                        if valid && best.map(|b| d < b.1).unwrap_or(true) {
                            best = Some((i, d));
                        }
                    }
                }

                if let Some((ti, _)) = best {
                    match self.entities[ti].kind {
                        EntityKind::Mount => {
                            let mid = self.entities[ti].id;
                            self.entities[pi].mounted_id = Some(mid);
                            if let Some(m) = &mut self.entities[ti].mind {
                                m.bond_to_player = (m.bond_to_player + self.cfg.mount_bond_rate).min(1.0);
                            }
                            self.toast("Mounted Bramble. Ride across the glen!");
                        }
                        EntityKind::Corpse => {
                            let cid = self.entities[ti].id;
                            let still_hung = self.constraints.iter().any(|c| c.entity_id == cid && c.intact);
                            if still_hung {
                                self.toast("Hemp rope holds them aloft — slash the rope with steel [Click/Space] first.");
                            } else {
                                self.entities[pi].carrying_id = Some(cid);
                                self.toast(format!(
                                    "Lifted {} onto your shoulder. Carry off the road and press [B] to build a burial cairn.",
                                    self.entities[ti].name
                                ));
                            }
                        }
                        EntityKind::Villager => {
                            let vname = self.entities[ti].name.clone();
                            let p_blood = self.entities[pi].blood_stain;
                            let rain = self.weather.rain_intensity;
                            let dark = self.cfg.era_darkness;
                            if let Some(mind) = &self.entities[ti].mind {
                                let utt = compose_villager_line(
                                    &vname,
                                    mind.dialect,
                                    &mind.memories,
                                    rain,
                                    p_blood,
                                    dark,
                                );
                                self.utterances.push(utt);
                                AudioBank::play(&sfx.voice_bark, 0.35);
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                // 4. Stoke nearby cottage hearth
                for b in &mut self.buildings {
                    if b.hearth_pos.distance(p_pos) < 2.5 {
                        b.hearth_lit = !b.hearth_lit;
                        let st = if b.hearth_lit { "Stoked the peat hearth." } else { "Banked the hearth embers." };
                        self.toast(st);
                        return;
                    }
                }
            }

            CommandPacket::BuryCarried { player } => {
                let Some(pi) = self.player_index(player) else { return; };
                let Some(cid) = self.entities[pi].carrying_id.take() else {
                    self.toast("Carry a fallen body first [E] to lay them beneath a stone cairn.");
                    return;
                };
                let bury_pos = self.entities[pi].pos + self.entities[pi].facing * 1.2;
                let mut buried_name = String::new();
                if let Some(ci) = self.entity_index(cid) {
                    self.entities[ci].buried = true;
                    self.entities[ci].pos = bury_pos;
                    buried_name = self.entities[ci].name.clone();
                }
                let cairn_id = self.next_id;
                self.next_id += 1;
                self.entities.push(Entity {
                    id: cairn_id,
                    owner_player: None,
                    name: format!("Cairn of {}", buried_name),
                    kind: EntityKind::Cairn,
                    clan: ClanId(0),
                    pos: bury_pos,
                    z: 0.0,
                    vel: Vec2::ZERO,
                    facing: Vec2::Y,
                    mass_kg: 320.0,
                    material: MaterialId::Granite,
                    wetness: 0.0,
                    temp_k: 285.0,
                    health: 999.0,
                    max_health: 999.0,
                    stamina: 0.0,
                    bleed_rate: 0.0,
                    blood_stain: 0.0,
                    blade_sharpness: 0.0,
                    powder_wetness: 0.0,
                    carrying_id: None,
                    mounted_id: None,
                    buried: false,
                    alive: false,
                    swing_timer: 0.0,
                    recoil_timer: 0.0,
                    trace_accum: 0.0,
                    mind: None,
                });

                self.ledger.push(DeltaEntry::CorpseBuried(cid, bury_pos.x, bury_pos.y));
                // LOOP 7: Quiet witness & gossip ripple across the clachan
                for i in 0..self.entities.len() {
                    if self.entities[i].kind == EntityKind::Villager {
                        let d = self.entities[i].pos.distance(bury_pos);
                        if d < self.cfg.gossip_spread_radius_m {
                            if let Some(m) = &mut self.entities[i].mind {
                                m.memories.push(MemoryKind::SawBurial, self.time, 0.45);
                                m.bond_to_player = (m.bond_to_player + 0.35).min(1.0);
                            }
                        }
                    }
                }
                self.toast(format!(
                    "Stacked a fieldstone cairn over {}. Quiet word will travel through the clachan.",
                    buried_name
                ));
            }

            CommandPacket::WhistleMount { player } => {
                let Some(pi) = self.player_index(player) else { return; };
                let p_pos = self.entities[pi].pos;
                AudioBank::play(&sfx.whistle, 0.42);
                self.emit_sound(p_pos, 72.0, false, true);
                for e in &mut self.entities {
                    if e.kind == EntityKind::Mount {
                        if let Some(m) = &mut e.mind {
                            m.goal = AiGoal::FollowOwner;
                        }
                    }
                }
                self.toast("Whistled for Bramble.");
            }

            CommandPacket::BindWounds { player } => {
                if let Some(pi) = self.player_index(player) {
                    if self.entities[pi].bleed_rate > 0.0 || self.entities[pi].health < 95.0 {
                        self.entities[pi].bleed_rate = 0.0;
                        self.entities[pi].health = (self.entities[pi].health + 18.0).min(100.0);
                        self.entities[pi].blood_stain = (self.entities[pi].blood_stain - 0.35).max(0.0);
                        self.toast("Bound your wounds with clean linen.");
                    } else {
                        self.toast("No open wounds to bind.");
                    }
                }
            }

            CommandPacket::WhetBlade { player } => {
                if let Some(pi) = self.player_index(player) {
                    self.entities[pi].blade_sharpness = 1.0;
                    AudioBank::play(&sfx.steel, 0.25);
                    self.toast("Honed your steel dirk to a razor edge.");
                }
            }

            CommandPacket::BuildCampfire { player } => {
                let Some(pi) = self.player_index(player) else { return; };
                let fpos = self.entities[pi].pos + self.entities[pi].facing * 1.6;
                if self.weather.rain_intensity > 0.65 && !self.is_sheltered(fpos) {
                    self.toast("Too much driving rain to strike peat outdoors!");
                    return;
                }
                let fid = self.next_id;
                self.next_id += 1;
                self.entities.push(Entity {
                    id: fid,
                    owner_player: None,
                    name: "Peat Campfire".to_string(),
                    kind: EntityKind::Campfire,
                    clan: ClanId(0),
                    pos: fpos,
                    z: 0.0,
                    vel: Vec2::ZERO,
                    facing: Vec2::Y,
                    mass_kg: 15.0,
                    material: MaterialId::Peat,
                    wetness: 0.0,
                    temp_k: 720.0,
                    health: 60.0, // fuel seconds
                    max_health: 60.0,
                    stamina: 0.0,
                    bleed_rate: 0.0,
                    blood_stain: 0.0,
                    blade_sharpness: 0.0,
                    powder_wetness: 0.0,
                    carrying_id: None,
                    mounted_id: None,
                    buried: false,
                    alive: true,
                    swing_timer: 0.0,
                    recoil_timer: 0.0,
                    trace_accum: 0.0,
                    mind: None,
                });
                self.toast("Lit a crackling peat fire. Stand close to dry wool & black powder.");
            }
        }
    }

    fn step(&mut self, dt: f32, sfx: &AudioBank) {
        self.time += dt;
        self.weather.step(dt, &self.cfg);
        if self.status_toast.1 > 0.0 {
            self.status_toast.1 -= dt;
        }
        self.utterances.retain_mut(|u| {
            u.ttl -= dt;
            u.ttl > 0.0
        });

        let player_pos = self
            .player_index(PlayerId(1))
            .map(|i| self.entities[i].pos)
            .unwrap_or(Vec2::ZERO);

        // Smooth roof fade for enterable buildings + chimney smoke
        for b in &mut self.buildings {
            let inside = b.contains(player_pos);
            let target_alpha = if inside { 0.0 } else { 1.0 };
            b.roof_alpha += (target_alpha - b.roof_alpha) * (dt * 7.5).min(1.0);
            if b.hearth_lit && self.rng.next_f32() < dt * 2.6 {
                self.smoke.push(SmokePuff {
                    pos: b.hearth_pos,
                    vel: self.weather.wind_vec * 0.28 + Vec2::new(0.0, -0.7),
                    radius: 0.45,
                    age: 0.0,
                    max_age: 4.0,
                    is_ember: self.rng.next_f32() < 0.2,
                });
            }
        }

        // Collect active heat sources (lit building hearths + campfires)
        let mut heat_sources: Vec<Vec2> = self
            .buildings
            .iter()
            .filter(|b| b.hearth_lit)
            .map(|b| b.hearth_pos)
            .collect();
        for e in &self.entities {
            if e.kind == EntityKind::Campfire && e.alive {
                heat_sources.push(e.pos);
            }
        }

        // Acoustic wave expansion + NPC hearing
        for sw in &mut self.sound_waves {
            sw.age += dt;
            sw.radius = (sw.radius + dt * 28.0).min(sw.max_radius);
        }
        let active_waves: Vec<(Vec2, f32, bool)> = self
            .sound_waves
            .iter()
            .filter(|w| w.age < 0.25)
            .map(|w| (w.pos, w.max_radius, w.hostiles_care))
            .collect();
        self.sound_waves.retain(|w| w.age < 1.1);

        // Smoke & traces update
        for sp in &mut self.smoke {
            sp.age += dt;
            sp.pos += (sp.vel + self.weather.wind_vec * 0.15) * dt;
            sp.radius += dt * 0.28;
        }
        self.smoke.retain(|s| s.age < s.max_age);
        for tr in &mut self.traces {
            tr.age += dt;
        }
        self.traces.retain(|t| t.age < t.max_age);

        // Pre-compute entity physics & causal loops
        let mut alert_events: Vec<(String, DialectId, bool)> = Vec::new();
        let mut player_hit_dmg = 0.0f32;
        let mut player_knock = Vec2::ZERO;

        for i in 0..self.entities.len() {
            let kind = self.entities[i].kind;
            if kind == EntityKind::Campfire {
                if self.weather.rain_intensity > 0.7 && !self.is_sheltered(self.entities[i].pos) {
                    self.entities[i].health -= dt * 8.0;
                } else {
                    self.entities[i].health -= dt * 0.4;
                }
                if self.entities[i].health <= 0.0 {
                    self.entities[i].alive = false;
                } else if self.rng.next_f32() < dt * 4.0 {
                    self.smoke.push(SmokePuff {
                        pos: self.entities[i].pos,
                        vel: self.weather.wind_vec * 0.3 + Vec2::new(0.0, -0.8),
                        radius: 0.35,
                        age: 0.0,
                        max_age: 2.5,
                        is_ember: self.rng.next_f32() < 0.35,
                    });
                }
                continue;
            }

            if self.entities[i].swing_timer > 0.0 {
                self.entities[i].swing_timer = (self.entities[i].swing_timer - dt).max(0.0);
            }
            if self.entities[i].recoil_timer > 0.0 {
                self.entities[i].recoil_timer = (self.entities[i].recoil_timer - dt).max(0.0);
            }

            // LOOP 2: Wetness, Black Powder Fouling, and Thermal Shelter
            let pos = self.entities[i].pos;
            let sheltered = self.is_sheltered(pos);
            let near_fire = heat_sources.iter().any(|h| h.distance(pos) < 3.6);
            if self.weather.rain_intensity > 0.05 && !sheltered {
                let soak = self.weather.rain_intensity * self.cfg.wetness_soak_rate * dt;
                self.entities[i].wetness = (self.entities[i].wetness + soak).min(1.0);
                self.entities[i].powder_wetness = (self.entities[i].powder_wetness + soak * 0.85).min(1.0);
                self.entities[i].blood_stain = (self.entities[i].blood_stain - soak * 0.3).max(0.0);
            } else {
                let dry_mult = if near_fire { 3.2 } else if sheltered { 1.4 } else { 0.45 };
                let dry = self.cfg.wetness_dry_rate * dry_mult * dt;
                self.entities[i].wetness = (self.entities[i].wetness - dry).max(0.0);
                self.entities[i].powder_wetness = (self.entities[i].powder_wetness - dry * 1.1).max(0.0);
            }

            if self.entities[i].alive {
                // Core body temperature equilibrium
                let heat_gain = if near_fire { 1.2 } else if sheltered { 0.25 } else { 0.0 };
                let chill = if !sheltered {
                    (290.0 - self.weather.ambient_temp_k).max(0.0)
                        * (0.015 + self.entities[i].wetness * 0.055)
                        * self.cfg.hypothermia_rate
                } else {
                    0.0
                };
                self.entities[i].temp_k = (self.entities[i].temp_k + (heat_gain - chill) * dt).clamp(305.0, 310.5);
                if self.entities[i].temp_k < 307.2 && self.cfg.lethality_mult > 0.0 {
                    self.entities[i].stamina = (self.entities[i].stamina - 4.0 * dt).max(0.0);
                }

                // LOOP 5: Bleeding & Stamina recovery
                if self.entities[i].bleed_rate > 0.0 {
                    self.entities[i].health -= self.entities[i].bleed_rate * dt;
                    if self.rng.next_f32() < dt * 2.2 {
                        self.traces.push(TraceMark {
                            pos,
                            kind: TraceKind::BloodDrop,
                            age: 0.0,
                            max_age: 45.0,
                        });
                    }
                }
                self.entities[i].stamina = (self.entities[i].stamina + 9.5 * dt).min(100.0);
            }

            // AI brains (Mount, Villager, Redcoat, Deer)
            if self.entities[i].alive && kind != EntityKind::Player {
                let mut desired_v = Vec2::ZERO;
                let mut new_goal = None;
                let mut speak_alert = None;

                if let Some(mind) = &mut self.entities[i].mind {
                    if mind.speak_cooldown > 0.0 {
                        mind.speak_cooldown -= dt;
                    }
                    if mind.attack_cooldown > 0.0 {
                        mind.attack_cooldown -= dt;
                    }

                    // Hear loud sounds
                    for (wpos, wrad, hostiles_care) in &active_waves {
                        if pos.distance(*wpos) < *wrad {
                            if kind == EntityKind::Redcoat && *hostiles_care {
                                mind.goal = AiGoal::Investigate(*wpos);
                                mind.awareness = (mind.awareness + 0.6).min(1.0);
                            } else if kind == EntityKind::Villager && *hostiles_care {
                                mind.memories.push(MemoryKind::HeardGunfire, self.time, -0.15);
                            } else if kind == EntityKind::Deer && *hostiles_care {
                                mind.goal = AiGoal::Flee;
                            }
                        }
                    }

                    match kind {
                        EntityKind::Mount => {
                            if mind.goal == AiGoal::FollowOwner {
                                let d = pos.distance(player_pos);
                                if d > 2.4 {
                                    desired_v = (player_pos - pos).normalize_or_zero() * 6.8;
                                } else {
                                    mind.goal = AiGoal::Graze;
                                }
                            }
                        }
                        EntityKind::Villager => {
                            if self.weather.rain_intensity > 0.45 {
                                let to_h = mind.home_pos - pos;
                                if to_h.length() > 1.2 {
                                    desired_v = to_h.normalize_or_zero() * 2.4;
                                }
                            } else if mind.goal == AiGoal::Flee {
                                desired_v = (pos - player_pos).normalize_or_zero() * 4.2;
                                if pos.distance(player_pos) > 16.0 {
                                    mind.goal = AiGoal::Idle;
                                }
                            }
                        }
                        EntityKind::Deer => {
                            let d = pos.distance(player_pos);
                            if d < 8.5 {
                                mind.goal = AiGoal::Flee;
                            }
                            if mind.goal == AiGoal::Flee {
                                desired_v = (pos - player_pos).normalize_or_zero() * 6.4;
                                if d > 22.0 {
                                    mind.goal = AiGoal::Graze;
                                }
                            }
                        }
                        EntityKind::Redcoat => {
                            // LOOP 7: Discover severed gibbet ropes organically via vision cone!
                            let vis_range = self.cfg.vision_range_m * (1.0 - self.weather.mist_intensity * 0.55);
                            for c in &mut self.constraints {
                                if !c.intact && !c.discovered_by_patrol && pos.distance(c.anchor) < vis_range * 0.65 {
                                    c.discovered_by_patrol = true;
                                    mind.awareness = 1.0;
                                    self.clans[1].alert_level = (self.clans[1].alert_level + 0.5).min(1.0);
                                    self.ledger.push(DeltaEntry::GarrisonAlert(self.clans[1].alert_level));
                                    speak_alert = Some((self.entities[i].name.clone(), mind.dialect, true));
                                    new_goal = Some(AiGoal::Investigate(c.anchor));
                                }
                            }

                            let dist_p = pos.distance(player_pos);
                            if dist_p < vis_range && self.has_line_of_sight(pos, player_pos) {
                                let alert_push = if self.clans[1].alert_level > 0.3 { 0.45 } else { 0.0 };
                                if alert_push > 0.0 || mind.goal == AiGoal::Engage {
                                    mind.awareness = (mind.awareness + dt * 0.65).min(1.0);
                                    if mind.awareness > 0.85 {
                                        mind.goal = AiGoal::Engage;
                                        if mind.speak_cooldown <= 0.0 {
                                            mind.speak_cooldown = 7.0;
                                            speak_alert = Some((self.entities[i].name.clone(), mind.dialect, false));
                                        }
                                    }
                                }
                            }

                            match mind.goal {
                                AiGoal::Patrol => {
                                    let tgt = if mind.patrol_to_b { mind.patrol_b } else { mind.patrol_a };
                                    if pos.distance(tgt) < 1.3 {
                                        mind.patrol_to_b = !mind.patrol_to_b;
                                    } else {
                                        desired_v = (tgt - pos).normalize_or_zero() * 2.5;
                                    }
                                }
                                AiGoal::Investigate(wp) => {
                                    if pos.distance(wp) < 2.0 {
                                        mind.goal = AiGoal::Patrol;
                                    } else {
                                        desired_v = (wp - pos).normalize_or_zero() * 3.5;
                                    }
                                }
                                AiGoal::Engage => {
                                    if dist_p > 1.8 {
                                        desired_v = (player_pos - pos).normalize_or_zero() * 4.1;
                                    } else if mind.attack_cooldown <= 0.0 {
                                        mind.attack_cooldown = 1.15;
                                        self.entities[i].swing_timer = 0.25;
                                        player_hit_dmg += 16.0 * self.cfg.lethality_mult;
                                        player_knock += (player_pos - pos).normalize_or_zero()
                                            * (6.5 * self.cfg.knockback_mult);
                                    }
                                    if dist_p > 26.0 {
                                        mind.goal = AiGoal::Patrol;
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    if let Some(g) = new_goal {
                        mind.goal = g;
                    }
                }
                if let Some(ev) = speak_alert {
                    alert_events.push(ev);
                }
                let cur = self.entities[i].vel;
                self.entities[i].vel = cur.lerp(desired_v, (6.0 * dt).min(1.0));
                if desired_v.length_squared() > 0.04 {
                    self.entities[i].facing = desired_v.normalize();
                }
            }

            // Integrate motion + wall collisions + terrain bounds + mud footprints
            let prev = self.entities[i].pos;
            let mut next = prev + self.entities[i].vel * dt;

            // Keep out of deep loch water (bounce off shore in Party Chaos!)
            let (_, tk) = sample_terrain(self.cfg.world_seed, next.x, next.y);
            if tk == TerrainKind::LochWater {
                next = prev;
                self.entities[i].vel.x = -self.entities[i].vel.x * self.cfg.global_restitution_mult;
            }
            next.x = next.x.clamp(-38.0, 44.0);
            next.y = next.y.clamp(-38.0, 38.0);

            for b in &self.buildings {
                next = b.resolve_wall_collision(prev, next, 0.45);
            }
            self.entities[i].pos = next;

            // Friction damping on corpses / idle bodies
            if !self.entities[i].alive {
                self.entities[i].vel *= (1.0 - 6.0 * self.cfg.global_friction_mult * dt).max(0.0);
            }

            // LOOP 3: Mud & hoof traces
            let step_dist = next.distance(prev);
            self.entities[i].trace_accum += step_dist;
            if self.entities[i].trace_accum > 1.35 && (tk == TerrainKind::MudRoad || tk == TerrainKind::PeatBog || self.weather.rain_intensity > 0.2) {
                self.entities[i].trace_accum = 0.0;
                let tkind = if kind == EntityKind::Mount {
                    TraceKind::HoofPrint
                } else {
                    TraceKind::BootPrint
                };
                self.traces.push(TraceMark {
                    pos: next,
                    kind: tkind,
                    age: 0.0,
                    max_age: 35.0,
                });
            }
        }

        // Sync mounted horse & carried corpse positions cleanly
        if let Some(pi) = self.player_index(PlayerId(1)) {
            let ppos = self.entities[pi].pos;
            let pface = self.entities[pi].facing;
            if let Some(mid) = self.entities[pi].mounted_id {
                if let Some(mi) = self.entity_index(mid) {
                    self.entities[mi].pos = ppos;
                    self.entities[mi].facing = pface;
                }
            }
            if let Some(cid) = self.entities[pi].carrying_id {
                if let Some(ci) = self.entity_index(cid) {
                    self.entities[ci].pos = ppos + Vec2::new(0.0, -0.35);
                }
            }
            if player_hit_dmg > 0.0 || player_knock.length_squared() > 0.01 {
                self.entities[pi].health -= player_hit_dmg;
                self.entities[pi].vel += player_knock;
                if player_hit_dmg > 0.0 {
                    AudioBank::play(&sfx.steel, 0.4);
                    self.entities[pi].bleed_rate += 0.9 * self.cfg.bleed_rate_mult;
                    self.entities[pi].blood_stain = (self.entities[pi].blood_stain + 0.25).min(1.0);
                }
            }
            // Defeat consequence hook (1744 Captivity vs Party Bounce)
            if self.entities[pi].health <= 0.0 {
                match self.cfg.defeat_mode {
                    DefeatMode::CaptivityTimeSkip => {
                        self.weather.day_count += self.cfg.defeat_time_skip_days;
                        self.entities[pi].health = 65.0;
                        self.entities[pi].bleed_rate = 0.0;
                        self.entities[pi].pos = Vec2::new(-9.0, -4.5); // Wakes recovering in bothy
                        self.entities[pi].mounted_id = None;
                        self.entities[pi].carrying_id = None;
                        let scar = format!("Day {} Garrison Clash Scar", self.weather.day_count);
                        self.scars.push(scar.clone());
                        self.ledger.push(DeltaEntry::ScarEarned(scar));
                        self.toast("Left for dead by the Garrison. 3 days passed before clan kin dragged you home.");
                    }
                    DefeatMode::KnockoutBounce => {
                        self.entities[pi].health = 100.0;
                        self.entities[pi].vel = Vec2::new(-14.0, -8.0);
                        self.toast("BOING! Knocked out of bounds — right back in!");
                    }
                }
            }
        }

        // Emit any Redcoat alerts
        for (name, dialect, found_rope) in alert_events {
            self.utterances.push(compose_redcoat_alert(&name, found_rope, dialect));
            AudioBank::play(&sfx.voice_bark, 0.5);
        }

        self.entities.retain(|e| e.kind != EntityKind::Campfire || e.alive);
    }

    fn apply_ledger(&mut self, ledger: DeltaLedger) {
        for e in &ledger.entries {
            match e {
                DeltaEntry::RopeSevered(cid) => {
                    if let Some(c) = self.constraints.iter_mut().find(|c| c.id == *cid) {
                        c.intact = false;
                        if let Some(ei) = self.entity_index(c.entity_id) {
                            self.entities[ei].z = 0.0;
                        }
                    }
                }
                DeltaEntry::CorpseBuried(eid, x, y) => {
                    if let Some(ei) = self.entity_index(*eid) {
                        self.entities[ei].buried = true;
                        self.entities[ei].pos = Vec2::new(*x, *y);
                    }
                    let cid = self.next_id;
                    self.next_id += 1;
                    self.entities.push(Entity {
                        id: cid,
                        owner_player: None,
                        name: "Fieldstone Cairn".to_string(),
                        kind: EntityKind::Cairn,
                        clan: ClanId(0),
                        pos: Vec2::new(*x, *y),
                        z: 0.0,
                        vel: Vec2::ZERO,
                        facing: Vec2::Y,
                        mass_kg: 320.0,
                        material: MaterialId::Granite,
                        wetness: 0.0,
                        temp_k: 285.0,
                        health: 999.0,
                        max_health: 999.0,
                        stamina: 0.0,
                        bleed_rate: 0.0,
                        blood_stain: 0.0,
                        blade_sharpness: 0.0,
                        powder_wetness: 0.0,
                        carrying_id: None,
                        mounted_id: None,
                        buried: false,
                        alive: false,
                        swing_timer: 0.0,
                        recoil_timer: 0.0,
                        trace_accum: 0.0,
                        mind: None,
                    });
                }
                DeltaEntry::GarrisonAlert(v) => {
                    self.clans[1].alert_level = *v;
                }
                DeltaEntry::ScarEarned(s) => {
                    self.scars.push(s.clone());
                }
            }
        }
        self.ledger = ledger;
    }
}

// ============================================================================
// MODULE 11: 3/4 TOP-DOWN PAINTERLY RENDERER & LIVE UNIVERSE FORGE UI
// ============================================================================
fn draw_world(u: &Universe, atlas: &PaintedAtlas, cam: Vec2, view_w_m: f32, view_h_m: f32) {
    let tile_m = 2.0f32;
    let min_tx = ((cam.x - view_w_m * 0.6) / tile_m).floor() as i32;
    let max_tx = ((cam.x + view_w_m * 0.6) / tile_m).ceil() as i32;
    let min_ty = ((cam.y - view_h_m * 0.6) / tile_m).floor() as i32;
    let max_ty = ((cam.y + view_h_m * 0.6) / tile_m).ceil() as i32;

    // 1. Oil-painted terrain tiles
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

    // 2. Surface traces (mud footprints, hoofprints, blood drops)
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

    // 3. Acoustic ripples
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

    // 4. Enterable stone buildings (floors, hearths, masonry walls)
    let shadow_off = u.weather.sun_dir * 0.75;
    for b in &u.buildings {
        // Directional cast shadow
        draw_rectangle(
            b.x + shadow_off.x,
            b.y + shadow_off.y,
            b.w,
            b.h,
            Color::new(0.05, 0.06, 0.08, 0.34 * u.weather.sun_warmth),
        );
        // Interior floor
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
        // Stone walls
        draw_rectangle_lines(b.x, b.y, b.w, b.h, 0.45, Color::new(0.36, 0.35, 0.33, 1.0));
        // Open doorway threshold
        draw_rectangle(b.door_pos.x - 0.8, b.y + b.h - 0.3, 1.6, 0.55, Color::new(0.45, 0.32, 0.18, 1.0));
        // Warm crackling peat hearth
        draw_circle(b.hearth_pos.x, b.hearth_pos.y, 0.72, Color::new(0.24, 0.22, 0.21, 1.0));
        if b.hearth_lit {
            let flick = 0.85 + (u.time * 14.0 + b.id as f32).sin() * 0.15;
            draw_circle(b.hearth_pos.x, b.hearth_pos.y, 2.4 * flick, Color::new(1.0, 0.56, 0.18, 0.16));
            draw_circle(b.hearth_pos.x, b.hearth_pos.y, 0.42 * flick, Color::new(1.0, 0.65, 0.18, 0.95));
        }
    }

    // 5. Gibbet timber frame on Wade's Road
    let gx = 12.0;
    let gy = -1.5;
    draw_rectangle(gx - 2.2 + shadow_off.x, gy - 0.3 + shadow_off.y, 4.4, 0.45, Color::new(0.05, 0.05, 0.07, 0.35));
    draw_rectangle(gx - 2.1, gy - 2.2, 0.35, 2.5, Color::new(0.30, 0.21, 0.14, 1.0));
    draw_rectangle(gx + 1.75, gy - 2.2, 0.35, 2.5, Color::new(0.30, 0.21, 0.14, 1.0));
    draw_rectangle(gx - 2.25, gy - 2.25, 4.5, 0.38, Color::new(0.34, 0.24, 0.16, 1.0));

    // Live physics hemp ropes
    for c in &u.constraints {
        if c.intact {
            draw_line(c.anchor.x, c.anchor.y - 2.0, c.anchor.x, c.anchor.y + 0.1, 0.09, Color::new(0.76, 0.66, 0.44, 1.0));
        } else {
            // Cut hemp stub left hanging — visible physical evidence!
            draw_line(c.anchor.x, c.anchor.y - 2.0, c.anchor.x + 0.08, c.anchor.y - 1.35, 0.09, Color::new(0.76, 0.66, 0.44, 0.9));
        }
    }

    // 6. Entities sorted by Y for natural 3/4 depth
    let mut order: Vec<usize> = (0..u.entities.len()).collect();
    order.sort_by(|&a, &b| u.entities[a].pos.y.partial_cmp(&u.entities[b].pos.y).unwrap_or(std::cmp::Ordering::Equal));

    for idx in order {
        let e = &u.entities[idx];
        if e.buried {
            continue;
        }
        let p = e.pos;
        // Directional shadow
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
                // Stacked Highland fieldstone memorial
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
                // Bramble — Highland dun horse
                let dir = e.facing.normalize_or_zero();
                draw_circle(p.x - dir.x * 0.35, p.y - dir.y * 0.2, 0.68, Color::new(0.47, 0.34, 0.22, 1.0));
                draw_circle(p.x + dir.x * 0.35, p.y + dir.y * 0.2, 0.62, Color::new(0.52, 0.38, 0.24, 1.0));
                draw_circle(p.x + dir.x * 0.85, p.y + dir.y * 0.55 - 0.25, 0.34, Color::new(0.38, 0.26, 0.17, 1.0));
                // Saddlebags
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
                // Wool plaid / tunic
                draw_circle(p.x, p.y + y_lift, 0.48, coat);
                draw_line(p.x - 0.3, p.y - 0.25 + y_lift, p.x + 0.3, p.y + 0.25 + y_lift, 0.16, accent);
                // Blood stain overlay
                if e.blood_stain > 0.1 {
                    draw_circle(p.x + 0.12, p.y + 0.1 + y_lift, 0.22, Color::new(0.58, 0.05, 0.05, e.blood_stain * 0.8));
                }
                // Head
                draw_circle(p.x, p.y - 0.46 + y_lift, 0.28, Color::new(0.84, 0.70, 0.58, 1.0));
                // Redcoat tricorne hat or Highland bonnet
                let hat = if e.kind == EntityKind::Redcoat {
                    Color::new(0.12, 0.12, 0.14, 1.0)
                } else {
                    Color::new(0.16, 0.22, 0.32, 1.0)
                };
                draw_circle(p.x, p.y - 0.62 + y_lift, 0.24, hat);

                // Steel blade swing arc
                if e.swing_timer > 0.0 {
                    let tip = p + e.facing * 1.65;
                    draw_line(p.x, p.y + y_lift, tip.x, tip.y + y_lift, 0.12, Color::new(0.88, 0.92, 0.96, 0.95));
                }

                // Alert indicator over investigating/hostile Redcoats
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

    // 7. Cottage roofs (fade smoothly when Callum steps inside!)
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
            // Roof ridge line
            draw_line(
                b.x - 0.2,
                b.y + b.h * 0.45,
                b.x + b.w + 0.2,
                b.y + b.h * 0.45,
                0.22,
                Color::new(0.28, 0.21, 0.13, b.roof_alpha),
            );
            // Stone chimney
            draw_rectangle(
                b.hearth_pos.x - 0.45,
                b.hearth_pos.y - 0.45,
                0.9,
                0.9,
                Color::new(0.38, 0.37, 0.35, b.roof_alpha),
            );
        }
    }

    // 8. Painted Scots Pines
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

    // 9. Smoke & hearth embers
    for sp in &u.smoke {
        let a = (1.0 - sp.age / sp.max_age).clamp(0.0, 1.0);
        let col = if sp.is_ember {
            Color::new(1.0, 0.62, 0.18, a)
        } else {
            Color::new(0.76, 0.78, 0.78, a * 0.38)
        };
        draw_circle(sp.pos.x, sp.pos.y, sp.radius, col);
    }

    // 10. Driving Highland rain streaks
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

fn draw_hud_and_forge(u: &mut Universe, forge_open: &mut bool) {
    let sw = screen_width();
    let sh = screen_height();

    // Top-left crisp Highland header
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

    // Bottom control strip
    draw_rectangle(14.0, sh - 38.0, sw - 28.0, 26.0, Color::new(0.08, 0.09, 0.08, 0.75));
    draw_text(
        "[WASD] Move  [Click/Space] Steel  [F] Flintlock  [E] Talk/Mount/Carry  [B] Bury Cairn  [H] Whistle Bramble  [C] Fire  [R] Bandage  [V] Whet  [TAB] Universe Forge  [F5/F9] Save/Load",
        22.0,
        sh - 20.0,
        15.0,
        Color::new(0.84, 0.82, 0.76, 0.92),
    );

    // Active NPC Dialogue Banner (Zero-LLM Highland Dialect)
    if let Some(utt) = u.utterances.last() {
        let bw = (sw - 120.0).min(860.0);
        let bx = (sw - bw) * 0.5;
        let by = sh - 126.0;
        draw_rectangle(bx, by, bw, 76.0, Color::new(0.10, 0.09, 0.08, 0.88));
        draw_rectangle_lines(bx, by, bw, 76.0, 1.5, Color::new(0.75, 0.64, 0.40, 0.9));
        draw_text(&utt.speaker, bx + 14.0, by + 22.0, 19.0, Color::new(0.96, 0.84, 0.52, 1.0));
        draw_text(&utt.text, bx + 14.0, by + 48.0, 17.0, Color::new(0.93, 0.92, 0.88, 1.0));
    }

    // Status toast
    if u.status_toast.1 > 0.0 {
        let tw = measure_text(&u.status_toast.0, None, 18, 1.0).width + 28.0;
        let tx = (sw - tw) * 0.5;
        draw_rectangle(tx, 18.0, tw, 30.0, Color::new(0.10, 0.12, 0.10, 0.86));
        draw_text(&u.status_toast.0, tx + 14.0, 38.0, 18.0, Color::new(0.95, 0.90, 0.72, 1.0));
    }

    // Live Universe Forge Panel (TAB)
    if *forge_open {
        let px = sw - 330.0;
        let py = 14.0;
        draw_rectangle(px, py, 316.0, 370.0, Color::new(0.08, 0.09, 0.11, 0.92));
        draw_rectangle_lines(px, py, 316.0, 370.0, 2.0, Color::new(0.82, 0.68, 0.36, 0.95));
        draw_text("UNIVERSE FORGE (Live Laws)", px + 14.0, py + 28.0, 20.0, Color::new(0.96, 0.86, 0.56, 1.0));
        draw_text(&format!("Preset: {}", u.cfg.preset_name), px + 14.0, py + 50.0, 15.0, WHITE);

        let (mx, my) = mouse_position();
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        let mut btn = |label: &str, x: f32, y: f32, w: f32, h: f32| -> bool {
            let hov = mx >= x && mx <= x + w && my >= y && my <= y + h;
            draw_rectangle(x, y, w, h, if hov { Color::new(0.28, 0.34, 0.26, 1.0) } else { Color::new(0.18, 0.20, 0.22, 1.0) });
            draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.65, 0.60, 0.45, 0.8));
            draw_text(label, x + 8.0, y + h * 0.68, 15.0, WHITE);
            hov && clicked
        };

        // Swap entire universe presets instantaneously!
        if btn("1744 Lochaber (Bright)", px + 14.0, py + 64.0, 288.0, 26.0) {
            *u = Universe::new(UniverseConfig::preset_1744_lochaber());
        }
        if btn("1746 Bleak Rising (Dark)", px + 14.0, py + 96.0, 288.0, 26.0) {
            *u = Universe::new(UniverseConfig::preset_1746_bleak_rising());
        }
        if btn("Party Chaos (Bounce Arena)", px + 14.0, py + 128.0, 288.0, 26.0) {
            *u = Universe::new(UniverseConfig::preset_party_chaos());
        }

        // Real-time parametric law knobs
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

fn window_conf() -> Conf {
    Conf {
        window_title: "Clachan".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let atlas = PaintedAtlas::build();
    let sfx = AudioBank::synthesize().await;
    let mut universe = Universe::new(UniverseConfig::preset_1744_lochaber());
    let mut forge_open = false;
    let mut zoom_m = 22.0f32; // Vertical meters visible on screen

    let mut rt_w = screen_width().max(64.0) as u32;
    let mut rt_h = screen_height().max(64.0) as u32;
    let mut rt = render_target(rt_w, rt_h);
    rt.texture.set_filter(FilterMode::Linear);

    let mut cam = Vec2::new(-8.0, 0.5);
    let mut accum = 0.0f32;
    let fixed_dt = 1.0 / 60.0;

    loop {
        let sw = screen_width().max(64.0);
        let sh = screen_height().max(64.0);
        if sw as u32 != rt_w || sh as u32 != rt_h {
            rt_w = sw as u32;
            rt_h = sh as u32;
            rt = render_target(rt_w, rt_h);
            rt.texture.set_filter(FilterMode::Linear);
        }

        let (_, wy) = mouse_wheel();
        if wy.abs() > 0.01 {
            zoom_m = (zoom_m - wy * 1.5).clamp(12.0, 38.0);
        }

        if is_key_pressed(KeyCode::Tab) {
            forge_open = !forge_open;
        }
        if is_key_pressed(KeyCode::F5) {
            match universe.ledger.save_to_disk() {
                Ok(_) => universe.toast("Delta Ledger saved (clachan_save.ledger)."),
                Err(e) => universe.toast(format!("Save error: {}", e)),
            }
        }
        if is_key_pressed(KeyCode::F9) {
            match DeltaLedger::load_from_disk() {
                Ok(l) => {
                    let cfg = universe.cfg.clone();
                    universe = Universe::new(cfg);
                    universe.apply_ledger(l);
                    universe.toast("Delta Ledger restored.");
                }
                Err(e) => universe.toast(format!("Load error: {}", e)),
            }
        }

        // Translate player controls into multiplayer-ready CommandPackets
        let mut dir = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { dir.y -= 1.0; }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { dir.y += 1.0; }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { dir.x -= 1.0; }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { dir.x += 1.0; }
        let sprint = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        let aspect = sw / sh;
        let view_h_m = zoom_m;
        let view_w_m = view_h_m * aspect;
        let (mx, my) = mouse_position();
        let mouse_world = Vec2::new(
            cam.x + (mx / sw - 0.5) * view_w_m,
            cam.y + (my / sh - 0.5) * view_h_m,
        );
        let ppos = universe
            .player_index(PlayerId(1))
            .map(|i| universe.entities[i].pos)
            .unwrap_or(Vec2::ZERO);
        let aim = (mouse_world - ppos).normalize_or_zero();

        universe.apply_command(CommandPacket::Move { player: PlayerId(1), dir, sprint }, &sfx);

        let click_in_world = is_mouse_button_pressed(MouseButton::Left) && (!forge_open || mx < sw - 335.0);
        if click_in_world || is_key_pressed(KeyCode::Space) {
            universe.apply_command(CommandPacket::SwingSteel { player: PlayerId(1), aim }, &sfx);
        }
        if is_key_pressed(KeyCode::F) {
            universe.apply_command(CommandPacket::FireFlintlock { player: PlayerId(1), aim }, &sfx);
        }
        if is_key_pressed(KeyCode::E) {
            universe.apply_command(CommandPacket::Interact { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::B) {
            universe.apply_command(CommandPacket::BuryCarried { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::H) {
            universe.apply_command(CommandPacket::WhistleMount { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::R) {
            universe.apply_command(CommandPacket::BindWounds { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::V) {
            universe.apply_command(CommandPacket::WhetBlade { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::C) {
            universe.apply_command(CommandPacket::BuildCampfire { player: PlayerId(1) }, &sfx);
        }

        // Fixed 60Hz deterministic simulation loop
        accum = (accum + get_frame_time().min(0.08)).min(0.16);
        while accum >= fixed_dt {
            universe.step(fixed_dt, &sfx);
            accum -= fixed_dt;
        }

        if let Some(pi) = universe.player_index(PlayerId(1)) {
            cam = cam.lerp(universe.entities[pi].pos, (get_frame_time() * 7.0).min(1.0));
        }

        // Pass 1: Render 3/4 top-down painted world into offscreen RenderTarget
        set_camera(&Camera2D {
            target: cam,
            zoom: vec2(2.0 / view_w_m, 2.0 / view_h_m),
            render_target: Some(rt.clone()),
            ..Default::default()
        });
        clear_background(Color::new(0.14, 0.24, 0.28, 1.0));
        draw_world(&universe, &atlas, cam, view_w_m, view_h_m);

        // Pass 2: Blit through the GLSL Oil-Painting, Sun-Warmth & Era-Grading Shader
        set_default_camera();
        clear_background(BLACK);
        atlas.post_mat.set_uniform("u_time", universe.time);
        atlas.post_mat.set_uniform("u_warmth", universe.weather.sun_warmth);
        atlas.post_mat.set_uniform("u_darkness", universe.cfg.era_darkness);
        atlas.post_mat.set_uniform("u_mist", universe.weather.mist_intensity);
        atlas.post_mat.set_uniform("u_res_x", sw);
        atlas.post_mat.set_uniform("u_res_y", sh);

        gl_use_material(&atlas.post_mat);
        draw_texture_ex(
            &rt.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(sw, sh)),
                ..Default::default()
            },
        );
        gl_use_default_material();

        // Pass 3: Crisp HUD + Live Universe Forge
        draw_hud_and_forge(&mut universe, &mut forge_open);

        next_frame().await;
    }
}
