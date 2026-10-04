//! Computer players (`AI.pas`): the bot brain, bot profiles (`configs/bots/*.bot`) and
//! `ControlBot`, which presses a bot's buttons every tick on the server.

use super::*;
use slotmap::SlotMap;

/// `DEFAULT_JETCOLOR` of bots.
const DEFAULT_JET_COLOR: u32 = 0xFFBD24;
const DIST_AWAY: i32 = 731;
const DIST_TOO_FAR: i32 = 730;
const DIST_VERY_FAR: i32 = 500;
const DIST_FAR: i32 = 350;
const DIST_ROCK_THROW: i32 = 180;
const DIST_CLOSE: i32 = 95;
const DIST_VERY_CLOSE: i32 = 55;
const DIST_TOO_CLOSE: i32 = 35;

const WAYPOINT_SEEK_RADIUS: i32 = 21;
const SECOND: i32 = 60;
const WAYPOINT_TIMEOUT_SMALL: i32 = SECOND * 5 + 20;
const WAYPOINT_TIMEOUT_BIG: i32 = SECOND * 8;
const MAX_WAYPOINTS: i32 = 5000;
const HURT_HEALTH: f32 = 25.0;
/// Bots pay ammo for bullets only with weapons firing slower than this (`FIREINTERVAL_NET`).
pub(crate) const FIREINTERVAL_NET: u16 = 5;

/// What a bot thinks (`TBotData`).
#[derive(Debug, Clone)]
pub struct Brain {
    pub fav_weapon: WeaponKind,
    /// `Player.SecWep`: 0 USSOCOM, 1 knife, 2 chainsaw, 3 LAW, otherwise none. Unused, like
    /// on Soldat's server: bots respawn without a secondary weapon.
    pub secondary: i32,
    /// Players with this name are never shot at.
    pub friend: String,
    pub accuracy: i32,
    /// Keeps shooting corpses (1).
    pub dead_kill: u8,
    /// -1 never throws grenades, otherwise about one throw in this many ticks.
    pub grenade_freq: i32,
    /// `OnStartUse`: 1 or 2 start an idle animation, 255 none.
    pub on_start_use: u8,
    pub chat_freq: i32,
    pub chat: BotChat,
    pub camper: u8,
    /// Stands still (`Dummy`).
    pub dummy: bool,
    pub waypoint_timeout_counter: i32,
    /// Who shot at it last (`PissedOff`).
    pub pissed_off: Option<SoldierId>,
    pub path_num: u8,
    /// `TargetNum`
    pub target: Option<SoldierId>,
    pub go_thing: bool,
    /// Waypoints are 1-based indices into the map's list, 0 for none.
    pub current_waypoint: i32,
    pub next_waypoint: i32,
    pub old_waypoint: i32,
    pub waypoint_time: i32,
    pub last_waypoint: i32,
    pub one_place_count: i32,
    pub fall_save: u8,
    /// The bot's own soldier.
    pub me: SoldierId,
    /// `bots_difficulty` as of the last tick.
    pub difficulty: i32,
}

/// What a bot says (`Chat_*` of its file), with `bots_chat` on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BotChat {
    pub kill: String,
    pub dead: String,
    pub low_health: String,
    pub see_enemy: String,
    pub winning: String,
}

/// A bot as described by its `.bot` file (`LoadBotConfig`).
#[derive(Debug, Clone)]
pub struct BotProfile {
    pub name: String,
    pub fav_weapon: WeaponKind,
    pub secondary: i32,
    pub friend: String,
    /// As in the file; scaled by `bots_difficulty` when the bot joins.
    pub accuracy: i32,
    pub dead_kill: u8,
    pub grenade_freq: i32,
    pub on_start_use: u8,
    pub chat_freq: i32,
    pub chat: BotChat,
    pub camper: u8,
    pub dummy: bool,
    /// 0 none, 1 helmet, 2 cap.
    pub head_cap: u8,
    /// The shirt is `Color1`, used only without a team.
    pub looks: Looks,
}

/// `StrToInt` as `LoadBotConfig` uses it for colours: decimal or `$`/`0x` hex.
fn str_to_int(text: &str) -> Option<u32> {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let hex = digits
        .strip_prefix('$')
        .or_else(|| digits.strip_prefix("0x"))
        .or_else(|| digits.strip_prefix("0X"));
    let value = match hex {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => u32::try_from(digits.parse::<i64>().ok()?).ok()?,
    };
    Some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

/// `ColorToHex`: a Delphi `TColor` ($00BBGGRR) as 0xRRGGBB.
fn color_to_hex(mut color: u32) -> u32 {
    while color & 0xFF00_0000 != 0 {
        color >>= 4;
    }
    (color & 0xFF) << 16 | (color & 0xFF00) | (color & 0xFF_0000) >> 16
}

impl BotProfile {
    /// Parses a `.bot` file; fails like Soldat on missing values or an unknown weapon.
    pub fn parse(text: &str, weapons: &WeaponTable) -> Result<BotProfile, String> {
        let ini = Ini::parse(text);
        if !ini.has_section("BOT") {
            return Err("Section \"[Bot]\" not found".to_string());
        }

        let string = |key: &str| {
            ini.get("BOT", key)
                .map(str::to_string)
                .ok_or_else(|| format!("Value \"{key}\" not found"))
        };
        let blank = |key: &str| ini.get("BOT", key).unwrap_or("").to_string();
        let int = |key: &str| -> Result<i32, String> {
            string(key)?
                .parse::<i32>()
                .map_err(|_| format!("Value \"{key}\" is not a number"))
        };

        let weapon_name = string("Favourite_Weapon")?;
        let fav_weapon = WeaponKind::values()
            .iter()
            .copied()
            .find(|&kind| weapons.get(kind).name == weapon_name)
            .ok_or_else(|| format!("Unknown weapon \"{weapon_name}\""))?;

        let color = |key: &str| -> Result<u32, String> {
            str_to_int(&string(key)?).ok_or_else(|| format!("Value \"{key}\" is not a number"))
        };
        let looks = Looks {
            shirt: ini
                .get("BOT", "Color1")
                .and_then(str_to_int)
                .map_or(0, color_to_hex),
            pants: color_to_hex(color("Color2")?),
            // ReadConfMagicColor: as written
            skin: color("Skin_Color")? & 0xFF_FFFF,
            hair: color_to_hex(color("Hair_Color")?),
            jet: DEFAULT_JET_COLOR,
            hair_style: int("Hair")? as u8,
            chain: int("Chain")? as u8,
        };

        let headgear = int("Headgear")? as u8;
        let dummy = matches!(blank("Dummy").to_ascii_lowercase().as_str(), "1" | "true");

        Ok(BotProfile {
            name: string("Name")?.chars().take(24).collect(),
            fav_weapon,
            secondary: int("Secondary_Weapon")?,
            friend: blank("Friend"),
            accuracy: int("Accuracy")?,
            dead_kill: int("Shoot_Dead")? as u8,
            grenade_freq: int("Grenade_Frequency")?,
            on_start_use: int("OnStartUse")? as u8,
            chat_freq: (2.5 * f64::from(int("Chat_Frequency")?)).round_ties_even() as i32,
            camper: int("Camping")? as u8,
            chat: BotChat {
                kill: blank("Chat_Kill"),
                dead: blank("Chat_Dead"),
                low_health: blank("Chat_LowHealth"),
                see_enemy: blank("Chat_SeeEnemy"),
                winning: blank("Chat_Winning"),
            },
            dummy,
            head_cap: match headgear {
                0 => 0,
                2 => 2,
                _ => 1,
            },
            looks,
        })
    }

    /// Loads `configs/bots/<name>.bot`.
    pub fn load(vfs: &assets::Vfs, name: &str, weapons: &WeaponTable) -> Result<Self, String> {
        let path = format!("configs/bots/{name}.bot");
        let text = vfs
            .read_to_string(&path)
            .map_err(|e| format!("{path}: {e}"))?;
        BotProfile::parse(&text, weapons).map_err(|e| format!("{path}: {e}"))
    }

    pub(crate) fn brain(&self, me: SoldierId, target: Option<SoldierId>, difficulty: i32) -> Brain {
        Brain {
            fav_weapon: self.fav_weapon,
            secondary: self.secondary,
            friend: self.friend.clone(),
            accuracy: (f64::from(self.accuracy) * (f64::from(difficulty) / 100.0)).trunc() as i32,
            dead_kill: self.dead_kill,
            grenade_freq: self.grenade_freq,
            on_start_use: self.on_start_use,
            chat_freq: self.chat_freq,
            camper: self.camper,
            chat: self.chat.clone(),
            dummy: self.dummy,
            waypoint_timeout_counter: WAYPOINT_TIMEOUT_SMALL,
            pissed_off: None,
            path_num: 0,
            target,
            go_thing: false,
            current_waypoint: 0,
            next_waypoint: 0,
            old_waypoint: 0,
            waypoint_time: 0,
            last_waypoint: 0,
            one_place_count: 0,
            fall_save: 0,
            me,
            difficulty,
        }
    }
}

/// `CheckDistance`: the distance on one axis, in steps.
fn check_distance(a: f32, b: f32) -> i32 {
    let distance = (a - b).abs();
    match distance {
        d if d <= DIST_TOO_CLOSE as f32 => DIST_TOO_CLOSE,
        d if d <= DIST_VERY_CLOSE as f32 => DIST_VERY_CLOSE,
        d if d <= DIST_CLOSE as f32 => DIST_CLOSE,
        d if d <= DIST_ROCK_THROW as f32 => DIST_ROCK_THROW,
        d if d <= DIST_FAR as f32 => DIST_FAR,
        d if d <= DIST_VERY_FAR as f32 => DIST_VERY_FAR,
        d if d <= DIST_TOO_FAR as f32 => DIST_TOO_FAR,
        _ => DIST_AWAY,
    }
}

static NO_WAYPOINT: Waypoint = Waypoint {
    active: false,
    id: 0,
    x: 0,
    y: 0,
    left: false,
    right: false,
    up: false,
    down: false,
    jets: false,
    path_num: 0,
    action: WaypointAction::None,
    connections_num: 0,
    connections: [0; MAX_CONNECTIONS],
};

/// `BotPath.Waypoint[k]`: unused slots are empty.
fn waypoint(map: &MapFile, k: i32) -> &Waypoint {
    usize::try_from(k - 1)
        .ok()
        .and_then(|i| map.waypoints.get(i))
        .unwrap_or(&NO_WAYPOINT)
}

/// `TWaypoints.FindClosest`: the first waypoint (not `current`) closer than `radius`.
fn find_closest(map: &MapFile, pos: Vec2, radius: i32, current: i32) -> i32 {
    for (i, w) in map.waypoints.iter().enumerate() {
        let k = i as i32 + 1;
        if w.active && k != current {
            let d = distance(pos, vec2(w.x as f32, w.y as f32));
            if d < radius as f32 {
                return k;
            }
        }
    }
    0
}

/// `Map.RayCast` with the defaults the AI uses: whether a wall is in the way, and the
/// distance (to the wall, or between the points when nothing is).
fn ray(map: &MapFile, a: Vec2, b: Vec2) -> (bool, f32) {
    match map.ray_cast(a, b, 651.0, RayCast::default()) {
        Some(d) => (true, d),
        None => (false, (a - b).length()),
    }
}

/// `TeamFlag[team]`: the thing slot of the team's flag.
fn team_flag(things: &[Thing], team: Team) -> Option<usize> {
    let kind = match team {
        Team::Alpha => ThingKind::AlphaFlag,
        Team::Bravo => ThingKind::BravoFlag,
        _ => return None,
    };
    things.iter().position(|t| t.active && t.kind == kind)
}

fn is_bow(kind: WeaponKind) -> bool {
    matches!(kind, WeaponKind::Bow | WeaponKind::FlameBow)
}

fn is_melee(kind: WeaponKind) -> bool {
    matches!(
        kind,
        WeaponKind::NoWeapon | WeaponKind::Knife | WeaponKind::Chainsaw
    )
}

/// Moves toward (`toward`) or away from `t` on the x axis.
fn steer(c: &mut Control, m: Vec2, t: Vec2, toward: bool) {
    c.right = false;
    c.left = false;
    if (t.x > m.x) == toward && t.x != m.x {
        c.right = true;
    } else if t.x != m.x {
        c.left = true;
    }
}

/// Shared state while a bot thinks.
struct Mind<'a> {
    id: SoldierId,
    map: &'a MapFile,
    config: &'a WorldConfig,
    soldiers: &'a SlotMap<SoldierId, Soldier>,
    things: &'a mut [Thing],
    rng: &'a mut PascalRandom,
    tick: u64,
    brain: Brain,
    c: Control,
    burst_count: u8,
    /// What the bot says this tick.
    said: Vec<String>,
}

impl World {
    /// `ControlBot`, run during the bot's update before its controls are applied.
    pub(crate) fn control_bot(&mut self, id: SoldierId) {
        let soldier = &mut self.soldiers[id];
        let Some(mut brain) = soldier.brain.take() else {
            return;
        };
        brain.difficulty = self.config.bots_difficulty;

        if soldier.dead_meat || brain.dummy {
            soldier.brain = Some(brain);
            return;
        }

        let (c, burst_count) = (soldier.control, soldier.burst_count);
        let mut mind = Mind {
            id,
            map: &self.map,
            config: &self.config,
            soldiers: &self.soldiers,
            things: &mut self.things,
            rng: &mut self.rng,
            tick: self.tick,
            c,
            burst_count,
            brain,
            said: Vec::new(),
        };
        mind.control_bot(&self.bullets);

        let (brain, control, burst_count) = (mind.brain, mind.c, mind.burst_count);
        let said = mind.said;
        let soldier = &mut self.soldiers[id];
        soldier.said.extend(said);
        soldier.brain = Some(brain);
        soldier.control = control;
        soldier.burst_count = burst_count;
    }
}

impl Mind<'_> {
    fn control_bot(&mut self, bullets: &[Bullet]) {
        let id = self.id;
        let soldiers = self.soldiers;
        let map = self.map;
        let me = &soldiers[id];
        let mode = self.config.game_mode;
        let team_game = mode.is_team_game();

        let throw_nade = self.c.throw_nade;
        self.c.free_controls();
        self.c.throw_nade = me.body_animation.id == Anim::Throw && throw_nade;

        let mut look = me.skeleton.pos(12);
        look.y -= 2.0;

        // see?
        let mut see_closest = false;
        let mut d: f32 = 999_999.0;
        for (other_id, other) in soldiers.iter() {
            if !other.active
                || other_id == id
                || other.name == self.brain.friend
                || !(other.alpha == 255 || other.holded_thing.is_some())
                || other.is_spectator()
            {
                continue;
            }
            if other.dead_meat && !(self.brain.dead_kill == 1 && other.dead_time < 180) {
                continue;
            }

            let mut start = other.skeleton.pos(12);
            // ray start point inside a polygon
            if map.collision_test(start, false).is_some() {
                start.y += 6.0;
            }

            let (hit, d2) = ray(map, look, start);
            if hit {
                continue;
            }

            if mode == GameMode::Rambo && is_bow(other.primary_weapon().kind) {
                self.brain.target = Some(other_id);
                see_closest = true;
                break;
            }

            if d > d2 {
                self.brain.target = Some(other_id);
                let dt = d;
                if !other.dead_meat {
                    d = d2;
                }
                see_closest = true;

                // stop throwing grenades and weapons at the dead
                if other.dead_meat {
                    self.c.throw_nade = false;
                    self.c.throw_weapon = false;
                }

                if mode == GameMode::Rambo && !is_bow(me.primary_weapon().kind) {
                    see_closest = false;
                    d = dt;
                }
                if team_game && me.team == other.team {
                    see_closest = false;
                }
            }
        }

        let target = self.brain.target.and_then(|t| soldiers.get(t));
        if target.is_some_and(|t| is_bow(t.primary_weapon().kind)) {
            self.brain.pissed_off = None;
        }
        if self.brain.pissed_off == Some(id) {
            self.brain.pissed_off = None;
        }
        if let Some(p) = self.brain.pissed_off {
            match soldiers.get(p) {
                Some(p) if team_game && !self.config.friendly_fire && p.team == me.team => {
                    self.brain.pissed_off = None
                }
                None => self.brain.pissed_off = None,
                _ => {}
            }
        }
        if target.is_some_and(|t| team_game && self.config.friendly_fire && t.team != me.team) {
            self.brain.pissed_off = None;
        }

        if let Some(p) = self.brain.pissed_off {
            let mut look = me.skeleton.pos(12);
            look.y -= 2.0;
            if !ray(map, look, soldiers[p].skeleton.pos(12)).0 {
                self.brain.target = Some(p);
                see_closest = true;
            } else {
                self.brain.pissed_off = None;
            }
        }

        let holds_flag = me.holded_thing.is_some_and(|h| {
            matches!(
                self.things[h].kind,
                ThingKind::AlphaFlag | ThingKind::BravoFlag
            )
        });

        // have the flag and not hurt, run away!
        let mut run_away = false;
        if see_closest
            && holds_flag
            && self
                .brain
                .target
                .and_then(|t| soldiers.get(t))
                .is_some_and(|t| t.holded_thing.is_none())
        {
            see_closest = false;
            run_away = true;
        }

        if !see_closest {
            // go with waypoints
            if !self.brain.go_thing && me.stat.is_none() {
                self.follow_waypoints(run_away, holds_flag);
            }
        } else {
            let current = self.brain.current_waypoint;
            if current != 0 && waypoint(map, current).action == WaypointAction::None {
                self.brain.current_waypoint = 0;
            }

            self.simple_decision();

            // camp
            let current = self.brain.current_waypoint;
            if current > 0
                && ((mode == GameMode::Infiltration && me.team == Team::Bravo)
                    || (mode == GameMode::CaptureTheFlag && me.holded_thing.is_none())
                    || (mode != GameMode::Infiltration && mode != GameMode::CaptureTheFlag))
                && waypoint(map, current).action == WaypointAction::StopAndCamp
            {
                self.stop();
            }

            if self.config.bots_chat {
                let freq = i64::from(self.brain.chat_freq);
                if self.rng.below_i64(115 * freq) == 0 {
                    self.said.push(self.brain.chat.see_enemy.clone());
                }
                if self.rng.below_i64(790 * freq) == 0 {
                    let target = self.brain.target.and_then(|t| soldiers.get(t));
                    let name = target.map_or("", |t| t.name.as_str());
                    self.said.push(format!("Die {name}!"));
                }
            }

            self.brain.waypoint_time = 0;
        }

        self.look_for_things(run_away);

        // run away from grenades!
        if self.brain.difficulty < 201 {
            for bullet in bullets.iter().filter(|b| b.active) {
                if bullet.style == BulletStyle::FragGrenade
                    && f64::from(distance(bullet.particle.pos, me.particle.pos))
                        < f64::from(FRAGGRENADE_EXPLOSION_RADIUS) * 1.4
                {
                    let right = bullet.particle.pos.x <= me.particle.pos.x;
                    self.c.left = !right;
                    self.c.right = right;
                }
            }
        }

        // release the grenade
        if me.body_animation.id == Anim::Throw && me.body_animation.frame > 35 {
            self.c.throw_nade = false;
        }

        self.brain.waypoint_timeout_counter -= 1;
        if self.brain.waypoint_timeout_counter < 0 {
            self.brain.current_waypoint = self.brain.old_waypoint;
            self.brain.waypoint_timeout_counter = WAYPOINT_TIMEOUT_SMALL;
            self.c.free_controls();
            self.c.up = true;
        }

        // the waypoint is no good
        if self.brain.waypoint_time > WAYPOINT_TIMEOUT_BIG {
            self.c.free_controls();
            self.brain.current_waypoint = 0;
            self.brain.go_thing = false;
            self.brain.waypoint_time = 0;
        }

        // fall damage save
        let vy = me.particle.velocity.y;
        if f64::from(vy) > 3.35 {
            self.brain.fall_save = 1;
        }
        if f64::from(vy) < 1.35 {
            self.brain.fall_save = 0;
        }
        if self.brain.fall_save > 0 {
            self.c.jets = true;
        }

        // winning, it boasts
        if self.config.bots_chat
            && self.rng.below_i64(i64::from(self.brain.chat_freq) * 150) == 0
            && self.config.now.leader == Some(id)
        {
            self.said.push(self.brain.chat.winning.clone());
        }

        if self.rng.below(190) == 0 {
            self.brain.pissed_off = None;
        }

        // on a stationary gun: bursts while it doesn't overheat
        if me.stat.is_some() {
            self.brain.one_place_count += 1;
            let count = self.brain.one_place_count;
            if (count > 120 && count < 220)
                || (count > 350 && count < 620)
                || (count > 700 && count < 740)
                || (count > 900 && count < 1100)
                || (count > 1300 && count < 1500)
            {
                self.c.fire = true;
                if self.rng.below(2) == 0 {
                    self.c.mouse_aim_y += self.rng.below(4);
                } else {
                    self.c.mouse_aim_y -= self.rng.below(4);
                }
            }

            if self.brain.one_place_count > 1500 {
                self.brain.one_place_count = 0;
            }
        }
    }

    fn stop(&mut self) {
        self.c.left = false;
        self.c.right = false;
        self.c.up = false;
        self.c.down = false;
        self.c.jets = false;
    }

    /// The waypoint part of `ControlBot`, when no enemy is in sight.
    fn follow_waypoints(&mut self, run_away: bool, holds_flag: bool) {
        let map = self.map;
        let soldiers = self.soldiers;
        let me = &soldiers[self.id];
        let mode = self.config.game_mode;
        let brain = &mut self.brain;

        let radius = if brain.current_waypoint == 0 {
            350
        } else {
            WAYPOINT_SEEK_RADIUS
        };
        let k = find_closest(map, me.particle.pos, radius, brain.current_waypoint);

        brain.old_waypoint = brain.current_waypoint;

        // (Soldat used to read out of bounds without a next waypoint)
        if brain.next_waypoint == 0 {
            brain.next_waypoint = 1;
        }
        brain.path_num = waypoint(map, brain.next_waypoint).path_num;

        let held = me.holded_thing.map(|h| self.things[h].kind);
        let (to_alpha, to_bravo) = (2, 1);
        match mode {
            GameMode::CaptureTheFlag => {
                brain.path_num = me.team as u8;
                // I have the flag!
                if holds_flag {
                    if me.team == Team::Alpha {
                        brain.path_num = to_alpha;
                    }
                    if me.team == Team::Bravo {
                        brain.path_num = to_bravo;
                    }
                }
            }
            GameMode::HoldTheFlag => {
                brain.path_num = me.team as u8;
                if held == Some(ThingKind::PointmatchFlag) {
                    if me.team == Team::Alpha {
                        brain.path_num = to_alpha;
                    }
                    if me.team == Team::Bravo {
                        brain.path_num = to_bravo;
                    }
                }
            }
            GameMode::Infiltration => {
                if me.team == Team::Alpha {
                    brain.path_num = 1;
                }
                if me.team == Team::Bravo {
                    brain.path_num = 2;
                }
                if team_flag(self.things, Team::Bravo).is_some_and(|f| !self.things[f].in_base)
                    && me.team == Team::Bravo
                {
                    brain.path_num = 2;
                }
                if holds_flag {
                    if me.team == Team::Alpha {
                        brain.path_num = to_alpha;
                    }
                    if me.team == Team::Bravo {
                        brain.path_num = to_bravo;
                    }
                }
            }
            _ => {}
        }

        if k > 0 && (brain.path_num == waypoint(map, k).path_num || brain.current_waypoint == 0) {
            brain.current_waypoint = k;
        }

        let current = brain.current_waypoint;
        if current <= 0 || current >= MAX_WAYPOINTS {
            return;
        }

        if brain.old_waypoint != current {
            let w = waypoint(map, current);
            let k = self.rng.below(w.connections_num) + 1;
            let next = usize::try_from(k - 1)
                .ok()
                .and_then(|i| w.connections.get(i))
                .copied()
                .unwrap_or(0);
            if k > 0 && k < MAX_WAYPOINTS && next > 0 && next < MAX_WAYPOINTS {
                brain.next_waypoint = next;
                // face the target
                let n = waypoint(map, next);
                self.c.mouse_aim_x = n.x;
                self.c.mouse_aim_y = n.y;
            }
        }

        // apply the waypoint's movements
        let next = waypoint(map, brain.next_waypoint);
        self.c.left = next.left;
        self.c.right = next.right;
        self.c.up = next.up;
        self.c.down = next.down;
        self.c.jets = next.jets;

        // special waypoints (not on an Infiltration escape path)
        let bravo_flag_in_base =
            team_flag(self.things, Team::Bravo).is_some_and(|f| self.things[f].in_base);
        if (mode == GameMode::Infiltration
            && me.team == Team::Bravo
            && bravo_flag_in_base
            && me.holded_thing.is_none())
            || (mode == GameMode::CaptureTheFlag && me.holded_thing.is_none())
            || !matches!(
                mode,
                GameMode::Infiltration | GameMode::CaptureTheFlag | GameMode::HoldTheFlag
            )
        {
            let count = brain.one_place_count;
            let wait = match waypoint(map, current).action {
                WaypointAction::None => false,
                WaypointAction::StopAndCamp => true,
                WaypointAction::Wait1Second => count < 60,
                WaypointAction::Wait5Seconds => count < 300,
                WaypointAction::Wait10Seconds => count < 600,
                WaypointAction::Wait15Seconds => count < 900,
                WaypointAction::Wait20Seconds => count < 1200,
            };
            if wait {
                self.c.left = false;
                self.c.right = false;
                self.c.up = false;
                self.c.down = false;
                self.c.jets = false;

                if me.stat.is_none() && brain.camper > 0 && brain.one_place_count > 180 {
                    self.c.down = true;
                }
            }
        }

        // fire at the guy shooting at me while running away
        if run_away && let Some(p) = brain.pissed_off.and_then(|p| soldiers.get(p)) {
            let speed = me.primary_weapon().speed;
            let acc = brain.accuracy;
            self.c.mouse_aim_x = p.particle.pos.x.round_ties_even() as i32;
            let y = p.particle.pos.y - (175.0 / speed) - acc as f32 + self.rng.below(acc) as f32;
            self.c.mouse_aim_y = y.round_ties_even() as i32;
            self.c.fire = true;
        }

        if brain.last_waypoint == current {
            brain.waypoint_time += 1;
        } else {
            brain.waypoint_time = 0;
        }
        brain.last_waypoint = current;

        // standing in place because stuck or something?
        let action = waypoint(map, current).action;
        if action == WaypointAction::None {
            if (self.c.left || self.c.right) && !self.c.down {
                if distance(me.particle.pos, me.particle.old_pos) < 3.0 {
                    brain.one_place_count += 1;
                } else {
                    brain.one_place_count = 0;
                }
            } else {
                brain.one_place_count = 0;
            }
        } else {
            brain.one_place_count += 1;
        }

        if action == WaypointAction::None && brain.one_place_count > 90 {
            if self.c.left && self.c.right {
                self.c.right = false;
            }
            self.c.up = true;
        }

        // change the weapon back
        let weapon = me.primary_weapon();
        if brain.difficulty < 201
            && matches!(
                weapon.kind,
                WeaponKind::USSOCOM
                    | WeaponKind::NoWeapon
                    | WeaponKind::Knife
                    | WeaponKind::Chainsaw
                    | WeaponKind::LAW
            )
            && me.secondary_weapon().kind != WeaponKind::NoWeapon
        {
            self.c.change_weapon = true;
        }

        // reload if low on ammo
        if brain.difficulty < 201 && weapon.ammo_count < 4 && weapon.ammo > 3 {
            self.c.reload = true;
        }

        // get up if prone
        if self.rng.below(150) == 0 && matches!(me.body_animation.id, Anim::Prone | Anim::ProneMove)
        {
            self.c.prone = true;
        }
    }

    /// `SimpleDecision`: fight the target.
    fn simple_decision(&mut self) {
        let soldiers = self.soldiers;
        let Some(target) = self.brain.target.and_then(|t| soldiers.get(t)) else {
            return;
        };
        let me = &soldiers[self.id];
        let map = self.map;
        let c = &mut self.c;
        let brain = &self.brain;
        let go = brain.go_thing;
        let m = me.particle.pos;
        let t = target.particle.pos;
        let weapon = *me.primary_weapon();
        let minigun = weapon.kind == WeaponKind::Minigun;
        let reloading = weapon.ammo_count == 0;

        if !go {
            steer(c, m, t, true);
        }

        // x distance
        let dx = check_distance(m.x, t.x);
        match dx {
            DIST_TOO_CLOSE => {
                if !go {
                    steer(c, m, t, false);
                }
                c.fire = true;
            }
            DIST_VERY_CLOSE => {
                if !go {
                    c.right = false;
                    c.left = false;
                }
                c.fire = true;

                if reloading {
                    if !go {
                        steer(c, m, t, false);
                    }
                    c.fire = false;
                }
            }
            DIST_CLOSE | DIST_ROCK_THROW => {
                if dx == DIST_CLOSE && !go {
                    c.right = false;
                    c.left = false;
                }
                c.down = true;
                c.fire = true;

                if reloading {
                    if !go {
                        steer(c, m, t, false);
                    }
                    c.down = false;
                    c.fire = false;
                }
            }
            DIST_FAR => {
                c.fire = true;
                if brain.camper > 127 && !go {
                    c.up = false;
                    c.down = true;
                }
            }
            DIST_VERY_FAR | DIST_TOO_FAR => {
                let (odds, prone_odds) = if dx == DIST_VERY_FAR {
                    c.up = true;
                    (2, 250)
                } else {
                    (4, 300)
                };
                if self.rng.below(odds) == 0 || minigun {
                    c.fire = true;
                }

                if brain.camper > 0 {
                    if self.rng.below(prone_odds) == 0 && me.body_animation.id != Anim::Prone {
                        c.prone = true;
                    }

                    if !go {
                        c.right = false;
                        c.left = false;
                        c.up = false;
                        c.down = true;
                    }
                }
            }
            _ => {}
        }

        // move when the other player camps
        let target_waypoint = target.brain.as_ref().map_or(0, |b| b.current_waypoint);
        if !go
            && target_waypoint > 0
            && waypoint(map, target_waypoint).action != WaypointAction::None
        {
            steer(c, m, t, true);
        }

        // hide behind a collider
        if brain.difficulty < 101 && me.collider_distance < 255 {
            c.down = true;

            if brain.camper > 0 {
                c.left = false;
                c.right = false;

                // shoot!
                if self.rng.below(4) == 0 || minigun {
                    c.fire = true;
                }
            }

            if me.body_animation.id == Anim::HandsUpAim && me.body_animation.frame != 11 {
                c.fire = false;
            }
        }

        // the target is behind a collider and the bot doesn't escape
        if brain.difficulty < 201
            && target.collider_distance < 255
            && me.collider_distance > 254
            && brain.camper > 0
        {
            if t.x < m.x {
                c.right = true;
            } else if t.x > m.x {
                c.left = true;
            }
        }

        // fists!
        if is_melee(weapon.kind) && (!is_melee(target.primary_weapon().kind) || t.y > m.y) {
            c.down = false;
            c.fire = true;
            steer(c, m, t, true);
        }

        // y distance
        let dy = check_distance(m.y, t.y);
        if !go && dy >= DIST_ROCK_THROW && m.y > t.y {
            c.jets = true;
        }

        // Flame God seen
        if target.bonus_style == Bonus::Flamegod {
            steer(c, m, t, false);
        }

        // realistic mode: burst fire
        if self.config.realistic_mode {
            let limit = if minigun { 30 } else { 3 };
            if self.burst_count > limit {
                c.fire = false;
                if self.tick.is_multiple_of(SECOND as u64) {
                    self.burst_count = 0;
                }
            }
        }

        // on a stationary gun: hold still and shoot
        if me.stat.is_some() {
            c.right = false;
            c.left = false;
            c.up = false;
            c.down = false;
            c.fire = true;
        }

        // grenade throw
        if brain.grenade_freq > -1 {
            let mut gr = brain.grenade_freq;
            if reloading || weapon.fire_interval_count > 125 {
                gr /= 2;
            }
            if brain.current_waypoint > 0
                && waypoint(map, brain.current_waypoint).action != WaypointAction::None
            {
                gr /= 2;
            }
            if brain.difficulty < 100 {
                gr /= 2;
            }

            if brain.difficulty < 201
                && self.rng.below(gr) == 0
                && dx < DIST_FAR
                && me.tertiary_weapon().ammo_count > 0
                && ((dy < DIST_VERY_CLOSE && m.y > t.y) || m.y < t.y)
            {
                c.throw_nade = true;
            }
        }

        // knife throw
        if me.ceasefire_counter < 30
            && weapon.kind == WeaponKind::Knife
            && brain.fav_weapon == WeaponKind::Knife
        {
            c.fire = false;
            c.throw_weapon = true;
        }

        // Soldat adds the target's velocity to `t` with a Vec2Add whose result is unused
        let acc = brain.accuracy;
        c.mouse_aim_x = t.x.round_ties_even() as i32;
        let lead = if dx < DIST_FAR { 0.5 } else { 1.75 };
        let y = t.y - (lead * dx as f32 / weapon.speed) - acc as f32 + self.rng.below(acc) as f32;
        c.mouse_aim_y = y.round_ties_even() as i32;

        // the stationary gun's bullets are slow
        if me.stat.is_some() {
            let y = t.y - (0.5 * dx as f32 / 30.0) - acc as f32 + self.rng.below(acc) as f32;
            c.mouse_aim_y = y.round_ties_even() as i32;
        }

        // impossible
        if brain.difficulty < 60
            && matches!(
                target.primary_weapon().kind,
                WeaponKind::Barrett | WeaponKind::Ruger77
            )
        {
            let dist = ((m.x - t.x).powi(2) + (m.y - t.y).powi(2))
                .sqrt()
                .round_ties_even();
            c.mouse_aim_x = t.x.round_ties_even() as i32;
            c.mouse_aim_y = t.y.round_ties_even() as i32;

            let steps = (dist / target.primary_weapon().speed).round_ties_even() as i32;
            for _ in 1..=steps {
                c.mouse_aim_x += target.particle.velocity.x.round_ties_even() as i32;
                c.mouse_aim_y += target.particle.velocity.y.round_ties_even() as i32;
            }

            if weapon.fire_interval_count < 3 {
                c.free_controls();
                c.fire = true;
                c.down = true;

                if !me.body_animation.is_any(&[
                    Anim::Stand,
                    Anim::Recoil,
                    Anim::Prone,
                    Anim::Shotgun,
                    Anim::Barret,
                    Anim::SmallRecoil,
                    Anim::AimRecoil,
                    Anim::HandsUpRecoil,
                    Anim::Aim,
                    Anim::HandsUpAim,
                ]) {
                    c.fire = false;
                }
            }
        }

        if self.config.realistic_mode {
            c.mouse_aim_y -= i32::from(self.burst_count) * 3;
        }
    }

    /// Looks for flags, bows and kits worth going for.
    fn look_for_things(&mut self, run_away: bool) {
        let id = self.id;
        let soldiers = self.soldiers;
        let me = &soldiers[id];
        let map = self.map;
        let mode = self.config.game_mode;
        let ctf_inf = matches!(mode, GameMode::CaptureTheFlag | GameMode::Infiltration);
        let team = me.team as u8;
        let tertiary = me.tertiary_weapon();

        let mut see_thing = false;
        let mut look = me.skeleton.pos(12);
        look.y -= 4.0;

        for i in 0..self.things.len() {
            let thing = &self.things[i];
            if see_thing || !thing.active || thing.holding == Some(id) {
                continue;
            }

            use ThingKind::*;
            let kind = thing.kind;
            let wanted = matches!(
                kind,
                AlphaFlag
                    | BravoFlag
                    | PointmatchFlag
                    | RamboBow
                    | FlamerKit
                    | PredatorKit
                    | VestKit
                    | BerserkKit
                    | CombatKnife
            ) || (kind == MedicalKit && me.health < self.config.start_health())
                || (kind == GrenadeKit
                    && i32::from(tertiary.ammo_count) < self.config.max_grenades
                    && (tertiary.kind != WeaponKind::ClusterGrenade || tertiary.ammo_count == 0));
            if !wanted {
                continue;
            }

            let mut start = thing.pos(2);
            start.y -= 5.0;
            let (hit, d2) = ray(map, look, start);
            if hit || d2 >= DIST_FAR as f32 {
                continue;
            }

            // I see the flag! Or bow or something
            see_thing = true;
            let style = kind as u8;

            // don't take my flag in base
            if ctf_inf && style == team && thing.in_base {
                see_thing = false;
                if let Some(h) = me.holded_thing
                    && h != i
                    && self.things[h].holding == Some(id)
                {
                    see_thing = true;
                }
            }
            // don't follow this flag if my flag is not in base
            if ctf_inf
                && style != team
                && team_flag(self.things, me.team).is_some_and(|f| !self.things[f].in_base)
            {
                see_thing = false;
            }
            let thing = &self.things[i];
            // don't take a flag in its base
            if ctf_inf
                && style != team
                && style < Ussocom as u8
                && thing.in_base
                && d2 > DIST_CLOSE as f32
            {
                see_thing = false;
            }
            // better take a close medikit when hurt
            if kind == MedicalKit && me.health < HURT_HEALTH && d2 < DIST_VERY_CLOSE as f32 {
                see_thing = true;
            }
            // don't take it when running away with the flag
            if matches!(
                kind,
                MedicalKit | GrenadeKit | FlamerKit | PredatorKit | BerserkKit
            ) && run_away
            {
                see_thing = false;
            }
            if matches!(kind, FlamerKit | PredatorKit | BerserkKit) && me.bonus_style != Bonus::None
            {
                see_thing = false;
            }
            if kind == CombatKnife {
                see_thing = true;
            }

            // throw away the weapon
            if d2 < 30.0 && kind == RamboBow {
                self.c.throw_weapon = true;
            }

            if see_thing {
                if thing.holding.is_none() {
                    self.things[i].interest -= 1;
                }

                if self.things[i].interest > 0 {
                    if self.config.bots_chat
                        && kind < PointmatchFlag
                        && self.rng.below_i64(400 * i64::from(self.brain.chat_freq)) == 0
                    {
                        self.said.push("Flag!".to_string());
                    }

                    self.brain.go_thing = true;
                    self.go_to_thing(i);
                } else {
                    self.brain.go_thing = false;
                }

                // pick up the knife!
                if kind == CombatKnife
                    && me.primary_weapon().kind == WeaponKind::NoWeapon
                    && self.brain.fav_weapon == WeaponKind::Knife
                {
                    self.c.fire = false;
                    self.brain.target = None;
                    self.brain.go_thing = true;
                    self.go_to_thing(i);
                }
            }
        }

        if !see_thing {
            self.brain.go_thing = false;
        }
    }

    /// `GoToThing`
    fn go_to_thing(&mut self, slot: usize) {
        let soldiers = self.soldiers;
        let me = &soldiers[self.id];
        let thing = &self.things[slot];
        let m = me.particle.pos;
        let (p1, p2) = (thing.pos(1), thing.pos(2));

        let mut t = p2;
        if p2.x > p1.x && m.x < p2.x {
            t = p2;
        }
        if p2.x > p1.x && m.x > p1.x {
            t = p1;
        }
        if p2.x < p1.x && m.x < p1.x {
            t = p1;
        }
        if p2.x < p1.x && m.x > p2.x {
            t = p2;
        }

        if thing.holding.is_some() {
            t.y += 5.0;
        }

        if t.x >= m.x {
            self.c.right = true;
        } else if t.x < m.x {
            self.c.left = true;
        }

        if let Some(holder) = thing.holding.and_then(|h| soldiers.get(h))
            && team_flag(self.things, me.team).is_some()
            && me.team == holder.team
            && !thing.in_base
        {
            let dx = check_distance(m.x, t.x);
            if dx == DIST_TOO_CLOSE || dx == DIST_VERY_CLOSE {
                self.c.right = false;
                self.c.left = false;
                self.c.down = true;
            }

            self.c.jets = holder.control.jets;
        }

        let dy = check_distance(m.y, t.y);
        if dy >= DIST_VERY_CLOSE && m.y > t.y {
            self.c.jets = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADMIRAL: &str = "[BOT]
Name=Admiral
Color1=$00E253EE
Color2=$00394F48
Skin_Color=$006D4A1A
Hair_Color=$005336F5
Favourite_Weapon=FN Minimi
Secondary_Weapon=0
Friend=
Accuracy=70
Shoot_Dead=0
Grenade_Frequency=160
Camping=0
OnStartUse=255
Hair=2
Headgear=1
Chain=2
Chat_Frequency=7
";

    #[test]
    fn bot_looks_swap_delphi_colors() {
        let profile = BotProfile::parse(ADMIRAL, &WeaponTable::default()).unwrap();
        let looks = profile.looks;
        assert_eq!(looks.shirt, 0xEE53E2);
        assert_eq!(looks.pants, 0x484F39);
        assert_eq!(looks.skin, 0x6D4A1A, "the skin colour is read as written");
        assert_eq!(looks.hair, 0xF53653);
        assert_eq!((looks.hair_style, looks.chain), (2, 2));
        assert_eq!(profile.head_cap, 1);
    }

    #[test]
    fn str_to_int_reads_pascal_numbers() {
        assert_eq!(str_to_int("$00FF00"), Some(0xFF00));
        assert_eq!(str_to_int("0x10"), Some(16));
        assert_eq!(str_to_int(" 42 "), Some(42));
        assert_eq!(str_to_int("-1"), Some(u32::MAX));
        assert_eq!(str_to_int("blue"), None);
        assert_eq!(color_to_hex(0x1234_5678), 0x563412, "shifted until it fits");
    }
}
