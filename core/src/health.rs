//! Damage, death and respawning (`TSprite.HealthHit`, `TSprite.Die`, `TSprite.Respawn`,
//! `RandomizeStart`), following the server, which is authoritative for damage in Soldat.

use super::*;
use slotmap::SlotMap;

pub const START_HEALTH: f32 = 150.0;
pub const REALISTIC_START_HEALTH: f32 = 65.0;
const BRUTAL_DEATH_HEALTH: f32 = -400.0;
const HEADCHOP_DEATH_HEALTH: f32 = -90.0;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeathKind {
    Normal,
    /// Head (or a leg) comes off.
    Headchop,
    /// Torn apart.
    Brutal,
}

/// `TSprite.HealthHit`. `where_` is the skeleton point that was hit (1-based).
/// Returns the death this caused, if any.
pub fn health_hit(
    soldiers: &mut SlotMap<SoldierId, Soldier>,
    victim: SoldierId,
    attacker: SoldierId,
    amount: f32,
    where_: usize,
    impact: Vec2,
    config: &WorldConfig,
) -> Option<DeathKind> {
    let attacker_team = soldiers.get(attacker).map_or(Team::None, |s| s.team);
    let soldier = soldiers.get_mut(victim)?;

    // friendly fire
    if !config.friendly_fire
        && soldier.team != Team::None
        && soldier.team == attacker_team
        && victim != attacker
    {
        return None;
    }

    let was_alive = !soldier.dead_meat;
    let how = soldier.take_damage(amount, where_, impact, config)?;

    // kills are counted by the killer (deathmatch scoring)
    if was_alive
        && attacker != victim
        && let Some(killer) = soldiers.get_mut(attacker)
    {
        killer.kills += 1;
    }

    Some(how)
}

impl Soldier {
    /// The victim's side of `TSprite.HealthHit` (after the friendly fire check): vest,
    /// health and death. Returns the death this caused, if any.
    pub(crate) fn take_damage(
        &mut self,
        amount: f32,
        where_: usize,
        impact: Vec2,
        config: &WorldConfig,
    ) -> Option<DeathKind> {
        // TODO: Flamegod bonus immunity, Rambo mode bow immunity, Berserker 4x damage

        let mut hm = amount;
        if self.vest > 0.0 {
            hm = (0.33 * ext(amount)).round_ties_even() as f32;
            self.vest -= hm;
            hm = (0.25 * ext(amount)).round_ties_even() as f32;
        }

        self.health -= hm;

        // TODO: helmet falls off below 70 health when hit in the head

        // safety precautions
        let start_health = config.start_health();
        if self.health < BRUTAL_DEATH_HEALTH - 1.0 {
            self.health = BRUTAL_DEATH_HEALTH;
        }
        if self.health > start_health {
            self.health = start_health;
        }

        let health = self.health;
        let how = if health < 1.0 && health > HEADCHOP_DEATH_HEALTH {
            DeathKind::Normal
        } else if health < HEADCHOP_DEATH_HEALTH + 1.0 && health > BRUTAL_DEATH_HEALTH {
            DeathKind::Headchop
        } else if health < BRUTAL_DEATH_HEALTH + 1.0 {
            DeathKind::Brutal
        } else {
            return None;
        };

        Some(self.die(how, where_, impact, config))
    }

    /// `TSprite.Die`, gameplay part. Also called for hits on corpses, which only tear them
    /// further apart.
    pub fn die(
        &mut self,
        mut how: DeathKind,
        where_: usize,
        _impact: Vec2,
        config: &WorldConfig,
    ) -> DeathKind {
        if !self.dead_meat {
            // TODO: wave respawns in team modes
            self.respawn_counter = config.respawn_time;
            self.deaths += 1;

            if self.idle_random == 7 && self.primary_weapon().kind == WeaponKind::NoWeapon {
                how = DeathKind::Brutal;
            }

            self.body_animation.frame = 0;

            // DropWeapon (the world creates the thing), still as a living soldier
            self.drop_weapon(config);
            self.release_things = true;
            self.control.free_controls();
        }

        match how {
            DeathKind::Normal => {}
            DeathKind::Headchop => match where_ {
                12 => self.skeleton.set_constraint_active(20, false),
                3 => self.skeleton.set_constraint_active(2, false),
                4 => self.skeleton.set_constraint_active(4, false),
                _ => {}
            },
            DeathKind::Brutal => {
                for c in [2, 4, 20, 21, 23] {
                    self.skeleton.set_constraint_active(c, false);
                }
            }
        }

        if !self.dead_meat && self.has_cigar == 10 {
            self.has_cigar = 0;
        }

        // TODO: survival mode, advance mode

        self.dead_meat = true;
        self.alpha = 255;
        self.vest = 0.0;
        self.dead_time = if self.dead_time > 0 && self.on_fire == 0 {
            self.dead_time / 2
        } else {
            0
        };
        self.particle.velocity = Vec2::ZERO;

        how
    }

    /// `TSprite.Respawn` (server path) with the soldier's own loadout.
    pub fn respawn(&mut self, map: &MapFile, config: &WorldConfig, rng: &mut PascalRandom) {
        let pos = randomize_start(map, self.team, rng);
        self.particle.pos = pos;

        self.dead_meat = false;
        self.half_dead = false;
        self.health = config.start_health();
        self.wear_helmet = 1;
        self.skeleton.activate_all_constraints();
        self.particle.velocity = Vec2::ZERO;
        self.particle.force = Vec2::ZERO;
        self.jets_count = map.start_jet;
        self.jets_count_prev = map.start_jet;
        self.ceasefire_counter = config.ceasefire_time;
        self.vest = 0.0;
        self.has_cigar = 0;
        self.idle_time = DEFAULT_IDLETIME;
        self.idle_random = -1;
        self.body_animation = self.anims.state(Anim::Stand);
        self.legs_animation = self.anims.state(Anim::Stand);
        self.position = POS_STAND;
        self.on_fire = 0;
        self.dead_collide_count = 0;
        self.respawn_counter = 0;
        self.on_ground = false;
        self.on_ground_last_frame = false;
        self.on_ground_permanent = false;

        // TODO: weapon selection menu; respawn with the loadout
        self.weapons = self.loadout.map(|kind| config.weapons.get(kind));
        self.active_weapon = 0;
        let grenades = &mut self.weapons[2];
        grenades.ammo_count = (config.max_grenades / 2) as u8;

        // TODO: parachute when spawning in the air
        self.next_push = Vec2::ZERO;
        self.control.free_controls();
        self.legs_apply_animation(Anim::Stand, 1);
        self.body_apply_animation(Anim::Stand, 1);
    }
}

/// `RandomizeStart`: a random spawn point of the team (any active one when the team has
/// none), jittered by a few pixels.
pub fn randomize_start(map: &MapFile, team: Team, rng: &mut PascalRandom) -> Vec2 {
    randomize_start_team(map, team as i32, rng).0
}

/// `RandomizeStart` by spawn point team number (soldier teams, flags, kits, ...).
/// Returns the position and whether a spawn point of that team existed.
pub fn randomize_start_team(map: &MapFile, team: i32, rng: &mut PascalRandom) -> (Vec2, bool) {
    let mut found = true;
    let mut spawns: Vec<&MapSpawnpoint> = map
        .spawnpoints
        .iter()
        .filter(|s| s.active && s.team == team)
        .collect();

    if spawns.is_empty() {
        found = false;
        spawns = map.spawnpoints.iter().filter(|s| s.active).collect();
    }

    if spawns.is_empty() {
        return (Vec2::ZERO, found);
    }

    let spawn = spawns[rng.below(spawns.len() as i32) as usize];
    let x = spawn.x - 4 + rng.below(8);
    let y = spawn.y - 4 + rng.below(4);
    (vec2(x as f32, y as f32), found)
}
