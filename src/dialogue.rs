use crate::config::DialectId;

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
    pub weight: f32,
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
