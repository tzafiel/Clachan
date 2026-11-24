use crate::config::{DialectId, MaterialId};
use crate::dialogue::MemoryRing;
use crate::world::Location;
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
    pub home_building_id: Option<u32>,
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
    pub location: Location,
    pub pos: Vec2,
    pub z: f32,
    pub vel: Vec2,
    pub facing: Vec2,
    pub mass_kg: f32,
    pub material: MaterialId,
    pub wetness: f32,
    pub temp_k: f32,
    pub health: f32,
    pub max_health: f32,
    pub stamina: f32,
    pub bleed_rate: f32,
    pub blood_stain: f32,
    pub blade_sharpness: f32,
    pub powder_wetness: f32,
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
        std::fs::write("clachan_save.ledger", self.serialize_text()).map_err(|e| e.to_string())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save_to_disk(&self) -> Result<(), String> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_disk() -> Result<Self, String> {
        let txt = std::fs::read_to_string("clachan_save.ledger").map_err(|e| e.to_string())?;
        Ok(Self::deserialize_text(&txt))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_from_disk() -> Result<Self, String> {
        Err("Disk load unavailable in WASM".to_string())
    }
}
