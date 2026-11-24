use crate::audio::AudioBank;
use crate::config::{spec, DefeatMode, DialectId, MaterialId, UniverseConfig};
use crate::dialogue::{
    compose_redcoat_alert, compose_villager_line, MemoryKind, MemoryRing, UtteranceOutput,
};
use crate::entities::{
    AgentMind, AiGoal, ClanId, ClanRecord, CommandPacket, Constraint, DeltaEntry, DeltaLedger,
    Entity, EntityKind, PlayerId,
};
use crate::world::{
    generate_clachan_buildings, sample_terrain, BuildingRecord, Location, PropKind, Rng,
    SmokePuff, SoundWave, TerrainKind, TraceKind, TraceMark, WeatherState,
};
use macroquad::prelude::{Color, Vec2};

#[derive(Clone, Debug)]
pub enum TransitAction {
    MovePlayer {
        loc: Location,
        pos: Vec2,
        msg: String,
    },
    SleepToDawn,
}

pub struct Universe {
    pub cfg: UniverseConfig,
    pub rng: Rng,
    pub time: f32,
    pub weather: WeatherState,
    pub buildings: Vec<BuildingRecord>,
    pub clans: Vec<ClanRecord>,
    pub entities: Vec<Entity>,
    pub constraints: Vec<Constraint>,
    pub trees: Vec<(Vec2, f32)>,
    pub traces: Vec<TraceMark>,
    pub sound_waves: Vec<SoundWave>,
    pub smoke: Vec<SmokePuff>,
    pub utterances: Vec<UtteranceOutput>,
    pub ledger: DeltaLedger,
    pub status_toast: (String, f32),
    pub scars: Vec<String>,
    pub next_id: u32,
    // Stardew-style interior fade transition state
    pub fade_alpha: f32,
    pub fade_dir: f32,
    pub pending_transit: Option<TransitAction>,
    pub door_cooldown: f32,
}

impl Universe {
    pub fn new(cfg: UniverseConfig) -> Self {
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
                "Welcome to Clachan (1744). Walk into any doorway to enter.".to_string(),
                6.0,
            ),
            scars: Vec::new(),
            next_id: 100,
            fade_alpha: 0.0,
            fade_dir: 0.0,
            pending_transit: None,
            door_cooldown: 0.0,
        };

        // Callum MacRae
        u.entities.push(Entity {
            id: 1,
            owner_player: Some(PlayerId(1)),
            name: "Callum MacRae".to_string(),
            kind: EntityKind::Player,
            clan: ClanId(0),
            location: Location::Overworld,
            pos: Vec2::new(-8.5, 0.5),
            z: 0.0,
            vel: Vec2::ZERO,
            facing: Vec2::new(0.0, -1.0),
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

        // Companion Horse: Bramble
        u.entities.push(Entity {
            id: 2,
            owner_player: Some(PlayerId(1)),
            name: "Bramble".to_string(),
            kind: EntityKind::Mount,
            clan: ClanId(0),
            location: Location::Overworld,
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
                home_building_id: None,
                patrol_a: Vec2::ZERO,
                patrol_b: Vec2::ZERO,
                patrol_to_b: false,
                speak_cooldown: 0.0,
                attack_cooldown: 0.0,
            }),
        });

        // Gibbet on Wade's Road
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
                location: Location::Overworld,
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

        // Clachan Villagers (Alistair tends the Alehouse interior, Duncan works the Smithy, others roam & shelter inside during rain)
        let villagers = [
            ("Mairi the Weaver", Location::Overworld, Vec2::new(-8.0, 16.5), Some(3u32)),
            ("Duncan the Smith", Location::Interior(2), Vec2::new(6.2, 3.6), Some(2u32)),
            ("Alistair of the Alehouse", Location::Interior(3), Vec2::new(9.8, 2.0), Some(3u32)),
            ("Morag the Crofter", Location::Overworld, Vec2::new(-3.0, -5.5), Some(3u32)),
        ];
        for (i, (vname, start_loc, pos, home_bld)) in villagers.iter().enumerate() {
            u.entities.push(Entity {
                id: 20 + i as u32,
                owner_player: None,
                name: vname.to_string(),
                kind: EntityKind::Villager,
                clan: ClanId(0),
                location: *start_loc,
                pos: *pos,
                z: 0.0,
                vel: Vec2::ZERO,
                facing: Vec2::new(0.0, 1.0),
                mass_kg: 72.0,
                material: MaterialId::Wool,
                wetness: 0.05,
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
                    home_pos: *pos,
                    home_building_id: *home_bld,
                    patrol_a: *pos,
                    patrol_b: *pos + Vec2::new(4.0, 2.0),
                    patrol_to_b: true,
                    speak_cooldown: 0.0,
                    attack_cooldown: 0.0,
                }),
            });
        }

        // Fort William Redcoat Patrols
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
                location: Location::Overworld,
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
                    home_pos: Vec2::new(23.0, -6.0),
                    home_building_id: Some(4),
                    patrol_a: *pa,
                    patrol_b: *pb,
                    patrol_to_b: true,
                    speak_cooldown: 0.0,
                    attack_cooldown: 0.0,
                }),
            });
        }

        // Highland Red Deer
        for i in 0..3 {
            let dp = Vec2::new(-18.0 + i as f32 * 4.5, 22.0 + (i as f32 * 2.1).sin() * 3.0);
            u.entities.push(Entity {
                id: 60 + i as u32,
                owner_player: None,
                name: "Highland Red Deer".to_string(),
                kind: EntityKind::Deer,
                clan: ClanId(2),
                location: Location::Overworld,
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
                    home_building_id: None,
                    patrol_a: dp,
                    patrol_b: dp,
                    patrol_to_b: false,
                    speak_cooldown: 0.0,
                    attack_cooldown: 0.0,
                }),
            });
        }

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
                tx > b.x - 2.4 && tx < b.x + b.w + 2.4 && ty > b.y - 2.4 && ty < b.y + b.h + 2.8
            }) {
                continue;
            }
            u.trees.push((p, rng.range_f32(1.4, 2.4)));
        }

        u
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.status_toast = (msg.into(), 4.5);
    }

    pub fn player_index(&self, pid: PlayerId) -> Option<usize> {
        self.entities
            .iter()
            .position(|e| e.kind == EntityKind::Player && e.owner_player == Some(pid))
    }

    pub fn player_location(&self, pid: PlayerId) -> Location {
        self.player_index(pid)
            .map(|i| self.entities[i].location)
            .unwrap_or(Location::Overworld)
    }

    pub fn entity_index(&self, id: u32) -> Option<usize> {
        self.entities.iter().position(|e| e.id == id)
    }

    pub fn building_by_id(&self, id: u32) -> Option<&BuildingRecord> {
        self.buildings.iter().find(|b| b.id == id)
    }

    pub fn start_transit(&mut self, action: TransitAction) {
        if self.fade_dir != 0.0 {
            return;
        }
        self.pending_transit = Some(action);
        self.fade_dir = 1.0;
    }

    pub fn has_line_of_sight(&self, loc: Location, a: Vec2, b: Vec2) -> bool {
        if loc != Location::Overworld {
            return true;
        }
        !self.buildings.iter().any(|bld| bld.blocks_segment(a, b))
    }

    pub fn emit_sound(
        &mut self,
        location: Location,
        pos: Vec2,
        decibels: f32,
        hostiles_care: bool,
        whistle: bool,
    ) {
        let radius = (decibels / 95.0) * self.cfg.hearing_range_m;
        self.sound_waves.push(SoundWave {
            location,
            pos,
            radius: 0.4,
            max_radius: radius.max(4.0),
            decibels,
            age: 0.0,
            hostiles_care,
            whistle,
        });
    }

    pub fn apply_command(&mut self, cmd: CommandPacket, sfx: &AudioBank) {
        if self.fade_dir > 0.0 {
            return;
        }

        match cmd {
            CommandPacket::Move { player, dir, sprint } => {
                if let Some(pi) = self.player_index(player) {
                    let p_loc = self.entities[pi].location;
                    let carrying_mass = if let Some(cid) = self.entities[pi].carrying_id {
                        self.entity_index(cid)
                            .map(|idx| self.entities[idx].mass_kg)
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let mounted = self.entities[pi].mounted_id.is_some();
                    let mud_drag = if p_loc == Location::Overworld {
                        let (_, tk) = sample_terrain(
                            self.cfg.world_seed,
                            self.entities[pi].pos.x,
                            self.entities[pi].pos.y,
                        );
                        match tk {
                            TerrainKind::PeatBog => 0.58,
                            TerrainKind::MudRoad => 1.0 - self.weather.rain_intensity * 0.22,
                            _ => 1.0,
                        }
                    } else {
                        1.0
                    };
                    let burden = 82.0 / (82.0 + carrying_mass * 0.65);
                    let base_speed = if mounted {
                        9.2
                    } else if p_loc != Location::Overworld {
                        4.2
                    } else {
                        4.8
                    };
                    let want_sprint =
                        sprint && self.entities[pi].stamina > 6.0 && carrying_mass < 1.0;
                    let mult = if want_sprint { 1.5 } else { 1.0 };
                    let target_v = dir.normalize_or_zero() * base_speed * mult * burden * mud_drag;
                    let accel = (9.5 * self.cfg.global_friction_mult).clamp(2.5, 14.0);
                    let dt_approx = 1.0 / 60.0;
                    let cur_v = self.entities[pi].vel;
                    self.entities[pi].vel = cur_v.lerp(target_v, (accel * dt_approx).min(1.0));
                    if dir.length_squared() > 0.01 {
                        self.entities[pi].facing = dir.normalize();
                    }
                    if want_sprint && dir.length_squared() > 0.01 {
                        self.entities[pi].stamina = (self.entities[pi].stamina
                            - 16.0 * dt_approx * self.cfg.stamina_drain_mult)
                            .max(0.0);
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
                self.entities[pi].stamina =
                    (self.entities[pi].stamina - 10.0 * self.cfg.stamina_drain_mult).max(0.0);
                let p_loc = self.entities[pi].location;
                let p_pos = self.entities[pi].pos;
                let p_face = self.entities[pi].facing;
                let sharp = self.entities[pi].blade_sharpness;
                AudioBank::play(&sfx.steel, 0.35);
                self.emit_sound(p_loc, p_pos, 52.0, false, false);

                if p_loc == Location::Overworld {
                    let mut snapped_indices = Vec::new();
                    for (ci, c) in self.constraints.iter().enumerate() {
                        if !c.intact {
                            continue;
                        }
                        if p_pos.distance(c.anchor) < 2.6 {
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
                            let cname = self.entities[ei].name.clone();
                            self.toast(format!(
                                "Severed the hemp rope — {} dropped into the wet grass. [E] Carry, [B] Bury.",
                                cname
                            ));
                        }
                        self.entities[pi].blade_sharpness =
                            (self.entities[pi].blade_sharpness - 0.02).max(0.1);
                    }
                }

                let mut hits = Vec::new();
                for i in 0..self.entities.len() {
                    if i == pi
                        || !self.entities[i].alive
                        || self.entities[i].location != p_loc
                        || self.entities[i].kind == EntityKind::Mount
                    {
                        continue;
                    }
                    let to_e = self.entities[i].pos - p_pos;
                    if to_e.length() < 2.35 && to_e.normalize_or_zero().dot(p_face) > 0.2 {
                        hits.push((i, to_e.normalize_or_zero()));
                    }
                }
                for (ti, dir) in hits {
                    let dmg = (34.0 * (0.45 + sharp * 0.65)) * self.cfg.lethality_mult;
                    self.entities[ti].health -= dmg;
                    self.entities[ti].vel += dir * (6.5 * self.cfg.knockback_mult);
                    if self.cfg.lethality_mult > 0.05 {
                        self.entities[ti].bleed_rate += 1.6 * self.cfg.bleed_rate_mult;
                        self.entities[ti].blood_stain =
                            (self.entities[ti].blood_stain + 0.35).min(1.0);
                        self.entities[pi].blood_stain =
                            (self.entities[pi].blood_stain + 0.22).min(1.0);
                        self.entities[pi].blade_sharpness =
                            (self.entities[pi].blade_sharpness - 0.05).max(0.15);
                    }
                    if self.entities[ti].health <= 0.0 && self.cfg.lethality_mult > 0.0 {
                        self.entities[ti].alive = false;
                        self.entities[ti].kind = EntityKind::Corpse;
                        let ename = self.entities[ti].name.clone();
                        self.toast(format!("{} fell.", ename));
                    } else {
                        let is_redcoat = self.entities[ti].kind == EntityKind::Redcoat;
                        if let Some(mind) = &mut self.entities[ti].mind {
                            if is_redcoat {
                                mind.goal = AiGoal::Engage;
                                mind.awareness = 1.0;
                            } else {
                                mind.goal = AiGoal::Flee;
                            }
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
                let p_loc = self.entities[pi].location;
                let p_pos = self.entities[pi].pos;

                let misfire_chance = (self.entities[pi].powder_wetness
                    * self.cfg.ignition_humidity_penalty)
                    .clamp(0.0, 0.92);
                if self.rng.next_f32() < misfire_chance {
                    AudioBank::play(&sfx.misfire, 0.55);
                    self.toast("CLICK-FIZZLE! Rain fouled your black powder — dry off by a hearth!");
                    return;
                }

                AudioBank::play(&sfx.gunshot, 0.65);
                self.emit_sound(p_loc, p_pos, 98.0, true, false);
                for _ in 0..8 {
                    self.smoke.push(SmokePuff {
                        location: p_loc,
                        pos: p_pos + dir * 0.9,
                        vel: dir * self.rng.range_f32(1.2, 2.8)
                            + if p_loc == Location::Overworld {
                                self.weather.wind_vec * 0.35
                            } else {
                                Vec2::ZERO
                            },
                        radius: self.rng.range_f32(0.5, 1.1),
                        age: 0.0,
                        max_age: 4.5,
                        is_ember: false,
                    });
                }

                let wind_deflect = if p_loc == Location::Overworld {
                    Vec2::new(-dir.y, dir.x) * (self.weather.wind_vec.length() * 0.008)
                } else {
                    Vec2::ZERO
                };
                let shot_dir = (dir + wind_deflect).normalize_or_zero();
                let mut hit_target = None;
                for i in 0..self.entities.len() {
                    if i == pi
                        || !self.entities[i].alive
                        || self.entities[i].location != p_loc
                        || self.entities[i].kind == EntityKind::Mount
                    {
                        continue;
                    }
                    let rel = self.entities[i].pos - p_pos;
                    let proj = rel.dot(shot_dir);
                    if proj > 0.5 && proj < 26.0 {
                        let perp = (rel - shot_dir * proj).length();
                        if perp < 0.95 && self.has_line_of_sight(p_loc, p_pos, self.entities[i].pos)
                        {
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
                            let ename = self.entities[ti].name.clone();
                            self.toast(format!("Musket ball dropped {}.", ename));
                        }
                    }
                }
            }

            CommandPacket::Interact { player } => {
                let Some(pi) = self.player_index(player) else { return; };
                let p_loc = self.entities[pi].location;
                let p_pos = self.entities[pi].pos;

                if self.entities[pi].mounted_id.is_some() {
                    self.entities[pi].mounted_id = None;
                    self.toast("Dismounted Bramble.");
                    return;
                }

                if let Some(cid) = self.entities[pi].carrying_id.take() {
                    let p_face = self.entities[pi].facing;
                    if let Some(ci) = self.entity_index(cid) {
                        self.entities[ci].location = p_loc;
                        self.entities[ci].pos = p_pos + p_face * 1.1;
                        let cname = self.entities[ci].name.clone();
                        self.toast(format!("Laid {} gently on the ground.", cname));
                    }
                    return;
                }

                // 1. Check nearby entities in same location
                let mut best: Option<(usize, f32)> = None;
                for i in 0..self.entities.len() {
                    if i == pi || self.entities[i].buried || self.entities[i].location != p_loc {
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
                                m.bond_to_player =
                                    (m.bond_to_player + self.cfg.mount_bond_rate).min(1.0);
                            }
                            self.toast("Mounted Bramble. Ride across the glen!");
                        }
                        EntityKind::Corpse => {
                            let cid = self.entities[ti].id;
                            let cname = self.entities[ti].name.clone();
                            let still_hung =
                                self.constraints.iter().any(|c| c.entity_id == cid && c.intact);
                            if still_hung {
                                self.toast("Hemp rope holds them aloft — slash it with steel [Click/Space] first.");
                            } else {
                                self.entities[pi].carrying_id = Some(cid);
                                self.toast(format!(
                                    "Lifted {} onto your shoulder. Press [B] outdoors to build a stone cairn.",
                                    cname
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

                // 2. Overworld doors OR Interior props/hearth/exit
                match p_loc {
                    Location::Overworld => {
                        let enter_target = self
                            .buildings
                            .iter()
                            .find(|b| b.door_pos.distance(p_pos) < 2.1)
                            .map(|b| (b.id, b.int_exit + Vec2::new(0.0, -1.35), b.name.clone()));
                        if let Some((bid, spawn, bname)) = enter_target {
                            self.start_transit(TransitAction::MovePlayer {
                                loc: Location::Interior(bid),
                                pos: spawn,
                                msg: format!("Entered {}.", bname),
                            });
                        }
                    }
                    Location::Interior(bid) => {
                        let mut exit_info = None;
                        let mut prop_hit = None;
                        let mut toggled_hearth = None;

                        if let Some(b) = self.buildings.iter_mut().find(|b| b.id == bid) {
                            if b.int_exit.distance(p_pos) < 1.65 {
                                exit_info = Some((b.door_pos + Vec2::new(0.0, 1.45), b.name.clone()));
                            } else if b.int_hearth.distance(p_pos) < 2.2 {
                                b.hearth_lit = !b.hearth_lit;
                                toggled_hearth = Some(b.hearth_lit);
                            } else {
                                for prop in &b.props {
                                    if !prop.prompt.is_empty()
                                        && prop.center().distance(p_pos) < 2.25
                                    {
                                        prop_hit = Some(prop.kind);
                                        break;
                                    }
                                }
                            }
                        }

                        if let Some((out_pos, bname)) = exit_info {
                            self.start_transit(TransitAction::MovePlayer {
                                loc: Location::Overworld,
                                pos: out_pos,
                                msg: format!("Stepped outside {}.", bname),
                            });
                        } else if let Some(lit) = toggled_hearth {
                            self.toast(if lit {
                                "Stoked the warm peat hearth."
                            } else {
                                "Banked the hearth embers."
                            });
                        } else if let Some(pk) = prop_hit {
                            match pk {
                                PropKind::Bed => {
                                    self.start_transit(TransitAction::SleepToDawn);
                                }
                                PropKind::Anvil => {
                                    self.entities[pi].blade_sharpness = 1.0;
                                    AudioBank::play(&sfx.steel, 0.4);
                                    self.toast("Honed your steel blade on Duncan's iron anvil.");
                                }
                                PropKind::Forge => {
                                    if let Some(b) = self.buildings.iter_mut().find(|b| b.id == bid)
                                    {
                                        b.hearth_lit = true;
                                    }
                                    self.toast("Pumped the bellows — the smithy forge roars bright.");
                                }
                                PropKind::BarCounter => {
                                    self.entities[pi].stamina = 100.0;
                                    self.entities[pi].temp_k = 310.4;
                                    self.entities[pi].health =
                                        (self.entities[pi].health + 12.0).min(100.0);
                                    self.toast("Shared a warm mug of heather ale. Stamina & warmth restored.");
                                }
                                PropKind::WeaponRack => {
                                    self.entities[pi].powder_wetness = 0.0;
                                    self.toast("Repacked fresh dry powder from the horn on the rack.");
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            CommandPacket::BuryCarried { player } => {
                let Some(pi) = self.player_index(player) else { return; };
                if self.entities[pi].location != Location::Overworld {
                    self.toast("Carry them outdoors to lay a fieldstone cairn on Highland soil.");
                    return;
                }
                let Some(cid) = self.entities[pi].carrying_id.take() else {
                    self.toast("Carry a fallen body first [E] to lay them beneath a stone cairn.");
                    return;
                };
                let bury_pos = self.entities[pi].pos + self.entities[pi].facing * 1.2;
                let mut buried_name = String::new();
                if let Some(ci) = self.entity_index(cid) {
                    self.entities[ci].buried = true;
                    self.entities[ci].location = Location::Overworld;
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
                    location: Location::Overworld,
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
                for i in 0..self.entities.len() {
                    if self.entities[i].kind == EntityKind::Villager {
                        let d = self.entities[i].pos.distance(bury_pos);
                        if d < self.cfg.gossip_spread_radius_m
                            || self.entities[i].location != Location::Overworld
                        {
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
                if self.entities[pi].location != Location::Overworld {
                    self.toast("Step outdoors to whistle for Bramble.");
                    return;
                }
                let p_pos = self.entities[pi].pos;
                AudioBank::play(&sfx.whistle, 0.42);
                self.emit_sound(Location::Overworld, p_pos, 72.0, false, true);
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
                        self.entities[pi].blood_stain =
                            (self.entities[pi].blood_stain - 0.35).max(0.0);
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
                if self.entities[pi].location != Location::Overworld {
                    self.toast("Use the stone hearth indoors rather than lighting peat on the floor.");
                    return;
                }
                let fpos = self.entities[pi].pos + self.entities[pi].facing * 1.6;
                if self.weather.rain_intensity > 0.65 {
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
                    location: Location::Overworld,
                    pos: fpos,
                    z: 0.0,
                    vel: Vec2::ZERO,
                    facing: Vec2::Y,
                    mass_kg: 15.0,
                    material: MaterialId::Peat,
                    wetness: 0.0,
                    temp_k: 720.0,
                    health: 60.0,
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

    fn execute_transit(&mut self, action: TransitAction) {
        let Some(pi) = self.player_index(PlayerId(1)) else { return; };
        match action {
            TransitAction::MovePlayer { loc, pos, msg } => {
                // Dismount Bramble safely outside when entering a building
                if loc != Location::Overworld {
                    if let Some(mid) = self.entities[pi].mounted_id.take() {
                        if let Some(mi) = self.entity_index(mid) {
                            self.entities[mi].location = Location::Overworld;
                        }
                    }
                }
                self.entities[pi].location = loc;
                self.entities[pi].pos = pos;
                self.entities[pi].vel = Vec2::ZERO;
                if let Some(cid) = self.entities[pi].carrying_id {
                    if let Some(ci) = self.entity_index(cid) {
                        self.entities[ci].location = loc;
                        self.entities[ci].pos = pos + Vec2::new(0.0, -0.35);
                    }
                }
                self.door_cooldown = 0.55;
                self.toast(msg);
            }
            TransitAction::SleepToDawn => {
                self.weather.day_count += 1;
                self.weather.time_of_day = 0.29; // Crisp 07:00 Highland morning
                self.entities[pi].health = self.entities[pi].max_health;
                self.entities[pi].stamina = 100.0;
                self.entities[pi].bleed_rate = 0.0;
                self.entities[pi].wetness = 0.0;
                self.entities[pi].powder_wetness = 0.0;
                self.entities[pi].temp_k = 310.2;
                self.toast(format!(
                    "Day {} dawns over Lochaber. Rested, warm, and dry.",
                    self.weather.day_count
                ));
            }
        }
    }

    pub fn step(&mut self, dt: f32, sfx: &AudioBank) {
        self.time += dt;
        self.weather.step(dt, &self.cfg);
        if self.door_cooldown > 0.0 {
            self.door_cooldown = (self.door_cooldown - dt).max(0.0);
        }
        if self.status_toast.1 > 0.0 {
            self.status_toast.1 -= dt;
        }
        self.utterances.retain_mut(|u| {
            u.ttl -= dt;
            u.ttl > 0.0
        });

        // Smooth Stardew-style black fade transition between Outside & Inside
        if self.fade_dir > 0.0 {
            self.fade_alpha = (self.fade_alpha + dt * 4.8).min(1.0);
            if self.fade_alpha >= 1.0 {
                if let Some(act) = self.pending_transit.take() {
                    self.execute_transit(act);
                }
                self.fade_dir = -1.0;
            }
        } else if self.fade_dir < 0.0 {
            self.fade_alpha = (self.fade_alpha - dt * 4.8).max(0.0);
            if self.fade_alpha <= 0.0 {
                self.fade_dir = 0.0;
            }
        }

        // Outdoor chimney smoke + indoor hearth embers
        for b in &self.buildings {
            if b.hearth_lit && self.rng.next_f32() < dt * 2.5 {
                self.smoke.push(SmokePuff {
                    location: Location::Overworld,
                    pos: b.chimney_pos,
                    vel: self.weather.wind_vec * 0.28 + Vec2::new(0.0, -0.75),
                    radius: 0.45,
                    age: 0.0,
                    max_age: 4.0,
                    is_ember: self.rng.next_f32() < 0.18,
                });
                self.smoke.push(SmokePuff {
                    location: Location::Interior(b.id),
                    pos: b.int_hearth + Vec2::new(self.rng.range_f32(-0.25, 0.25), 0.1),
                    vel: Vec2::new(self.rng.range_f32(-0.15, 0.15), -0.7),
                    radius: 0.16,
                    age: 0.0,
                    max_age: 1.4,
                    is_ember: true,
                });
            }
        }

        // Active heat sources keyed by Location
        let mut heat_sources: Vec<(Location, Vec2)> = self
            .buildings
            .iter()
            .filter(|b| b.hearth_lit)
            .map(|b| (Location::Interior(b.id), b.int_hearth))
            .collect();
        for e in &self.entities {
            if e.kind == EntityKind::Campfire && e.alive {
                heat_sources.push((e.location, e.pos));
            }
        }

        for sw in &mut self.sound_waves {
            sw.age += dt;
            sw.radius = (sw.radius + dt * 28.0).min(sw.max_radius);
        }
        let active_waves: Vec<(Location, Vec2, f32, bool)> = self
            .sound_waves
            .iter()
            .filter(|w| w.age < 0.25)
            .map(|w| (w.location, w.pos, w.max_radius, w.hostiles_care))
            .collect();
        self.sound_waves.retain(|w| w.age < 1.1);

        for sp in &mut self.smoke {
            sp.age += dt;
            let wind = if sp.location == Location::Overworld {
                self.weather.wind_vec * 0.15
            } else {
                Vec2::ZERO
            };
            sp.pos += (sp.vel + wind) * dt;
            sp.radius += dt * 0.22;
        }
        self.smoke.retain(|s| s.age < s.max_age);
        for tr in &mut self.traces {
            tr.age += dt;
        }
        self.traces.retain(|t| t.age < t.max_age);

        let (player_loc, player_pos) = self
            .player_index(PlayerId(1))
            .map(|i| (self.entities[i].location, self.entities[i].pos))
            .unwrap_or((Location::Overworld, Vec2::ZERO));

        // Automatic door transit when walking into a doorway (Stardew feel)
        if self.fade_dir == 0.0 && self.door_cooldown <= 0.0 {
            if let Some(pi) = self.player_index(PlayerId(1)) {
                let pvel = self.entities[pi].vel;
                match player_loc {
                    Location::Overworld => {
                        let hit = self
                            .buildings
                            .iter()
                            .find(|b| {
                                let d = b.door_pos.distance(player_pos);
                                d < 1.05 && pvel.y < -0.15
                            })
                            .map(|b| (b.id, b.int_exit + Vec2::new(0.0, -1.35), b.name.clone()));
                        if let Some((bid, spawn, bname)) = hit {
                            self.start_transit(TransitAction::MovePlayer {
                                loc: Location::Interior(bid),
                                pos: spawn,
                                msg: format!("Entered {}.", bname),
                            });
                        }
                    }
                    Location::Interior(bid) => {
                        let hit = self.building_by_id(bid).and_then(|b| {
                            if b.int_exit.distance(player_pos) < 0.95 && pvel.y > 0.15 {
                                Some((b.door_pos + Vec2::new(0.0, 1.45), b.name.clone()))
                            } else {
                                None
                            }
                        });
                        if let Some((out_pos, bname)) = hit {
                            self.start_transit(TransitAction::MovePlayer {
                                loc: Location::Overworld,
                                pos: out_pos,
                                msg: format!("Stepped outside {}.", bname),
                            });
                        }
                    }
                }
            }
        }

        let mut alert_events: Vec<(String, DialectId, bool)> = Vec::new();
        let mut player_hit_dmg = 0.0f32;
        let mut player_knock = Vec2::ZERO;

        for i in 0..self.entities.len() {
            let kind = self.entities[i].kind;
            let e_loc = self.entities[i].location;

            if kind == EntityKind::Campfire {
                let cpos = self.entities[i].pos;
                if self.weather.rain_intensity > 0.7 && e_loc == Location::Overworld {
                    self.entities[i].health -= dt * 8.0;
                } else {
                    self.entities[i].health -= dt * 0.4;
                }
                if self.entities[i].health <= 0.0 {
                    self.entities[i].alive = false;
                } else if self.rng.next_f32() < dt * 4.0 {
                    self.smoke.push(SmokePuff {
                        location: e_loc,
                        pos: cpos,
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

            let pos = self.entities[i].pos;
            let sheltered = e_loc != Location::Overworld;
            let near_fire = heat_sources
                .iter()
                .any(|(hl, hp)| *hl == e_loc && hp.distance(pos) < 4.5);

            if self.weather.rain_intensity > 0.05 && !sheltered {
                let soak = self.weather.rain_intensity * self.cfg.wetness_soak_rate * dt;
                self.entities[i].wetness = (self.entities[i].wetness + soak).min(1.0);
                self.entities[i].powder_wetness =
                    (self.entities[i].powder_wetness + soak * 0.85).min(1.0);
                self.entities[i].blood_stain = (self.entities[i].blood_stain - soak * 0.3).max(0.0);
            } else {
                let dry_mult = if near_fire {
                    3.6
                } else if sheltered {
                    1.6
                } else {
                    0.45
                };
                let dry = self.cfg.wetness_dry_rate * dry_mult * dt;
                self.entities[i].wetness = (self.entities[i].wetness - dry).max(0.0);
                self.entities[i].powder_wetness =
                    (self.entities[i].powder_wetness - dry * 1.1).max(0.0);
            }

            if self.entities[i].alive {
                let heat_gain = if near_fire {
                    1.4
                } else if sheltered {
                    0.45
                } else {
                    0.0
                };
                let chill = if !sheltered {
                    (290.0 - self.weather.ambient_temp_k).max(0.0)
                        * (0.015 + self.entities[i].wetness * 0.055)
                        * self.cfg.hypothermia_rate
                } else {
                    0.0
                };
                self.entities[i].temp_k =
                    (self.entities[i].temp_k + (heat_gain - chill) * dt).clamp(305.0, 310.5);
                if self.entities[i].temp_k < 307.2 && self.cfg.lethality_mult > 0.0 {
                    self.entities[i].stamina = (self.entities[i].stamina - 4.0 * dt).max(0.0);
                }

                if self.entities[i].bleed_rate > 0.0 {
                    self.entities[i].health -= self.entities[i].bleed_rate * dt;
                    if self.rng.next_f32() < dt * 2.2 {
                        self.traces.push(TraceMark {
                            location: e_loc,
                            pos,
                            kind: TraceKind::BloodDrop,
                            age: 0.0,
                            max_age: 45.0,
                        });
                    }
                }
                self.entities[i].stamina = (self.entities[i].stamina + 9.5 * dt).min(100.0);
            }

            // NPC AI (works seamlessly across Overworld and Interiors!)
            if self.entities[i].alive && kind != EntityKind::Player {
                let mut desired_v = Vec2::ZERO;
                let mut new_goal = None;
                let mut speak_alert = None;
                let mut did_swing = false;
                let mut npc_warp: Option<(Location, Vec2)> = None;
                let entity_name = self.entities[i].name.clone();
                let entity_id = self.entities[i].id;
                let same_loc_as_player = e_loc == player_loc;
                let los_to_player = same_loc_as_player
                    && self.has_line_of_sight(e_loc, pos, player_pos);

                let home_door_info = self.entities[i]
                    .mind
                    .as_ref()
                    .and_then(|m| m.home_building_id)
                    .and_then(|bid| {
                        self.building_by_id(bid)
                            .map(|b| (bid, b.door_pos, b.int_hearth, b.int_exit))
                    });

                if let Some(mind) = &mut self.entities[i].mind {
                    if mind.speak_cooldown > 0.0 {
                        mind.speak_cooldown -= dt;
                    }
                    if mind.attack_cooldown > 0.0 {
                        mind.attack_cooldown -= dt;
                    }

                    for (wloc, wpos, wrad, hostiles_care) in &active_waves {
                        if *wloc == e_loc && pos.distance(*wpos) < *wrad {
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
                            if mind.goal == AiGoal::FollowOwner && same_loc_as_player {
                                let d = pos.distance(player_pos);
                                if d > 2.4 {
                                    desired_v = (player_pos - pos).normalize_or_zero() * 6.8;
                                } else {
                                    mind.goal = AiGoal::Graze;
                                }
                            }
                        }
                        EntityKind::Villager => {
                            // Villagers duck indoors into the Alehouse/Smithy when it pours rain
                            if let Some((bid, door_p, int_h, int_ex)) = home_door_info {
                                if self.weather.rain_intensity > 0.42
                                    && e_loc == Location::Overworld
                                {
                                    let to_d = door_p + Vec2::new(0.0, 0.9) - pos;
                                    if to_d.length() < 1.1 {
                                        let slot_x = ((entity_id % 3) as f32 - 1.0) * 1.1;
                                        npc_warp = Some((
                                            Location::Interior(bid),
                                            int_h + Vec2::new(1.2 + slot_x, 1.5),
                                        ));
                                    } else {
                                        desired_v = to_d.normalize_or_zero() * 2.6;
                                    }
                                } else if self.weather.rain_intensity < 0.12
                                    && e_loc == Location::Interior(bid)
                                    && entity_id != 21
                                    && entity_id != 22
                                {
                                    // Outdoor rovers step back out when skies clear
                                    let to_ex = int_ex - pos;
                                    if to_ex.length() < 1.0 {
                                        npc_warp = Some((
                                            Location::Overworld,
                                            door_p + Vec2::new(0.0, 1.5),
                                        ));
                                    } else {
                                        desired_v = to_ex.normalize_or_zero() * 2.2;
                                    }
                                }
                            }
                            if mind.goal == AiGoal::Flee && same_loc_as_player {
                                desired_v = (pos - player_pos).normalize_or_zero() * 4.2;
                                if pos.distance(player_pos) > 16.0 {
                                    mind.goal = AiGoal::Idle;
                                }
                            }
                        }
                        EntityKind::Deer => {
                            if same_loc_as_player {
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
                        }
                        EntityKind::Redcoat => {
                            let vis_range = self.cfg.vision_range_m
                                * (1.0 - self.weather.mist_intensity * 0.55);
                            if e_loc == Location::Overworld {
                                for c in &mut self.constraints {
                                    if !c.intact
                                        && !c.discovered_by_patrol
                                        && pos.distance(c.anchor) < vis_range * 0.65
                                    {
                                        c.discovered_by_patrol = true;
                                        mind.awareness = 1.0;
                                        self.clans[1].alert_level =
                                            (self.clans[1].alert_level + 0.5).min(1.0);
                                        self.ledger.push(DeltaEntry::GarrisonAlert(
                                            self.clans[1].alert_level,
                                        ));
                                        speak_alert =
                                            Some((entity_name.clone(), mind.dialect, true));
                                        new_goal = Some(AiGoal::Investigate(c.anchor));
                                    }
                                }
                            }

                            let dist_p = pos.distance(player_pos);
                            if same_loc_as_player && dist_p < vis_range && los_to_player {
                                let alert_push = if self.clans[1].alert_level > 0.3 {
                                    0.45
                                } else {
                                    0.0
                                };
                                if alert_push > 0.0 || mind.goal == AiGoal::Engage {
                                    mind.awareness = (mind.awareness + dt * 0.65).min(1.0);
                                    if mind.awareness > 0.85 {
                                        mind.goal = AiGoal::Engage;
                                        if mind.speak_cooldown <= 0.0 {
                                            mind.speak_cooldown = 7.0;
                                            speak_alert =
                                                Some((entity_name.clone(), mind.dialect, false));
                                        }
                                    }
                                }
                            }

                            match mind.goal {
                                AiGoal::Patrol => {
                                    let tgt = if mind.patrol_to_b {
                                        mind.patrol_b
                                    } else {
                                        mind.patrol_a
                                    };
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
                                    if !same_loc_as_player || dist_p > 26.0 {
                                        mind.goal = AiGoal::Patrol;
                                    } else if dist_p > 1.8 {
                                        desired_v = (player_pos - pos).normalize_or_zero() * 4.1;
                                    } else if mind.attack_cooldown <= 0.0 {
                                        mind.attack_cooldown = 1.15;
                                        did_swing = true;
                                        player_hit_dmg += 16.0 * self.cfg.lethality_mult;
                                        player_knock += (player_pos - pos).normalize_or_zero()
                                            * (6.5 * self.cfg.knockback_mult);
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
                if let Some((wloc, wpos)) = npc_warp {
                    self.entities[i].location = wloc;
                    self.entities[i].pos = wpos;
                    self.entities[i].vel = Vec2::ZERO;
                    continue;
                }
                if did_swing {
                    self.entities[i].swing_timer = 0.25;
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

            // Motion + Location-Aware Collision
            let prev = self.entities[i].pos;
            let mut next = prev + self.entities[i].vel * dt;

            match e_loc {
                Location::Overworld => {
                    let (_, tk) = sample_terrain(self.cfg.world_seed, next.x, next.y);
                    if tk == TerrainKind::LochWater {
                        next = prev;
                        self.entities[i].vel.x =
                            -self.entities[i].vel.x * self.cfg.global_restitution_mult;
                    }
                    next.x = next.x.clamp(-38.0, 44.0);
                    next.y = next.y.clamp(-38.0, 38.0);
                    for b in &self.buildings {
                        next = b.resolve_exterior_collision(prev, next, 0.45);
                    }
                    let step_dist = next.distance(prev);
                    self.entities[i].trace_accum += step_dist;
                    if self.entities[i].trace_accum > 1.35
                        && (tk == TerrainKind::MudRoad
                            || tk == TerrainKind::PeatBog
                            || self.weather.rain_intensity > 0.2)
                    {
                        self.entities[i].trace_accum = 0.0;
                        let tkind = if kind == EntityKind::Mount {
                            TraceKind::HoofPrint
                        } else {
                            TraceKind::BootPrint
                        };
                        self.traces.push(TraceMark {
                            location: Location::Overworld,
                            pos: next,
                            kind: tkind,
                            age: 0.0,
                            max_age: 35.0,
                        });
                    }
                }
                Location::Interior(bid) => {
                    if let Some(b) = self.building_by_id(bid) {
                        next = b.resolve_interior_collision(prev, next, 0.42);
                    }
                }
            }

            self.entities[i].pos = next;
            if !self.entities[i].alive {
                self.entities[i].vel *= (1.0 - 6.0 * self.cfg.global_friction_mult * dt).max(0.0);
            }
        }

        // Sync mounted horse & carried corpse
        if let Some(pi) = self.player_index(PlayerId(1)) {
            let p_loc = self.entities[pi].location;
            let ppos = self.entities[pi].pos;
            let pface = self.entities[pi].facing;
            if let Some(mid) = self.entities[pi].mounted_id {
                if let Some(mi) = self.entity_index(mid) {
                    self.entities[mi].location = p_loc;
                    self.entities[mi].pos = ppos;
                    self.entities[mi].facing = pface;
                }
            }
            if let Some(cid) = self.entities[pi].carrying_id {
                if let Some(ci) = self.entity_index(cid) {
                    self.entities[ci].location = p_loc;
                    self.entities[ci].pos = ppos + Vec2::new(0.0, -0.35);
                }
            }
            if player_hit_dmg > 0.0 || player_knock.length_squared() > 0.01 {
                self.entities[pi].health -= player_hit_dmg;
                self.entities[pi].vel += player_knock;
                if player_hit_dmg > 0.0 {
                    AudioBank::play(&sfx.steel, 0.4);
                    self.entities[pi].bleed_rate += 0.9 * self.cfg.bleed_rate_mult;
                    self.entities[pi].blood_stain =
                        (self.entities[pi].blood_stain + 0.25).min(1.0);
                }
            }
            if self.entities[pi].health <= 0.0 {
                match self.cfg.defeat_mode {
                    DefeatMode::CaptivityTimeSkip => {
                        self.weather.day_count += self.cfg.defeat_time_skip_days;
                        self.entities[pi].health = 65.0;
                        self.entities[pi].bleed_rate = 0.0;
                        self.entities[pi].location = Location::Interior(1);
                        self.entities[pi].pos = Vec2::new(3.4, 3.6);
                        self.entities[pi].mounted_id = None;
                        self.entities[pi].carrying_id = None;
                        let scar = format!("Day {} Garrison Clash Scar", self.weather.day_count);
                        self.scars.push(scar.clone());
                        self.ledger.push(DeltaEntry::ScarEarned(scar));
                        self.toast("Left for dead by the Garrison. 3 days passed before clan kin carried you home to your bothy.");
                    }
                    DefeatMode::KnockoutBounce => {
                        self.entities[pi].health = 100.0;
                        self.entities[pi].vel = Vec2::new(-14.0, -8.0);
                        self.toast("BOING! Knocked out of bounds — right back in!");
                    }
                }
            }
        }

        for (name, dialect, found_rope) in alert_events {
            self.utterances
                .push(compose_redcoat_alert(&name, found_rope, dialect));
            AudioBank::play(&sfx.voice_bark, 0.5);
        }

        self.entities
            .retain(|e| e.kind != EntityKind::Campfire || e.alive);
    }

    pub fn apply_ledger(&mut self, ledger: DeltaLedger) {
        for e in &ledger.entries {
            match e {
                DeltaEntry::RopeSevered(cid) => {
                    let target_eid = self.constraints.iter_mut().find(|c| c.id == *cid).map(|c| {
                        c.intact = false;
                        c.entity_id
                    });
                    if let Some(eid) = target_eid {
                        if let Some(ei) = self.entity_index(eid) {
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
                        location: Location::Overworld,
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
