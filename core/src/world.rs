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
    cvars.register(
        // Soldat's default: capture the flag
        Cvar::int("sv_gamemode", 3, "Sets the gamemode")
            .flags(rules | CvarFlags::INIT_ONLY)
            .range(0.0, 6.0),
    );
    for (team, what) in [
        ("noteam", "DM, PM and RM modes"),
        ("alpha", "Alpha in INF, CTF, HTF, PM and TM"),
        ("bravo", "Bravo in INF, CTF, HTF, PM and TM"),
        ("charlie", "Charlie in INF, CTF, HTF, PM and TM"),
        ("delta", "Delta in INF, CTF, HTF, PM and TM"),
    ] {
        cvars.register(
            Cvar::int(
                &format!("bots_random_{team}"),
                0,
                &format!("Number of bots on {what}"),
            )
            .flags(CvarFlags::SERVER)
            .range(0.0, 32.0),
        );
    }
    cvars.register(
        Cvar::int(
            "bots_difficulty",
            100,
            "Sets the skill level of the bots: 300=stupid, 200=poor, 100=normal, 50=hard, 10=impossible",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 300.0),
    );
    cvars.register(
        Cvar::int("sv_timelimit", 36000, "Time limit of map")
            .flags(rules)
            .range(0.0, f64::from(i32::MAX)),
    );
    for (name, default, what) in [
        ("sv_dm_limit", 30, "Deathmatch"),
        ("sv_pm_limit", 30, "Pointmatch"),
        ("sv_tm_limit", 60, "Teammatch"),
        ("sv_ctf_limit", 10, "Capture the Flag"),
        ("sv_rm_limit", 30, "Rambomatch"),
        ("sv_inf_limit", 90, "Infiltration"),
        ("sv_htf_limit", 80, "Hold the Flag"),
    ] {
        cvars.register(
            Cvar::int(name, default, &format!("{what} point limit"))
                .flags(CvarFlags::SERVER)
                .range(0.0, 9999.0),
        );
    }
    for (name, default, what) in [
        (
            "sv_inf_redaward",
            30,
            "Infiltration: Points awarded for a flag capture",
        ),
        (
            "sv_inf_bluelimit",
            5,
            "Infiltration: Time for blue team to get points in seconds",
        ),
        ("sv_htf_pointstime", 5, "Hold The Flag points time"),
    ] {
        cvars.register(
            Cvar::int(name, default, what)
                .flags(CvarFlags::SERVER)
                .range(0.0, 9999.0),
        );
    }
    cvars.register(
        Cvar::int(
            "sv_respawntime_minwave",
            120,
            "Min wave respawn time in ticks",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 9999.0),
    );
    cvars.register(
        Cvar::int(
            "sv_respawntime_maxwave",
            240,
            "Max wave respawn time in ticks",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 9999.0),
    );
    cvars.register(
        Cvar::bool(
            "sv_stationaryguns",
            false,
            "Enables/disables Stationary Guns ingame",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::bool("sv_survivalmode", false, "Enables survival mode")
            .flags(rules | CvarFlags::INIT_ONLY),
    );
    cvars.register(
        Cvar::bool(
            "sv_survivalmode_clearweapons",
            false,
            "Clear weapons in between survivalmode rounds",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::bool(
            "sv_survivalmode_antispy",
            false,
            "Enables anti spy chat in survival mode",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::bool(
            "sv_pauseonidle",
            true,
            "Pauses the server when no human players are connected",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::bool(
            "sv_lockedmode",
            false,
            "When Locked Mode is enabled, admins will not be able to type /loadcon, /password or /maxplayers",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::bool("sv_advancemode", false, "Enables advance mode")
            .flags(rules | CvarFlags::INIT_ONLY),
    );
    cvars.register(
        Cvar::int(
            "sv_advancemode_amount",
            2,
            "Number of kills required in Advance Mode to gain a weapon.",
        )
        .flags(rules)
        .range(1.0, 9999.0),
    );
    cvars.register(
        Cvar::int("r_maxsparks", i64::from(MAX_SPARKS), "Most sparks at once")
            .flags(CvarFlags::CLIENT)
            .range(0.0, f64::from(MAX_SPARKS)),
    );
    cvars.register(
        Cvar::string("sv_hostname", "OpenSoldat Server", "Name of the server")
            .flags(CvarFlags::SERVER | CvarFlags::SYNC)
            .range(0.0, 24.0),
    );
    cvars.register(
        Cvar::string(
            "sv_info",
            "",
            "A website or e-mail address, or any other short text describing your server",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC)
        .range(0.0, 60.0),
    );
    cvars.register(
        Cvar::bool("bots_chat", true, "Enables/disables bots chatting").flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::bool("sv_radio", true, "Enables/disables radio chat")
            .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(Cvar::string("sv_password", "", "Sets game password").flags(CvarFlags::SERVER));
    cvars.register(
        Cvar::bool(
            "sv_anticheatkick",
            false,
            "Enables/Disables anti cheat kicks",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::bool(
            "sv_pure",
            false,
            "Requires clients to use the same game files (.smod) as the server",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::string(
            "fs_mod",
            "",
            "File name of mod placed in mods directory (without .smod extension)",
        )
        .flags(CvarFlags::SERVER | CvarFlags::INIT_ONLY),
    );
    cvars.register(
        Cvar::string("sv_adminpassword", "", "Sets admin password").flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::bool(
            "sv_echokills",
            false,
            "Echoes kills done to the admin console",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(Cvar::bool("sv_antimassflag", true, "").flags(CvarFlags::SERVER));
    cvars.register(
        Cvar::bool(
            "sv_movecheck",
            true,
            "Puts back players who move farther than the game lets them (teleports, speed hacks)",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::int(
            "sv_healthcooldown",
            HEALTH_COOLDOWN,
            "Amount of time (in seconds) a player needs to wait before he's able to pick up a \
             second medikit. Use 0 to disable",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 100.0),
    );
    for (name, description) in [
        (
            "sv_bullettime",
            "Enables/disables the Bullet Time effect on server",
        ),
        ("sv_guns_collide", "Enables colliding guns"),
        ("sv_kits_collide", "Enables colliding kits"),
    ] {
        cvars.register(
            Cvar::bool(name, false, description).flags(CvarFlags::SERVER | CvarFlags::SYNC),
        );
    }
    cvars.register(
        Cvar::bool(
            "sv_punishtk",
            false,
            "Enables/disables the built-in TK punish feature",
        )
        .flags(CvarFlags::SERVER),
    );
    for (name, default, description) in [
        (
            "sv_minping",
            0,
            "The minimum ping a player must have to play in your server",
        ),
        (
            "sv_maxping",
            400,
            "The maximum ping a player can have to play in your server",
        ),
    ] {
        cvars.register(
            Cvar::int(name, default, description)
                .flags(CvarFlags::SERVER)
                .range(0.0, 9999.0),
        );
    }
    for (name, default, description) in [
        (
            "sv_warnings_flood",
            4,
            "How many warnings someone who is flooding the server gets before getting kicked \
             for 20 minutes",
        ),
        (
            "sv_warnings_ping",
            10,
            "How many warnings someone who has a ping outside the required values above gets \
             before being kicked for 15 minutes",
        ),
        (
            "sv_warnings_votecheat",
            8,
            "How many warnings someone gets before determining that they are automatically \
             vote kicked",
        ),
        (
            "sv_warnings_knifecheat",
            14,
            "How many warnings someone gets before determining that they are using a knife \
             cheat",
        ),
        (
            "sv_warnings_tk",
            5,
            "Number of teamkills that needs to be done before a temporary ban is handed out",
        ),
    ] {
        cvars.register(
            Cvar::int(name, default, description)
                .flags(CvarFlags::SERVER)
                .range(0.0, 100.0),
        );
    }
    cvars.register(Cvar::bool("demo_autorecord", false, "Auto record demos"));
    cvars.register(
        Cvar::int("net_lan", 0, "Set to 1 to set server to LAN mode")
            .flags(CvarFlags::SERVER)
            .range(0.0, 1.0),
    );
    cvars.register(
        Cvar::int(
            "net_t1_deadsnapshot",
            50,
            "How often to send dead sprite snapshot packets on the internet in ticks (60 ticks = 1 second)",
        )
        .flags(CvarFlags::SERVER)
        .range(1.0, 1000.0),
    );
    cvars.register(
        Cvar::int(
            "net_t1_thingsnapshot",
            31,
            "How often to send thing snapshot packets on the internet in ticks (60 ticks = 1 second)",
        )
        .flags(CvarFlags::SERVER)
        .range(1.0, 1000.0),
    );
    for (name, default, description) in [
        (
            "net_floodingpacketslan",
            80,
            "When running on a LAN, controls how many packets should be considered flooding",
        ),
        (
            "net_floodingpacketsinternet",
            42,
            "When running on the Internet, controls how many packets should be considered \
             flooding",
        ),
    ] {
        cvars.register(
            Cvar::int(name, default, description)
                .flags(CvarFlags::SERVER)
                .range(0.0, 100.0),
        );
    }
    for (name, default, description) in [
        ("sv_greeting", "Welcome", "First greeting message"),
        ("sv_greeting2", "", "Second greeting message"),
        ("sv_greeting3", "", "Third greeting message"),
        (
            "sv_maplist",
            "mapslist.txt",
            "Sets the name of maplist file",
        ),
    ] {
        cvars.register(Cvar::string(name, default, description).flags(CvarFlags::SERVER));
    }
    cvars.register(
        Cvar::bool("sv_balanceteams", false, "Enables/disables team balancing")
            .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::bool(
            "sv_botbalance",
            false,
            "Whether or not bots should count as players in the team balance",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(Cvar::bool("log_enable", true, "Enables logging to file"));
    cvars.register(
        Cvar::int(
            "log_level",
            0,
            "Sets log level. 0 = Off, 1 = Debug/Noteworthy events, 2 = Trace function entries",
        )
        .range(0.0, 2.0),
    );
    cvars.register(Cvar::int(
        "log_filesupdate",
        3600,
        "How often the log files should be updated in ticks (60 ticks = 1 second)",
    ));
    cvars.register(
        Cvar::int(
            "sv_votepercent",
            60,
            "Percentage of players in favor of a map/kick vote to let it pass",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 200.0),
    );
    cvars.register(
        Cvar::int(
            "sv_maxplayers",
            24,
            "Max number of players that can play on server",
        )
        .flags(CvarFlags::SERVER)
        .range(1.0, 32.0),
    );
    cvars.register(
        Cvar::int("sv_maxspectators", 10, "Sets the limit of spectators")
            .flags(CvarFlags::SERVER)
            .range(0.0, 32.0),
    );
    cvars.register(
        Cvar::bool(
            "sv_advancedspectator",
            true,
            "Enables/disables advanced spectator mode",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::bool(
            "sv_sniperline",
            false,
            "Enables/disables the Sniper Line on server",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::bool(
            "sv_minimap_locations",
            true,
            "Enables/disables drawing player and object location indicators on minimap",
        )
        .flags(CvarFlags::SERVER | CvarFlags::SYNC),
    );
    cvars.register(
        Cvar::bool(
            "sv_teamcolors",
            true,
            "Overwrites shirt color in team games",
        )
        .flags(CvarFlags::SERVER),
    );
    cvars.register(
        Cvar::int(
            "sv_bonus_frequency",
            0,
            "The interval of bonuses occurring ingame",
        )
        .flags(CvarFlags::SERVER)
        .range(0.0, 5.0),
    );
    for (name, what) in [
        ("sv_bonus_flamer", "Flamer"),
        ("sv_bonus_predator", "Predator"),
        ("sv_bonus_berserker", "Berserker"),
        ("sv_bonus_vest", "Bulletproof Vest"),
        ("sv_bonus_cluster", "Cluster Grenades"),
    ] {
        cvars.register(
            Cvar::bool(name, false, &format!("{what} bonus availability")).flags(CvarFlags::SERVER),
        );
    }
}

/// Which bonuses spawn and how often (`sv_bonus_*`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BonusConfig {
    pub frequency: i32,
    pub flamer: bool,
    pub predator: bool,
    pub berserker: bool,
    pub vest: bool,
    pub cluster: bool,
}

/// Spawn protection ticks (`DEFAULT_CEASEFIRE_TIME`).
pub const CEASEFIRE_TIME: i32 = 90;
const ILUMINATESPEED: f64 = 0.085;
const MELEE_DIST: f32 = 12.0;
/// Bullet time comes when nobody playing is farther than this from the killer
/// (`BULLETTIME_MINDISTANCE`), and lasts this many (slow) ticks.
const BULLETTIME_MINDISTANCE: f32 = 320.0;
const BULLET_TIME_TICKS: i32 = 30;
/// Commands a player can give its soldier (`CommandPlayerCommand`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    /// Chew tobacco and spit.
    Tabac,
    /// Light a cigar.
    Smoke,
    /// Take the helmet off (or put it back on).
    TakeOff,
    Victory,
    Piss,
    /// Shoot yourself.
    Mercy,
    Pwn,
    Kill,
    BrutalKill,
}

impl PlayerCommand {
    pub fn from_name(name: &str) -> Option<PlayerCommand> {
        Some(match name {
            "tabac" => PlayerCommand::Tabac,
            "smoke" => PlayerCommand::Smoke,
            "takeoff" => PlayerCommand::TakeOff,
            "victory" => PlayerCommand::Victory,
            "piss" => PlayerCommand::Piss,
            "mercy" => PlayerCommand::Mercy,
            "pwn" => PlayerCommand::Pwn,
            "kill" => PlayerCommand::Kill,
            "brutalkill" => PlayerCommand::BrutalKill,
            _ => return None,
        })
    }
}

/// `SURVIVAL_RESPAWNTIME`
const SURVIVAL_RESPAWNTIME: i32 = 60 * 5;

/// Game rules derived from cvars, read by the simulation every tick.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    pub gravity: f32,
    pub realistic_mode: bool,
    /// A network client's world: it simulates the players from their controls, but damage,
    /// deaths, respawns, things being taken or timing out, captures and the end of the
    /// match come from the server.
    pub client: bool,
    pub friendly_fire: bool,
    pub respawn_time: i32,
    pub ceasefire_time: i32,
    pub max_grenades: i32,
    pub survival_mode: bool,
    /// `sv_survivalmode_clearweapons`
    pub survival_clear_weapons: bool,
    /// `sv_advancemode`: kills give weapons, deaths take them.
    pub advance_mode: bool,
    /// `sv_advancemode_amount`: every this many kills (or deaths).
    pub advance_mode_amount: i32,
    pub bonuses: BonusConfig,
    pub game_mode: GameMode,
    /// `sv_killlimit`, from the mode's limit cvar.
    pub kill_limit: i32,
    pub time_limit: i32,
    pub respawn_min_wave: i32,
    pub respawn_max_wave: i32,
    /// Infiltration: points for a capture, seconds per blue point; HTF points time.
    pub inf_red_award: i32,
    pub inf_blue_limit: i32,
    pub htf_points_time: i32,
    /// `bots_difficulty`: 300 stupid, 200 poor, 100 normal, 50 hard, 10 impossible.
    pub bots_difficulty: i32,
    /// `sv_teamcolors`: team games dress each team in its colour.
    pub team_colors: bool,
    /// `r_maxsparks`, which some effects go by.
    pub max_sparks: i32,
    /// `bots_chat`: bots talk (and draw from Random for it, like Soldat's server).
    pub bots_chat: bool,
    /// `sv_healthcooldown`: seconds between medikits (0: no wait).
    pub health_cooldown: i64,
    /// `sv_bullettime`: the game slows down around a kill with everyone near.
    pub bullet_time: bool,
    /// `sv_guns_collide`, `sv_kits_collide`: bullets hit dropped guns, kits.
    pub guns_collide: bool,
    pub kits_collide: bool,
    /// Weapon stats for this mode (`Guns`).
    pub weapons: Arc<WeaponTable>,
    /// The game as it is this tick, for what sees only the config: never from the cvars, so a
    /// config made again from them loses nothing ([`World::set_rules`]).
    pub now: TickState,
}

/// The game this tick, as the config passes it on: the match's, copied by the world at each
/// step, and the client's spark count.
#[derive(Debug, Clone, Copy, Default)]
pub struct TickState {
    /// `WaveRespawnCounter`, for team mode respawns.
    pub wave_respawn_counter: i32,
    /// `SinusCounter`: the phase of the spawn protection's blinking.
    pub sinus_counter: f32,
    /// `SurvivalEndRound`
    pub survival_end_round: bool,
    /// `SortedPlayers[1]` as of the last `SortPlayers`.
    pub leader: Option<SoldierId>,
    /// The client's `SparksCount`, which some effects go by (set by the client).
    pub sparks_count: i32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            gravity: GRAV,
            realistic_mode: false,
            client: false,
            friendly_fire: false,
            respawn_time: 180,
            ceasefire_time: CEASEFIRE_TIME,
            max_grenades: 2,
            survival_mode: false,
            survival_clear_weapons: false,
            advance_mode: false,
            advance_mode_amount: 2,
            bonuses: BonusConfig::default(),
            game_mode: GameMode::Deathmatch,
            kill_limit: 30,
            time_limit: 36000,
            respawn_min_wave: 120,
            respawn_max_wave: 240,
            inf_red_award: 30,
            inf_blue_limit: 5,
            htf_points_time: 5,
            bots_difficulty: 100,
            team_colors: true,
            max_sparks: MAX_SPARKS,
            bots_chat: false,
            health_cooldown: HEALTH_COOLDOWN,
            bullet_time: false,
            guns_collide: false,
            kits_collide: false,
            weapons: Arc::new(WeaponTable::default()),
            now: TickState::default(),
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
        let game_mode = GameMode::from_num(cvars.int("sv_gamemode"));
        WorldConfig {
            gravity: cvars.float("sv_gravity"),
            realistic_mode,
            client: false,
            friendly_fire: cvars.bool("sv_friendlyfire"),
            respawn_time: cvars.int("sv_respawntime") as i32,
            ceasefire_time: CEASEFIRE_TIME,
            max_grenades: cvars.int("sv_maxgrenades") as i32,
            survival_mode: cvars.bool("sv_survivalmode"),
            survival_clear_weapons: cvars.bool("sv_survivalmode_clearweapons"),
            advance_mode: cvars.bool("sv_advancemode"),
            advance_mode_amount: cvars.int("sv_advancemode_amount").max(1) as i32,
            game_mode,
            kill_limit: cvars.int(game_mode.limit_cvar()) as i32,
            time_limit: cvars.int("sv_timelimit") as i32,
            respawn_min_wave: cvars.int("sv_respawntime_minwave") as i32,
            respawn_max_wave: cvars.int("sv_respawntime_maxwave") as i32,
            inf_red_award: cvars.int("sv_inf_redaward") as i32,
            inf_blue_limit: cvars.int("sv_inf_bluelimit") as i32,
            htf_points_time: cvars.int("sv_htf_pointstime") as i32,
            bots_difficulty: cvars.int("bots_difficulty") as i32,
            team_colors: cvars.bool("sv_teamcolors"),
            max_sparks: cvars.int("r_maxsparks") as i32,
            bots_chat: cvars.bool("bots_chat"),
            bonuses: BonusConfig {
                frequency: cvars.int("sv_bonus_frequency") as i32,
                flamer: cvars.bool("sv_bonus_flamer"),
                predator: cvars.bool("sv_bonus_predator"),
                berserker: cvars.bool("sv_bonus_berserker"),
                vest: cvars.bool("sv_bonus_vest"),
                cluster: cvars.bool("sv_bonus_cluster"),
            },
            health_cooldown: cvars.int("sv_healthcooldown"),
            bullet_time: cvars.bool("sv_bullettime"),
            guns_collide: cvars.bool("sv_guns_collide"),
            kits_collide: cvars.bool("sv_kits_collide"),
            weapons: data.weapons(realistic_mode).clone(),
            now: TickState::default(),
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
    /// A bullet hurt another soldier who was alive until then, the first soldier it
    /// hurt (`Bullet.HasHit`): a hit in the weapon stats.
    Hit {
        attacker: SoldierId,
        victim: SoldierId,
        /// The bullet's weapon.
        weapon: WeaponKind,
    },
    Killed {
        victim: SoldierId,
        killer: SoldierId,
        how: DeathKind,
        /// The bullet's weapon (`KillBullet`), `None` for polygons, falls and suicides.
        weapon: Option<WeaponKind>,
        /// The killing bullet hit the head.
        headshot: bool,
        /// The skeleton point the killing hit struck (`Where`; 1 without a bullet).
        hit: u8,
        /// The last killing shot (the server's `ShotDistance`, `ShotLife`, `ShotRicochet`,
        /// which a kill without one leaves as they were).
        shot: Shot,
    },
    /// A soldier took a thing (`ServerThingTaken`): a weapon, kit, flag or stationary gun.
    ThingTaken {
        slot: usize,
        kind: ThingKind,
        who: SoldierId,
        /// Where the thing was.
        pos: Vec2,
    },
    /// A flag was brought home: `team` scores (`ServerFlagInfo`).
    FlagCaptured { team: Team, who: SoldierId },
    /// A spark for the client's effects.
    Spark(SparkSpawn),
    /// A player said something (bots chat: `ServerSendStringMessage`).
    Chat { who: SoldierId, text: String },
    /// An idle animation or taunt started (`ServerIdleAnimation`).
    IdleAnimation { who: SoldierId, style: i8 },
    /// A sound to play; a soldier's sounds can use its channels.
    Sound {
        soldier: Option<SoldierId>,
        sound: SoundEvent,
    },
    /// A limit was reached; the map changes after [`MAP_CHANGE_TIME`] ticks.
    MatchEnded,
    /// Time to load the next map.
    ChangeMap,
}

/// A killing shot, for the killer's screen: how far the bullet flew (metres), how long
/// (seconds) and how often it bounced.
#[derive(Debug, Clone, Copy, Default, PartialEq, bitcode::Encode, bitcode::Decode)]
pub struct Shot {
    pub distance: f32,
    pub life: f32,
    pub ricochets: u8,
}

#[derive(Clone)]
pub struct World {
    pub data: Arc<GameData>,
    pub map: MapFile,
    pub config: WorldConfig,
    pub soldiers: SlotMap<SoldierId, Soldier>,
    pub bullets: Vec<Bullet>,
    /// Thing slots (`Thing[1..MAX_THINGS]`).
    pub things: Vec<Thing>,
    pub game: Match,
    /// `HTFTime`: ticks between Hold the Flag points.
    htf_time: u64,
    pub tick: u64,
    /// Gameplay randomness, bit-compatible with Soldat's.
    pub rng: PascalRandom,
    emitter: Vec<EmitterItem>,
    /// Sounds of bullets and things this tick.
    sounds: Vec<SoundEvent>,
    /// Sparks of bullets and things this tick.
    sparks: Vec<SparkSpawn>,
    /// Events of things this tick (and of bullets from the network since the last one).
    pub(crate) thing_events: Vec<GameEvent>,
    /// In a network game, the bullets to send: a client's own, the server's to the other
    /// players (`ClientSendBullet`, `ServerBulletSnapshot`). `None` plays alone.
    pub net_bullets: Option<Vec<crate::net::NetBullet>>,
    /// Bullet time ticks left (`BulletTimeTimer`), -1 without.
    pub bullet_time: i32,
    /// The game's clocks (the time limit, the map change, the respawn waves) ran this tick:
    /// they stand still in bullet time.
    pub clocks_ran: bool,
    /// The last killing shot.
    pub shot: Shot,
}

/// `CreateSprite` for a spectator: at rest far off the map, empty-handed. The skeleton
/// stays where the model has it (`MoveSkeleton(0, 0)` on the server).
fn park_spectator(soldier: &mut Soldier, map: &MapFile, data: &GameData, weapons: &WeaponTable) {
    soldier.weapons[soldier.active_weapon] = weapons.get(WeaponKind::NoWeapon);
    let far = (f64::from(MIN_SECTORZ * map.sectors_division) * 0.8) as f32;
    let pos = vec2(far, far);
    soldier.particle.pos = pos;
    soldier.particle.old_pos = pos;
    soldier.particle.velocity = Vec2::ZERO;
    soldier.particle.force = Vec2::ZERO;
    soldier.skeleton = data.soldier_skeleton.clone();
}

impl World {
    pub fn new(data: Arc<GameData>, map: MapFile, config: WorldConfig) -> World {
        let game = Match::new(&config, 0);
        World {
            data,
            map,
            config,
            soldiers: SlotMap::with_key(),
            bullets: vec![Bullet::default(); MAX_BULLETS],
            things: vec![Thing::default(); MAX_THINGS],
            game,
            htf_time: 300,
            tick: 0,
            rng: PascalRandom::new(1),
            emitter: Vec::new(),
            sounds: Vec::new(),
            sparks: Vec::new(),
            thing_events: Vec::new(),
            net_bullets: None,
            bullet_time: -1,
            clocks_ran: false,
            shot: Shot::default(),
        }
    }

    /// Weapons with new stats (`LoadWeapons`): each soldier's weapon in hand again, its
    /// bullets kept (`ApplyWeaponByNum(Weapon.Num, 1, AmmoCount)`).
    pub fn reapply_weapons(&mut self) {
        let weapons = self.config.weapons.clone();
        for soldier in self.soldiers.values_mut().filter(|s| s.active) {
            let weapon = &mut soldier.weapons[soldier.active_weapon];
            let ammo = weapon.ammo_count;
            *weapon = weapons.get(weapon.kind);
            weapon.ammo_count = ammo;
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
        // advance mode: no primaries to begin with (`ChangeMap`)
        if self.config.advance_mode {
            soldier.weapon_sel = ALL_WEAPONS & !PRIMARIES;
        }
        let id = self.soldiers.insert(soldier);
        self.sort_players(&mut Vec::new());
        id
    }

    /// A player command (`CommandPlayerCommand`): the taunt animations, `mercy` and the
    /// suicides.
    pub fn player_command(
        &mut self,
        id: SoldierId,
        command: PlayerCommand,
        events: &mut Vec<GameEvent>,
    ) {
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        let idle = match command {
            PlayerCommand::Tabac => Some(0),
            PlayerCommand::Smoke => Some(1),
            PlayerCommand::TakeOff => Some(4),
            PlayerCommand::Victory => Some(5),
            PlayerCommand::Piss => Some(6),
            PlayerCommand::Mercy => Some(7),
            PlayerCommand::Pwn => Some(8),
            PlayerCommand::Kill | PlayerCommand::BrutalKill => None,
        };
        if let Some(idle) = idle {
            soldier.idle_random = idle;
            soldier.idle_time = 1;
        }
        if matches!(
            command,
            PlayerCommand::Mercy | PlayerCommand::Kill | PlayerCommand::BrutalKill
        ) && soldier.kills > 0
        {
            soldier.kills -= 1;
        }

        let amount = match command {
            PlayerCommand::Kill => 150.0,
            PlayerCommand::BrutalKill => 3423.0,
            _ => return,
        };
        self.suicide(id, amount, events);
    }

    /// No vest, and `amount` of damage from itself (`HealthHit(amount, Num, 1, -1, ...)`):
    /// the suicide commands, and an admin's `pkill`.
    pub fn suicide(&mut self, id: SoldierId, amount: f32, events: &mut Vec<GameEvent>) {
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        soldier.vest = 0.0;
        if let Some(how) = soldier.self_hit(amount, 1, Vec2::ZERO, &self.config, &mut self.rng) {
            events.push(GameEvent::Killed {
                victim: id,
                killer: id,
                how,
                weapon: None,
                headshot: false,
                hit: 1,
                shot: self.shot,
            });
            self.on_death(id, id, events);
        }
    }

    /// Stationary guns on the map's spawnpoints of team 16 (`sv_stationaryguns`).
    pub fn spawn_stationary_guns(&mut self) {
        let spots: Vec<Vec2> = self
            .map
            .spawnpoints
            .iter()
            .filter(|s| s.active && s.team == 16)
            .map(|s| vec2(s.x as f32, s.y as f32))
            .collect();
        for pos in spots {
            self.create_thing(ThingKind::StationaryGun, pos);
        }
    }

    /// Adds a computer player (`AddBotPlayer`): it joins `team` and spawns.
    pub fn spawn_bot(&mut self, profile: &BotProfile, team: Team) -> SoldierId {
        // AddBotPlayer picks a start point the respawn replaces
        let _ = randomize_start(&self.map, team, &mut self.rng);
        let id = self.spawn_soldier();
        // `TargetNum := 1`: the first player
        let first = self.soldiers.keys().next();
        let difficulty = self.config.bots_difficulty;

        let soldier = &mut self.soldiers[id];
        soldier.team = team;
        soldier.name = profile.name.clone();
        soldier.looks = profile.looks;
        // LoadBotConfig reads the shirt colour only for players without a team
        if team != Team::None {
            soldier.looks.shirt = 0;
        }
        soldier.looks.apply_team_shirt(team, &self.config);
        soldier.head_cap = profile.head_cap;
        soldier.wear_helmet = u8::from(profile.head_cap != 0);
        soldier.brain = Some(profile.brain(id, first, difficulty));
        soldier.connection_quality = 0;
        if team == Team::Spectator {
            park_spectator(soldier, &self.map, &self.data, &self.config.weapons);
        }
        self.respawn_soldier(id);
        self.sort_players(&mut Vec::new());
        id
    }

    /// A player leaves (`TSprite.Kill`): its flag falls, its stationary gun is free again,
    /// and a team left without players loses its score.
    pub fn remove_soldier(&mut self, id: SoldierId) {
        let Some(soldier) = self.soldiers.remove(id) else {
            return;
        };
        for thing in self.things.iter_mut() {
            if thing.holding == Some(id) && thing.kind.is_flag() {
                thing.holding = None;
            }
        }
        if let Some(gun) = soldier.stat {
            self.things[gun].static_type = false;
        }
        let solo = self.config.game_mode == GameMode::Deathmatch || soldier.team == Team::None;
        if !solo
            && !self
                .soldiers
                .values()
                .any(|s| s.active && s.team == soldier.team)
        {
            self.game.team_scores[soldier.team as usize] = 0;
        }
        self.sort_players(&mut Vec::new());
    }

    /// `ChangeTeam` to the spectators: the weapon drops, a held flag goes home, and the
    /// soldier waits far off the map, forever dead (`CreateSprite` puts spectators at
    /// `MIN_SECTORZ * SectorsDivision * 0.8`; `Respawn` leaves them there).
    pub fn join_spectators(&mut self, id: SoldierId) {
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        soldier.drop_weapon(&self.config);
        self.process_drops();
        self.add_spectator(id);
    }

    /// A soldier watches from the spectators without having played (a joining player's,
    /// `ServerHandlePlayerInfo`): nothing to drop.
    pub fn add_spectator(&mut self, id: SoldierId) {
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        soldier.team = Team::Spectator;
        park_spectator(soldier, &self.map, &self.data, &self.config.weapons);
        soldier.dead_meat = true;
        soldier.respawn_held = soldier.holded_thing.take();
        soldier.holds_flag = false;
        soldier.parachute = false;
        soldier.stat = None;
        self.process_parachute(id);
        self.sort_players(&mut Vec::new());
    }

    /// Respawns a soldier right away (`TSprite.Respawn`), e.g. when joining a team.
    pub fn respawn_soldier(&mut self, id: SoldierId) {
        self.config.now.survival_end_round = self.game.survival_end_round;
        self.soldiers[id].respawn(&self.map, &self.config, &mut self.rng);
        self.clear_weapons(id);
        self.process_parachute(id);
        self.process_survival(id, &mut Vec::new());
    }

    /// `sv_survivalmode_clearweapons`: the first respawn once a survival round is over
    /// takes the dropped weapons away (`TSprite.Respawn`); a weapon dropped since makes it
    /// clear again (`DropWeapon`).
    fn clear_weapons(&mut self, id: SoldierId) {
        // anyone's drop since (also on the bullets' turn)
        for soldier in self.soldiers.values_mut() {
            if std::mem::take(&mut soldier.dropped_weapon) {
                self.game.weapons_cleaned = false;
            }
        }
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        let respawned = std::mem::take(&mut soldier.respawned);
        if respawned
            && self.config.survival_clear_weapons
            && self.game.survival_end_round
            && !self.game.weapons_cleaned
            && !self.config.client
        {
            for thing in self
                .things
                .iter_mut()
                .filter(|t| t.active && t.kind.is_gun())
            {
                thing.kill();
            }
            self.game.weapons_cleaned = true;
        }
    }

    /// The scoring of `TSprite.Die`.
    fn score_kill(
        &mut self,
        victim: SoldierId,
        killer: SoldierId,
        victim_weapon: WeaponKind,
        bullet_weapon: Option<WeaponKind>,
    ) {
        let holds_flag = self
            .soldiers
            .get(killer)
            .and_then(|k| k.holded_thing)
            .is_some_and(|t| {
                self.things[t].active && self.things[t].kind == ThingKind::PointmatchFlag
            });
        let kill = ScoredKill {
            victim,
            killer,
            victim_weapon,
            bullet_weapon,
            killer_holds_pointmatch_flag: holds_flag,
        };
        score_kill(
            &mut self.soldiers,
            &mut self.game.team_scores,
            self.config.game_mode,
            kill,
        );
        self.advance_kill(victim, killer);
    }

    /// Advance mode (`BREAD`, as Soldat's clients have it: its server's is inverted): every
    /// `sv_advancemode_amount` kills of an enemy win the killer a random primary.
    fn advance_kill(&mut self, victim: SoldierId, killer: SoldierId) {
        if !self.config.advance_mode || self.config.client || victim == killer {
            return;
        }
        let (Some(v), Some(k)) = (self.soldiers.get(victim), self.soldiers.get(killer)) else {
            return;
        };
        let enemies = v.team != k.team || v.team == Team::None;
        let locked = PRIMARIES & !k.weapon_sel;
        if !enemies || k.kills % self.config.advance_mode_amount != 0 || locked == 0 {
            return;
        }
        let won = loop {
            let j = self.rng.below(10);
            if locked & (1 << j) != 0 {
                break j;
            }
        };
        self.soldiers[killer].weapon_sel |= 1 << won;
    }

    /// Advance mode: every `sv_advancemode_amount` deaths lose a random primary.
    fn advance_death(&mut self, victim: SoldierId) {
        if !self.config.advance_mode || self.config.client {
            return;
        }
        let Some(v) = self.soldiers.get(victim) else {
            return;
        };
        let owned = v.weapon_sel & PRIMARIES;
        if v.deaths % self.config.advance_mode_amount != 0 || owned == 0 {
            return;
        }
        let lost = loop {
            let j = self.rng.below(10);
            if owned & (1 << j) != 0 {
                break j;
            }
        };
        self.soldiers[victim].weapon_sel &= !(1 << lost);
    }

    /// `SortPlayers` (server side): kill limits and the wave respawn time.
    fn sort_players(&mut self, events: &mut Vec<GameEvent>) {
        self.game.leader = sort_players(&self.soldiers).first().copied();
        self.config.now.leader = self.game.leader;
        if !self.config.client {
            self.game.check_limits(&self.config, &self.soldiers, events);
        }
        let players = self.soldiers.values().filter(|s| s.active).count();
        self.game.update_wave_respawn_time(&self.config, players);
    }

    pub(crate) fn thing_ctx(&mut self) -> ThingCtx<'_> {
        ThingCtx {
            data: &self.data,
            map: &self.map,
            config: &self.config,
            soldiers: &mut self.soldiers,
            rng: &mut self.rng,
            game: &mut self.game,
            scored: false,
            survival_capture: false,
            tick: self.tick,
            bullets: Vec::new(),
            sounds: &mut self.sounds,
            sparks: &mut self.sparks,
            events: &mut self.thing_events,
        }
    }

    /// A stationary gun bullet. A bot pays it with its own weapon's ammo, like any bullet
    /// the server sends on (`ServerBulletSnapshot`).
    fn fire_stationary_gun(
        &mut self,
        owner: SoldierId,
        params: &BulletParams,
        events: &mut Vec<GameEvent>,
    ) {
        if !self.create_bullet(params, owner) {
            return;
        }
        events.push(GameEvent::BulletFired {
            owner,
            weapon: params.weapon,
            pos: params.position,
        });
        if let Some(soldier) = self.soldiers.get_mut(owner)
            && soldier.brain.is_some()
        {
            let weapon = &mut soldier.weapons[soldier.active_weapon];
            if weapon.ammo_count > 0 && pays_ammo(weapon, params.style) {
                weapon.ammo_count -= 1;
            }
        }
    }

    /// `TSprite.Die`'s bullet time (`sv_bullettime`): a kill with everyone playing near the
    /// killer slows the game down a while (`ToggleBulletTime`).
    pub(crate) fn bullet_time_kill(&mut self, killer: SoldierId) {
        if !self.config.bullet_time || self.bullet_time > 0 {
            return;
        }
        let Some(at) = self.soldiers.get(killer).map(|s| s.particle.pos) else {
            return;
        };
        let far = self.soldiers.iter().any(|(id, s)| {
            s.active
                && id != killer
                && !s.is_spectator()
                && distance(s.particle.pos, at) > BULLETTIME_MINDISTANCE
        });
        if !far {
            self.bullet_time = BULLET_TIME_TICKS;
        }
    }

    /// New rules (cvars changed): the game's state in the config stays, and whether this is
    /// a network client's world.
    pub fn set_rules(&mut self, rules: WorldConfig) {
        let (now, client) = (self.config.now, self.config.client);
        self.config = rules;
        self.config.now = now;
        self.config.client = client;
    }

    /// Ticks a second: fewer in bullet time (`GOALTICKS`).
    pub fn goal_ticks(&self) -> u32 {
        if self.bullet_time > 0 {
            TICKS_PER_SECOND / 3
        } else {
            TICKS_PER_SECOND
        }
    }

    fn set_survival_end_round(&mut self, over: bool) {
        self.game.survival_end_round = over;
        self.config.now.survival_end_round = over;
    }

    /// A new death, after `TSprite.Die`: bullet time, and the survival mode part, which ends
    /// the round when only one player (or team) is left.
    fn on_death(&mut self, victim: SoldierId, killer: SoldierId, events: &mut Vec<GameEvent>) {
        self.bullet_time_kill(killer);
        self.advance_death(victim);
        self.sort_players(events);

        // TSprite.Die compares the stationary gun's thing number with the soldier's own
        // sprite number (`if Stat = Num`), and then frees the first thing slot
        let num = self
            .soldiers
            .keys()
            .position(|k| k == victim)
            .map(|i| i + 1);
        if let Some(soldier) = self.soldiers.get_mut(victim)
            && soldier.stat.is_some_and(|gun| Some(gun + 1) == num)
        {
            soldier.stat = None;
            self.things[0].static_type = true;
        }

        if !self.config.survival_mode {
            return;
        }

        let alive = |s: &&Soldier| s.active && !s.dead_meat;
        let round_over = match self.config.game_mode {
            GameMode::Deathmatch | GameMode::Rambo => {
                let over = self.soldiers.values().filter(alive).count() < 2;
                if over {
                    // the last one standing roars
                    for s in self
                        .soldiers
                        .values_mut()
                        .filter(|s| s.active && !s.dead_meat)
                    {
                        s.play(Sfx::Roar);
                    }
                }
                over
            }
            GameMode::CaptureTheFlag
            | GameMode::Infiltration
            | GameMode::HoldTheFlag
            | GameMode::Teammatch => {
                let mut teams = [0usize; 6];
                for s in self.soldiers.values().filter(alive) {
                    teams[s.team as usize] += 1;
                }
                let teams_alive = teams[1..].iter().filter(|&&n| n > 0).count();
                if teams_alive <= 1 && !self.game.survival_end_round {
                    // the team left standing scores
                    match self.config.game_mode {
                        GameMode::CaptureTheFlag => {
                            let scores = &mut self.game.team_scores;
                            for (score, alive) in scores[1..=2].iter_mut().zip(&teams[1..=2]) {
                                if *alive > 0 {
                                    *score += 1;
                                }
                            }
                        }
                        GameMode::Infiltration => {
                            let scores = &mut self.game.team_scores;
                            if teams[1] > 0 {
                                scores[1] += self.config.inf_red_award;
                            }
                            // penalty
                            let (alpha, bravo) = team_sizes(&self.config, &self.soldiers);
                            if alpha > bravo {
                                scores[1] -= 5 * (alpha - bravo);
                            }
                            scores[1] = scores[1].max(0);
                        }
                        _ => {}
                    }
                }
                if teams_alive <= 1 {
                    // the survivors cheer
                    for s in self
                        .soldiers
                        .values_mut()
                        .filter(|s| s.active && !s.dead_meat)
                    {
                        s.idle_random = 5;
                        s.idle_time = 1;
                    }
                }
                teams_alive <= 1
            }
            _ => false,
        };

        if round_over {
            for s in self.soldiers.values_mut().filter(|s| s.active) {
                s.respawn_counter = SURVIVAL_RESPAWNTIME;
            }
            self.set_survival_end_round(true);
        }
    }

    /// `HealthHit(amount, self)`: kills a soldier by itself, without counting the death
    /// (the end of a survival round, joining a running round).
    pub fn kill_soldier(&mut self, id: SoldierId, amount: f32, events: &mut Vec<GameEvent>) {
        let soldier = &mut self.soldiers[id];
        if let Some(how) = soldier.self_hit(amount, 1, Vec2::ZERO, &self.config, &mut self.rng) {
            soldier.deaths -= 1;
            events.push(GameEvent::Killed {
                victim: id,
                killer: id,
                how,
                weapon: None,
                headshot: false,
                hit: 1,
                shot: self.shot,
            });
            self.on_death(id, id, events);
        }
    }

    /// Kills everybody still alive when a survival round is over.
    fn kill_survivors(&mut self, events: &mut Vec<GameEvent>) {
        let alive: Vec<SoldierId> = self
            .soldiers
            .iter()
            .filter(|(_, s)| s.active && !s.dead_meat)
            .map(|(id, _)| id)
            .collect();
        for id in alive {
            self.kill_soldier(id, 4000.0, events);
        }
    }

    /// Survival mode requests of a soldier's update.
    fn process_survival(&mut self, id: SoldierId, events: &mut Vec<GameEvent>) {
        let soldier = &mut self.soldiers[id];
        let died = std::mem::take(&mut soldier.survival_died);
        let respawned = std::mem::take(&mut soldier.survival_respawned);
        let round_over = std::mem::take(&mut soldier.survival_round_over);

        if died {
            events.push(GameEvent::Killed {
                victim: id,
                killer: id,
                how: DeathKind::Normal,
                weapon: None,
                headshot: false,
                hit: 1,
                shot: self.shot,
            });
            self.on_death(id, id, events);
        }

        // the round goes on until the last one is back
        if respawned {
            let anyone_dead = self.soldiers.values().any(|s| s.active && s.dead_meat);
            self.set_survival_end_round(anyone_dead);
        }

        if round_over {
            self.kill_survivors(events);
            // flags go home
            if self.config.game_mode != GameMode::HoldTheFlag {
                let flag = |kind| self.things.iter().position(|t| t.active && t.kind == kind);
                if let (Some(a), Some(b)) = (flag(ThingKind::AlphaFlag), flag(ThingKind::BravoFlag))
                {
                    for slot in [a, b] {
                        if !self.things[slot].in_base {
                            let mut thing = std::mem::take(&mut self.things[slot]);
                            let others = std::mem::take(&mut self.things);
                            thing.respawn(slot, &mut self.thing_ctx());
                            self.things = others;
                            self.things[slot] = thing;
                        }
                    }
                }
            }
        }
    }

    /// Turns weapons soldiers let go of (thrown, or dropped by dying) into things.
    pub(crate) fn process_drops(&mut self) {
        let ids: Vec<SoldierId> = self.soldiers.keys().collect();
        for id in ids {
            let soldier = &mut self.soldiers[id];
            let (drop, released) = (soldier.pending_drop.take(), soldier.release_things);
            soldier.release_things = false;
            // a client's things come from the server
            let drop = drop.filter(|_| !self.config.client);

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

            // a dead soldier drops its flag and no longer owns anything (TSprite.Die)
            if released {
                for thing in self.things.iter_mut() {
                    if thing.holding == Some(id) && thing.kind.is_flag() {
                        thing.holding = None;
                    }
                    if thing.owner == Some(id) {
                        thing.owner = None;
                    }
                }
            }
        }
    }

    /// Random bonus kits (`UpdateFrame`, `sv_bonus_*`).
    fn spawn_bonuses(&mut self) {
        let bonuses = self.config.bonuses;
        if self.config.survival_mode || self.config.realistic_mode || bonuses.frequency <= 0 {
            return;
        }

        let freq: u64 = match bonuses.frequency {
            1 => 7400,
            2 => 4300,
            3 => 2500,
            4 => 1600,
            _ => 800,
        };
        let tick = self.tick;
        // the flag modes make cluster kits more likely (Round(4 * 0.75))
        let cluster_random = match self.config.game_mode {
            GameMode::CaptureTheFlag | GameMode::Infiltration | GameMode::HoldTheFlag => 3,
            _ => 4,
        };
        let rolls = [
            (bonuses.berserker, freq, 4, ThingKind::BerserkKit),
            (bonuses.flamer, 444, 5, ThingKind::FlamerKit),
            (bonuses.predator, freq, 5, ThingKind::PredatorKit),
            (bonuses.vest, freq / 2, 4, ThingKind::VestKit),
            (
                bonuses.cluster,
                freq / 2,
                cluster_random,
                ThingKind::ClusterKit,
            ),
        ];

        for (enabled, interval, random, kind) in rolls {
            if enabled && tick.is_multiple_of(interval) && self.rng.below(random) == 0 {
                let mut things = std::mem::take(&mut self.things);
                spawn_things(&mut things, &mut self.thing_ctx(), kind, 1);
                self.things = things;
            }
        }
    }

    /// `TSprite.ThrowFlag`: throws the held flag unless it would hit a wall right away.
    fn process_flag_throw(&mut self, id: SoldierId) {
        const FLAGTHROW_POWER: f32 = 4.225;
        let Some(throw) = self.soldiers[id].flag_throw.take() else {
            return;
        };
        let Some(i) = self
            .things
            .iter()
            .position(|t| t.active && t.holding == Some(id) && t.kind.is_flag())
        else {
            return;
        };

        let cursor = throw.aim * FLAGTHROW_POWER;
        // offset from the flagger so it isn't instantly grabbed again
        let offset = cursor * 5.0;
        let b = cursor + throw.velocity;

        let thing = &self.things[i];
        let shift = offset + b;
        let look = thing.skeleton.pos(1) + shift;
        let future = [
            vec2(-10.0, -8.0),
            vec2(10.0, -8.0),
            vec2(-10.0, 8.0),
            vec2(10.0, 8.0),
        ]
        .map(|d| look + d);
        let filter = RayCast {
            player: false,
            flag: true,
            bullet: false,
            check_collider: false,
            team: Team::None,
        };
        let clear = (2..=4).all(|n| {
            self.map
                .ray_cast(throw.hand, thing.skeleton.pos(n) + shift, 200.0, filter)
                .is_none()
        }) && future
            .iter()
            .all(|&p| self.map.collision_test(p, true).is_none());
        if !clear {
            return;
        }

        let thing = &mut self.things[i];
        for n in 1..=4 {
            let p = thing.skeleton.pos(n) + offset + b;
            *thing.skeleton.pos_mut(n) = p;
            *thing.skeleton.old_pos_mut(n) = p - b;
        }

        // some spin for visual effect
        let spin = vec2normalize(vec2(-b.y, b.x)) * f32::from(throw.direction);
        *thing.skeleton.pos_mut(1) -= spin;
        *thing.skeleton.pos_mut(2) += spin;

        thing.holding = None;
        thing.bg = BackgroundState::default();
        thing.static_type = false;
        let soldier = &mut self.soldiers[id];
        soldier.holded_thing = None;
        soldier.holds_flag = false;
        soldier.flag_grab_cooldown = 60 / 4;
    }

    /// `TSprite.Parachute` and letting go of it, on the thing side.
    fn process_parachute(&mut self, id: SoldierId) {
        // steering bends the parachute
        if let Some((t, bend)) = self.soldiers[id].parachute_bend.take()
            && self.things[t].active
        {
            let skeleton = &mut self.things[t].skeleton;
            let (down, up) = if bend > 0 { (3, 2) } else { (2, 3) };
            skeleton.force_mut(down).y -= 0.5;
            skeleton.force_mut(up).y += 0.5;
        }

        // Respawn: a held flag goes home, a parachute disappears
        if let Some(t) = self.soldiers[id].respawn_held.take()
            && self.things[t].active
        {
            if self.things[t].kind == ThingKind::Parachute {
                self.things[t].kill();
            } else {
                let mut thing = std::mem::take(&mut self.things[t]);
                let others = std::mem::take(&mut self.things);
                thing.respawn(t, &mut self.thing_ctx());
                self.things = others;
                self.things[t] = thing;
            }
        }

        let soldier = &mut self.soldiers[id];
        let call = std::mem::take(&mut soldier.parachute_call);
        let spawn = soldier.parachute_spawn.take();
        let release = std::mem::take(&mut soldier.release_parachute);

        if call {
            for thing in self.things.iter_mut().filter(|t| t.holding == Some(id)) {
                thing.holding = None;
                thing.kill();
            }
        }

        if let Some(pos) = spawn {
            let mut things = std::mem::take(&mut self.things);
            let ctx = &mut self.thing_ctx();
            if let Some(n) = create_thing(
                &mut things,
                ctx,
                pos,
                Some(id),
                ThingKind::Parachute,
                None,
                None,
            ) {
                things[n].holding = Some(id);
                things[n].color = self.soldiers[id].looks.shirt;
                self.soldiers[id].holded_thing = Some(n);
            }
            self.things = things;
        }

        if release {
            let held = self
                .things
                .iter_mut()
                .find(|t| t.active && t.kind == ThingKind::Parachute && t.holding == Some(id));
            if let Some(thing) = held {
                thing.holding = None;
                thing.skeleton.pop_constraint();
                thing.timeout = 3 * 60;
            }
        }
    }

    /// Game mode rules at the end of `UpdateFrame`: Infiltration and Hold the Flag
    /// points, the Rambo bow and keeping exactly one of each flag around.
    fn mode_rules(&mut self, events: &mut Vec<GameEvent>) {
        use GameMode::*;
        const HTF_SEC_POINT: u64 = 300;
        let mode = self.config.game_mode;
        let tick = self.tick;
        let (alpha, bravo) = team_sizes(&self.config, &self.soldiers);

        // Infiltration: the defenders score while their flag is home
        let mut j = (self.config.inf_blue_limit * 60) as u64;
        if alpha < bravo {
            j += 120 * (bravo - alpha) as u64;
        }
        let blue_flag_home = self
            .things
            .iter()
            .any(|t| t.active && t.kind == ThingKind::BravoFlag && t.in_base);
        if mode == Infiltration
            && !self.game.ended()
            && blue_flag_home
            && alpha > 0
            && bravo > 0
            && j > 0
            && tick.is_multiple_of(j)
        {
            self.game.team_scores[2] += 1;
            self.sort_players(events);
        }

        // Hold the Flag: the team holding the flag scores
        if bravo == alpha {
            self.htf_time = (self.config.htf_points_time * 60) as u64;
        }
        if mode == HoldTheFlag
            && !self.game.ended()
            && alpha > 0
            && bravo > 0
            && self.htf_time > 0
            && tick.is_multiple_of(self.htf_time)
        {
            let ids: Vec<SoldierId> = self.soldiers.keys().collect();
            for id in ids {
                let s = &self.soldiers[id];
                let holds = s.active
                    && s.holded_thing
                        .is_some_and(|t| self.things[t].kind == ThingKind::PointmatchFlag);
                if !holds {
                    continue;
                }
                let team = s.team;
                self.game.team_scores[team as usize] += 1;
                let bigger = match team {
                    Team::Alpha => alpha - bravo,
                    Team::Bravo => bravo - alpha,
                    _ => 0,
                };
                if matches!(team, Team::Alpha | Team::Bravo) {
                    self.htf_time = (HTF_SEC_POINT as i64 + 120 * bigger as i64)
                        .max(HTF_SEC_POINT as i64) as u64;
                }
                self.sort_players(events);
            }
        }

        // Rambo: a new bow when nobody has it
        if mode == Rambo && tick.is_multiple_of(60) {
            let bow_around = self
                .things
                .iter()
                .any(|t| t.active && t.kind == ThingKind::RamboBow)
                || self.soldiers.values().any(|s| {
                    s.active
                        && s.primary_weapon()
                            .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
                });
            if !bow_around {
                let (pos, _) = randomize_start_team(&self.map, 15, &mut self.rng);
                self.create_thing(ThingKind::RamboBow, pos);
            }
        }

        // destroy duplicate flags, bring back missing ones
        if tick.is_multiple_of(120) {
            let flags: &[(ThingKind, i32)] = match mode {
                CaptureTheFlag | Infiltration => {
                    &[(ThingKind::AlphaFlag, 5), (ThingKind::BravoFlag, 6)]
                }
                Pointmatch | HoldTheFlag => &[(ThingKind::PointmatchFlag, 14)],
                _ => &[],
            };
            for &(kind, team) in flags {
                let count = self
                    .things
                    .iter()
                    .filter(|t| t.active && t.kind == kind)
                    .count();
                if count > 1
                    && let Some(last) = self
                        .things
                        .iter_mut()
                        .rev()
                        .find(|t| t.active && t.kind == kind)
                {
                    last.kill();
                }
                if count == 0 {
                    let (pos, found) = randomize_start_team(&self.map, team, &mut self.rng);
                    if found {
                        self.create_thing(kind, pos);
                    }
                }
            }
        }
    }

    /// Flags or the bow of the game mode at map start.
    pub fn spawn_mode_things(&mut self) {
        use GameMode::*;
        match self.config.game_mode {
            Pointmatch | HoldTheFlag => {
                let (pos, _) = randomize_start_team(&self.map, 14, &mut self.rng);
                self.create_thing(ThingKind::PointmatchFlag, pos);
            }
            CaptureTheFlag | Infiltration => {
                for (kind, team) in [(ThingKind::AlphaFlag, 5), (ThingKind::BravoFlag, 6)] {
                    let (pos, found) = randomize_start_team(&self.map, team, &mut self.rng);
                    if found {
                        self.create_thing(kind, pos);
                    }
                }
            }
            Rambo => {
                let (pos, _) = randomize_start_team(&self.map, 15, &mut self.rng);
                self.create_thing(ThingKind::RamboBow, pos);
            }
            _ => {}
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
        self.create_bullet_in_slot(params, owner).is_some()
    }

    /// [`World::create_bullet`], telling the slot.
    pub(crate) fn create_bullet_in_slot(
        &mut self,
        params: &BulletParams,
        owner: SoldierId,
    ) -> Option<usize> {
        if self.net_skips(params, owner) {
            return None;
        }
        let slot = self.bullets.iter().position(|b| !b.active)?;

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
        // and the server never resets DegradeCount: power loss over distance stops early
        // in a slot whose last bullet flew far
        let stale_degrade = self.bullets[slot].degrade_count;
        self.bullets[slot] = Bullet::new(params, owner, seed, self.config.gravity);
        self.bullets[slot].start_tick = self.tick;
        self.bullets[slot].particle.force = stale_force;
        self.bullets[slot].degrade_count = stale_degrade;
        self.record_net_bullet(slot, params, owner);
        Some(slot)
    }

    /// Advances the simulation by one tick, in Soldat's `UpdateFrame` order: soldiers
    /// (creating bullets as they fire), then bullets in slot order, then bullet integration.
    /// Soldiers, bullets and things (`UpdateFrame` while `MapChangeCounter < 0`).
    fn simulate(&mut self, events: &mut Vec<GameEvent>) {
        // update soldiers

        let ids: Vec<SoldierId> = self.soldiers.keys().collect();

        // integrate every soldier's particle before any of them moves (spectators stay put)
        for soldier in self.soldiers.values_mut().filter(|s| !s.is_spectator()) {
            soldier.particle.euler();
        }

        for id in ids {
            let soldier = &mut self.soldiers[id];
            soldier.update_begin(self.config.client);
            if soldier.brain.is_some() {
                self.control_bot(id);
            }
            let pos = self.soldiers[id].particle.pos;
            let reach = self.soldiers.iter().any(|(other_id, other)| {
                other_id != id
                    && other.active
                    && !other.dead_meat
                    && other.position == POS_STAND
                    && distance(pos, other.particle.pos) < MELEE_DIST
            });
            self.soldiers[id].melee_reach = reach;
            self.soldiers[id].update_rest(
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
                    EmitterItem::Died(how) => {
                        events.push(GameEvent::Killed {
                            victim: id,
                            killer: id,
                            how,
                            weapon: None,
                            headshot: false,
                            hit: 1,
                            shot: self.shot,
                        });
                        self.on_death(id, id, events);
                    }
                }
            }

            self.process_drops();
            self.clear_weapons(id);
            self.process_flag_throw(id);
            self.process_parachute(id);
            self.process_survival(id, events);
            // the local player's mercy shot is followed by `kill`
            if std::mem::take(&mut self.soldiers[id].mercy_kill) {
                self.player_command(id, PlayerCommand::Kill, events);
            }
            // got off a stationary gun
            if let Some(gun) = self.soldiers[id].stat_release.take() {
                self.things[gun].static_type = false;
            }
        }

        // update bullets (bullets created on the way are updated if their slot comes later)

        let mut outcomes = Vec::new();

        for slot in 0..MAX_BULLETS {
            if self.bullets[slot].active {
                self.update_bullet(slot, &mut outcomes, events);
            }
            // (a client's late shots' trails shorten, also once they're gone)
            let bullet = &mut self.bullets[slot];
            if self.config.client && bullet.ping_add > 0 {
                bullet.ping_add -= 4;
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
                let others = std::mem::take(&mut self.things);
                let mut ctx = self.thing_ctx();
                thing.update(slot, &mut ctx, &others);
                let (scored, survival_capture) = (ctx.scored, ctx.survival_capture);
                let bullets = std::mem::take(&mut ctx.bullets);
                self.things = others;
                self.things[slot] = thing;
                for (owner, params) in bullets {
                    self.fire_stationary_gun(owner, &params, events);
                }
                if scored {
                    self.sort_players(events);
                }
                // a capture ends the survival round: everybody dies
                if survival_capture {
                    self.config.now.survival_end_round = true;
                    self.kill_survivors(events);
                }
            }
        }

        if !self.config.client {
            self.spawn_bonuses();
        }
    }

    /// `TBullet.Update` for one bullet, and what came of it.
    pub(crate) fn update_bullet(
        &mut self,
        slot: usize,
        outcomes: &mut Vec<BulletOutcome>,
        events: &mut Vec<GameEvent>,
    ) {
        let mut bullet = std::mem::take(&mut self.bullets[slot]);
        bullet.update(
            &self.map,
            &mut BulletCtx {
                config: &self.config,
                things: &mut self.things,
                tick: self.tick,
                slot,
                soldiers: &mut self.soldiers,
                bullets: &mut self.bullets,
                rng: &mut self.rng,
                outcomes,
                sounds: &mut self.sounds,
                sparks: &mut self.sparks,
            },
        );
        let owner = bullet.owner;
        for outcome in outcomes.iter() {
            if let BulletOutcome::Hurt { victim } = *outcome
                && !std::mem::replace(&mut bullet.has_hit, true)
            {
                events.push(GameEvent::Hit {
                    attacker: owner,
                    victim,
                    weapon: bullet.weapon,
                });
            }
        }
        self.bullets[slot] = bullet;
        self.process_drops();

        for outcome in outcomes.drain(..) {
            match outcome {
                BulletOutcome::Hit { kind, pos } => {
                    events.push(GameEvent::BulletHit { owner, kind, pos });
                }
                BulletOutcome::Hurt { .. } => {}
                BulletOutcome::Killed {
                    victim,
                    killer,
                    kill,
                    weapon,
                    shot,
                } => {
                    self.score_kill(victim, killer, kill.victim_weapon, Some(weapon));
                    if let Some(shot) = shot {
                        self.shot = shot;
                    }
                    events.push(GameEvent::Killed {
                        victim,
                        killer,
                        how: kill.how,
                        weapon: Some(weapon),
                        headshot: kill.head,
                        hit: kill.hit,
                        shot: self.shot,
                    });
                    self.on_death(victim, killer, events);
                }
                BulletOutcome::KnifeThing(pos) => {
                    let owner = self.soldiers.contains_key(owner).then_some(owner);
                    let mut things = std::mem::take(&mut self.things);
                    let ctx = &mut self.thing_ctx();
                    create_thing(
                        &mut things,
                        ctx,
                        pos,
                        owner,
                        ThingKind::CombatKnife,
                        None,
                        None,
                    );
                    self.things = things;
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

    pub fn step(&mut self, inputs: &[(SoldierId, Input)]) -> Vec<GameEvent> {
        let mut events = Vec::new();

        for &(id, ref input) in inputs {
            if let Some(soldier) = self.soldiers.get_mut(id) {
                soldier.apply_input(input);
            }
        }

        // the game stands still while the map is about to change
        let now = &mut self.config.now;
        now.wave_respawn_counter = self.game.wave_respawn_counter;
        now.leader = self.game.leader;
        now.survival_end_round = self.game.survival_end_round;
        now.sinus_counter = self.game.sinus_counter;
        if !self.game.ended() {
            self.simulate(&mut events);
        }

        // bullet time runs out; while it lasts, the game's clocks stand still
        if self.bullet_time > -1 {
            self.bullet_time -= 1;
        }
        self.clocks_ran = false;
        if self.bullet_time == 0 {
            self.bullet_time = -1;
        } else if self.bullet_time < 1 {
            self.clocks_ran = true;
            // medikit cooldown
            let cooldown = self.config.health_cooldown as u64 * 60;
            if cooldown > 0 && self.tick.is_multiple_of(cooldown) {
                for soldier in self.soldiers.values_mut() {
                    soldier.has_pack = false;
                }
            }

            self.game.tick(&mut events);
        }
        if !self.config.client {
            self.mode_rules(&mut events);
        }
        if self.clocks_ran {
            self.game.sinus_counter = fpc(ext(self.game.sinus_counter) + ILUMINATESPEED);
        }

        events.append(&mut self.thing_events);
        for sound in self.sounds.drain(..) {
            events.push(GameEvent::Sound {
                soldier: None,
                sound,
            });
        }
        for spark in self.sparks.drain(..) {
            events.push(GameEvent::Spark(spark));
        }
        for (id, soldier) in self.soldiers.iter_mut() {
            for sound in soldier.sounds.drain(..) {
                events.push(GameEvent::Sound {
                    soldier: Some(id),
                    sound,
                });
            }
            // ServerSendStringMessage drops empty texts (bots without a line for it)
            for text in soldier.said.drain(..) {
                if !text.is_empty() {
                    events.push(GameEvent::Chat { who: id, text });
                }
            }
            if let Some(style) = soldier.idle_started.take() {
                events.push(GameEvent::IdleAnimation { who: id, style });
            }
            for (who, text) in soldier.killer_said.drain(..) {
                if !text.is_empty() {
                    events.push(GameEvent::Chat { who, text });
                }
            }
            for mut spark in soldier.sparks.drain(..) {
                if spark.owner == SparkOwner::Me {
                    spark.owner = SparkOwner::Soldier(id);
                }
                events.push(GameEvent::Spark(spark));
            }
        }

        self.tick += 1;
        events
    }
}
