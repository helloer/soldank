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
}

/// Game rules derived from cvars, read by the simulation every tick.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WorldConfig {
    pub gravity: f32,
    pub realistic_mode: bool,
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            gravity: GRAV,
            realistic_mode: false,
        }
    }
}

impl WorldConfig {
    pub fn from_cvars(cvars: &Cvars) -> WorldConfig {
        WorldConfig {
            gravity: cvars.float("sv_gravity"),
            realistic_mode: cvars.bool("sv_realisticmode"),
        }
    }
}

/// Something that happened during a tick, for audio, effects, HUD, scripting and networking.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    BulletFired {
        owner: SoldierId,
        weapon: WeaponKind,
        pos: Vec2,
    },
}

pub struct World {
    pub data: Arc<GameData>,
    pub map: MapFile,
    pub config: WorldConfig,
    pub soldiers: SlotMap<SoldierId, Soldier>,
    pub bullets: Vec<Bullet>,
    pub tick: u64,
    emitter: Vec<EmitterItem>,
}

impl World {
    pub fn new(data: Arc<GameData>, map: MapFile, config: WorldConfig) -> World {
        World {
            data,
            map,
            config,
            soldiers: SlotMap::with_key(),
            bullets: Vec::new(),
            tick: 0,
            emitter: Vec::new(),
        }
    }

    /// Adds a soldier at the first spawn point.
    pub fn spawn_soldier(&mut self) -> SoldierId {
        let soldier = Soldier::new(&self.map.spawnpoints[0], &self.data);
        self.soldiers.insert(soldier)
    }

    /// Advances the simulation by one tick.
    pub fn step(&mut self, inputs: &[(SoldierId, Input)]) -> Vec<GameEvent> {
        let mut events = Vec::new();

        // remove inactive bullets

        self.bullets.retain(|bullet| bullet.active);

        // update soldiers

        for &(id, ref input) in inputs {
            if let Some(soldier) = self.soldiers.get_mut(id) {
                soldier.apply_input(input);
            }
        }

        let mut emitted = Vec::new();

        for (id, soldier) in self.soldiers.iter_mut() {
            soldier.update(&self.map, &self.config, &mut self.emitter);
            emitted.extend(self.emitter.drain(..).map(|item| (id, item)));
        }

        // update bullets

        for bullet in self.bullets.iter_mut() {
            bullet.update(&self.map);
        }

        // create emitted objects (after the bullet update, so they first move next tick)

        for (owner, item) in emitted {
            match item {
                EmitterItem::Bullet(params) => {
                    events.push(GameEvent::BulletFired {
                        owner,
                        weapon: params.weapon,
                        pos: params.position,
                    });
                    self.bullets.push(Bullet::new(&params));
                }
            };
        }

        self.tick += 1;
        events
    }
}
