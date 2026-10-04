//! Damage, death and respawning (`TSprite.HealthHit`, `TSprite.Die`, `TSprite.Respawn`,
//! `RandomizeStart`), following the server, which is authoritative for damage in Soldat.

use super::*;
use slotmap::SlotMap;

pub const START_HEALTH: f32 = 150.0;
pub const REALISTIC_START_HEALTH: f32 = 65.0;
const HELMET_FALL_HEALTH: f32 = 70.0;
const PARA_DISTANCE: f32 = 500.0;
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

/// A soldier killed by a hit.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Kill {
    pub how: DeathKind,
    /// What the victim held when it died (Rambo scoring).
    pub victim_weapon: WeaponKind,
    /// The hit was on the head (skeleton point 12): a headshot in the weapon stats.
    pub head: bool,
    /// The skeleton point hit (`Where`).
    pub hit: u8,
}

/// A hit that got through (`HealthHit` past friendly fire and the flame god).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Hurt {
    /// The victim was alive when hit (hits on corpses only tear them apart).
    pub was_alive: bool,
    /// The kill, if the hit killed it.
    pub kill: Option<Kill>,
}

/// Who and what hit a soldier (`Who`, `What` of `HealthHit` and `Die`), for the
/// client's sounds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HitBy {
    pub killer: Option<SoldierId>,
    /// The killer is someone else.
    pub other: bool,
    /// The bullet's weapon (`Bullet[What].OwnerWeapon`).
    pub weapon: Option<WeaponKind>,
    /// A bot killer's `ChatFreq` and what it says on a kill.
    pub killer_chat: Option<(i32, String)>,
}

/// `TSprite.HealthHit`. `where_` is the skeleton point that was hit (1-based).
/// Returns what the hit did, if it got through.
#[allow(clippy::too_many_arguments)]
pub fn health_hit(
    soldiers: &mut SlotMap<SoldierId, Soldier>,
    victim: SoldierId,
    attacker: SoldierId,
    amount: f32,
    where_: usize,
    impact: Vec2,
    config: &WorldConfig,
    weapon: Option<WeaponKind>,
    rng: &mut PascalRandom,
) -> Option<Hurt> {
    let attacker_team = soldiers.get(attacker).map_or(Team::None, |s| s.team);
    let attacker_watches = soldiers
        .get(attacker)
        .is_some_and(|s| s.is_spectator() && s.brain.is_none());
    let attacker_berserk = soldiers
        .get(attacker)
        .is_some_and(|s| s.bonus_style == Bonus::Berserker);
    let killer_chat = soldiers
        .get(attacker)
        .and_then(|s| s.brain.as_ref())
        .filter(|_| attacker != victim)
        .map(|b| (b.chat_freq, b.chat.kill.clone()));
    // a client's damage comes from the server
    if config.client {
        return None;
    }
    // Rambo mode: while someone else holds the bow, nobody else hurts anybody
    let bow = [WeaponKind::Bow, WeaponKind::FlameBow];
    let rambo_elsewhere = config.game_mode == GameMode::Rambo
        && victim != attacker
        && soldiers.iter().any(|(id, s)| {
            s.active && id != attacker && id != victim && s.primary_weapon().is_any(&bow)
        });
    let soldier = soldiers.get_mut(victim)?;

    // friendly fire
    if !config.friendly_fire
        && soldier.team != Team::None
        && soldier.team == attacker_team
        && victim != attacker
    {
        return None;
    }

    // a human spectator's bullets don't hurt
    if attacker_watches {
        return None;
    }

    if soldier.bonus_style == Bonus::Flamegod || rambo_elsewhere {
        return None;
    }

    let was_alive = !soldier.dead_meat;
    let victim_weapon = soldier.primary_weapon().kind;
    let by = HitBy {
        killer: Some(attacker),
        other: attacker != victim,
        weapon,
        killer_chat,
    };
    let how = soldier.take_damage(amount, where_, impact, config, attacker_berserk, by, rng);
    if let Some(brain) = soldier.brain.as_mut() {
        brain.target = Some(attacker);
    }
    soldier.low_health_chat(config, rng);

    let Some(how) = how else {
        return Some(Hurt {
            was_alive,
            kill: None,
        });
    };
    // TSprite.Die: the killer calms down
    if let Some(brain) = soldiers.get_mut(attacker).and_then(|s| s.brain.as_mut()) {
        brain.pissed_off = None;
    }

    // the world scores kills (score_kill)
    let kill = was_alive.then_some(Kill {
        how,
        victim_weapon,
        head: where_ == 12,
        hit: where_ as u8,
    });
    Some(Hurt { was_alive, kill })
}

impl Soldier {
    /// `HealthHit` with the soldier as the attacker (polygons, falls).
    pub(crate) fn self_hit(
        &mut self,
        amount: f32,
        where_: usize,
        impact: Vec2,
        config: &WorldConfig,
        rng: &mut PascalRandom,
    ) -> Option<DeathKind> {
        if self.bonus_style == Bonus::Flamegod || config.client {
            return None;
        }
        let berserk = self.bonus_style == Bonus::Berserker;
        let how = self.take_damage(
            amount,
            where_,
            impact,
            config,
            berserk,
            HitBy::default(),
            rng,
        );
        if let Some(brain) = self.brain.as_mut() {
            if how.is_some() {
                brain.pissed_off = None;
            }
            brain.target = Some(brain.me);
        }
        self.low_health_chat(config, rng);
        how
    }

    /// The end of `HealthHit`: a hurt bot (or dead one) may complain.
    fn low_health_chat(&mut self, config: &WorldConfig, rng: &mut PascalRandom) {
        if !config.bots_chat || self.health >= HURT_HEALTH {
            return;
        }
        if let Some(brain) = &self.brain
            && rng.below_i64(10 * i64::from(brain.chat_freq)) == 0
        {
            let text = brain.chat.low_health.clone();
            self.said.push(text);
        }
    }

    /// The victim's side of `TSprite.HealthHit` (after the friendly fire check): vest,
    /// health and death. Returns the death this caused, if any.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn take_damage(
        &mut self,
        amount: f32,
        where_: usize,
        impact: Vec2,
        config: &WorldConfig,
        attacker_berserk: bool,
        by: HitBy,
        rng: &mut PascalRandom,
    ) -> Option<DeathKind> {
        if self.bonus_style == Bonus::Flamegod {
            return None;
        }

        let mut hm = amount;
        if self.vest > 0.0 {
            hm = (0.33 * ext(amount)).round_ties_even() as f32;
            self.vest -= hm;
            hm = (0.25 * ext(amount)).round_ties_even() as f32;
        }

        // on the server this also applies to Berserkers hurting themselves
        if attacker_berserk {
            hm = 4.0 * amount;
        }

        self.health -= hm;

        // helmet falls off
        if self.health < HELMET_FALL_HEALTH
            && self.wear_helmet == 1
            && where_ == 12
            && !self
                .primary_weapon()
                .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
            && self.head_cap > 0
        {
            self.wear_helmet = 0;
            let head = self.skeleton.pos(12);
            let v = self.particle.velocity;
            self.spark(head, v, 6, 198);
            self.play_sound(Sound::new(Sfx::Headchop).at(head));
        }

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

        Some(self.die(how, where_, impact, config, attacker_berserk, by, rng))
    }

    /// `TSprite.Die`, gameplay part. Also called for hits on corpses, which only tear them
    /// further apart.
    #[allow(clippy::too_many_arguments)]
    pub fn die(
        &mut self,
        mut how: DeathKind,
        where_: usize,
        impact: Vec2,
        config: &WorldConfig,
        killer_berserk: bool,
        by: HitBy,
        rng: &mut PascalRandom,
    ) -> DeathKind {
        if !self.dead_meat {
            self.respawn_counter = if config.game_mode.is_team_game() {
                config.now.wave_respawn_counter + config.respawn_min_wave
            } else {
                config.respawn_time
            };
            self.deaths += 1;

            if self.idle_random == 7 && self.primary_weapon().kind == WeaponKind::NoWeapon {
                how = DeathKind::Brutal;
            }

            self.body_animation.frame = 0;

            // Bot Chat: the dead and its killer have their say (`ChatFreq div 2` is an
            // Int64 on 64-bit FPC, so these are Random(Int64))
            if config.bots_chat {
                if let Some(brain) = &self.brain
                    && rng.below_i64(i64::from(brain.chat_freq / 2)) == 0
                {
                    let text = brain.chat.dead.clone();
                    self.said.push(text);
                }
                if let (Some(killer), Some((freq, text))) = (by.killer, &by.killer_chat)
                    && rng.below_i64(i64::from(freq / 3)) == 0
                {
                    self.killer_said.push((killer, text.clone()));
                }
            }

            // DropWeapon (the world creates the thing), still as a living soldier; the
            // empty hands keep the weapon's damage
            let hit_multiply = self.primary_weapon().hit_multiply;
            self.drop_weapon(config);
            self.weapons[self.active_weapon].hit_multiply = hit_multiply;
            self.release_things = true;
            self.control.free_controls();
        }

        let by_other = by.other;
        self.death_sounds(how, where_, killer_berserk, by);

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

        // Berserker kills tear the body apart (on the server also hits on corpses)
        if killer_berserk {
            for c in [2, 4, 20, 21, 23] {
                self.skeleton.set_constraint_active(c, false);
            }
        }

        if !self.dead_meat && self.has_cigar == 10 {
            self.spark(self.skeleton.pos(12), impact, 34, 245);
            self.has_cigar = 0;
        }

        // "BREAD": advance mode's weapons. The server's condition is inverted (it runs without
        // advance mode, on kills by others), and as the server marks every primary selectable
        // each tick nothing changes, but every `sv_advancemode_amount`th death still takes a
        // random one away: one Random(10). (The goldens can't check it: both harnesses reseed
        // the random numbers every tick, and nothing later in the tick shows the draw.)
        if !config.client
            && !config.advance_mode
            && !self.dead_meat
            && by_other
            && self.deaths % config.advance_mode_amount == 0
        {
            rng.below(10);
        }

        self.dead_meat = true;
        self.alpha = 255;
        self.vest = 0.0;
        self.dead_time = if self.dead_time > 0 && self.on_fire == 0 {
            self.dead_time / 2
        } else {
            0
        };
        self.particle.velocity = Vec2::ZERO;
        self.bonus_style = Bonus::None;
        self.bonus_time = 0;
        // a parachute keeps holding the corpse and links itself again (TThing.Update);
        // flags are dropped (the world lets go of them)
        self.holded_thing = None;
        self.holds_flag = false;
        self.parachute = false;
        self.stop_sound(Channel::Reload);

        how
    }

    /// The client's sounds of `TSprite.Die` (it takes a head shot with a Ruger for a head
    /// chop). Corpses only make noise when torn apart.
    fn death_sounds(&mut self, how: DeathKind, where_: usize, killer_berserk: bool, by: HitBy) {
        let head = self.skeleton.pos(12);
        let headshot = where_ == 12 && by.weapon == Some(WeaponKind::Ruger77);
        match if headshot { DeathKind::Headchop } else { how } {
            DeathKind::Normal => {
                if !self.dead_meat {
                    self.play_random(Sfx::Death, 3);
                }
            }
            DeathKind::Headchop => {
                let sniper = matches!(by.weapon, Some(WeaponKind::Barrett | WeaponKind::Ruger77));
                if !self.dead_meat && where_ == 12 && sniper {
                    // a fountain of blood (the client also keeps the head on the corpse)
                    if fx::random(100) > 50 {
                        let neck = self.skeleton.pos(9);
                        for i in 0..=50 {
                            let angle = (360.0 / 50.0 * i as f32).to_radians();
                            let (sin, cos) = angle.sin_cos();
                            let a = neck + vec2(cos, sin) * 2.0;
                            let b = vec2(
                                cos * fx::random_range(1, 3) as f32,
                                sin * fx::random_range(1, 3) as f32,
                            );
                            let style = if i < 25 {
                                fx::random_range(4, 5) as u8
                            } else {
                                5
                            };
                            self.spark(a, b, style, 100 - fx::random(20));
                        }
                    }
                    if by.weapon == Some(WeaponKind::Barrett) {
                        // the corpse explodes
                        self.play_sound(Sound::new(Sfx::Bryzg).at(head));
                    }
                    if let Some(killer) = by.killer {
                        let audience = Audience::Player(killer);
                        self.play_sound(Sound::new(Sfx::Boomheadshot).audience(audience));
                    }
                }
                if !self.dead_meat {
                    self.play_sound(Sound::new(Sfx::Headchop).at(head));
                }
            }
            DeathKind::Brutal => self.play_sound(Sound::new(Sfx::Bryzg).at(head)),
        }
        if self.dead_meat && killer_berserk {
            self.play_sound(Sound::new(Sfx::Killberserk).at(head));
        }
        if !self.dead_meat && by.weapon == Some(WeaponKind::Flamer) {
            self.play_sound(Sound::new(Sfx::Burn).at(head));
        }
    }

    /// `TSprite.Respawn` (server path) with the soldier's own loadout.
    pub fn respawn(&mut self, map: &MapFile, config: &WorldConfig, rng: &mut PascalRandom) {
        self.respawned = true;
        if self.is_spectator() {
            return;
        }
        let was_dead = self.dead_meat;
        let pos = self.particle.pos;
        self.play_sound(Sound::new(Sfx::Wermusic).at(pos).audience(Audience::Own));
        // a held flag goes home, a parachute disappears (the world does it)
        self.respawn_held = self.holded_thing.take();
        self.holds_flag = false;
        self.parachute = false;

        let pos = randomize_start(map, self.team, rng);
        self.particle.pos = pos;

        self.dead_meat = false;
        self.half_dead = false;
        self.health = config.start_health();
        self.wear_helmet = if self.head_cap == 0 { 0 } else { 1 };
        self.bonus_style = Bonus::None;
        self.bonus_time = 0;
        self.bg = BackgroundState::default();
        self.hit_spray_counter = 0;
        self.skeleton.activate_all_constraints();
        self.particle.velocity = Vec2::ZERO;
        self.particle.force = Vec2::ZERO;
        self.jets_count = map.start_jet;
        self.jets_count_prev = map.start_jet;
        self.ceasefire_counter = config.ceasefire_time;
        self.vest = 0.0;
        self.has_cigar = 0;
        self.can_mercy = true;
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

        // the loadout, as far as the player may have it (a locked primary leaves the hands)
        let loadout = self.loadout.map(|kind| {
            if self.may_use(kind) {
                kind
            } else {
                WeaponKind::NoWeapon
            }
        });
        self.weapons = loadout.map(|kind| config.weapons.get(kind));
        if self.brain.is_some() {
            self.bot_weapons(config, rng);
        }
        self.active_weapon = 0;
        let grenades = &mut self.weapons[2];
        grenades.ammo_count = (config.max_grenades / 2) as u8;

        // Parachute: in the air with nothing below, the world hands out a parachute
        if self.holded_thing.is_none() {
            self.parachute_call = true;
            let a = self.particle.pos;
            let b = vec2(a.x, a.y + PARA_DISTANCE);
            let filter = RayCast {
                player: true,
                flag: false,
                bullet: false,
                check_collider: false,
                team: self.team,
            };
            if map.ray_cast(a, b, PARA_DISTANCE + 50.0, filter).is_none() {
                self.parachute = true;
                self.parachute_spawn = Some(vec2(a.x, a.y + 70.0));
            }
        }
        self.next_push = [Vec2::ZERO; MAX_PUSHTICK + 1];
        let pos = self.particle.pos;
        self.play_sound(Sound::new(Sfx::Spawn).at(pos).audience(Audience::Others));
        let v = self.particle.velocity;
        self.spark(pos, v, 25, 33);
        self.control.free_controls();
        self.legs_apply_animation(Anim::Stand, 1);
        self.body_apply_animation(Anim::Stand, 1);

        // CanRespawn: in a running survival round the dead stay dead
        if config.survival_mode {
            if !config.now.survival_end_round && was_dead {
                let pos = self.particle.pos;
                for i in 1..=20 {
                    *self.skeleton.pos_mut(i) = pos;
                    *self.skeleton.old_pos_mut(i) = pos;
                }
                let head = self.skeleton.pos(12);
                self.die(
                    DeathKind::Normal,
                    1,
                    head,
                    config,
                    false,
                    HitBy::default(),
                    rng,
                );
                self.deaths -= 1;
                self.survival_died = true;
            } else if config.now.survival_end_round {
                self.survival_respawned = true;
            }
        }
    }
}

impl Soldier {
    /// A bot's weapons on respawn (`TSprite.Respawn`, server, all weapons allowed).
    fn bot_weapons(&mut self, config: &WorldConfig, rng: &mut PascalRandom) {
        let selectable = self.weapon_sel & PRIMARIES;
        let Some(brain) = self.brain.as_mut() else {
            return;
        };
        let guns = WeaponKind::values();

        brain.pissed_off = None;
        brain.go_thing = false;
        brain.current_waypoint = 0;
        if matches!(
            config.game_mode,
            GameMode::CaptureTheFlag | GameMode::Infiltration | GameMode::HoldTheFlag
        ) {
            brain.path_num = self.team as u8;
        }

        // Player.SecWep would pick the secondary, but the server never marks secondaries
        // as selectable (WeaponSel[Num][11..14] stay 0), so bots respawn without one
        let mut primary = WeaponKind::NoWeapon;

        // randomize the bot's weapon
        let fav = brain.fav_weapon;
        // (only with a primary it may pick: none in advance mode before its kills)
        if !matches!(
            fav,
            WeaponKind::NoWeapon | WeaponKind::Knife | WeaponKind::Chainsaw | WeaponKind::LAW
        ) && !brain.dummy
            && selectable != 0
        {
            primary = if rng.below(2) == 0 {
                fav
            } else {
                guns[rng.below(9) as usize]
            };
            // advance mode: the first primary it has won
            if config.advance_mode {
                primary = guns[selectable.trailing_zeros() as usize];
            }
        }

        let fav_secondary = matches!(
            fav,
            WeaponKind::USSOCOM | WeaponKind::Knife | WeaponKind::Chainsaw | WeaponKind::LAW
        );
        if fav == WeaponKind::NoWeapon || fav_secondary || brain.dummy {
            primary = fav;
        }

        match brain.on_start_use {
            1 => {
                self.idle_time = 0;
                self.idle_random = 1;
            }
            2 => {
                self.idle_time = 0;
                self.idle_random = 0;
            }
            _ => {}
        }

        self.weapons[0] = config.weapons.get(primary);
        self.weapons[1] = config.weapons.get(WeaponKind::NoWeapon);
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
