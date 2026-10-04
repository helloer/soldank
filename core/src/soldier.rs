use super::*;
use std::sync::Arc;

const SLIDELIMIT: f64 = 0.2;
const SURFACECOEFX: f64 = 0.970;
const SURFACECOEFY: f64 = 0.970;
const CROUCHMOVESURFACECOEFX: f64 = 0.85;
const CROUCHMOVESURFACECOEFY: f64 = 0.97;
const STANDSURFACECOEFX: f64 = 0.00;
const STANDSURFACECOEFY: f64 = 0.00;

pub const POS_STAND: u8 = 1;
pub(crate) const POS_CROUCH: u8 = 2;
pub(crate) const POS_PRONE: u8 = 3;

const MAX_INACCURACY: f32 = 0.5;

/// The fastest a soldier goes on each axis, a tick (`MAX_VELOCITY`).
pub const MAX_VELOCITY: f32 = 11.0;
/// `DEFAULTAIMDIST`, `SNIPERAIMDIST`, `CROUCHAIMDIST`, `AIMDISTINCR`: the camera's reach.
pub const DEFAULT_AIM_DIST: f32 = 7.0;
pub(crate) const SNIPER_AIM_DIST: f32 = 3.5;
pub(crate) const CROUCH_AIM_DIST: f32 = 4.5;
pub(crate) const AIM_DIST_INCR: f64 = 0.05;
/// `LESSBLEED_TIME`, `NOBLEED_TIME`, `ONFIRE_TIME`: how long a corpse bleeds and burns.
const LESSBLEED_TIME: i32 = 60 * 2;
/// `BLOOD_RANDOM_NORMAL`, `BLOOD_RANDOM_HIGH`, `HURT_HEALTH`
const BLOOD_RANDOM_NORMAL: i32 = 10;
const BLOOD_RANDOM_HIGH: i32 = 6;
pub(crate) const HURT_HEALTH: f32 = 25.0;
const NOBLEED_TIME: i32 = 60 * 5;
const ONFIRE_TIME: i32 = 60 * 4;
const SOLDIER_COL_RADIUS: f32 = 3.0;

/// How a player looks (`TPlayer` colours and styles). Colours are `0xRRGGBB`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Looks {
    pub shirt: u32,
    pub pants: u32,
    pub skin: u32,
    pub hair: u32,
    pub jet: u32,
    /// `HairStyle`: 0 bald, 1 dreadlocks, 2 punk, 3 Mr. T, 4 normal.
    pub hair_style: u8,
    /// `Chain`: 0 none, 1 silver, 2 golden.
    pub chain: u8,
}

impl Default for Looks {
    /// The `cl_player_*` defaults.
    fn default() -> Looks {
        Looks {
            shirt: 0x304289,
            pants: 0xFF0000,
            skin: 0xE6B478,
            hair: 0x000000,
            jet: 0x00008B,
            hair_style: 0,
            chain: 0,
        }
    }
}

impl Soldier {
    /// `PlaySound(sfx, SpriteParts.Pos[Num])`
    pub(crate) fn play(&mut self, sfx: Sfx) {
        self.sounds.push(SoundEvent::at(sfx, self.particle.pos));
    }

    /// A sound from somewhere on the soldier, or a variation of [`Soldier::play`].
    pub(crate) fn play_sound(&mut self, sound: Sound) {
        self.sounds.push(sound.into());
    }

    /// `PlaySound(sfx + Random(variants), SpriteParts.Pos[Num])`
    pub(crate) fn play_random(&mut self, sfx: Sfx, variants: u8) {
        let pos = self.particle.pos;
        self.play_sound(Sound::new(sfx).at(pos).variants(variants));
    }

    /// `if Random(one_in) = 0 then PlaySound(sfx, SpriteParts.Pos[Num])`, the dice rolled
    /// by the client.
    pub(crate) fn play_maybe(&mut self, sfx: Sfx, one_in: u8) {
        let pos = self.particle.pos;
        self.play_sound(Sound::new(sfx).at(pos).one_in(one_in));
    }

    /// `PlaySound(sfx, SpriteParts.Pos[Num], <channel>)`
    pub(crate) fn play_on(&mut self, sfx: Sfx, channel: Channel) {
        let pos = self.particle.pos;
        self.play_sound(Sound::new(sfx).at(pos).channel(channel));
    }

    pub(crate) fn stop_sound(&mut self, channel: Channel) {
        self.sounds.push(SoundEvent::Stop(channel));
    }

    /// `IsSpectator`
    pub fn is_spectator(&self) -> bool {
        self.team == Team::Spectator
    }

    pub(crate) fn pause_sound(&mut self, channel: Channel, paused: bool) {
        self.sounds.push(SoundEvent::Pause(channel, paused));
    }
}

impl Looks {
    /// `ApplyShirtColorFromTeam`: team games with `sv_teamcolors` dress the teams alike.
    pub fn apply_team_shirt(&mut self, team: Team, config: &WorldConfig) {
        if !config.team_colors || !config.game_mode.is_team_game() {
            return;
        }
        self.shirt = match team {
            Team::Alpha => 0xD20F05,
            Team::Bravo => 0x151FD9,
            Team::Charlie => 0xD2D205,
            Team::Delta => 0x05D205,
            Team::None | Team::Spectator => return,
        };
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct Soldier {
    pub active: bool,
    pub dead_meat: bool,
    pub style: u8,
    pub num: usize,
    pub visible: u8,
    pub on_ground: bool,
    pub on_ground_for_law: bool,
    pub on_ground_last_frame: bool,
    pub on_ground_permanent: bool,
    pub direction: i8,
    pub old_direction: i8,
    pub health: f32,
    pub alpha: u8,
    pub jets_count: i32,
    pub jets_count_prev: i32,
    pub wear_helmet: u8,
    pub has_cigar: u8,
    pub vest: f32,
    pub idle_time: i32,
    pub idle_random: i8,
    pub position: u8,
    pub on_fire: u8,
    pub collider_distance: u8,
    pub half_dead: bool,
    pub skeleton: ParticleSystem,
    pub legs_animation: AnimState,
    pub body_animation: AnimState,
    pub control: Control,
    pub active_weapon: usize,
    pub weapons: [Weapon; 3],
    pub fired: u8,
    pub particle: Particle,
    pub anims: Arc<Animations>,
    pub burst_count: u8,
    pub can_auto_reload_spas: bool,
    /// Reload the Spas as soon as it can fire again (reload pressed while it couldn't).
    pub auto_reload_when_can_fire: bool,
    /// Seeds the Desert Eagle / shotgun spread so it can be replayed (`TSprite.BulletCount`).
    pub bullet_count: u16,
    /// In a network game, played on another machine: a human on the server, everyone but
    /// the player on a client. Its slow weapons' bullets come from that machine.
    pub remote: bool,
    /// Spawn protection ticks left, -1 when inactive.
    pub ceasefire_counter: i32,
    pub team: Team,
    /// Knock-back from hits, applied at the start of the next update (server semantics).
    pub next_push: Vec2,
    pub respawn_counter: i32,
    pub dead_time: i32,
    pub dead_collide_count: i32,
    pub kills: i32,
    pub deaths: i32,
    /// Weapons given on respawn (primary, secondary, grenades).
    pub loadout: [WeaponKind; 3],
    /// The weapons it may pick (`WeaponSel`): bits by weapon menu number, the 10 primaries
    /// then the 4 secondaries (advance mode, an admin's `weaponoff`).
    pub weapon_sel: u16,
    /// It called `DropWeapon` / respawned this tick (for the world's survival weapon clearing).
    pub dropped_weapon: bool,
    pub respawned: bool,
    /// The grenade key was released since the last throw.
    pub grenade_can_throw: bool,
    /// Thing slot of the flag or parachute held (`HoldedThing`).
    pub holded_thing: Option<usize>,
    /// Picked up a medikit recently (`sv_healthcooldown`).
    pub has_pack: bool,
    /// Flags grabbed and brought home since the server last looked, and whether the last
    /// grab was in its base (`GrabsPerSecond`, `ScoresPerSecond`, `GrabbedInBase`: the
    /// server's `sv_antimassflag`).
    pub grabs_per_second: i32,
    pub scores_per_second: i32,
    pub grabbed_in_base: bool,
    /// Weapon let go of this tick, turned into a thing by the world.
    pub pending_drop: Option<WeaponDrop>,
    /// Died this tick: the world clears the ownership of its things.
    pub release_things: bool,
    /// The soldier of the player at this computer: gets bink and recoil (client-side
    /// effects in Soldat, so golden traces never see them).
    pub local_player: bool,
    /// Bink and self-bink accumulated (`HitSprayCounter`), local player only.
    pub hit_spray_counter: u16,
    /// Cursor kick from the last shot this tick (radians), for the client.
    pub recoil_kick: f32,
    /// Background polygon state (`BGState`).
    pub bg: BackgroundState,
    /// Holding a parachute (`HoldedThing` is an `OBJECT_PARACHUTE`).
    pub parachute: bool,
    /// `Para`: `parachute` as of the last update, which is what the controls see.
    pub para: bool,
    /// Steering the parachute this tick: thing slot and 1 right, -1 left (the world
    /// bends it).
    pub parachute_bend: Option<(usize, i8)>,
    /// `Parachute` ran this tick: the world drops things held so far, creates the
    /// parachute at `parachute_spawn` and releases it when `release_parachute` is set.
    pub parachute_call: bool,
    pub parachute_spawn: Option<Vec2>,
    pub release_parachute: bool,
    /// Headgear (`Player.HeadCap`): 0 none, 1 helmet, 2 cap.
    pub head_cap: u8,
    /// The held thing is a flag.
    pub holds_flag: bool,
    /// `ThrowFlag` requested this tick (the world moves the flag).
    pub flag_throw: Option<FlagThrow>,
    /// The thing held when respawning: a flag goes home, a parachute disappears.
    pub respawn_held: Option<usize>,
    /// Flag captures (`Player.Flags`).
    pub flags: i32,
    /// `Player.RealPing`: the round trip to the server in milliseconds (network play).
    pub ping: u16,
    /// Ticks until a thrown flag can be grabbed again (`FlagGrabCooldown`).
    pub flag_grab_cooldown: i32,
    /// Multikill window (`MultiKillTime`) and count.
    pub multi_kill_time: i32,
    pub multi_kills: i32,
    pub bonus_style: Bonus,
    /// Ticks left of the bonus.
    pub bonus_time: i32,
    /// `Player.Name`
    pub name: String,
    pub looks: Looks,
    /// Sounds asked for this tick (the world passes them on as events).
    pub sounds: Vec<SoundEvent>,
    /// Sparks asked for this tick.
    pub sparks: Vec<SparkSpawn>,
    /// What the soldier (a bot) says this tick, or what its killer says about it.
    pub said: Vec<String>,
    /// The idle animation (`IdleRandom`) that started this tick, for the server to tell.
    pub idle_started: Option<i8>,
    pub killer_said: Vec<(SoldierId, String)>,
    /// `AimDistCoef`: how far the camera looks towards the cursor (smaller is farther; the
    /// Barrett's scope shrinks it).
    pub aim_dist_coef: f32,
    /// A weapon with a start up time spun up since the trigger was pulled: it winds down
    /// when let go (the client's `StartUpTimeCount`, kept apart from the simulation's).
    pub spin: bool,
    /// A living soldier stands within melee distance (set by the world each update).
    pub melee_reach: bool,
    /// The stationary gun used (`Stat`, a thing slot) and how hot it got (`UseTime`).
    pub stat: Option<usize>,
    pub use_time: i16,
    /// Stopped using a stationary gun this tick (the world unfreezes the thing).
    pub stat_release: Option<usize>,
    /// The mercy command may start (`CanMercy`); the local player's mercy shot asks the
    /// world to kill it (the client's `kill` command).
    pub can_mercy: bool,
    pub mercy_kill: bool,
    /// Survival mode requests for the world: this soldier died again on respawn
    /// (`CanRespawn`), respawned while the round was over, or its countdown ended the round.
    pub survival_died: bool,
    pub survival_respawned: bool,
    pub survival_round_over: bool,
    /// A computer player's mind (`ControlMethod = BOT`).
    pub brain: Option<Brain>,
}

/// Power-ups from bonus kits (`BONUS_*`).
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum Bonus {
    #[default]
    None,
    /// Flamer kit: invulnerable, flamer only.
    Flamegod,
    /// Nearly invisible.
    Predator,
    /// Quadruple damage.
    Berserker,
}

impl Bonus {
    /// The network's number for it (`Bonus as u8`).
    pub fn from_num(n: u8) -> Bonus {
        match n {
            1 => Bonus::Flamegod,
            2 => Bonus::Predator,
            3 => Bonus::Berserker,
            _ => Bonus::None,
        }
    }
}

/// Whether the server takes a bullet of this weapon from a bot's ammo.
/// `weapon` is the soldier's current weapon, `style` the bullet's.
pub(crate) fn pays_ammo(weapon: &Weapon, style: BulletStyle) -> bool {
    weapon.fire_interval > FIREINTERVAL_NET
        && !matches!(
            style,
            BulletStyle::FragGrenade | BulletStyle::ClusterGrenade | BulletStyle::Cluster
        )
}

/// [`Soldier::weapon_sel`] with every weapon.
pub const ALL_WEAPONS: u16 = (1 << 14) - 1;
/// The primaries' bits of [`Soldier::weapon_sel`].
pub const PRIMARIES: u16 = (1 << 10) - 1;

/// A weapon's bit in [`Soldier::weapon_sel`], if it's on the weapons menu.
pub fn weapon_bit(kind: WeaponKind) -> Option<u16> {
    let index = kind.index();
    (index < 14).then(|| 1 << index)
}

impl Soldier {
    /// The weapon may be picked (weapons off the menu always).
    pub fn may_use(&self, kind: WeaponKind) -> bool {
        weapon_bit(kind).is_none_or(|bit| self.weapon_sel & bit != 0)
    }
}

/// `CalculateBink`: adding bink has diminishing returns as more gets accumulated.
pub(crate) fn calculate_bink(accumulated: u16, bink: u16) -> u16 {
    let (a, b) = (f64::from(accumulated), f64::from(bink));
    let diminish = (a * (a / (10.0 * b + a))).round_ties_even() as i64;
    (i64::from(accumulated) + i64::from(bink) - diminish) as u16
}

/// What `TSprite.ThrowFlag` needs from the soldier at the moment it throws.
#[derive(Debug, Copy, Clone)]
pub struct FlagThrow {
    /// Hand position (skeleton point 15).
    pub hand: Vec2,
    pub aim: Vec2,
    pub velocity: Vec2,
    pub direction: i8,
}

/// `PARA_SPEED`: an untyped extended constant.
const PARA_SPEED: f64 = -0.5 * 0.06;

/// `PREDATORALPHA`
pub(crate) const PREDATOR_ALPHA: u8 = 5;

/// What polygon effects need besides the map (`HandleSpecialPolyTypes`).
pub struct PolyEnv<'a> {
    pub config: &'a WorldConfig,
    pub tick: u64,
    pub rng: &'a mut PascalRandom,
    pub emitter: &'a mut Vec<EmitterItem>,
}

impl Soldier {
    pub fn primary_weapon(&self) -> &Weapon {
        &self.weapons[self.active_weapon]
    }

    pub fn secondary_weapon(&self) -> &Weapon {
        &self.weapons[(self.active_weapon + 1) % 2]
    }

    /// Picks up guns lying around (bots only with a favourite weapon other than hands).
    pub fn takes_guns(&self) -> bool {
        self.brain
            .as_ref()
            .is_none_or(|b| b.fav_weapon != WeaponKind::NoWeapon)
    }

    pub fn tertiary_weapon(&self) -> &Weapon {
        &self.weapons[2]
    }

    pub fn switch_weapon(&mut self) {
        let w = (self.active_weapon + 1) % 2;
        self.active_weapon = w;
        self.weapons[w].start_up_time_count = self.weapons[w].start_up_time;
        self.weapons[w].reload_time_prev = self.weapons[w].reload_time_count;
        // burst_count = 0;
    }

    pub fn new(spawn: &MapSpawnpoint, data: &GameData, weapons: &WeaponTable) -> Soldier {
        let particle = Particle {
            active: true,
            pos: vec2(spawn.x as f32, spawn.y as f32),
            old_pos: vec2(spawn.x as f32, spawn.y as f32),
            one_over_mass: 1.0,
            timestep: 1.0,
            gravity: GRAV,
            e_damping: 0.99,
            ..Default::default()
        };

        let weapons = [
            weapons.get(WeaponKind::DesertEagles),
            weapons.get(WeaponKind::Chainsaw),
            weapons.get(WeaponKind::FragGrenade),
        ];

        Soldier {
            active: true,
            dead_meat: false,
            style: 0,
            num: 1,
            visible: 1,
            on_ground: false,
            on_ground_for_law: false,
            on_ground_last_frame: false,
            on_ground_permanent: false,
            // CreateSprite leaves it at 0 until the first update
            direction: 0,
            old_direction: 0,
            health: 150.0,
            alpha: 255,
            jets_count: 0,
            jets_count_prev: 0,
            wear_helmet: 1,
            has_cigar: 0,
            vest: 0.0,
            idle_time: DEFAULT_IDLETIME,
            idle_random: -1,
            position: POS_STAND,
            on_fire: 0,
            collider_distance: 255,
            half_dead: false,
            skeleton: data.soldier_skeleton.clone(),
            legs_animation: data.anims.state(Anim::Stand),
            body_animation: data.anims.state(Anim::Stand),
            anims: data.anims.clone(),
            control: Default::default(),
            active_weapon: 0,
            weapons,
            fired: 0,
            burst_count: 0,
            can_auto_reload_spas: true,
            auto_reload_when_can_fire: false,
            bullet_count: 0,
            remote: false,
            ceasefire_counter: -1,
            team: Team::None,
            next_push: Vec2::ZERO,
            respawn_counter: 0,
            dead_time: 0,
            dead_collide_count: 0,
            kills: 0,
            deaths: 0,
            grenade_can_throw: false,
            holded_thing: None,
            has_pack: false,
            grabs_per_second: 0,
            scores_per_second: 0,
            grabbed_in_base: false,
            pending_drop: None,
            release_things: false,
            head_cap: 1,
            local_player: false,
            hit_spray_counter: 0,
            recoil_kick: 0.0,
            bg: BackgroundState::default(),
            parachute: false,
            para: false,
            parachute_bend: None,
            parachute_call: false,
            parachute_spawn: None,
            release_parachute: false,
            holds_flag: false,
            flag_throw: None,
            respawn_held: None,
            flags: 0,
            ping: 0,
            flag_grab_cooldown: 0,
            multi_kill_time: 0,
            multi_kills: 0,
            bonus_style: Bonus::None,
            bonus_time: 0,
            loadout: [
                WeaponKind::DesertEagles,
                WeaponKind::Chainsaw,
                WeaponKind::FragGrenade,
            ],
            weapon_sel: ALL_WEAPONS,
            dropped_weapon: false,
            respawned: false,
            particle,
            name: "Major".to_string(),
            looks: Looks::default(),
            sounds: Vec::new(),
            sparks: Vec::new(),
            said: Vec::new(),
            idle_started: None,
            killer_said: Vec::new(),
            aim_dist_coef: DEFAULT_AIM_DIST,
            spin: false,
            melee_reach: false,
            stat: None,
            use_time: 0,
            stat_release: None,
            can_mercy: true,
            mercy_kill: false,
            survival_died: false,
            survival_respawned: false,
            survival_round_over: false,
            brain: None,
        }
    }

    pub fn legs_apply_animation(&mut self, id: Anim, frame: usize) {
        if !self.legs_animation.is_any(&[Anim::Prone, Anim::ProneMove])
            && self.legs_animation.id != id
        {
            self.legs_animation = self.anims.state(id);
            self.legs_animation.frame = frame;
        }
    }

    pub fn body_apply_animation(&mut self, id: Anim, frame: usize) {
        if self.body_animation.id != id {
            self.body_animation = self.anims.state(id);
            self.body_animation.frame = frame;
        }
    }

    /// `TSprite.DropWeapon`: lets go of the primary weapon; the world turns
    /// `pending_drop` into a thing.
    pub fn drop_weapon(&mut self, config: &WorldConfig) {
        self.dropped_weapon = true;
        let weapon = *self.primary_weapon();
        if let Some(kind) = ThingKind::for_weapon(weapon.kind) {
            self.pending_drop = Some(WeaponDrop {
                kind,
                ammo_count: weapon.ammo_count,
                pos: self.skeleton.pos(16),
                velocity: self.particle.velocity,
                aim: self.cursor_aim_direction(),
                dead: self.dead_meat,
            });
        }
        // TODO: Rambo mode drops the bow
        // ApplyWeaponByNum(NOWEAPON) even with empty hands: a fresh fire interval
        self.weapons[self.active_weapon] = config.weapons.get(WeaponKind::NoWeapon);
    }

    /// Lets go of the parachute (the world detaches the thing).
    fn drop_parachute(&mut self) {
        self.parachute = false;
        self.holded_thing = None;
        self.release_parachute = true;
    }

    /// Whether a point is outside the map area soldiers may be in (`CheckOutOfBounds`).
    fn out_of_bounds(&self, map: &MapFile, pos: Vec2) -> bool {
        let bound = (map.sectors_num * map.sectors_division - 50) as f32;
        pos.x.abs() > bound || pos.y.abs() > bound
    }

    /// Port of `TSprite.HandleSpecialPolyTypes` (server side: damage, lava flames,
    /// explosive polygons).
    pub fn handle_special_polytypes(&mut self, polytype: PolyType, pos: Vec2, env: &mut PolyEnv) {
        let velocity = self.particle.velocity;
        let hit = |soldier: &mut Soldier, amount: f32, env: &mut PolyEnv| {
            if let Some(how) = soldier.self_hit(amount, 12, velocity, env.config, env.rng) {
                env.emitter.push(EmitterItem::Died(how));
            }
        };

        match polytype {
            PolyType::Deadly => hit(self, 50.0 + self.health, env),
            PolyType::BloodyDeadly => hit(self, 450.0 + self.health, env),
            PolyType::Hurts | PolyType::Lava => {
                if !self.dead_meat {
                    // (the client rolls its own die for the moan)
                    if env.rng.below(10) == 0 {
                        self.health -= 5.0;
                        let sfx = if polytype == PolyType::Hurts {
                            Sfx::Arg
                        } else {
                            Sfx::Lava
                        };
                        self.play(sfx);
                    }
                    if self.health < 1.0 {
                        hit(self, 10.0, env);
                    }
                }

                if env.rng.below(3) == 0 && polytype == PolyType::Lava {
                    let a = vec2(pos.x, pos.y - 3.0);
                    self.spark(a, vec2(0.0, -1.3), 36, 40);
                    if env.rng.below(3) == 0 {
                        let flamer = env.config.weapons.get(WeaponKind::Flamer);
                        env.emitter.push(EmitterItem::Bullet(BulletParams {
                            style: flamer.bullet_style,
                            weapon: flamer.kind,
                            position: a,
                            velocity: -velocity,
                            timeout: flamer.timeout as i16,
                            hit_multiply: flamer.hit_multiply,
                            team: self.team,
                            sprite: flamer.bullet_sprite,
                            seed: None,
                            must_create: true,
                            net: false,
                            owner_immune: false,
                        }));
                    }
                }
            }
            PolyType::Regenerates => {
                if self.health < env.config.start_health() && env.tick.is_multiple_of(12) {
                    hit(self, -2.0, env);
                    self.play(Sfx::Regenerate);
                }
            }
            PolyType::Explosive => {
                if !self.dead_meat {
                    self.spark(vec2(pos.x, pos.y - 3.0), vec2(0.0, -1.3), 36, 40);
                    let m79 = env.config.weapons.get(WeaponKind::M79);
                    env.emitter.push(EmitterItem::Bullet(BulletParams {
                        style: m79.bullet_style,
                        weapon: m79.kind,
                        position: vec2(pos.x, pos.y - 3.0),
                        velocity: Vec2::ZERO,
                        timeout: m79.timeout as i16,
                        hit_multiply: m79.hit_multiply,
                        team: self.team,
                        sprite: m79.bullet_sprite,
                        seed: None,
                        must_create: true,
                        net: false,
                        owner_immune: false,
                    }));
                    hit(self, 4000.0, env);
                    self.health = -600.0;
                }
            }
            PolyType::HurtsFlaggers => {
                if !self.dead_meat && self.holds_flag && env.rng.below(10) == 0 {
                    self.health -= 10.0;
                    self.play(Sfx::Arg);
                }
                if self.health < 1.0 {
                    hit(self, 10.0, env);
                }
            }
            _ => {}
        }
    }

    pub fn update(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        tick: u64,
        rng: &mut PascalRandom,
        emitter: &mut Vec<EmitterItem>,
    ) {
        self.particle.euler();
        self.update_begin();
        self.update_rest(map, config, tick, rng, emitter);
    }

    /// The start of an update, before the controls are read (a bot thinks after it).
    /// The particle has been integrated already (the world integrates every soldier
    /// first, like `UpdateFrame`).
    pub fn update_begin(&mut self) {
        self.recoil_kick = 0.0;
        // spray counter (UpdateFrame on the client)
        if self.local_player {
            self.hit_spray_counter = self.hit_spray_counter.saturating_sub(1);
        }

        // knock-back from hits last tick (NextPush[0] on the server)
        self.particle.velocity += self.next_push;
        self.next_push = Vec2::ZERO;

        // reload the spas after the shooting delay is over
        let weapon = *self.primary_weapon();
        if self.auto_reload_when_can_fire
            && (weapon.kind != WeaponKind::Spas12 || weapon.fire_interval_count == 0)
        {
            self.auto_reload_when_can_fire = false;

            if weapon.kind == WeaponKind::Spas12
                && !self
                    .body_animation
                    .is_any(&[Anim::Roll, Anim::RollBack, Anim::Change])
                && weapon.ammo_count != weapon.ammo
            {
                self.body_apply_animation(Anim::Reload, 1);
            }
        }
    }

    /// The rest of `TSprite.Update`, from `ControlSprite` on.
    pub fn update_rest(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        tick: u64,
        rng: &mut PascalRandom,
        emitter: &mut Vec<EmitterItem>,
    ) {
        let mut body_y = 0.0;
        let mut arm_s;

        self.control(map, config, tick, rng, emitter);

        // spectators are forever dead
        if self.is_spectator() {
            self.dead_meat = true;
        }

        *self.skeleton.old_pos_mut(21) = self.skeleton.pos(21);
        *self.skeleton.old_pos_mut(23) = self.skeleton.pos(23);
        // *self.skeleton.old_pos_mut(25) = self.skeleton.pos(25);
        *self.skeleton.pos_mut(21) = self.skeleton.pos(9);
        *self.skeleton.pos_mut(23) = self.skeleton.pos(12);
        // *self.skeleton.pos_mut(25) = self.skeleton.pos(5);

        // Soldat also "adds" the velocity to 21, 23 and 25 here, but with Vec2Add, a function
        // whose result is discarded, so it has no effect. Point 25 doesn't exist in gostek.po.

        match self.position {
            POS_STAND => body_y = 8.0,
            POS_CROUCH => body_y = 9.0,
            POS_PRONE => {
                if self.body_animation.id == Anim::Prone {
                    if self.body_animation.frame > 9 {
                        body_y = -2.0
                    } else {
                        body_y = 14.0 - self.body_animation.frame as f32;
                    }
                } else {
                    body_y = 9.0;
                }

                if self.body_animation.id == Anim::ProneMove {
                    body_y = 0.0;
                }
            }
            _ => {}
        }

        if self.body_animation.id == Anim::GetUp {
            if self.body_animation.frame > 18 {
                body_y = 8.0;
            } else {
                body_y = 4.0;
            }
        }

        if self.flag_grab_cooldown > 0 {
            self.flag_grab_cooldown -= 1;
        }

        // reset the background poly test before collision checks on the corpse
        if self.dead_meat {
            self.bg.prepare();
        }

        if self.control.mouse_aim_x as f32 >= self.particle.pos.x {
            self.direction = 1;
        } else {
            self.direction = -1;
        }

        for i in 1..21 {
            if self.skeleton.active(i) && !self.dead_meat {
                *self.skeleton.old_pos_mut(i) = self.skeleton.pos(i);
                let (dir, p) = (f32::from(self.direction), self.particle.pos);

                if !self.half_dead && ((1..=6).contains(&i) || (i == 17) || (i == 18)) {
                    let anim_pos = self.legs_animation.pos(i);
                    *self.skeleton.pos_mut(i) = vec2(p.x + dir * anim_pos.x, p.y + anim_pos.y);
                }

                if (7..=16).contains(&i) || i == 19 || i == 20 {
                    let anim_pos = self.body_animation.pos(i);
                    // same summation order as Soldat, floats aren't associative
                    let y = if self.half_dead {
                        9.0 + p.y + anim_pos.y
                    } else {
                        (self.skeleton.pos(6).y - (p.y - body_y)) + p.y + anim_pos.y
                    };
                    *self.skeleton.pos_mut(i) = vec2(p.x + dir * anim_pos.x, y);
                }
            }
        }

        let aim = vec2(
            self.control.mouse_aim_x as f32,
            self.control.mouse_aim_y as f32,
        );

        if !self.dead_meat {
            let pos = self.skeleton.pos(9);
            let r_norm = 0.1 * vec2normalize(self.skeleton.pos(12) - aim);
            let dir = f32::from(self.direction);

            *self.skeleton.pos_mut(12) = pos + vec2(-dir * r_norm.y, dir * r_norm.x);
            *self.skeleton.pos_mut(23) = pos + vec2(-dir * r_norm.y, dir * r_norm.x) * 50.0;
        }

        let not_aiming_anims = [
            Anim::Reload,
            Anim::ReloadBow,
            Anim::ClipIn,
            Anim::ClipOut,
            Anim::SlideBack,
            Anim::Change,
            Anim::ThrowWeapon,
            Anim::WeaponNone,
            Anim::Punch,
            Anim::Roll,
            Anim::RollBack,
            Anim::Cigar,
            Anim::Match,
            Anim::Smoke,
            Anim::Wipe,
            Anim::TakeOff,
            Anim::Groin,
            Anim::Piss,
            Anim::Mercy,
            Anim::Mercy2,
            Anim::Victory,
            Anim::Own,
            Anim::Melee,
        ];

        if self.body_animation.id == Anim::Throw {
            arm_s = -5.00;
        } else {
            arm_s = -7.00;
        }

        if !self.dead_meat && !self.body_animation.is_any(&not_aiming_anims) {
            let r_norm = arm_s * vec2normalize(self.skeleton.pos(15) - aim);
            *self.skeleton.pos_mut(15) = self.skeleton.pos(16) + r_norm;
        }

        if self.body_animation.id == Anim::Throw {
            arm_s = -6.00;
        } else {
            arm_s = -8.00;
        }

        if !self.dead_meat && !self.body_animation.is_any(&not_aiming_anims) {
            let r_norm = arm_s * vec2normalize(self.skeleton.pos(19) - aim);
            *self.skeleton.pos_mut(19) = self.skeleton.pos(16) - vec2(0.0, 4.0) + r_norm;
        }

        // dead part
        for i in 1..21 {
            if self.dead_meat || self.half_dead {
                if (i < 17) && (i != 7) && (i != 8) {
                    let (x, y) = self.skeleton.pos(i).into();
                    self.on_ground = self.check_skeleton_map_collision(map, i, x, y);
                }
                self.corpse_effects(i, config);
            }
        }

        // no background poly contact on the corpse: reset the background status
        if self.dead_meat {
            self.bg.reset();
        }

        if !self.dead_meat {
            self.body_animation.do_animation();
            self.legs_animation.do_animation();

            // CheckOutOfBounds
            if !config.now.survival_end_round
                && !config.client
                && self.out_of_bounds(map, self.particle.pos)
            {
                self.respawn(map, config, rng);
            }

            self.on_ground = false;

            // reset the background poly test before collision checks
            self.bg.prepare();

            let (x, y) = self.particle.pos.into();
            self.check_map_collision(
                map,
                x - 3.5,
                y - 12.0,
                1,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            let (x, y) = self.particle.pos.into();
            self.check_map_collision(
                map,
                x + 3.5,
                y - 12.0,
                1,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            body_y = 0.0;
            arm_s = 0.0;

            // Walking either left or right (though only one can be active at once)
            if self.control.left ^ self.control.right {
                if self.control.left ^ (self.direction == 1) {
                    arm_s = 0.25;
                } else {
                    body_y = 0.25;
                }
            }
            // If a leg is inside a polygon, caused by the modification of ArmS and
            // BodyY, this is there to not lose contact to ground on slope polygons
            let (x, y) = self.particle.pos.into();

            if body_y == 0.0 {
                let leg = vec2(x + 2.0, y + 1.9);
                if map.ray_cast(leg, leg, 10.0, RayCast::default()).is_some() {
                    body_y = 0.25;
                }
            }
            if arm_s == 0.0 {
                let leg = vec2(x - 2.0, y + 1.9);
                if map.ray_cast(leg, leg, 10.0, RayCast::default()).is_some() {
                    arm_s = 0.25;
                }
            }

            let (x, y) = self.particle.pos.into();
            self.on_ground = self.check_map_collision(
                map,
                x + 2.0,
                y + 2.0 - body_y,
                0,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            // Legs collision check. If collided then don't check the other side, as a double
            // collision would result in too much of a ground repelling force (Pascal's `or`
            // short-circuits here).
            if !self.on_ground {
                let (x, y) = self.particle.pos.into();
                self.on_ground = self.check_map_collision(
                    map,
                    x - 2.0,
                    y + 2.0 - arm_s,
                    0,
                    &mut PolyEnv {
                        config,
                        tick,
                        rng,
                        emitter,
                    },
                );
            }

            let (x, y) = self.particle.pos.into();
            let grounded = self.on_ground;
            self.on_ground_for_law = self.check_radius_map_collision(
                map,
                x,
                y - 1.0,
                grounded,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            let (x, y) = self.particle.pos.into();
            let grounded = self.on_ground || self.on_ground_for_law;
            self.on_ground |= self.check_map_vertices_collision(
                map,
                x,
                y,
                3.0,
                grounded,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            if !(self.on_ground ^ self.on_ground_last_frame) {
                self.on_ground_permanent = self.on_ground;
            }

            self.on_ground_last_frame = self.on_ground;

            // no background poly contact: reset the background status
            self.bg.reset();

            self.weapon_handling();

            // chainsaw noise (with its smoke, while sparks are on: `r_maxsparks`)
            if self.primary_weapon().kind == WeaponKind::Chainsaw && self.stat.is_none() {
                let idle = self.primary_weapon().ammo_count == 0;
                if tick.is_multiple_of(15) {
                    let dir = f32::from(self.direction);
                    let a = self.skeleton.pos(9) + vec2(dir * 3.0, -2.0);
                    self.spark(a, vec2(0.0, -0.25), 1, 20);
                    if idle {
                        self.play_on(Sfx::ChainsawO, Channel::Gattling);
                    } else {
                        self.play(Sfx::ChainsawM);
                    }
                }
                if self.control.fire && !idle {
                    self.play_on(Sfx::ChainsawR, Channel::Gattling);
                }
            }

            // an empty LAW or chainsaw smokes, a flame arrow burns
            let kind = self.primary_weapon().kind;
            let dir = f32::from(self.direction);
            if matches!(kind, WeaponKind::LAW | WeaponKind::Chainsaw)
                && self.primary_weapon().ammo_count == 0
                && many_sparks(config)
                && fx::random(4) == 0
            {
                let p = self.skeleton.pos(9);
                let a = vec2(
                    p.x + dir * 3.0 - 8.0 + fx::random(80) as f32 / 10.0,
                    p.y - 2.0 - 1.0 + fx::random(60) as f32 / 10.0,
                );
                self.spark(a, vec2(0.0, -0.3), 1, 20);
            }
            if kind == WeaponKind::FlameBow && fx::random(10) == 0 {
                let a = self.skeleton.pos(15) + vec2(dir * 6.0, -5.0);
                self.spark(a, vec2(0.0, -0.5), 36, 40);
            }

            // JETS
            if (self.jets_count < map.start_jet)
                && !(self.control.jets)
                && (self.on_ground || tick.is_multiple_of(2))
            {
                self.jets_count += 1;
            }

            if self.ceasefire_counter > -1 {
                self.ceasefire_counter -= 1;
                // spawn protection blinks
                let blink = 100.0 + 70.0 * ext(config.now.sinus_counter).sin();
                self.alpha = blink.abs().round_ties_even() as u8;
            } else {
                self.alpha = 255;
            }

            if self.bonus_style == Bonus::Predator {
                self.alpha = PREDATOR_ALPHA;
            }

            self.bleed(config);

            // bonus time
            if self.bonus_time > -1 {
                self.bonus_time -= 1;
                if self.bonus_time < 1 {
                    if self.bonus_style == Bonus::Predator {
                        self.alpha = 255;
                    }
                    self.bonus_style = Bonus::None;
                }
            } else {
                self.bonus_style = Bonus::None;
            }

            // multikill timer
            if self.multi_kill_time > -1 {
                self.multi_kill_time -= 1;
            } else {
                self.multi_kills = 0;
            }

            // gain health from the bow
            if tick.is_multiple_of(3)
                && self
                    .primary_weapon()
                    .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
                && self.health < config.start_health()
            {
                self.health += 1.0;
            }

            // a puff of the cigar now and then
            let dir = f32::from(self.direction);
            let mouth = self.skeleton.pos(9) - vec2(0.0, 2.0);
            if self.has_cigar == 10 && tick.is_multiple_of(160) {
                self.spark(mouth + vec2(dir * 4.0, 0.0), vec2(0.0, -0.75), 31, 55);
                if fx::random(2) == 0 {
                    self.spark(mouth + vec2(dir * 4.1, 0.0), vec2(0.0, -0.69), 31, 55);
                    self.play(Sfx::Smoke);
                    if fx::random(2) == 0 {
                        self.spark(mouth + vec2(dir * 3.9, 0.0), vec2(0.0, -0.81), 31, 55);
                    }
                }
            }
            // winter breath
            if map.weather == 3 && many_sparks(config) && tick.is_multiple_of(160) {
                self.spark(mouth + vec2(dir * 4.0, 0.0), vec2(0.0, -0.75), 31, 55);
            }

            // parachuter
            self.para = self.parachute;
            if self.para {
                self.particle.force.y = fpc(PARA_SPEED);
                if self.ceasefire_counter < 1 && (self.on_ground || self.control.jets) {
                    self.drop_parachute();
                }
            }

            self.skeleton.do_verlet_timestep_for(22, 29);
            self.skeleton.do_verlet_timestep_for(24, 30);
        }

        if self.dead_meat && !self.is_spectator() {
            // physically integrate skeleton particles
            self.skeleton.do_verlet_timestep();
            self.particle.pos = self.skeleton.pos(12);

            // CheckSkeletonOutOfBounds
            if !config.now.survival_end_round
                && !config.client
                && (1..=20).any(|i| self.out_of_bounds(map, self.skeleton.pos(i)))
            {
                self.respawn(map, config, rng);
            }

            // respawn countdown (a client waits for the server's respawn)
            if self.respawn_counter < 1 && !config.client {
                self.respawn(map, config, rng);
            }
            self.respawn_counter -= 1;

            // survival: nobody respawns until the round is over, then the survivors die
            // (the world kills them) and everyone comes back together
            if config.survival_mode && self.respawn_counter == 1 {
                if !config.now.survival_end_round {
                    self.respawn_counter += 2;
                } else {
                    self.survival_round_over = true;
                }
            }

            // parachuter
            self.para = self.parachute;
            if self.para {
                self.skeleton.force_mut(12).y = fpc(25.0 * PARA_SPEED);
                if self.on_ground {
                    self.drop_parachute();
                }
            }

            self.dead_time += 1;
        }

        // safety (Sprites.pas clamps both axes symmetrically)
        self.particle.velocity = self
            .particle
            .velocity
            .clamp(Vec2::splat(-MAX_VELOCITY), Vec2::splat(MAX_VELOCITY));
    }

    /// Berserkers bleed, flame gods burn and the badly hurt bleed (client sparks).
    fn bleed(&mut self, config: &WorldConfig) {
        let few = few_sparks(config);
        let hand = self.skeleton.pos(19);
        let around = hand + vec2(-5.0 + fx::random(11) as f32, -5.0 + fx::random(11) as f32);
        let moved = hand - self.skeleton.old_pos(19) - vec2(0.0, 1.38);
        let rnd = if few {
            2 * BLOOD_RANDOM_HIGH
        } else {
            BLOOD_RANDOM_HIGH
        };
        match self.bonus_style {
            Bonus::Berserker if fx::random(rnd) == 0 => {
                self.spark(around, moved, 5, 55 - fx::random(20));
            }
            Bonus::Flamegod if fx::random(rnd) == 0 => {
                self.spark(around, moved, 36, 40 - fx::random(10));
            }
            _ => {}
        }
        if self.health < HURT_HEALTH {
            let rnd = if few {
                2 * BLOOD_RANDOM_NORMAL
            } else {
                BLOOD_RANDOM_NORMAL
            };
            if fx::random(rnd) == 0 {
                let a = self.skeleton.pos(5) + vec2(2.0, 0.0);
                let b = self.skeleton.pos(5) - self.skeleton.old_pos(5);
                self.spark(a, b, 4, 65 - fx::random(10));
            }
        }
    }

    /// A corpse bleeds where it was torn apart, and burns (client sparks of `Update`).
    fn corpse_effects(&mut self, i: usize, config: &WorldConfig) {
        let count = config.now.sparks_count;
        let few = few_sparks(config);
        let pos = self.skeleton.pos(i);
        let moved = pos - self.skeleton.old_pos(i);

        let cut = self
            .skeleton
            .constraints()
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.active && (c.particle_num.0 == i || c.particle_num.1 == i))
            .map(|(k, _)| k + 1)
            .collect::<Vec<_>>();
        for k in cut {
            let mut rnd = if count > 300 {
                22
            } else if count > 50 {
                10
            } else {
                6
            };
            if self.dead_time > LESSBLEED_TIME {
                rnd *= 2;
            }
            if self.dead_time > NOBLEED_TIME {
                rnd *= 100;
            }
            if few {
                rnd *= 2;
            }
            if k != 10 && k != 11 {
                let (a, b) = (pos + vec2(0.0, 2.0), moved * 0.35);
                if fx::random(rnd) == 0 {
                    self.spark(a, b, 5, 85 - fx::random(25));
                } else if fx::random(rnd / 3) == 0 {
                    self.spark(a, b, 4, 85 - fx::random(25));
                }
            }
        }

        if self.dead_time < ONFIRE_TIME
            && self.on_fire > 0
            && i.is_multiple_of(usize::from(self.on_fire))
        {
            let mut rnd = 50;
            if count > 170 {
                rnd = 70;
            }
            if count < 17 {
                rnd = 30;
            }
            if few {
                rnd *= 2;
            }
            let (a, b) = (pos + vec2(0.0, 3.0), moved * 0.3);
            if fx::random(rnd) == 0 {
                self.spark(a, b, 36, 35);
                self.play_maybe(Sfx::Onfire, 8);
                self.play_maybe(Sfx::Firecrack, 2);
            } else if fx::random(rnd / 3) == 0 {
                self.spark(a, b, 37, 75);
            }
        }
    }

    /// Footsteps on the ground and the dust they raise (client side of
    /// `CheckMapCollision`).
    fn step_effects(&mut self, map: &MapFile, pos: Vec2, config: &WorldConfig) {
        let legs = &self.legs_animation;
        let (frame, count) = (legs.frame, legs.count);
        let running = legs.is_any(&[Anim::Run, Anim::RunBack]);
        let v = self.particle.velocity;
        let dust = |s: &mut Soldier, y: f32| {
            let b = vec2(v.x / 4.0, y) * (0.4 + fx::random(4) as f32 / 10.0);
            s.spark(pos, b, 1, 70);
        };
        if running && (frame == 16 || frame == 32) && many_sparks(config) {
            if v.x.abs() > 1.0 {
                dust(self, -0.8);
            }
            let backwards =
                (self.direction == 1 && v.x < 0.01) || (self.direction == -1 && v.x > 0.01);
            if backwards && self.legs_animation.id == Anim::Run {
                dust(self, -1.3);
            }
        }
        if v.x.abs() > 2.4 && !running && fx::random(4) == 0 {
            dust(self, -0.9);
        }

        let legs = &self.legs_animation;
        if legs.is_any(&[Anim::Run, Anim::RunBack]) && (frame == 16 || frame == 32) {
            match map.steps {
                0 => self.play_random(Sfx::Step, 4),
                1 => self.play_random(Sfx::Step5, 4),
                _ => {}
            }
            if map.weather == 1 {
                self.play(Sfx::WaterStep);
            }
        } else if legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
            && (frame == 15 || frame == 1)
            && count == 1
        {
            // one or the other or none, half, a quarter and a quarter of the time (here
            // rolled apart)
            self.play_maybe(Sfx::CrouchMove, 2);
            self.play_maybe(Sfx::CrouchMovel, 4);
        } else if legs.id == Anim::ProneMove && frame == 8 && count == 1 {
            self.play(Sfx::ProneMove);
        }
    }

    pub fn check_map_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        area: i32,
        env: &mut PolyEnv,
    ) -> bool {
        let pos = vec2(x, y) + self.particle.velocity;
        let rx = ((pos.x / map.sectors_division as f32).round_ties_even()) as i32 + 25;
        let ry = ((pos.y / map.sectors_division as f32).round_ties_even()) as i32 + 25;

        if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
            self.bg.big_poly_center(map, pos);

            for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                let polytype = map.polygons[poly].polytype;

                if soldier_collides(polytype, self.team, self.holded_thing.is_some()) {
                    let polygons = map.polygons[poly];
                    if map.point_in_poly(pos, &polygons) {
                        if self.bg.test(map, poly) {
                            continue;
                        }

                        self.handle_special_polytypes(polytype, pos, env);

                        let fall = self.particle.velocity.y.abs();
                        if fall > 2.2 && fall < 3.4 && polytype != PolyType::Bouncy {
                            self.play(Sfx::Fall);
                        }
                        if fall > 3.5 {
                            self.play(Sfx::FallHard);
                        }

                        // hit the ground hard
                        if env.config.realistic_mode
                            && self.particle.velocity.y > 3.5
                            && polytype != PolyType::Bouncy
                        {
                            let amount = self.particle.velocity.y * 5.0;
                            if let Some(how) =
                                self.self_hit(amount, 12, vec2(x, y), env.config, env.rng)
                            {
                                env.emitter.push(EmitterItem::Died(how));
                            }
                            self.play(Sfx::Fall);
                        }

                        self.step_effects(map, pos, env.config);

                        let mut dist = 0.0;
                        let mut k = 0;

                        let mut perp =
                            map.closest_perpendicular(poly as i32, pos, &mut dist, &mut k);

                        let step = perp;

                        perp = vec2normalize(perp);
                        perp *= dist;
                        dist = vec2length(self.particle.velocity);

                        if vec2length(perp) > dist {
                            perp = vec2normalize(perp);
                            perp *= dist;
                        }
                        if (area == 0)
                            || ((area == 1)
                                && ((self.particle.velocity.y < 0.0)
                                    || (ext(self.particle.velocity.x) > SLIDELIMIT)
                                    || (ext(self.particle.velocity.x) < -SLIDELIMIT)))
                        {
                            self.particle.old_pos = self.particle.pos;
                            self.particle.pos -= perp;
                            if map.polygons[poly].polytype == PolyType::Bouncy {
                                perp = vec2normalize(perp);
                                perp *= map.polygons[poly].bounciness * dist;
                                if vec2length(perp) > 1.0 {
                                    self.play(Sfx::Bounce);
                                }
                            }
                            self.particle.velocity -= perp;
                        }

                        if area == 0 {
                            if (self.legs_animation.id == Anim::Stand)
                                || (self.legs_animation.id == Anim::Crouch)
                                || (self.legs_animation.id == Anim::Prone)
                                || (self.legs_animation.id == Anim::ProneMove)
                                || (self.legs_animation.id == Anim::GetUp)
                                || (self.legs_animation.id == Anim::Fall)
                                || (self.legs_animation.id == Anim::Mercy)
                                || (self.legs_animation.id == Anim::Mercy2)
                                || (self.legs_animation.id == Anim::Own)
                            {
                                if (ext(self.particle.velocity.x) < SLIDELIMIT)
                                    && (ext(self.particle.velocity.x) > -SLIDELIMIT)
                                    && (ext(step.y) > SLIDELIMIT)
                                {
                                    self.particle.pos = self.particle.old_pos;
                                    self.particle.force.y -= GRAV;
                                } else if many_sparks(env.config) && fx::random(15) == 0 {
                                    // dust when sliding
                                    let v = self.particle.velocity;
                                    let b = vec2(v.x * 3.0, -v.y * 2.0)
                                        * (0.4 + fx::random(4) as f32 / 10.0);
                                    self.spark(pos, b, 1, 70);
                                }

                                if (ext(step.y) > SLIDELIMIT)
                                    && (polytype != PolyType::Ice)
                                    && (polytype != PolyType::Bouncy)
                                {
                                    if (self.legs_animation.id == Anim::Stand)
                                        || (self.legs_animation.id == Anim::Fall)
                                        || (self.legs_animation.id == Anim::Crouch)
                                    {
                                        self.particle.velocity.x =
                                            fpc(ext(self.particle.velocity.x) * STANDSURFACECOEFX);
                                        self.particle.velocity.y =
                                            fpc(ext(self.particle.velocity.y) * STANDSURFACECOEFY);

                                        self.particle.force.x -= self.particle.velocity.x;
                                    } else if self.legs_animation.id == Anim::Prone {
                                        if self.legs_animation.frame > 24 {
                                            if !(self.control.down
                                                && (self.control.left || self.control.right))
                                            {
                                                self.particle.velocity.x =
                                                    fpc(ext(self.particle.velocity.x)
                                                        * STANDSURFACECOEFX);
                                                self.particle.velocity.y =
                                                    fpc(ext(self.particle.velocity.y)
                                                        * STANDSURFACECOEFY);

                                                self.particle.force.x -= self.particle.velocity.x;
                                            }
                                        } else {
                                            self.particle.velocity.x =
                                                fpc(ext(self.particle.velocity.x) * SURFACECOEFX);
                                            self.particle.velocity.y =
                                                fpc(ext(self.particle.velocity.y) * SURFACECOEFY);
                                        }
                                    } else if self.legs_animation.id == Anim::GetUp {
                                        self.particle.velocity.x =
                                            fpc(ext(self.particle.velocity.x) * SURFACECOEFX);
                                        self.particle.velocity.y =
                                            fpc(ext(self.particle.velocity.y) * SURFACECOEFY);
                                    } else if self.legs_animation.id == Anim::ProneMove {
                                        self.particle.velocity.x =
                                            fpc(ext(self.particle.velocity.x) * STANDSURFACECOEFX);
                                        self.particle.velocity.y =
                                            fpc(ext(self.particle.velocity.y) * STANDSURFACECOEFY);
                                    }
                                }
                            } else if (self.legs_animation.id == Anim::CrouchRun)
                                || (self.legs_animation.id == Anim::CrouchRunBack)
                            {
                                self.particle.velocity.x =
                                    fpc(ext(self.particle.velocity.x) * CROUCHMOVESURFACECOEFX);
                                self.particle.velocity.y =
                                    fpc(ext(self.particle.velocity.y) * CROUCHMOVESURFACECOEFY);
                            } else {
                                self.particle.velocity.x =
                                    fpc(ext(self.particle.velocity.x) * SURFACECOEFX);
                                self.particle.velocity.y =
                                    fpc(ext(self.particle.velocity.y) * SURFACECOEFY);
                            }
                        }

                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn check_map_vertices_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        r: f32,
        has_collided: bool,
        env: &mut PolyEnv,
    ) -> bool {
        let pos = vec2(x, y);
        let rx = ((pos.x / map.sectors_division as f32).round_ties_even()) as i32 + 25;
        let ry = ((pos.y / map.sectors_division as f32).round_ties_even()) as i32 + 25;

        if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
            for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                let polytype = map.polygons[poly].polytype;

                if soldier_collides(polytype, self.team, self.holded_thing.is_some()) {
                    for i in 0..3 {
                        let vert = vec2(
                            map.polygons[poly].vertices[i].x,
                            map.polygons[poly].vertices[i].y,
                        );

                        let dist = distance(vert, pos);
                        if dist < r {
                            if self.bg.test(map, poly) {
                                continue;
                            }

                            if !has_collided {
                                self.handle_special_polytypes(polytype, pos, env);
                            }
                            let mut dir = pos - vert;
                            dir = vec2normalize(dir);
                            self.particle.pos += dir;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn check_radius_map_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        has_collided: bool,
        env: &mut PolyEnv,
    ) -> bool {
        let mut s_pos = vec2(x, y - 3.0);

        let mut det_acc = vec2length(self.particle.velocity).trunc() as i32;
        if det_acc == 0 {
            det_acc = 1;
        }

        let step = self.particle.velocity * (1.0 / det_acc as f32);

        for _z in 0..det_acc {
            s_pos += step;

            let rx = ((s_pos.x / map.sectors_division as f32).round_ties_even()) as i32 + 25;
            let ry = ((s_pos.y / map.sectors_division as f32).round_ties_even()) as i32 + 25;

            if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
                for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                    let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                    let polytype = map.polygons[poly].polytype;

                    if soldier_radius_collides(polytype, self.team, self.holded_thing.is_some()) {
                        for k in 0..3 {
                            let mut norm = map.perps[poly][k];
                            norm *= -SOLDIER_COL_RADIUS;

                            let pos = s_pos + norm;

                            if map.point_in_poly_edges(pos.x, pos.y, poly as i32) {
                                if self.bg.test(map, poly) {
                                    continue;
                                }

                                if !has_collided {
                                    self.handle_special_polytypes(polytype, pos, env);
                                }
                                let mut d = 0.0;
                                let mut b = 0;
                                let mut perp =
                                    map.closest_perpendicular(poly as i32, s_pos, &mut d, &mut b);

                                let mut p1 = vec2(0.0, 0.0);
                                let mut p2 = vec2(0.0, 0.0);
                                match b {
                                    1 => {
                                        p1 = vec2(
                                            map.polygons[poly].vertices[0].x,
                                            map.polygons[poly].vertices[0].y,
                                        );
                                        p2 = vec2(
                                            map.polygons[poly].vertices[1].x,
                                            map.polygons[poly].vertices[1].y,
                                        );
                                    }
                                    2 => {
                                        p1 = vec2(
                                            map.polygons[poly].vertices[1].x,
                                            map.polygons[poly].vertices[1].y,
                                        );
                                        p2 = vec2(
                                            map.polygons[poly].vertices[2].x,
                                            map.polygons[poly].vertices[2].y,
                                        );
                                    }
                                    3 => {
                                        p1 = vec2(
                                            map.polygons[poly].vertices[2].x,
                                            map.polygons[poly].vertices[2].y,
                                        );
                                        p2 = vec2(
                                            map.polygons[poly].vertices[0].x,
                                            map.polygons[poly].vertices[0].y,
                                        );
                                    }
                                    _ => {}
                                }

                                let p3 = pos;
                                let d = point_line_distance(p1, p2, p3);
                                perp *= d;

                                self.particle.pos = self.particle.old_pos;
                                self.particle.velocity = self.particle.force - perp;

                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    pub fn check_skeleton_map_collision(
        &mut self,
        map: &MapFile,
        i: usize,
        x: f32,
        y: f32,
    ) -> bool {
        let mut result = false;
        let pos = vec2(x - 1.0, y + 4.0);
        let rx = ((pos.x / map.sectors_division as f32).round_ties_even()) as i32 + 25;
        let ry = ((pos.y / map.sectors_division as f32).round_ties_even()) as i32 + 25;

        if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
            self.bg.big_poly_center(map, pos);

            for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] - 1;
                let polytype = map.polygons[poly as usize].polytype;

                if soldier_collides(polytype, self.team, self.holded_thing.is_some())
                    && map.point_in_poly_edges(pos.x, pos.y, i32::from(poly))
                {
                    if self.bg.test(map, poly as usize) {
                        continue;
                    }

                    let mut dist = 0.0;
                    let mut b = 0;
                    let mut perp =
                        map.closest_perpendicular(i32::from(poly), pos, &mut dist, &mut b);
                    perp = vec2normalize(perp);
                    perp *= dist;

                    *self.skeleton.pos_mut(i) = self.skeleton.old_pos(i) - perp;

                    let pos = self.skeleton.pos(i);
                    let fall = (pos.y - self.skeleton.old_pos(i).y).abs();
                    if fall > 0.8 && self.dead_collide_count < 13 {
                        self.play_sound(Sound::new(Sfx::Bodyfall).at(pos));
                    }
                    if fall > 2.1 && self.dead_collide_count < 4 {
                        self.play_sound(Sound::new(Sfx::Bonecrack).at(pos));
                    }
                    self.dead_collide_count += 1;
                    result = true;
                }
            }
        }

        if result {
            let pos = vec2(x, y + 1.0);
            let rx = ((pos.x / map.sectors_division as f32).round_ties_even()) as i32 + 25;
            let ry = ((pos.y / map.sectors_division as f32).round_ties_even()) as i32 + 25;

            if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
                self.bg.big_poly_center(map, pos);

                for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                    let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] - 1;
                    let polytype = map.polygons[poly as usize].polytype;

                    if !matches!(polytype, PolyType::NoCollide | PolyType::OnlyBulletsCollide)
                        && map.point_in_poly_edges(pos.x, pos.y, i32::from(poly))
                    {
                        if self.bg.test(map, poly as usize) {
                            continue;
                        }

                        let mut dist = 0.0;
                        let mut b = 0;
                        let mut perp =
                            map.closest_perpendicular(i32::from(poly), pos, &mut dist, &mut b);
                        perp = vec2normalize(perp);
                        perp *= dist;

                        *self.skeleton.pos_mut(i) = self.skeleton.old_pos(i) - perp;
                        result = true;
                    }
                }
            }
        }

        result
    }

    /// Fire interval countdown and reloading (`TSprite.Update`, "WEAPON HANDLING"),
    /// as run by the owning client.
    fn weapon_handling(&mut self) {
        let index = self.active_weapon;
        let kind = self.weapons[index].kind;
        let spas = kind == WeaponKind::Spas12;

        {
            let w = &mut self.weapons[index];
            if w.fire_interval_count > 0 && (w.ammo_count > 0 || spas) {
                w.fire_interval_prev = w.fire_interval_count;
                w.fire_interval_count -= 1;
            }
        }

        // If fire button is released, then the reload can begin
        if !self.control.fire {
            self.can_auto_reload_spas = true;
        }

        let busy = [
            Anim::Roll,
            Anim::RollBack,
            Anim::Melee,
            Anim::Change,
            Anim::Throw,
            Anim::ThrowWeapon,
        ];

        // reload
        if self.weapons[index].ammo_count == 0
            && (kind == WeaponKind::Chainsaw || !self.body_animation.is_any(&busy))
        {
            self.pause_sound(Channel::Reload, false);

            if self.body_animation.id != Anim::GetUp {
                // spas does the fire interval delay and then reloads, other weapons the opposite
                if spas {
                    if self.weapons[index].fire_interval_count == 0 && self.can_auto_reload_spas {
                        self.body_apply_animation(Anim::Reload, 1);
                    }
                } else if kind == WeaponKind::Bow || kind == WeaponKind::FlameBow {
                    self.body_apply_animation(Anim::ReloadBow, 1);
                } else if !self.body_animation.is_any(&[Anim::ClipIn, Anim::SlideBack])
                    && (kind != WeaponKind::Chainsaw || !self.body_animation.is_any(&busy))
                {
                    self.body_apply_animation(Anim::ClipOut, 1);
                }

                self.burst_count = 0;
            }

            let w = &self.weapons[index];
            if w.reload_time_count == w.reload_time {
                let sound = match kind {
                    WeaponKind::DesertEagles => Some(Sfx::DeserteagleReload),
                    WeaponKind::MP5 => Some(Sfx::Mp5Reload),
                    WeaponKind::Ak74 => Some(Sfx::Ak74Reload),
                    WeaponKind::SteyrAUG => Some(Sfx::SteyraugReload),
                    WeaponKind::Ruger77 => Some(Sfx::Ruger77Reload),
                    WeaponKind::M79 => Some(Sfx::M79Reload),
                    WeaponKind::Barrett => Some(Sfx::Barretm82Reload),
                    WeaponKind::Minimi => Some(Sfx::M249Reload),
                    WeaponKind::Minigun => Some(Sfx::MinigunReload),
                    WeaponKind::USSOCOM => Some(Sfx::Colt1911Reload),
                    _ => None,
                };
                if let Some(sfx) = sound {
                    self.play_on(sfx, Channel::Reload);
                }
            }
            // the clip falls out
            let w = &self.weapons[index];
            if w.reload_time_count == w.clip_out_time {
                let hand = self.skeleton.pos(15);
                let v = self.particle.velocity;
                let (a, b) = (hand + vec2(0.0, 6.0), v - vec2(0.0, 0.001));
                let style = match kind {
                    WeaponKind::DesertEagles => {
                        self.spark(a, b, 18, 255);
                        let a = hand + vec2(-2.0, 7.0);
                        self.spark(a, v + vec2(0.3, -0.003), 18, 255);
                        None
                    }
                    WeaponKind::MP5 => Some(11),
                    WeaponKind::Ak74 => Some(9),
                    WeaponKind::SteyrAUG => Some(19),
                    WeaponKind::Barrett => Some(20),
                    WeaponKind::Minimi => Some(10),
                    WeaponKind::USSOCOM => Some(23),
                    _ => None,
                };
                if let Some(style) = style {
                    self.spark(a, b, style, 255);
                }
            }

            if !spas {
                let w = &mut self.weapons[index];
                w.reload_time_prev = w.reload_time_count;
                w.reload_time_count = w.reload_time_count.saturating_sub(1);
                w.fire_interval_prev = w.fire_interval;
                w.fire_interval_count = w.fire_interval;

                if w.reload_time_count < 1 {
                    w.refill();
                }
            }
        }

        // weapon jam fix
        let w = &mut self.weapons[index];
        if w.ammo_count == 0 && !spas {
            if w.reload_time_count < 1 {
                w.refill();
            }
            if w.reload_time_count > w.reload_time {
                w.reload_time_prev = w.reload_time;
                w.reload_time_count = w.reload_time;
            }
        }
    }

    /// `TSprite.GetCursorAimDirection`
    pub fn cursor_aim_direction(&self) -> Vec2 {
        let mouse_aim = vec2(
            self.control.mouse_aim_x as f32,
            self.control.mouse_aim_y as f32,
        );
        vec2normalize(mouse_aim - self.skeleton.pos(15))
    }

    /// `TSprite.GetHandsAimDirection`
    pub fn hands_aim_direction(&self) -> Vec2 {
        vec2normalize(self.skeleton.pos(15) - self.skeleton.pos(16))
    }

    /// `TSprite.GetMoveacc`: extra inaccuracy from moving.
    pub fn moveacc(&self) -> f32 {
        // no moveacc for bots on harder difficulties
        let hard_bot = self.brain.as_ref().is_some_and(|b| b.difficulty < 50);
        let moveacc = iif!(hard_bot, 0.0, self.primary_weapon().movement_acc);
        let legs = &self.legs_animation;

        if moveacc <= 0.0 {
            return 0.0;
        }

        if (self.control.jets && self.jets_count > 0)
            || legs.is_any(&[
                Anim::Jump,
                Anim::JumpSide,
                Anim::Run,
                Anim::RunBack,
                Anim::Roll,
                Anim::RollBack,
            ])
        {
            moveacc * 7.0
        } else if (!self.on_ground_permanent
            && !legs.is_any(&[
                Anim::Prone,
                Anim::ProneMove,
                Anim::Crouch,
                Anim::CrouchRun,
                Anim::CrouchRunBack,
            ]))
            || legs.id == Anim::GetUp
            || (legs.id == Anim::Prone && legs.frame < legs.num_frames())
        {
            moveacc * 3.0
        } else {
            0.0
        }
    }

    /// Port of `TSprite.Fire`.
    pub fn fire(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        rng: &mut PascalRandom,
        emitter: &mut Vec<EmitterItem>,
    ) {
        let weapon = *self.primary_weapon();
        let kind = weapon.kind;
        let team = self.team;
        let mercy = self.body_animation.is_any(&[Anim::Mercy, Anim::Mercy2]);
        let legs = self.legs_animation.clone();

        // Create a normalized directional vector
        let aim_direction = if weapon.bullet_style == BulletStyle::Blade || mercy {
            self.hands_aim_direction()
        } else {
            self.cursor_aim_direction()
        };

        let mut b = aim_direction;
        let mut a = vec2(
            self.skeleton.pos(15).x - b.x * 4.0,
            self.skeleton.pos(15).y - b.y * 4.0 - 2.0,
        );

        // bink and self-bink
        let mut inaccuracy = 0.0;
        if self.local_player && self.hit_spray_counter > 0 {
            inaccuracy = fpc(f64::from(self.hit_spray_counter) * 0.01);
        }
        inaccuracy += self.moveacc();

        // Bullet spread
        if kind != WeaponKind::DesertEagles
            && kind != WeaponKind::Spas12
            && weapon.bullet_style != BulletStyle::GaugeBullet
            && weapon.bullet_spread > 0.0
        {
            if legs.id == Anim::ProneMove || (legs.id == Anim::Prone && legs.frame > 23) {
                inaccuracy += weapon.bullet_spread / 1.625;
            } else if legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
                || (legs.id == Anim::Crouch && legs.frame > 13)
            {
                // untyped constant: the sum is evaluated in extended precision
                inaccuracy = fpc(ext(inaccuracy) + ext(weapon.bullet_spread) / 1.3);
            } else {
                inaccuracy += weapon.bullet_spread;
            }
        }

        inaccuracy = f32::min(inaccuracy * 0.25, MAX_INACCURACY);

        // Maximum deviation between 0 and MAX_INACCURACY, scaled like sin(0..pi/2)
        let max_deviation = MAX_INACCURACY * f32::sin((inaccuracy / MAX_INACCURACY) * (PI / 2.0));
        let d = vec2(
            random_spread(rng, max_deviation, 0.0),
            random_spread(rng, max_deviation, 0.0),
        );

        // Add inaccuracies to the direction, re-normalize and scale by speed
        b = vec2normalize(b + d) * weapon.speed;

        // Add some of the player's velocity to the bullet
        b += self.particle.velocity * weapon.inherited_velocity;

        // Head inside a polygon: offset the origin downward slightly
        if map.collision_test(a, false).is_some() {
            a.y += 2.5;
        }

        // `bn`: the bullet the mercy shot marks
        let mut bn: Option<usize> = None;
        let mut create = |position: Vec2, velocity: Vec2, seed: Option<u16>, net: bool| {
            emitter.push(EmitterItem::Bullet(BulletParams {
                style: weapon.bullet_style,
                weapon: kind,
                position,
                velocity,
                timeout: weapon.timeout as i16,
                hit_multiply: weapon.hit_multiply,
                team,
                sprite: weapon.bullet_sprite,
                seed,
                must_create: false,
                net,
                owner_immune: false,
            }));
            emitter.len() - 1
        };

        let plain = !matches!(
            kind,
            WeaponKind::DesertEagles
                | WeaponKind::Spas12
                | WeaponKind::Flamer
                | WeaponKind::NoWeapon
                | WeaponKind::Knife
                | WeaponKind::Chainsaw
                | WeaponKind::LAW
        );

        if plain || mercy {
            // CreateBullet takes the next BulletCount right away; it matters when the
            // Eagles or the shotgun reseed from it next (only the mercy shot does both)
            let seed = if plain {
                None
            } else {
                self.bullet_count = self.bullet_count.checked_add(1).unwrap_or(0);
                Some(self.bullet_count)
            };
            bn = Some(create(a, b, seed, true));
        }

        if kind == WeaponKind::DesertEagles {
            self.bullet_count = self.bullet_count.wrapping_add(1);
            let saved_seed = rng.rand_seed;
            rng.rand_seed = u32::from(self.bullet_count);

            let d1 = vec2(
                random_spread(rng, weapon.bullet_spread, b.x),
                random_spread(rng, weapon.bullet_spread, b.y),
            );
            bn = Some(create(a, d1, Some(self.bullet_count), true));

            let d2 = vec2(
                random_spread(rng, weapon.bullet_spread, b.x),
                random_spread(rng, weapon.bullet_spread, b.y),
            );
            rng.rand_seed = saved_seed;

            let b_norm = vec2normalize(b);
            a.x -= pascal_sign(b.x) * b_norm.y.abs() * 3.0;
            a.y += pascal_sign(b.y) * b_norm.x.abs() * 3.0;
            let _ = create(a, d2, None, false);
        }

        if weapon.bullet_style == BulletStyle::GaugeBullet {
            self.bullet_count = self.bullet_count.wrapping_add(1);
            let saved_seed = rng.rand_seed;
            rng.rand_seed = u32::from(self.bullet_count);
            let first = vec2(
                random_spread(rng, weapon.bullet_spread, b.x),
                random_spread(rng, weapon.bullet_spread, b.y),
            );
            // Soldat draws the other pellets unseeded, so its shooter sees other pellets than
            // the ones the server and the other players replay from the seed; a network
            // client draws them from the seed too
            if !config.client {
                rng.rand_seed = saved_seed;
            }
            bn = Some(create(a, first, Some(self.bullet_count), true));

            // remaining 5 pellets
            for _ in 0..5 {
                let pellet = vec2(
                    random_spread(rng, weapon.bullet_spread, b.x),
                    random_spread(rng, weapon.bullet_spread, b.y),
                );
                let _ = create(a, pellet, None, false);
            }
            if config.client {
                rng.rand_seed = saved_seed;
            }

            // untyped constants: extended precision products
            let d = vec2(fpc(ext(b.x) * 0.0412), fpc(ext(b.y) * 0.041));
            self.particle.velocity -= d;
        }

        if kind == WeaponKind::Minigun {
            // untyped constants: extended precision products, stored in singles
            let (cx, cy) = if self.control.jets && self.jets_count > 0 {
                (0.0012, 0.0009)
            } else {
                (0.0082, 0.0078)
            };
            let mut push = vec2(fpc(ext(b.x) * cx), fpc(ext(b.y) * cy));
            if self.holded_thing.is_some() {
                push.x *= 0.5;
                push.y = fpc(ext(push.y) * 0.7);
            }
            push.x = fpc(ext(push.x) * 0.6);
            self.particle.velocity -= push;
        }

        if kind == WeaponKind::Flamer || kind == WeaponKind::Chainsaw {
            a += b * 2.0;
            bn = Some(create(a, b, None, true));
            if kind == WeaponKind::Flamer {
                self.play_on(Sfx::Flamer, Channel::Gattling);
            }
        }

        if kind == WeaponKind::LAW {
            let low_stance = (legs.id == Anim::Crouch && legs.frame > 13)
                || legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
                || (legs.id == Anim::Prone && legs.frame > 23);

            if (self.on_ground || self.on_ground_permanent || self.on_ground_for_law) && low_stance
            {
                bn = Some(create(a, b, None, true));
            } else {
                return;
            }
        }

        // the mercy shot can't hit the shooter (the client kills itself with `kill`)
        if mercy && let Some(EmitterItem::Bullet(params)) = bn.and_then(|i| emitter.get_mut(i)) {
            params.owner_immune = true;
        }

        // a client counts its own shots; the server counts a bot's when sending the
        // bullet to the other players (ServerBulletSnapshot)
        let bot = self.brain.is_some();
        let weapon = &mut self.weapons[self.active_weapon];
        if weapon.ammo_count > 0 && (!bot || pays_ammo(weapon, weapon.bullet_style)) {
            weapon.ammo_count -= 1;
        }
        if kind == WeaponKind::Spas12 {
            self.can_auto_reload_spas = false;
        }
        weapon.fire_interval_prev = weapon.fire_interval;
        weapon.fire_interval_count = weapon.fire_interval;

        self.apply_recoil_animation(kind);
        // spent shells and smoke from the muzzle (client sparks)
        self.shells_and_smoke(map, config, kind, aim_direction, b);

        self.burst_count = self.burst_count.saturating_add(1);

        if self.local_player {
            let weapon = *self.primary_weapon();
            // increase self-bink for the next shot
            if weapon.bink < 0 {
                let crouched = self.legs_animation.is_any(&[
                    Anim::Crouch,
                    Anim::CrouchRun,
                    Anim::CrouchRunBack,
                    Anim::Prone,
                    Anim::ProneMove,
                ]);
                let bink = if crouched {
                    (-f64::from(weapon.bink) / 2.0).round_ties_even() as u16
                } else {
                    -weapon.bink as u16
                };
                self.hit_spray_counter = calculate_bink(self.hit_spray_counter, bink);
            }

            // recoil: kicks the cursor up
            let mut rc = f32::from(self.burst_count) / 10.0 * f32::from(weapon.recoil);
            if self.on_ground {
                if self.legs_animation.id == Anim::Crouch && self.legs_animation.frame > 13 {
                    rc /= 2.0;
                }
                if self.legs_animation.id == Anim::Prone && self.legs_animation.frame > 23 {
                    rc /= 3.0;
                }
            }
            if rc > 0.0 {
                let degrees = weapon.speed * f32::from(weapon.fire_interval) / 364.0 * rc;
                self.recoil_kick = -PI * degrees.to_radians().sin();
            }
        }
    }

    /// `HitSpray`: the local player got hit and the aim shakes.
    pub(crate) fn hit_spray(&mut self) {
        let bink = self.primary_weapon().bink;
        if self.local_player && bink > 0 {
            self.hit_spray_counter = calculate_bink(self.hit_spray_counter, bink as u16);
        }
    }

    /// Body animation played when a weapon fires (from `TSprite.Fire`).
    fn apply_recoil_animation(&mut self, kind: WeaponKind) {
        let body = self.body_animation.id;
        let free = !matches!(body, Anim::Throw | Anim::GetUp | Anim::Melee);
        let standing = self.position == POS_STAND;
        let crouching = self.position == POS_CROUCH;
        let prone = self.position == POS_PRONE;

        let crouch_recoil = |soldier: &mut Soldier| {
            if crouching {
                if soldier.body_animation.id == Anim::HandsUpAim {
                    soldier.body_apply_animation(Anim::HandsUpRecoil, 1);
                } else {
                    soldier.body_apply_animation(Anim::AimRecoil, 1);
                }
            }
        };

        // the bang (Predators shoot silently)
        if self.bonus_style != Bonus::Predator {
            let sound = match kind {
                WeaponKind::Ak74 => Some(Sfx::Ak74Fire),
                WeaponKind::Minimi => Some(Sfx::M249Fire),
                WeaponKind::Ruger77 => Some(Sfx::Ruger77Fire),
                WeaponKind::MP5 => Some(Sfx::Mp5Fire),
                WeaponKind::Spas12 => Some(Sfx::Spas12Fire),
                WeaponKind::M79 => Some(Sfx::M79Fire),
                WeaponKind::DesertEagles => Some(Sfx::DeserteagleFire),
                WeaponKind::SteyrAUG => Some(Sfx::SteyraugFire),
                WeaponKind::Barrett => Some(Sfx::Barretm82Fire),
                WeaponKind::Minigun => Some(Sfx::MinigunFire),
                WeaponKind::USSOCOM => Some(Sfx::Colt1911Fire),
                WeaponKind::Bow | WeaponKind::FlameBow => Some(Sfx::BowFire),
                WeaponKind::LAW => Some(Sfx::Law),
                _ => None,
            };
            if let Some(sfx) = sound {
                self.play(sfx);
            }
        }

        match kind {
            WeaponKind::Ak74
            | WeaponKind::Minimi
            | WeaponKind::MP5
            | WeaponKind::DesertEagles
            | WeaponKind::SteyrAUG
            | WeaponKind::USSOCOM
            | WeaponKind::Bow
            | WeaponKind::FlameBow => {
                if free && standing {
                    self.body_apply_animation(Anim::SmallRecoil, 1);
                }
                crouch_recoil(self);
            }
            WeaponKind::Ruger77 => {
                if free && standing {
                    self.body_apply_animation(Anim::Recoil, 1);
                }
                crouch_recoil(self);
            }
            WeaponKind::Spas12 => {
                if free && !prone {
                    self.body_apply_animation(Anim::Shotgun, 1);
                }
                // make sure firing interrupts reloading when prone
                if prone && self.body_animation.id == Anim::Reload {
                    self.body_animation.frame = self.body_animation.num_frames();
                }
            }
            WeaponKind::M79 => {
                if free && !prone {
                    self.body_apply_animation(Anim::SmallRecoil, 1);
                }
            }
            WeaponKind::Barrett if free => self.body_apply_animation(Anim::Barret, 1),
            WeaponKind::Minigun if free && standing => {
                self.body_apply_animation(Anim::SmallRecoil, 2)
            }
            _ => {}
        }
    }
}

impl Soldier {
    /// The spent shell of a shot and a puff of smoke from the muzzle (`b` is the bullet's
    /// velocity).
    fn shells_and_smoke(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        kind: WeaponKind,
        aim: Vec2,
        b: Vec2,
    ) {
        let dir = f32::from(self.direction);
        let v = self.particle.velocity;
        let shell_velocity = || {
            vec2(
                v.x + dir * aim.y * (fx::random_float() * 0.5 + 0.8),
                v.y - dir * aim.x * (fx::random_float() * 0.5 + 0.8),
            )
        };
        let hand = self.skeleton.pos(15);
        let mut a = vec2(
            hand.x + 2.0 - dir * 0.015 * b.x,
            hand.y - 2.0 - dir * 0.015 * b.y,
        );
        let c = shell_velocity();
        // with fewer sparks (`r_maxsparks`) half the shells stay in the gun
        let mut blocked = map.collision_test(a, false).is_some();
        if few_sparks(config) && fx::random(2) == 0 {
            blocked = true;
        }

        let style = match kind {
            WeaponKind::Ak74 => Some(68),
            WeaponKind::Minimi => Some(72),
            WeaponKind::Ruger77 => Some(70),
            WeaponKind::MP5 => {
                a = hand + vec2(2.0 - 0.2 * b.x, -2.0 - 0.2 * b.y);
                Some(67)
            }
            WeaponKind::DesertEagles => {
                a = hand + vec2(3.0 - 0.17 * b.x, -2.0 - 0.15 * b.y);
                if !blocked {
                    self.spark(a, c, 66, 255);
                    a = hand + vec2(-3.0 - 0.25 * b.x, -3.0 - 0.3 * b.y);
                    self.spark(a, shell_velocity(), 66, 255);
                }
                None
            }
            WeaponKind::SteyrAUG => Some(69),
            WeaponKind::Barrett => Some(71),
            WeaponKind::Minigun => Some(73),
            WeaponKind::USSOCOM => {
                a = hand + vec2(2.0 - 0.2 * b.x, -2.0 - 0.2 * b.y);
                Some(65)
            }
            _ => None,
        };
        if let Some(style) = style
            && !blocked
        {
            self.spark(a, c, style, 255);
        }

        let smoke = b * 0.5;
        self.spark(a + smoke, smoke * 0.2, 35, 10);
    }
}

/// `(Random * 2 - 1) * spread`, evaluated in double precision like Free Pascal's
/// extended arithmetic before rounding to single.
pub(crate) fn random_spread(rng: &mut PascalRandom, spread: f32, offset: f32) -> f32 {
    ((rng.float() * 2.0 - 1.0) * f64::from(spread) + f64::from(offset)) as f32
}

/// Pascal's `Sign`: -1, 0 or 1.
pub(crate) fn pascal_sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}
