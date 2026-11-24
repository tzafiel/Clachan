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
    MaterialSpec { density_kgm3: 1.225, tensile_pa: 0.0, shear_pa: 0.0, friction_mu: 0.0, restitution: 0.0, porosity: 1.0, ignition_k: 9999.0, sound_absorb: 0.02 },
    MaterialSpec { density_kgm3: 1000.0, tensile_pa: 0.0, shear_pa: 0.0, friction_mu: 0.05, restitution: 0.02, porosity: 1.0, ignition_k: 9999.0, sound_absorb: 0.20 },
    MaterialSpec { density_kgm3: 1500.0, tensile_pa: 1.0e4, shear_pa: 2.0e4, friction_mu: 0.65, restitution: 0.05, porosity: 0.45, ignition_k: 9999.0, sound_absorb: 0.55 },
    MaterialSpec { density_kgm3: 900.0, tensile_pa: 5.0e3, shear_pa: 8.0e3, friction_mu: 0.45, restitution: 0.03, porosity: 0.80, ignition_k: 520.0, sound_absorb: 0.70 },
    MaterialSpec { density_kgm3: 2700.0, tensile_pa: 1.5e7, shear_pa: 3.0e7, friction_mu: 0.75, restitution: 0.25, porosity: 0.02, ignition_k: 9999.0, sound_absorb: 0.92 },
    MaterialSpec { density_kgm3: 750.0, tensile_pa: 9.0e7, shear_pa: 1.2e7, friction_mu: 0.55, restitution: 0.20, porosity: 0.25, ignition_k: 573.0, sound_absorb: 0.65 },
    MaterialSpec { density_kgm3: 520.0, tensile_pa: 6.0e7, shear_pa: 7.0e6, friction_mu: 0.50, restitution: 0.18, porosity: 0.35, ignition_k: 530.0, sound_absorb: 0.55 },
    MaterialSpec { density_kgm3: 180.0, tensile_pa: 2.0e5, shear_pa: 1.5e5, friction_mu: 0.60, restitution: 0.05, porosity: 0.90, ignition_k: 495.0, sound_absorb: 0.75 },
    MaterialSpec { density_kgm3: 850.0, tensile_pa: 6.5e7, shear_pa: 180.0, friction_mu: 0.62, restitution: 0.06, porosity: 0.65, ignition_k: 510.0, sound_absorb: 0.40 },
    MaterialSpec { density_kgm3: 320.0, tensile_pa: 1.5e6, shear_pa: 2.5e5, friction_mu: 0.65, restitution: 0.04, porosity: 0.85, ignition_k: 620.0, sound_absorb: 0.80 },
    MaterialSpec { density_kgm3: 940.0, tensile_pa: 2.5e7, shear_pa: 8.0e6, friction_mu: 0.60, restitution: 0.10, porosity: 0.20, ignition_k: 640.0, sound_absorb: 0.50 },
    MaterialSpec { density_kgm3: 1060.0, tensile_pa: 1.5e6, shear_pa: 4.5e5, friction_mu: 0.45, restitution: 0.05, porosity: 0.10, ignition_k: 750.0, sound_absorb: 0.45 },
    MaterialSpec { density_kgm3: 1900.0, tensile_pa: 1.2e8, shear_pa: 5.0e7, friction_mu: 0.40, restitution: 0.15, porosity: 0.05, ignition_k: 900.0, sound_absorb: 0.60 },
    MaterialSpec { density_kgm3: 7870.0, tensile_pa: 3.5e8, shear_pa: 2.2e8, friction_mu: 0.35, restitution: 0.35, porosity: 0.0, ignition_k: 1800.0, sound_absorb: 0.85 },
    MaterialSpec { density_kgm3: 7850.0, tensile_pa: 6.0e8, shear_pa: 4.2e8, friction_mu: 0.30, restitution: 0.45, porosity: 0.0, ignition_k: 1850.0, sound_absorb: 0.88 },
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
    pub gravity_mps2: f32,
    pub air_density_kgm3: f32,
    pub day_length_sec: f32,
    pub global_friction_mult: f32,
    pub global_restitution_mult: f32,
    pub knockback_mult: f32,
    pub base_temp_kelvin: f32,
    pub rain_probability: f32,
    pub base_wind_mps: f32,
    pub mist_baseline: f32,
    pub era_darkness: f32,
    pub wetness_soak_rate: f32,
    pub wetness_dry_rate: f32,
    pub ignition_humidity_penalty: f32,
    pub lethality_mult: f32,
    pub stamina_drain_mult: f32,
    pub bleed_rate_mult: f32,
    pub hypothermia_rate: f32,
    pub defeat_mode: DefeatMode,
    pub defeat_time_skip_days: u32,
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
