use super::*;
use crate::config::{Cvar, CvarFlags, Cvars};
use slotmap::{SlotMap, new_key_type};
use std::sync::Arc;

/// Default gravity (`sv_gravity`).
pub const GRAV: f32 = 0.06;

new_key_type! {
    pub struct SoldierId;
}

/// Registers the cvars the simulation reads.
pub fn register_cvars(cvars: &mut Cvars) {
    let rules = CvarFlags::SERVER | CvarFlags::SYNC;

    cvars.register(Cvar::float("sv_gravity", GRAV, "Gravity").flags(rules));
    cvars.register(
        Cvar::bool("sv_realisticmode", false, "Enables realistic mode")
            .flags(rules | CvarFlags::INIT_ONLY),
    );
    cvars.register(Cvar::bool("sv_friendlyfire", false, "Enables friendly fire").flags(rules));
    cvars.register(
        Cvar::int(
            "sv_respawntime",
            180,
            "Respawn time in ticks (60 ticks = 1 second)",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 9999.0),
    );
    cvars.register(
        Cvar::int(
            "sv_maxgrenades",
            2,
            "Sets the max number of grenades a player can carry",
        )
        .flags(rules)
        .range(0.0, 5.0),
    );
}

/// Spawn protection ticks (`DEFAULT_CEASEFIRE_TIME`).
pub const CEASEFIRE_TIME: i32 = 90;

/// Game rules derived from cvars, read by the simulation every tick.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    pub gravity: f32,
    pub realistic_mode: bool,
    pub friendly_fire: bool,
    pub respawn_time: i32,
    pub ceasefire_time: i32,
    pub max_grenades: i32,
    /// Weapon stats for this mode (`Guns`).
    pub weapons: Arc<WeaponTable>,
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            gravity: GRAV,
            realistic_mode: false,
            friendly_fire: false,
            respawn_time: 180,
            ceasefire_time: CEASEFIRE_TIME,
            max_grenades: 2,
            weapons: Arc::new(WeaponTable::default()),
        }
    }
}

impl WorldConfig {
    /// Defaults with the data's (weapons mod) weapon table.
    pub fn with_data(data: &GameData) -> WorldConfig {
        WorldConfig {
            weapons: data.weapons(false).clone(),
            ..WorldConfig::default()
        }
    }

    pub fn from_cvars(cvars: &Cvars, data: &GameData) -> WorldConfig {
        let realistic_mode = cvars.bool("sv_realisticmode");
        WorldConfig {
            gravity: cvars.float("sv_gravity"),
            realistic_mode,
            friendly_fire: cvars.bool("sv_friendlyfire"),
            respawn_time: cvars.int("sv_respawntime") as i32,
            ceasefire_time: CEASEFIRE_TIME,
            max_grenades: cvars.int("sv_maxgrenades") as i32,
            weapons: data.weapons(realistic_mode).clone(),
        }
    }

    pub fn start_health(&self) -> f32 {
        if self.realistic_mode {
            REALISTIC_START_HEALTH
        } else {
            START_HEALTH
        }
    }
}

/// Something that happened during a tick, for audio, effects, HUD, scripting and networking.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    /// A bullet was created (fired, or spawned by another bullet like cluster fragments).
    BulletFired {
        owner: SoldierId,
        weapon: WeaponKind,
        pos: Vec2,
    },
    BulletHit {
        owner: SoldierId,
        kind: HitKind,
        pos: Vec2,
    },
    Killed {
        victim: SoldierId,
        killer: SoldierId,
        how: DeathKind,
    },
}

pub struct World {
    pub data: Arc<GameData>,
    pub map: MapFile,
    pub config: WorldConfig,
    pub soldiers: SlotMap<SoldierId, Soldier>,
    pub bullets: Vec<Bullet>,
    /// Thing slots (`Thing[1..MAX_THINGS]`).
    pub things: Vec<Thing>,
    pub tick: u64,
    /// Gameplay randomness, bit-compatible with Soldat's.
    pub rng: PascalRandom,
    emitter: Vec<EmitterItem>,
}

impl World {
    pub fn new(data: Arc<GameData>, map: MapFile, config: WorldConfig) -> World {
        World {
            data,
            map,
            config,
            soldiers: SlotMap::with_key(),
            bullets: vec![Bullet::default(); MAX_BULLETS],
            things: vec![Thing::default(); MAX_THINGS],
            tick: 0,
            rng: PascalRandom::new(1),
            emitter: Vec::new(),
        }
    }

    /// Adds a soldier at the first spawn point.
    pub fn spawn_soldier(&mut self) -> SoldierId {
        let mut soldier = Soldier::new(&self.map.spawnpoints[0], &self.data, &self.config.weapons);
        soldier.jets_count = self.map.start_jet;
        soldier.jets_count_prev = self.map.start_jet;
        soldier.health = self.config.start_health();
        // CreateSprite randomizes the bullet seed counter
        soldier.bullet_count = self.rng.below(i32::from(u16::MAX)) as u16;
        self.soldiers.insert(soldier)
    }

    fn thing_ctx(&mut self) -> ThingCtx<'_> {
        ThingCtx {
            data: &self.data,
            map: &self.map,
            config: &self.config,
            soldiers: &mut self.soldiers,
            rng: &mut self.rng,
        }
    }

    /// Turns weapons soldiers let go of (thrown, or dropped by dying) into things.
    fn process_drops(&mut self) {
        let ids: Vec<SoldierId> = self.soldiers.keys().collect();
        for id in ids {
            let soldier = &mut self.soldiers[id];
            let (drop, released) = (soldier.pending_drop.take(), soldier.release_things);
            soldier.release_things = false;

            if let Some(drop) = drop {
                let mut things = std::mem::take(&mut self.things);
                if let Some(i) = create_thing(
                    &mut things,
                    &mut self.thing_ctx(),
                    drop.pos,
                    Some(id),
                    drop.kind,
                    None,
                    Some(&drop),
                ) {
                    things[i].ammo_count = drop.ammo_count;
                }
                self.things = things;
            }

            // a dead soldier no longer owns anything (TSprite.Die)
            if released {
                for thing in self.things.iter_mut().filter(|t| t.owner == Some(id)) {
                    thing.owner = None;
                }
            }
        }
    }

    /// Spawns the map's medikits and grenade kits (`SpawnThings` at map start).
    pub fn spawn_kits(&mut self) {
        let mut things = std::mem::take(&mut self.things);
        let (medikits, grenades) = (self.map.medikits, self.map.grenade_packs);
        spawn_things(
            &mut things,
            &mut self.thing_ctx(),
            ThingKind::MedicalKit,
            medikits,
        );
        if self.config.max_grenades > 0 {
            spawn_things(
                &mut things,
                &mut self.thing_ctx(),
                ThingKind::GrenadeKit,
                grenades,
            );
        }
        self.things = things;
    }

    /// Creates a thing at `pos` (`CreateThing` without an owner).
    pub fn create_thing(&mut self, kind: ThingKind, pos: Vec2) -> Option<usize> {
        let mut things = std::mem::take(&mut self.things);
        let i = create_thing(
            &mut things,
            &mut self.thing_ctx(),
            pos,
            None,
            kind,
            None,
            None,
        );
        self.things = things;
        i
    }

    /// `CreateBullet`: takes the first free slot. Returns false when all slots are in use.
    pub fn create_bullet(&mut self, params: &BulletParams, owner: SoldierId) -> bool {
        let Some(slot) = self.bullets.iter().position(|b| !b.active) else {
            return false;
        };

        let seed = match params.seed {
            Some(seed) => seed,
            None => match self.soldiers.get_mut(owner) {
                Some(soldier) => {
                    soldier.bullet_count = soldier.bullet_count.checked_add(1).unwrap_or(0);
                    soldier.bullet_count
                }
                None => 0,
            },
        };

        // BulletParts.CreatePart doesn't reset forces: a bullet killed during its update
        // (e.g. a rising flame) leaves its last force to the next bullet in the slot
        let stale_force = self.bullets[slot].particle.force;
        self.bullets[slot] = Bullet::new(params, owner, seed, self.config.gravity);
        self.bullets[slot].particle.force = stale_force;
        true
    }

    /// Advances the simulation by one tick, in Soldat's `UpdateFrame` order: soldiers
    /// (creating bullets as they fire), then bullets in slot order, then bullet integration.
    pub fn step(&mut self, inputs: &[(SoldierId, Input)]) -> Vec<GameEvent> {
        let mut events = Vec::new();

        for &(id, ref input) in inputs {
            if let Some(soldier) = self.soldiers.get_mut(id) {
                soldier.apply_input(input);
            }
        }

        // update soldiers

        let ids: Vec<SoldierId> = self.soldiers.keys().collect();

        for id in ids {
            let soldier = &mut self.soldiers[id];
            soldier.update(
                &self.map,
                &self.config,
                self.tick,
                &mut self.rng,
                &mut self.emitter,
            );

            let emitted = std::mem::take(&mut self.emitter);
            for item in emitted {
                match item {
                    EmitterItem::Bullet(params) => {
                        if self.create_bullet(&params, id) {
                            events.push(GameEvent::BulletFired {
                                owner: id,
                                weapon: params.weapon,
                                pos: params.position,
                            });
                        }
                    }
                    EmitterItem::Died(how) => events.push(GameEvent::Killed {
                        victim: id,
                        killer: id,
                        how,
                    }),
                }
            }

            self.process_drops();
        }

        // update bullets (bullets created on the way are updated if their slot comes later)

        let mut outcomes = Vec::new();

        for slot in 0..MAX_BULLETS {
            if !self.bullets[slot].active {
                continue;
            }

            let mut bullet = std::mem::take(&mut self.bullets[slot]);
            bullet.update(
                &self.map,
                &mut BulletCtx {
                    config: &self.config,
                    soldiers: &mut self.soldiers,
                    bullets: &mut self.bullets,
                    rng: &mut self.rng,
                    outcomes: &mut outcomes,
                },
            );
            let owner = bullet.owner;
            self.bullets[slot] = bullet;
            self.process_drops();

            for outcome in outcomes.drain(..) {
                match outcome {
                    BulletOutcome::Hit { kind, pos } => {
                        events.push(GameEvent::BulletHit { owner, kind, pos });
                    }
                    BulletOutcome::Killed {
                        victim,
                        killer,
                        how,
                    } => {
                        events.push(GameEvent::Killed {
                            victim,
                            killer,
                            how,
                        });
                    }
                    BulletOutcome::Spawn(params) => {
                        if self.create_bullet(&params, owner) {
                            events.push(GameEvent::BulletFired {
                                owner,
                                weapon: params.weapon,
                                pos: params.position,
                            });
                        }
                    }
                }
            }
        }

        // BulletParts.DoEulerTimeStep

        for bullet in self.bullets.iter_mut().filter(|b| b.active) {
            bullet.particle.euler();
        }

        // update things

        for slot in 0..MAX_THINGS {
            if self.things[slot].active {
                let mut thing = std::mem::take(&mut self.things[slot]);
                thing.update(slot, &mut self.thing_ctx());
                self.things[slot] = thing;
            }
        }

        // medikit cooldown
        if self.tick.is_multiple_of(HEALTH_COOLDOWN * 60) {
            for soldier in self.soldiers.values_mut() {
                soldier.has_pack = false;
            }
        }

        self.tick += 1;
        events
    }
}
