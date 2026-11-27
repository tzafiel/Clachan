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
    pub territory_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Player,
    Mount,
    Villager,
    Corpse,
    Cairn,
    Deer,
    Campfire,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiGoal {
    Idle,
    WarmAtHearth,
    Flee,
    FollowOwner,
    Graze,
}

#[derive(Clone, Debug)]
pub struct AgentMind {
    pub courage: f32,
    pub loyalty: f32,
    pub suspicion: f32,
    pub bond_to_player: f32,
    pub goal: AiGoal,
    pub dialect: DialectId,
    pub memories: MemoryRing,
    pub home_pos: Vec2,
    pub home_building_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryItem {
    Dirk,
    Hatchet,
    Whistle,
    Bandage,
    PeatFire,
}

#[derive(Clone, Copy, Debug)]
pub struct InventoryStack {
    pub item: InventoryItem,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct Inventory {
    pub selected_slot: usize,
    pub hotbar: [Option<InventoryStack>; 5],
    pub backpack: Vec<InventoryStack>,
}

impl Inventory {
    pub fn starter() -> Self {
        Self {
            selected_slot: 0,
            hotbar: [
                Some(InventoryStack {
                    item: InventoryItem::Dirk,
                    count: 1,
                }),
                Some(InventoryStack {
                    item: InventoryItem::Hatchet,
                    count: 1,
                }),
                Some(InventoryStack {
                    item: InventoryItem::Whistle,
                    count: 1,
                }),
                Some(InventoryStack {
                    item: InventoryItem::Bandage,
                    count: 3,
                }),
                Some(InventoryStack {
                    item: InventoryItem::PeatFire,
                    count: 2,
                }),
            ],
            backpack: Vec::new(),
        }
    }

    pub fn count(&self, item: InventoryItem) -> u32 {
        self.hotbar
            .iter()
            .flatten()
            .chain(self.backpack.iter())
            .filter(|stack| stack.item == item)
            .map(|stack| stack.count)
            .sum()
    }

    pub fn consume(&mut self, item: InventoryItem, amount: u32) -> bool {
        if self.count(item) < amount {
            return false;
        }
        let mut remaining = amount;
        for stack in self
            .hotbar
            .iter_mut()
            .flatten()
            .chain(self.backpack.iter_mut())
        {
            if stack.item == item {
                let used = stack.count.min(remaining);
                stack.count -= used;
                remaining -= used;
                if remaining == 0 {
                    break;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::{Inventory, InventoryItem};

    #[test]
    fn consuming_stacks_does_not_overdraw_bandages() {
        let mut inventory = Inventory::starter();
        assert_eq!(inventory.count(InventoryItem::Bandage), 3);
        assert!(inventory.consume(InventoryItem::Bandage, 2));
        assert_eq!(inventory.count(InventoryItem::Bandage), 1);
        assert!(!inventory.consume(InventoryItem::Bandage, 2));
        assert_eq!(inventory.count(InventoryItem::Bandage), 1);
    }

    #[test]
    fn starter_hotbar_is_the_way_warden_kit() {
        let inventory = Inventory::starter();
        let items = inventory
            .hotbar
            .iter()
            .map(|stack| stack.map(|stack| stack.item))
            .collect::<Vec<_>>();
        assert_eq!(
            items,
            [
                Some(InventoryItem::Dirk),
                Some(InventoryItem::Hatchet),
                Some(InventoryItem::Whistle),
                Some(InventoryItem::Bandage),
                Some(InventoryItem::PeatFire),
            ]
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MicroState {
    pub hood_up: bool,
    pub hood_timer: f32,
    pub in_shelter: bool,
    pub warming_hands: bool,
    pub idle_timer: f32,
}
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
    pub walk_phase: f32,
    pub posture: MicroState,
    pub inventory: Option<Inventory>,
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
    Move {
        player: PlayerId,
        dir: Vec2,
        sprint: bool,
    },
    SelectHotbar {
        player: PlayerId,
        slot: usize,
    },
    UseHotbar {
        player: PlayerId,
        aim: Vec2,
    },
    SwingSteel {
        player: PlayerId,
        aim: Vec2,
    },
    ChopWithHatchet {
        player: PlayerId,
        aim: Vec2,
    },
    Interact {
        player: PlayerId,
    },
    BuryCarried {
        player: PlayerId,
    },
    WhistleMount {
        player: PlayerId,
    },
    BindWounds {
        player: PlayerId,
    },
    WhetBlade {
        player: PlayerId,
    },
    BuildCampfire {
        player: PlayerId,
    },
}

#[derive(Clone, Debug)]
pub enum DeltaEntry {
    RopeSevered(u32),
    CorpseBuried(u32, f32, f32),
    GarrisonAlert(f32),
    ScarEarned(String),
    FoliageChopped(u64),
}

#[derive(Clone, Debug, Default)]
pub struct DeltaLedger {
    pub entries: Vec<DeltaEntry>,
}

impl DeltaLedger {
    pub fn push(&mut self, entry: DeltaEntry) {
        if let DeltaEntry::FoliageChopped(id) = &entry {
            if self
                .entries
                .iter()
                .any(|existing| matches!(existing, DeltaEntry::FoliageChopped(existing_id) if existing_id == id))
            {
                return;
            }
        }
        self.entries.push(entry);
    }

    pub fn serialize_text(&self) -> String {
        let mut out = String::from("ANTIR_LEDGER_V1\n");
        for e in &self.entries {
            match e {
                DeltaEntry::RopeSevered(id) => out.push_str(&format!("ROPE_CUT {}\n", id)),
                DeltaEntry::CorpseBuried(id, x, y) => {
                    out.push_str(&format!("BURIED {} {:.2} {:.2}\n", id, x, y))
                }
                DeltaEntry::GarrisonAlert(v) => out.push_str(&format!("ALERT {:.2}\n", v)),
                DeltaEntry::ScarEarned(s) => out.push_str(&format!("SCAR {}\n", s)),
                DeltaEntry::FoliageChopped(id) => {
                    out.push_str(&format!("FOLIAGE_CHOPPED {}\n", id))
                }
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
                "ANTIR_LEDGER_V1" | "CLACHAN_LEDGER_V1" => continue,
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
                "FOLIAGE_CHOPPED" if parts.len() >= 2 => {
                    if let Ok(id) = parts[1].parse::<u64>() {
                        ledger.push(DeltaEntry::FoliageChopped(id));
                    }
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

#[cfg(test)]
mod ledger_tests {
    use super::{DeltaEntry, DeltaLedger};

    #[test]
    fn chopped_foliage_is_sparse_deduplicated_and_persistent() {
        let mut ledger = DeltaLedger::default();
        ledger.push(DeltaEntry::FoliageChopped(1234));
        ledger.push(DeltaEntry::FoliageChopped(1234));
        ledger.push(DeltaEntry::FoliageChopped(5678));

        assert_eq!(ledger.entries.len(), 2);
        let restored = DeltaLedger::deserialize_text(&ledger.serialize_text());
        assert_eq!(restored.entries.len(), 2);
        assert!(restored
            .entries
            .iter()
            .any(|entry| matches!(entry, DeltaEntry::FoliageChopped(1234))));
    }

    #[test]
    fn new_ledger_header_and_legacy_header_both_load() {
        let mut ledger = DeltaLedger::default();
        ledger.push(DeltaEntry::FoliageChopped(81));
        assert!(ledger.serialize_text().starts_with("ANTIR_LEDGER_V1\n"));

        for header in ["ANTIR_LEDGER_V1", "CLACHAN_LEDGER_V1"] {
            let restored =
                DeltaLedger::deserialize_text(&format!("{header}\nFOLIAGE_CHOPPED 81\n"));
            assert!(restored
                .entries
                .iter()
                .any(|entry| matches!(entry, DeltaEntry::FoliageChopped(81))));
        }
    }
}
