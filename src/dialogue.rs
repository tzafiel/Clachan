use crate::config::DialectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryKind {
    SawBurial,
    HeardWhistle,
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
        self.slots
            .iter()
            .map(|m| m.weight)
            .sum::<f32>()
            .clamp(-1.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct UtteranceOutput {
    pub speaker: String,
    pub gaelic: String,
    pub text: String,
    pub volume_db: f32,
    pub formant_f0: f32,
    pub formant_f1: f32,
    pub formant_f2: f32,
    pub ttl: f32,
}

pub fn apply_dialect(base: &str, dialect: DialectId) -> String {
    match dialect {
        DialectId::HighlandGaelic => {
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
    let (gaelic_greet, eng_greet) = if rain > 0.35 {
        (
            "Tha an t-uisge trom a' tighinn far a' bheinn.",
            "Heavy rain is rolling down off the ridge.",
        )
    } else if era_darkness > 0.45 {
        (
            "Tha an ceò a' laighe air a' choille an-diugh.",
            "The mist lies thick across the pinewood today.",
        )
    } else {
        (
            "Fàilte dhan choille, a Fhir-Faire.",
            "Welcome to the wood, Way-Warden.",
        )
    };

    let (gaelic_body, eng_body) = if player_blood > 0.35 {
        (
            "Tha fuil air do bhreacan — thig faisg air an teine.",
            "There is blood on your plaid — come rest beside the hearth.",
        )
    } else if memories.has(MemoryKind::SawBurial) || memories.has(MemoryKind::GossipHonored) {
        (
            "Chuala a' chlachan mun chàrn a thog thu. Tha àite agad ri taobh ar teine.",
            "Word reached the glens of the stone cairn you raised. You always have a place by our fire.",
        )
    } else if memories.has(MemoryKind::HeardWhistle) {
        (
            "Chuala sinn do ghlaodh air Bramble — tha e dlùth dhut.",
            "We heard you call for Bramble — that horse is close to you.",
        )
    } else {
        (
            "Cha do dhìochuimhnich na clachan an t-seann rathad. Gabh do thìde.",
            "The standing stones have not forgotten the old road. Take your time.",
        )
    };

    let gaelic = format!("{} {}", gaelic_greet, gaelic_body);
    let raw_english = format!("{} {}", eng_greet, eng_body);
    let text = apply_dialect(&raw_english, dialect);
    let f0 = if dialect == DialectId::PartyAnnouncer {
        230.0
    } else {
        135.0
    };

    UtteranceOutput {
        speaker: speaker.to_string(),
        gaelic,
        text,
        volume_db: 58.0,
        formant_f0: f0,
        formant_f1: 540.0,
        formant_f2: 1480.0,
        ttl: 8.5,
    }
}
