//! The network protocol: what clients and the server send each other, encoded with bitcode.
//!
//! Like Soldat's: the server runs the match, and every client simulates it too from the
//! players' controls. A client reports its own soldier's movement (`ClientSpriteSnapshot`)
//! and takes health, deaths, respawns, things and scores from the server.

use super::*;
use crate::assets::Vfs;
use bitcode::{Decode, Encode};
use std::collections::{HashMap, VecDeque};

/// Bumped when the messages change; the server turns away other versions.
pub const PROTOCOL_VERSION: u32 = 9;

/// The netcode protocol id (`u64` in renet's connect handshake).
pub const PROTOCOL_ID: u64 = 0x50_4c_44_4b_00_00_00_01;

/// The default port (Soldat's 23073).
pub const DEFAULT_PORT: u16 = 23073;

/// Player numbers, 1 to 32 like Soldat's `Sprite[]` (`SoldierId`s differ on every machine).
pub type PlayerNum = u8;

/// Message channels (renet's default ones).
/// renet's default channels (`ConnectionConfig::default()`).
pub mod channel {
    /// Controls and snapshots: the next one replaces a lost one.
    pub const UNRELIABLE: u8 = 0;
    /// Everything else, in order (renet's `DefaultChannel::ReliableOrdered`; 1 is unordered).
    pub const RELIABLE: u8 = 2;
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct NetLooks {
    pub shirt: u32,
    pub pants: u32,
    pub skin: u32,
    pub hair: u32,
    pub jet: u32,
    pub hair_style: u8,
    pub chain: u8,
    pub head_cap: u8,
}

impl NetLooks {
    pub fn of(soldier: &Soldier) -> NetLooks {
        let l = &soldier.looks;
        NetLooks {
            shirt: l.shirt,
            pants: l.pants,
            skin: l.skin,
            hair: l.hair,
            jet: l.jet,
            hair_style: l.hair_style,
            chain: l.chain,
            head_cap: soldier.head_cap,
        }
    }

    pub fn apply(&self, soldier: &mut Soldier) {
        soldier.looks = Looks {
            shirt: self.shirt,
            pants: self.pants,
            skin: self.skin,
            hair: self.hair,
            jet: self.jet,
            hair_style: self.hair_style,
            chain: self.chain,
        };
        soldier.head_cap = self.head_cap;
        soldier.wear_helmet = u8::from(self.head_cap != 0);
    }
}

/// A player's controls and where its soldier is (`ClientSpriteSnapshot`).
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct ControlState {
    pub tick: u64,
    pub buttons: u16,
    pub aim: [f32; 2],
    pub pos: [f32; 2],
    pub velocity: [f32; 2],
    /// The newest snapshot the client has: the server's next ones only say what changed since.
    pub snapshot: u64,
    /// The server's resets of the player's position the client has had (its respawns and
    /// [`ServerMessage::ForcePosition`]s, counting on): till then `pos` is from before.
    pub resets: u8,
}

impl ControlState {
    pub fn input(&self) -> Input {
        Input {
            buttons: Buttons::from_bits_truncate(self.buttons),
            aim: Vec2::from(self.aim),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum ClientMessage {
    /// The first message: who's joining.
    Hello {
        version: u32,
        /// `sv_password`, if the server has one.
        password: String,
        name: String,
        looks: NetLooks,
        /// The team asked for (`cl_player_team`), `None` for the team menu.
        team: Option<u8>,
    },
    /// The map is loaded: put me in the match (after the welcome and each map change). With
    /// the [`GameMod::hash`] of the mod the client plays with: 0 none, 1 mods of its own
    /// (`CustomModChecksum`, for `sv_pure`).
    Ready {
        mod_hash: u64,
    },
    /// Asks for a file the client lacks: the map, its textures and scenery
    /// ([`downloadable`] paths).
    Download(String),
    /// Changes to a team (`ChangeTeam`): 0 no team, 1-4, 5 spectators.
    JoinTeam(u8),
    /// The weapons to respawn with, by `WeaponKind` index (`SelWeapon`, `SecWep`).
    Loadout {
        primary: u8,
        secondary: u8,
    },
    /// Sent every tick (unreliable).
    Control(ControlState),
    Chat {
        text: String,
        team: bool,
    },
    /// A command line (without the `/`): player commands (`kill`, `smoke`, ...), `adminlog`,
    /// `votemap`, and an admin's commands and server cvars (`ParseInput` from MSGTYPE_CMD).
    Command(String),
    /// A shot of the player's slow weapon, or a grenade (`ClientSendBullet`).
    Bullet(BulletState),
    /// Starts a vote, or says yes to the running one (`ClientVoteKick`, `votemap`).
    Vote {
        kind: VoteKind,
        reason: String,
    },
    /// Asks for the server's maps (`ClientVoteMap`, for the map menu).
    MapList,
}

/// A bullet as it left the gun (`TMsg_ClientBulletSnapshot`, `TMsg_BulletSnapshot`): the
/// Desert Eagles' second bullet and the shotgun's other pellets follow from the seed.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct BulletState {
    pub weapon: u8,
    pub pos: [f32; 2],
    pub velocity: [f32; 2],
    pub seed: u16,
}

/// What a vote is about (`VOTE_MAP`, `VOTE_KICK`).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum VoteKind {
    Map(String),
    Kick(PlayerNum),
}

/// Why a player is gone (`KICK_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum LeaveReason {
    Left,
    /// `KICK_VOTED`
    VoteKicked,
    /// By an admin (`KICK_CONSOLE`).
    Kicked,
    /// For a possible cheat (`KICK_CHEAT`, with `sv_anticheatkick`).
    Cheat,
    /// For a ping out of `sv_minping`..`sv_maxping` (`KICK_PING`).
    Ping,
    /// For flooding the server with packets or chat (`KICK_FLOODING`).
    Flooding,
}

/// Why the server turned a client away (`ServerSendUnaccepted`).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum Refusal {
    /// The server's protocol version.
    WrongVersion(u32),
    WrongPassword,
    ServerFull,
    /// `BANNED_IP`, with the ban's reason.
    Banned(String),
    /// `WRONG_CHECKSUM`: `sv_pure` and the client doesn't play with the server's mod.
    WrongChecksum,
}

impl std::fmt::Display for Refusal {
    /// Soldat's texts.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Refusal::WrongVersion(server) => write!(
                f,
                "Wrong game versions. Your version: {PROTOCOL_VERSION} Server Version: {server}"
            ),
            Refusal::WrongPassword => write!(f, "Wrong server password"),
            Refusal::ServerFull => write!(f, "Server is full"),
            Refusal::Banned(reason) => {
                write!(f, "You have been banned on this server. Reason: {reason}")
            }
            Refusal::WrongChecksum => write!(f, "This server requires a different smod file."),
        }
    }
}

/// A player as everyone sees it (`ServerSendNewPlayerInfo`).
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct PlayerInfo {
    pub num: PlayerNum,
    pub name: String,
    pub team: u8,
    pub looks: NetLooks,
    pub bot: bool,
}

/// A state struct keyed by `$key`, and its delta: the same fields, each `Some` when it
/// changed since a baseline (bitcode packs a `None` into one bit).
macro_rules! state_with_delta {
    (
        $(#[$meta:meta])*
        pub struct $state:ident / $delta:ident {
            $(#[$key_meta:meta])* pub $key:ident: $key_ty:ty,
            $($(#[$field_meta:meta])* pub $field:ident: $ty:ty,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Encode, Decode)]
        pub struct $state {
            $(#[$key_meta])* pub $key: $key_ty,
            $($(#[$field_meta])* pub $field: $ty,)*
        }

        #[doc = concat!("What changed in a [`", stringify!($state), "`] since a baseline.")]
        #[derive(Debug, Clone, PartialEq, Encode, Decode)]
        pub struct $delta {
            pub $key: $key_ty,
            $(pub $field: Option<$ty>,)*
        }

        impl $delta {
            /// What `state` has that `base` hasn't; `None` if nothing.
            pub fn between(base: &$state, state: &$state) -> Option<$delta> {
                let delta = $delta {
                    $key: state.$key.clone(),
                    $($field: (base.$field != state.$field).then(|| state.$field.clone()),)*
                };
                (false $(|| delta.$field.is_some())*).then_some(delta)
            }

            /// `base` with the changes.
            pub fn apply(&self, base: &$state) -> $state {
                let mut state = base.clone();
                $(if let Some(value) = &self.$field {
                    state.$field = value.clone();
                })*
                state
            }
        }
    };
}

state_with_delta! {
/// A soldier's state for clients (`ServerSpriteSnapshot`).
pub struct SoldierState / SoldierDelta {
    pub num: PlayerNum,
    /// Its controls (`Buttons`) and aim point.
    pub buttons: u16,
    pub aim: [f32; 2],
    pub pos: [f32; 2],
    pub velocity: [f32; 2],
    pub health: f32,
    pub vest: f32,
    pub dead: bool,
    pub jets: i32,
    /// `WeaponKind` index and bullets left of the primary, secondary and grenades.
    pub weapons: [(u8, u8); 3],
    pub active_weapon: u8,
    pub kills: i32,
    pub deaths: i32,
    pub flags: i32,
    pub bonus: u8,
    pub bonus_time: i32,
    /// Milliseconds to the server and back (0 for bots).
    pub ping: u16,
    /// The stationary gun's thing slot, while on one.
    pub stat: Option<u8>,
    /// The weapons the player may pick (`WeaponSel`, [`Soldier::weapon_sel`]).
    pub weapon_sel: u16,
}
}

state_with_delta! {
/// A thing for clients (`ServerThingSnapshot`).
pub struct ThingState / ThingDelta {
    pub slot: u8,
    pub kind: u8,
    /// The skeleton's points (two to four).
    pub points: Vec<[f32; 2]>,
    pub holder: Option<PlayerNum>,
    pub in_base: bool,
    pub ammo: u8,
}
}

/// The match as clients see it, 30 times a second: the soldiers (by number) and the active
/// things (by slot).
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Snapshot {
    /// Counts the server's ticks (across maps too): newer snapshots have larger ones.
    pub tick: u64,
    pub soldiers: Vec<SoldierState>,
    pub things: Vec<ThingState>,
    pub team_scores: [i32; 6],
    pub time_left: i32,
}

/// A snapshot as what changed since one the client has (`base`, which it acknowledged):
/// soldiers and things by what changed, the new ones whole, and those that are gone.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct SnapshotDelta {
    pub tick: u64,
    pub base: u64,
    pub soldiers: Vec<SoldierDelta>,
    pub new_soldiers: Vec<SoldierState>,
    pub gone_soldiers: Vec<PlayerNum>,
    pub things: Vec<ThingDelta>,
    pub new_things: Vec<ThingState>,
    pub gone_things: Vec<u8>,
    pub team_scores: Option<[i32; 6]>,
    pub time_left: Option<i32>,
}

impl SnapshotDelta {
    /// `snapshot` as changes to `base`.
    pub fn between(base: &Snapshot, snapshot: &Snapshot) -> SnapshotDelta {
        let (soldiers, new_soldiers, gone_soldiers) = changes(
            &base.soldiers,
            &snapshot.soldiers,
            |s| s.num,
            SoldierDelta::between,
        );
        let (things, new_things, gone_things) = changes(
            &base.things,
            &snapshot.things,
            |t| t.slot,
            ThingDelta::between,
        );
        SnapshotDelta {
            tick: snapshot.tick,
            base: base.tick,
            soldiers,
            new_soldiers,
            gone_soldiers,
            things,
            new_things,
            gone_things,
            team_scores: (base.team_scores != snapshot.team_scores).then_some(snapshot.team_scores),
            time_left: (base.time_left != snapshot.time_left).then_some(snapshot.time_left),
        }
    }

    /// The whole snapshot again, from its `base`.
    pub fn apply(&self, base: &Snapshot) -> Snapshot {
        let mut soldiers: Vec<SoldierState> = base
            .soldiers
            .iter()
            .filter(|s| !self.gone_soldiers.contains(&s.num))
            .map(|s| match self.soldiers.iter().find(|d| d.num == s.num) {
                Some(delta) => delta.apply(s),
                None => s.clone(),
            })
            .chain(self.new_soldiers.iter().cloned())
            .collect();
        soldiers.sort_by_key(|s| s.num);
        let mut things: Vec<ThingState> = base
            .things
            .iter()
            .filter(|t| !self.gone_things.contains(&t.slot))
            .map(|t| match self.things.iter().find(|d| d.slot == t.slot) {
                Some(delta) => delta.apply(t),
                None => t.clone(),
            })
            .chain(self.new_things.iter().cloned())
            .collect();
        things.sort_by_key(|t| t.slot);
        Snapshot {
            tick: self.tick,
            soldiers,
            things,
            team_scores: self.team_scores.unwrap_or(base.team_scores),
            time_left: self.time_left.unwrap_or(base.time_left),
        }
    }
}

/// Keyed states from `base` to `now`: what changed in those in both, the new ones, the keys
/// of the gone ones.
fn changes<S: Clone, D, K: PartialEq + Copy>(
    base: &[S],
    now: &[S],
    key: impl Fn(&S) -> K,
    between: impl Fn(&S, &S) -> Option<D>,
) -> (Vec<D>, Vec<S>, Vec<K>) {
    let mut changed = Vec::new();
    let mut new = Vec::new();
    for state in now {
        match base.iter().find(|b| key(b) == key(state)) {
            Some(old) => changed.extend(between(old, state)),
            None => new.push(state.clone()),
        }
    }
    let gone = base
        .iter()
        .map(&key)
        .filter(|k| !now.iter().any(|s| key(s) == *k))
        .collect();
    (changed, new, gone)
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum ServerMessage {
    /// The answer to `Hello`: the client's player number and the match.
    Welcome {
        you: PlayerNum,
        map: String,
        /// [`file_hash`] of the map file: a client with another one downloads the server's.
        map_hash: u64,
        /// Server cvars clients need (`CVAR_SYNC`): game mode, gravity, limits, ...
        cvars: Vec<(String, String)>,
        /// The server's weapons.ini and weapons_realistic.ini (`None`: the defaults), to play
        /// with its weapons (`ServerVars`).
        weapons_mods: [Option<String>; 2],
        /// The server's mod (`fs_mod`): clients play with it too (`cl_servermods`).
        game_mod: Option<GameMod>,
    },
    /// Turned away (version, full server, ...).
    Refused(Refusal),
    PlayerJoined(PlayerInfo),
    PlayerLeft {
        num: PlayerNum,
        why: LeaveReason,
    },
    TeamChanged {
        num: PlayerNum,
        team: u8,
    },
    /// Sent every other tick (unreliable), whole while the client hasn't acknowledged any
    /// the server still has, else as a [`SnapshotDelta`].
    Snapshot(Snapshot),
    SnapshotDelta(SnapshotDelta),
    Killed {
        victim: PlayerNum,
        killer: PlayerNum,
        /// `DeathKind`: 0 normal, 1 head chop, 2 brutal.
        how: u8,
        /// The killing bullet's `WeaponKind` index.
        weapon: Option<u8>,
        headshot: bool,
    },
    Respawned {
        num: PlayerNum,
        pos: [f32; 2],
    },
    /// The player's soldier is here (`ServerForcePosition`, `ServerForceVelocity`): the
    /// server put back a move it didn't believe (`sv_movecheck`).
    ForcePosition {
        pos: [f32; 2],
        velocity: [f32; 2],
    },
    Chat {
        from: PlayerNum,
        text: String,
        team: bool,
    },
    ThingTaken {
        slot: u8,
        kind: u8,
        by: PlayerNum,
        pos: [f32; 2],
    },
    FlagCaptured {
        team: u8,
        by: PlayerNum,
    },
    /// A limit was reached: the scoreboard, then the next map.
    MatchEnded,
    MapChange {
        map: String,
        map_hash: u64,
    },
    /// A piece of a downloaded file (`offset` into `size` bytes); `path` may differ from the
    /// request in its extension (`stopa.bmp` shipped as `stopa.png`).
    FileChunk {
        request: String,
        path: String,
        size: u32,
        offset: u32,
        data: Vec<u8>,
    },
    /// A vote started (`ServerSendVoteOn`); `starter` 0 is the server.
    VoteOn {
        kind: VoteKind,
        starter: PlayerNum,
        reason: String,
    },
    /// The vote passed (`ServerSendVoteOff`); one that runs out just ends.
    VoteOff,
    /// The server's maps (`configs/mapslist.txt`).
    MapList(Vec<String>),
    /// From the server itself (`ServerSendStringMessage` from 255).
    ServerText(String),
    /// The game is paused, or goes on (`ServerSyncMsg`).
    Paused(bool),
    /// An idle animation or taunt starts (`ServerIdleAnimation`): `IdleRandom`.
    IdleAnimation {
        num: PlayerNum,
        style: i8,
    },
    /// Someone else's shot (`ServerBulletSnapshot`).
    Bullet {
        owner: PlayerNum,
        bullet: BulletState,
    },
    /// The server doesn't have the file (or won't give it).
    NoFile(String),
}

/// A file's fingerprint (64-bit FNV-1a): good enough to tell two versions of a map apart.
pub fn file_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Files a server gives out: maps and the images they use, nothing outside those folders.
/// A server's mod (`fs_mod`; `ModName` and `ModChecksum` in `TMsg_PlayersList`): the
/// archive `mods/<name>.smod`, mounted over the game's files, the same on every machine.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct GameMod {
    pub name: String,
    /// [`file_hash`] of the archive.
    pub hash: u64,
}

impl GameMod {
    /// Where the archive is kept (in the config directory), and the download's request.
    pub fn path(&self) -> String {
        format!("mods/{}.smod", self.name)
    }

    /// A mod's name is a plain file name: no directories, nothing odd (Soldat strips `..`).
    pub fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 64
            && !name.starts_with('.')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_. ".contains(c))
    }
}

pub fn downloadable(path: &str) -> bool {
    let path = assets::normalize(path);
    let allowed = ["maps/", "textures/", "scenery-gfx/"];
    allowed.iter().any(|dir| path.starts_with(dir))
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !path.contains(':')
}

pub fn encode<T: Encode>(message: &T) -> Vec<u8> {
    bitcode::encode(message)
}

pub fn decode<'a, T: Decode<'a>>(bytes: &'a [u8]) -> Option<T> {
    bitcode::decode(bytes).ok()
}

pub fn weapon_index(kind: WeaponKind) -> u8 {
    WeaponKind::values()
        .iter()
        .position(|k| *k == kind)
        .unwrap_or(0) as u8
}

pub fn weapon_kind(index: u8) -> WeaponKind {
    WeaponKind::values()
        .get(usize::from(index))
        .copied()
        .unwrap_or(WeaponKind::NoWeapon)
}

pub fn team_from_num(n: u8) -> Team {
    match n {
        1 => Team::Alpha,
        2 => Team::Bravo,
        3 => Team::Charlie,
        4 => Team::Delta,
        5 => Team::Spectator,
        _ => Team::None,
    }
}

pub fn death_kind(how: u8) -> DeathKind {
    match how {
        1 => DeathKind::Headchop,
        2 => DeathKind::Brutal,
        _ => DeathKind::Normal,
    }
}

pub fn death_num(how: DeathKind) -> u8 {
    match how {
        DeathKind::Normal => 0,
        DeathKind::Headchop => 1,
        DeathKind::Brutal => 2,
    }
}

impl SoldierState {
    /// What a client needs to know about `soldier` (`num` on the server).
    pub fn of(num: PlayerNum, soldier: &Soldier) -> SoldierState {
        let c = &soldier.control;
        let buttons = [
            (c.left, Buttons::LEFT),
            (c.right, Buttons::RIGHT),
            (c.up, Buttons::JUMP),
            (c.down, Buttons::CROUCH),
            (c.fire, Buttons::FIRE),
            (c.jets, Buttons::JETS),
            (c.change_weapon, Buttons::CHANGE_WEAPON),
            (c.throw_nade, Buttons::THROW),
            (c.throw_weapon, Buttons::DROP),
            (c.prone, Buttons::PRONE),
            (c.flag_throw, Buttons::FLAG_THROW),
            (c.reload, Buttons::RELOAD),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(Buttons::empty(), |b, (_, button)| b | button);
        let weapon = |w: &Weapon| (weapon_index(w.kind), w.ammo_count);
        SoldierState {
            num,
            buttons: buttons.bits(),
            aim: [c.mouse_aim_x as f32, c.mouse_aim_y as f32],
            pos: soldier.particle.pos.into(),
            velocity: soldier.particle.velocity.into(),
            health: soldier.health,
            vest: soldier.vest,
            dead: soldier.dead_meat,
            jets: soldier.jets_count,
            weapons: [
                weapon(&soldier.weapons[0]),
                weapon(&soldier.weapons[1]),
                weapon(&soldier.weapons[2]),
            ],
            active_weapon: soldier.active_weapon as u8,
            kills: soldier.kills,
            deaths: soldier.deaths,
            flags: soldier.flags,
            bonus: soldier.bonus_style as u8,
            bonus_time: soldier.bonus_time,
            ping: soldier.ping,
            stat: soldier.stat.map(|slot| slot as u8),
            weapon_sel: soldier.weapon_sel,
        }
    }

    pub fn input(&self) -> Input {
        Input {
            buttons: Buttons::from_bits_truncate(self.buttons),
            aim: Vec2::from(self.aim),
        }
    }
}

impl World {
    /// A client takes the server's view of a soldier. `own` is the client's own soldier,
    /// whose movement and controls the client knows better.
    pub fn apply_soldier_state(&mut self, id: SoldierId, state: &SoldierState, own: bool) {
        let weapons = self.config.weapons.clone();
        // a death or respawn whose message hasn't come (or got here first)
        match self.soldiers.get(id).map(|s| s.dead_meat) {
            Some(false) if state.dead => self.apply_death(id, None, DeathKind::Normal),
            Some(true) if !state.dead => self.apply_respawn(id, Vec2::from(state.pos)),
            None => return,
            _ => {}
        }
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        if !own {
            soldier.apply_input(&state.input());
            soldier.particle.pos = Vec2::from(state.pos);
            soldier.particle.velocity = Vec2::from(state.velocity);
            soldier.jets_count = state.jets;
        }
        soldier.health = state.health;
        soldier.vest = state.vest;
        for (slot, &(kind, ammo)) in state.weapons.iter().enumerate() {
            let kind = weapon_kind(kind);
            if soldier.weapons[slot].kind != kind {
                soldier.weapons[slot] = weapons.get(kind);
            }
            soldier.weapons[slot].ammo_count = ammo;
        }
        soldier.active_weapon = usize::from(state.active_weapon).min(1);
        soldier.kills = state.kills;
        soldier.deaths = state.deaths;
        soldier.flags = state.flags;
        soldier.bonus_time = state.bonus_time;
        soldier.bonus_style = Bonus::from_num(state.bonus);
        soldier.ping = state.ping;
        soldier.stat = state.stat.map(usize::from);
        soldier.weapon_sel = state.weapon_sel;
    }

    /// A client: the server says `id` died.
    pub fn apply_death(&mut self, id: SoldierId, killer: Option<SoldierId>, how: DeathKind) {
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        if soldier.dead_meat {
            return;
        }
        let head = soldier.skeleton.pos(12);
        let by = HitBy {
            killer,
            ..Default::default()
        };
        self.bullet_time_kill(killer.unwrap_or(id));
        let soldier = &mut self.soldiers[id];
        soldier.die(how, 1, head, &self.config, false, by, &mut self.rng);
        self.process_drops();
    }

    /// A client: the server respawned `id` at `pos`.
    pub fn apply_respawn(&mut self, id: SoldierId, pos: Vec2) {
        if !self.soldiers.contains_key(id) {
            return;
        }
        self.respawn_soldier(id);
        let soldier = &mut self.soldiers[id];
        soldier.particle.pos = pos;
        soldier.particle.old_pos = pos;
    }
}

impl ThingState {
    pub fn of(
        slot: usize,
        thing: &Thing,
        num: impl Fn(SoldierId) -> Option<PlayerNum>,
    ) -> ThingState {
        ThingState {
            slot: slot as u8,
            kind: thing.kind as u8,
            points: (1..=thing.skeleton.len())
                .map(|i| thing.skeleton.pos(i).into())
                .collect(),
            holder: thing.holding.and_then(num),
            in_base: thing.in_base,
            ammo: thing.ammo_count,
        }
    }
}

impl World {
    /// A client takes the server's things, slot by slot; the others are gone.
    pub fn apply_things(
        &mut self,
        states: &[ThingState],
        soldier: impl Fn(PlayerNum) -> Option<SoldierId>,
    ) {
        let mut seen = vec![false; self.things.len()];
        for state in states {
            let slot = usize::from(state.slot);
            let Some(kind) = ThingKind::from_num(state.kind) else {
                continue;
            };
            if slot >= self.things.len() {
                continue;
            }
            seen[slot] = true;
            if !self.things[slot].active || self.things[slot].kind != kind {
                let mut things = std::mem::take(&mut self.things);
                let pos = state.points.first().map_or(Vec2::ZERO, |p| Vec2::from(*p));
                create_thing(
                    &mut things,
                    &mut self.thing_ctx(),
                    pos,
                    None,
                    kind,
                    Some(slot),
                    None,
                );
                self.things = things;
            }
            let thing = &mut self.things[slot];
            // moved, keeping the verlet velocity
            let points = state.points.len().min(thing.skeleton.len());
            for (i, p) in state.points.iter().take(points).enumerate() {
                let p = Vec2::from(*p);
                let shift = p - thing.skeleton.pos(i + 1);
                *thing.skeleton.pos_mut(i + 1) = p;
                *thing.skeleton.old_pos_mut(i + 1) += shift;
            }
            thing.holding = state.holder.and_then(&soldier);
            thing.in_base = state.in_base;
            thing.ammo_count = state.ammo;
        }
        for (slot, thing) in self.things.iter_mut().enumerate() {
            if thing.active && !seen[slot] {
                thing.kill();
            }
        }
    }
}

/// What a client's game should show or do about a server message, besides what
/// [`NetClient::apply`] already did to the world.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// Joined: load `map` (downloading it if it's missing or another one) with the server's
    /// cvars, then say [`ClientMessage::Ready`]; the players arrive after.
    Welcome {
        map: String,
        map_hash: u64,
        cvars: Vec<(String, String)>,
        weapons_mods: [Option<String>; 2],
        game_mod: Option<GameMod>,
    },
    Refused(Refusal),
    Joined {
        id: SoldierId,
        bot: bool,
    },
    Left {
        name: String,
        why: LeaveReason,
    },
    TeamChanged {
        id: SoldierId,
    },
    Killed {
        victim: SoldierId,
        killer: SoldierId,
        how: DeathKind,
        weapon: Option<WeaponKind>,
        headshot: bool,
    },
    Respawned {
        id: SoldierId,
    },
    Chat {
        who: SoldierId,
        text: String,
        team: bool,
    },
    ThingTaken {
        kind: ThingKind,
        who: SoldierId,
        pos: Vec2,
    },
    FlagCaptured {
        team: Team,
        who: SoldierId,
    },
    MatchEnded,
    MapChange {
        map: String,
        map_hash: u64,
    },
    FileChunk {
        request: String,
        path: String,
        size: u32,
        offset: u32,
        data: Vec<u8>,
    },
    NoFile(String),
    VoteOn {
        kind: VoteKind,
        /// `None`: the server.
        starter: Option<SoldierId>,
        reason: String,
    },
    VoteOff,
    MapList(Vec<String>),
    ServerText(String),
}

/// A client's side of the protocol, without the transport: which soldier each player number
/// is, and how server messages change the client's world.
#[derive(Debug, Default, Clone)]
pub struct NetClient {
    /// The client's own player number, after the welcome.
    pub you: Option<PlayerNum>,
    pub players: HashMap<PlayerNum, SoldierId>,
    /// The tick of the newest snapshot (older ones arriving late are dropped).
    pub snapshot_tick: u64,
    /// The latest snapshots, newest last: what the server's deltas start from.
    snapshots: VecDeque<Snapshot>,
    /// The server's resets of the own soldier's position so far ([`ControlState::resets`]);
    /// kept over map changes, like the server's count.
    pub resets: u8,
}

/// Snapshots a client keeps (two seconds' worth), for the server's deltas to start from.
pub const CLIENT_SNAPSHOTS: usize = 64;

impl NetClient {
    pub fn soldier(&self, num: PlayerNum) -> Option<SoldierId> {
        self.players.get(&num).copied()
    }

    /// The client's own soldier.
    pub fn own(&self) -> Option<SoldierId> {
        self.you.and_then(|num| self.soldier(num))
    }

    /// A new world (map change): the players arrive again.
    pub fn reset(&mut self) {
        self.players.clear();
        self.snapshot_tick = 0;
        self.snapshots.clear();
    }

    /// A snapshot delta made whole again (from the snapshot it started from), and kept with
    /// whole snapshots for the next deltas; other messages as they are. `None` for a snapshot
    /// that's late, or a delta from one the client doesn't have.
    pub fn expand(&mut self, message: ServerMessage) -> Option<ServerMessage> {
        let snapshot = match message {
            ServerMessage::Snapshot(snapshot) => snapshot,
            ServerMessage::SnapshotDelta(delta) => {
                let base = self.snapshots.iter().find(|s| s.tick == delta.base)?;
                delta.apply(base)
            }
            message => return Some(message),
        };
        if self
            .snapshots
            .back()
            .is_some_and(|s| s.tick >= snapshot.tick)
        {
            return None;
        }
        if self.snapshots.len() == CLIENT_SNAPSHOTS {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot.clone());
        Some(ServerMessage::Snapshot(snapshot))
    }

    /// The control message for this tick: the input, and where the own soldier is.
    pub fn control(&self, world: &World, input: &Input) -> Option<ClientMessage> {
        let soldier = world.soldiers.get(self.own()?)?;
        Some(ClientMessage::Control(ControlState {
            tick: world.tick,
            buttons: input.buttons.bits(),
            aim: input.aim.into(),
            pos: soldier.particle.pos.into(),
            velocity: soldier.particle.velocity.into(),
            snapshot: self.snapshots.back().map_or(0, |s| s.tick),
            resets: self.resets,
        }))
    }

    /// The messages about the connection, not the match (they need no world): their notice,
    /// or the message back (moved, as it came: no smaller than the message itself).
    #[allow(clippy::result_large_err)]
    pub fn apply_session(&mut self, message: ServerMessage) -> Result<Notice, ServerMessage> {
        Ok(match message {
            ServerMessage::Welcome {
                you,
                map,
                map_hash,
                cvars,
                weapons_mods,
                game_mod,
            } => {
                self.you = Some(you);
                self.reset();
                Notice::Welcome {
                    map,
                    map_hash,
                    cvars,
                    weapons_mods,
                    game_mod,
                }
            }
            ServerMessage::Refused(reason) => Notice::Refused(reason),
            ServerMessage::MapChange { map, map_hash } => Notice::MapChange { map, map_hash },
            ServerMessage::FileChunk {
                request,
                path,
                size,
                offset,
                data,
            } => Notice::FileChunk {
                request,
                path,
                size,
                offset,
                data,
            },
            ServerMessage::NoFile(path) => Notice::NoFile(path),
            ServerMessage::VoteOff => Notice::VoteOff,
            ServerMessage::MapList(maps) => Notice::MapList(maps),
            ServerMessage::ServerText(text) => Notice::ServerText(text),
            message => return Err(message),
        })
    }

    /// Applies a server message to `world`.
    pub fn apply(&mut self, world: &mut World, message: ServerMessage) -> Option<Notice> {
        let message = match self.apply_session(message) {
            Ok(notice) => return Some(notice),
            Err(message) => message,
        };
        match message {
            ServerMessage::PlayerJoined(info) => {
                let new = !self.players.contains_key(&info.num);
                let id = match self.soldier(info.num) {
                    Some(id) if world.soldiers.contains_key(id) => id,
                    _ => {
                        let id = world.spawn_soldier();
                        self.players.insert(info.num, id);
                        id
                    }
                };
                let team = team_from_num(info.team);
                let soldier = &mut world.soldiers[id];
                soldier.name = info.name;
                soldier.remote = Some(info.num) != self.you;
                info.looks.apply(soldier);
                let changed = soldier.team != team;
                if team == Team::Spectator {
                    if !soldier.is_spectator() {
                        world.join_spectators(id);
                    }
                } else {
                    soldier.team = team;
                }
                return Some(if new {
                    Notice::Joined { id, bot: info.bot }
                } else if changed {
                    Notice::TeamChanged { id }
                } else {
                    return None;
                });
            }
            ServerMessage::PlayerLeft { num, why } => {
                let id = self.players.remove(&num)?;
                let name = world.soldiers.get(id)?.name.clone();
                world.remove_soldier(id);
                return Some(Notice::Left { name, why });
            }
            ServerMessage::VoteOn {
                kind,
                starter,
                reason,
            } => {
                return Some(Notice::VoteOn {
                    kind,
                    starter: self.soldier(starter),
                    reason,
                });
            }
            ServerMessage::TeamChanged { num, team } => {
                let id = self.soldier(num)?;
                world.soldiers.get_mut(id)?.team = team_from_num(team);
                return Some(Notice::TeamChanged { id });
            }
            ServerMessage::Snapshot(snapshot) => {
                if snapshot.tick < self.snapshot_tick {
                    return None;
                }
                self.snapshot_tick = snapshot.tick;
                for state in &snapshot.soldiers {
                    if let Some(id) = self.soldier(state.num) {
                        let own = Some(state.num) == self.you;
                        world.apply_soldier_state(id, state, own);
                    }
                }
                world.apply_things(&snapshot.things, |num| self.soldier(num));
                world.game.team_scores = snapshot.team_scores;
                world.game.time_left = snapshot.time_left;
            }
            // made whole by `expand` first
            ServerMessage::SnapshotDelta(_) => {}
            ServerMessage::Killed {
                victim,
                killer,
                how,
                weapon,
                headshot,
            } => {
                let victim = self.soldier(victim)?;
                let killer = self.soldier(killer)?;
                let how = death_kind(how);
                world.apply_death(victim, Some(killer), how);
                return Some(Notice::Killed {
                    victim,
                    killer,
                    how,
                    weapon: weapon.map(weapon_kind),
                    headshot,
                });
            }
            ServerMessage::Respawned { num, pos } => {
                if Some(num) == self.you {
                    self.resets = self.resets.wrapping_add(1);
                }
                let id = self.soldier(num)?;
                world.apply_respawn(id, Vec2::from(pos));
                return Some(Notice::Respawned { id });
            }
            ServerMessage::ForcePosition { pos, velocity } => {
                self.resets = self.resets.wrapping_add(1);
                let soldier = world.soldiers.get_mut(self.own()?)?;
                soldier.particle.pos = Vec2::from(pos);
                soldier.particle.old_pos = soldier.particle.pos;
                soldier.particle.velocity = Vec2::from(velocity);
            }
            ServerMessage::Chat { from, text, team } => {
                let who = self.soldier(from)?;
                return Some(Notice::Chat { who, text, team });
            }
            ServerMessage::ThingTaken { kind, by, pos, .. } => {
                return Some(Notice::ThingTaken {
                    kind: ThingKind::from_num(kind)?,
                    who: self.soldier(by)?,
                    pos: Vec2::from(pos),
                });
            }
            ServerMessage::FlagCaptured { team, by } => {
                return Some(Notice::FlagCaptured {
                    team: team_from_num(team),
                    who: self.soldier(by)?,
                });
            }
            ServerMessage::MatchEnded => {
                if world.game.ended() {
                    return None;
                }
                world.game.end(&mut Vec::new());
                return Some(Notice::MatchEnded);
            }
            ServerMessage::Paused(paused) => {
                if paused {
                    world.game.pause();
                } else {
                    world.game.unpause();
                }
            }
            ServerMessage::IdleAnimation { num, style } => {
                let soldier = world.soldiers.get_mut(self.soldier(num)?)?;
                soldier.idle_time = 1;
                soldier.idle_random = style;
            }
            ServerMessage::Bullet { owner, bullet } => {
                let id = self.soldier(owner).filter(|&id| Some(id) != self.own())?;
                // both players' pings (`PingTicks`, round trips) and `PingTicksAdd`
                let own_ping = self.own().and_then(|own| world.soldiers.get(own));
                let advance = ping_ticks(own_ping.map_or(0, |s| s.ping))
                    + ping_ticks(world.soldiers.get(id)?.ping)
                    + PING_TICKS_ADD;
                world.receive_bullet(id, &bullet, advance);
            }
            ServerMessage::Welcome { .. }
            | ServerMessage::Refused(_)
            | ServerMessage::MapChange { .. }
            | ServerMessage::FileChunk { .. }
            | ServerMessage::NoFile(_)
            | ServerMessage::VoteOff
            | ServerMessage::MapList(_)
            | ServerMessage::ServerText(_) => unreachable!("session messages are applied above"),
        }
        None
    }
}

/// A client moves others' bullets on by this many ticks more than the pings (`PingTicksAdd`).
const PING_TICKS_ADD: u32 = 2;

/// Milliseconds in ticks (`PingTicks`).
fn ping_ticks(ms: u16) -> u32 {
    (u32::from(ms) * 60 + 500) / 1000
}

/// A bullet to send over the network (see [`World::net_bullets`]).
#[derive(Debug, Clone, PartialEq)]
pub struct NetBullet {
    pub owner: SoldierId,
    pub weapon: WeaponKind,
    pub pos: Vec2,
    pub velocity: Vec2,
    pub seed: u16,
}

impl NetBullet {
    pub fn state(&self) -> BulletState {
        BulletState {
            weapon: weapon_index(self.weapon),
            pos: self.pos.into(),
            velocity: self.velocity.into(),
            seed: self.seed,
        }
    }
}

/// Grenades and their clusters, which go over the network whatever the thrower holds.
fn is_grenade(style: BulletStyle) -> bool {
    matches!(
        style,
        BulletStyle::FragGrenade | BulletStyle::ClusterGrenade | BulletStyle::Cluster
    )
}

impl World {
    /// `CreateBullet`'s early exit in a network game: a remote player's slow weapon (on the
    /// server also its grenades) shoots here without bullets; they come from its machine.
    /// (A Soldat client creates them anyway when off screen, as the server doesn't send
    /// those; ours sends all.)
    pub(crate) fn net_skips(&self, params: &BulletParams, owner: SoldierId) -> bool {
        let Some(soldier) = self.soldiers.get(owner) else {
            return false;
        };
        let grenade = matches!(
            params.style,
            BulletStyle::FragGrenade | BulletStyle::ClusterGrenade
        );
        !params.must_create
            && soldier.remote
            && (soldier.primary_weapon().fire_interval > FIREINTERVAL_NET
                || (grenade && !self.config.client))
    }

    /// `CreateBullet`'s sending (of `Net` bullets: the Eagles' and shotgun's other bullets
    /// follow from the first's seed): a client sends its player's slow shots, grenades and
    /// clusters (`ClientSendBullet`); the server the slow shots to the other players
    /// (`ServerBulletSnapshot`).
    pub(crate) fn record_net_bullet(
        &mut self,
        slot: usize,
        params: &BulletParams,
        owner: SoldierId,
    ) {
        let (Some(sent), Some(soldier)) = (self.net_bullets.as_mut(), self.soldiers.get(owner))
        else {
            return;
        };
        let slow = soldier.primary_weapon().fire_interval > FIREINTERVAL_NET;
        let send = params.net
            && if self.config.client {
                !soldier.remote && (slow || params.must_create || is_grenade(params.style))
            } else {
                slow
            };
        if send {
            let bullet = &self.bullets[slot];
            sent.push(NetBullet {
                owner,
                weapon: params.weapon,
                pos: bullet.particle.pos,
                velocity: bullet.particle.velocity,
                seed: bullet.seed,
            });
        }
    }

    /// A bullet from another machine (`ServerHandleBulletSnapshot`, `ClientHandleBulletSnapshot`):
    /// created as it was there, with the Desert Eagles' second bullet and the shotgun's other
    /// pellets replayed from its seed. A client shows the shot and moves the bullets on by
    /// `advance` ticks, to about where they are by now.
    pub fn receive_bullet(&mut self, owner: SoldierId, state: &BulletState, advance: u32) {
        let Some(soldier) = self.soldiers.get(owner) else {
            return;
        };
        let weapons = &self.config.weapons;
        let kind = weapon_kind(state.weapon);
        let gun = weapons.get(kind);
        let style = gun.bullet_style;
        // the server takes the hit multiplier of the gun the player holds (`LastWeaponHM`)
        let held = soldier.primary_weapon().hit_multiply;
        let hit_multiply = match style {
            BulletStyle::FragGrenade => weapons.get(WeaponKind::FragGrenade).hit_multiply,
            _ if self.config.client => held,
            BulletStyle::Fist => weapons.get(WeaponKind::NoWeapon).hit_multiply,
            BulletStyle::ClusterGrenade => weapons.get(WeaponKind::FragGrenade).hit_multiply,
            BulletStyle::ThrownKnife => weapons.get(WeaponKind::ThrownKnife).hit_multiply,
            BulletStyle::M2Bullet => weapons.get(WeaponKind::M2).hit_multiply,
            _ => held,
        };
        let params = |position, velocity, seed, net| BulletParams {
            style,
            weapon: kind,
            position,
            velocity,
            timeout: gun.timeout as i16,
            hit_multiply,
            team: soldier.team,
            sprite: gun.bullet_sprite,
            seed,
            must_create: true,
            net,
            owner_immune: false,
        };
        let (pos, velocity) = (Vec2::from(state.pos), Vec2::from(state.velocity));
        let first = params(pos, velocity, Some(state.seed), !self.config.client);

        // undo the first pellet's spread for the straight shot, then the same randomness
        let mut others = Vec::new();
        if kind == WeaponKind::DesertEagles || style == BulletStyle::GaugeBullet {
            let spread = gun.bullet_spread;
            let saved_seed = self.rng.rand_seed;
            self.rng.rand_seed = u32::from(state.seed);
            let straight = vec2(
                random_unspread(&mut self.rng, spread, velocity.x),
                random_unspread(&mut self.rng, spread, velocity.y),
            );
            let mut a = pos;
            let pellets = if kind == WeaponKind::DesertEagles {
                let norm = vec2normalize(straight);
                a.x -= pascal_sign(straight.x) * norm.y.abs() * 3.0;
                a.y += pascal_sign(straight.y) * norm.x.abs() * 3.0;
                1
            } else {
                5
            };
            for _ in 0..pellets {
                let b = vec2(
                    random_spread(&mut self.rng, spread, straight.x),
                    random_spread(&mut self.rng, spread, straight.y),
                );
                others.push(params(a, b, None, false));
            }
            self.rng.rand_seed = saved_seed;
        }

        let mut slots: Vec<usize> = self
            .create_bullet_in_slot(&first, owner)
            .into_iter()
            .collect();
        for params in &others {
            slots.extend(self.create_bullet_in_slot(params, owner));
        }

        if self.config.client {
            // the shot itself (`Sprite[Owner].Fire()`; its bullets are the ones above), unless
            // the player's controls fired it here already
            let soldier = &self.soldiers[owner];
            let weapon = soldier.primary_weapon();
            let shoots = !is_grenade(style)
                && !matches!(style, BulletStyle::ThrownKnife | BulletStyle::M2Bullet);
            if shoots
                && weapon.kind == kind
                && weapon.fire_interval_count == 0
                && !soldier.dead_meat
            {
                let mut emitter = Vec::new();
                self.soldiers[owner].fire(&self.map, &self.config, &mut self.rng, &mut emitter);
                for item in emitter {
                    if let EmitterItem::Bullet(params) = item {
                        self.create_bullet(&params, owner);
                    }
                }
            }
            for slot in slots {
                self.advance_bullet(slot, advance);
            }
        }
    }

    /// A bullet on by `ticks` (`DoEulerTimeStepFor`, `Update`); what it hits shows next tick.
    fn advance_bullet(&mut self, slot: usize, ticks: u32) {
        let (mut outcomes, mut events) = (Vec::new(), Vec::new());
        for _ in 0..ticks {
            if !self.bullets[slot].active {
                break;
            }
            self.bullets[slot].particle.euler();
            self.update_bullet(slot, &mut outcomes, &mut events);
        }
        self.thing_events.append(&mut events);
    }
}

/// `b - (Random * 2 - 1) * spread`, like [`random_spread`] the other way.
fn random_unspread(rng: &mut PascalRandom, spread: f32, offset: f32) -> f32 {
    (f64::from(offset) - (rng.float() * 2.0 - 1.0) * f64::from(spread)) as f32
}

/// Image files may be shipped as any of these (`stopa.bmp` as `stopa.png`).
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "gif", "bmp"];

/// Larger files are refused.
pub const MAX_DOWNLOAD: u32 = 64 << 20;

/// Getting the server's map: the map file and the images it uses, downloaded from the server
/// when missing or different, before the client says it's [`ClientMessage::Ready`].
#[derive(Debug, Default)]
pub struct MapDownload {
    pub map: String,
    map_hash: u64,
    /// The files asked for: their size once known, and what's arrived.
    pending: HashMap<String, (u32, Vec<u8>)>,
    /// The map file is here and right (its images may still be on their way).
    have_map: bool,
    /// Why the map can't be had.
    pub failed: Option<String>,
}

impl MapDownload {
    /// What `map` needs, and the requests for what's missing (none if it's all here).
    pub fn new(vfs: &Vfs, map: &str, map_hash: u64) -> (MapDownload, Vec<ClientMessage>) {
        let mut download = MapDownload {
            map: map.to_string(),
            map_hash,
            ..MapDownload::default()
        };
        let path = download.map_path();
        let here = vfs
            .read(&path)
            .is_ok_and(|bytes| file_hash(&bytes) == map_hash);
        let requests = if here {
            download.have_map(vfs)
        } else {
            download.request(path)
        };
        (download, requests)
    }

    fn map_path(&self) -> String {
        format!("maps/{}.pms", self.map)
    }

    fn request(&mut self, path: String) -> Vec<ClientMessage> {
        self.pending.insert(path.clone(), (0, Vec::new()));
        vec![ClientMessage::Download(path)]
    }

    /// The map is here: the images it uses that aren't.
    fn have_map(&mut self, vfs: &Vfs) -> Vec<ClientMessage> {
        self.have_map = true;
        let map = match MapFile::load(vfs, &self.map) {
            Ok(map) => map,
            Err(error) => {
                self.failed = Some(format!("bad map {}: {error}", self.map));
                return Vec::new();
            }
        };
        let images = std::iter::once(format!("textures/{}", map.texture_name)).chain(
            map.scenery
                .iter()
                .map(|s| format!("scenery-gfx/{}", s.filename)),
        );
        let mut requests = Vec::new();
        for path in images {
            let missing = vfs.find_with_extensions(&path, IMAGE_EXTENSIONS).is_none();
            if missing && downloadable(&path) && !self.pending.contains_key(&path) {
                requests.extend(self.request(path));
            }
        }
        requests
    }

    /// A piece of a file. A whole one goes into `vfs` and comes back (to be kept on disk),
    /// with the requests it leads to (the map's images).
    pub fn chunk(
        &mut self,
        vfs: &mut Vfs,
        request: &str,
        path: &str,
        size: u32,
        offset: u32,
        data: &[u8],
    ) -> (Option<(String, Vec<u8>)>, Vec<ClientMessage>) {
        let Some((total, bytes)) = self.pending.get_mut(request) else {
            return (None, Vec::new());
        };
        let fits = offset as usize == bytes.len() && bytes.len() + data.len() <= size as usize;
        if !fits || size > MAX_DOWNLOAD || !same_file(request, path) {
            self.pending.remove(request);
            self.failed = Some(format!("bad download of {request}"));
            return (None, Vec::new());
        }
        *total = size;
        bytes.extend_from_slice(data);
        if bytes.len() < size as usize {
            return (None, Vec::new());
        }

        let (_, bytes) = self.pending.remove(request).unwrap_or_default();
        vfs.add(path, bytes.clone());
        let mut requests = Vec::new();
        if request == self.map_path() {
            if file_hash(&bytes) == self.map_hash {
                requests = self.have_map(vfs);
            } else {
                self.failed = Some(format!("the downloaded {request} is not the server's"));
            }
        }
        (Some((path.to_string(), bytes)), requests)
    }

    /// The server hasn't got a file: without an image the map still plays, not without itself.
    pub fn no_file(&mut self, request: &str) {
        if self.pending.remove(request).is_some() && request == self.map_path() {
            self.failed = Some(format!("the server can't give out {request}"));
        }
    }

    /// Everything's here: the map can be loaded.
    pub fn done(&self) -> bool {
        self.have_map && self.pending.is_empty() && self.failed.is_none()
    }

    /// A file on its way: the map's first, then an image.
    pub fn current(&self) -> Option<&str> {
        let map = self.map_path();
        let file = match self.pending.contains_key(&map) {
            true => self.pending.get_key_value(&map).map(|(k, _)| k),
            false => self.pending.keys().min(),
        };
        file.map(String::as_str)
    }

    /// Bytes arrived and expected so far (sizes are known from each file's first piece).
    pub fn progress(&self) -> (usize, usize) {
        self.pending
            .values()
            .fold((0, 0), |(got, of), (size, bytes)| {
                (got + bytes.len(), of + *size as usize)
            })
    }
}

/// Getting the server's mod archive (Soldat's `TDownloadThread` for `mods/<name>.smod`),
/// before its map.
#[derive(Debug)]
pub struct ModDownload {
    pub game_mod: GameMod,
    size: u32,
    bytes: Vec<u8>,
    /// Why it can't be had.
    pub failed: Option<String>,
}

impl ModDownload {
    pub fn new(game_mod: GameMod) -> (ModDownload, ClientMessage) {
        let request = ClientMessage::Download(game_mod.path());
        let download = ModDownload {
            game_mod,
            size: 0,
            bytes: Vec::new(),
            failed: None,
        };
        (download, request)
    }

    /// The download's request.
    pub fn wants(&self, request: &str) -> bool {
        request == self.game_mod.path()
    }

    /// A piece of the archive: the whole of it once it's all here and right.
    pub fn chunk(&mut self, path: &str, size: u32, offset: u32, data: &[u8]) -> Option<Vec<u8>> {
        let fits =
            offset as usize == self.bytes.len() && self.bytes.len() + data.len() <= size as usize;
        if self.failed.is_some() || !fits || size > MAX_DOWNLOAD || !self.wants(path) {
            self.failed = Some(format!("bad download of {}", self.game_mod.path()));
            return None;
        }
        self.size = size;
        self.bytes.extend_from_slice(data);
        if self.bytes.len() < size as usize {
            return None;
        }
        let bytes = std::mem::take(&mut self.bytes);
        if file_hash(&bytes) != self.game_mod.hash {
            self.failed = Some("Checksum mismatch".to_string());
            return None;
        }
        Some(bytes)
    }

    /// The server can't give it out.
    pub fn no_file(&mut self) {
        self.failed = Some(format!(
            "the server can't give out {}",
            self.game_mod.path()
        ));
    }

    /// Bytes arrived and expected (the size is known from the first piece).
    pub fn progress(&self) -> (usize, usize) {
        (self.bytes.len(), self.size as usize)
    }
}

/// `path` is the file `request` asked for: the same, or an image under another extension.
fn same_file(request: &str, path: &str) -> bool {
    let (request, path) = (assets::normalize(request), assets::normalize(path));
    let stem = |p: &str| {
        p.rsplit_once('.')
            .map_or(p.to_string(), |(s, _)| s.to_string())
    };
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    downloadable(&path)
        && (path == request || (stem(&path) == stem(&request) && IMAGE_EXTENSIONS.contains(&ext)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mod_download_checks_its_pieces_and_hash() {
        let bytes: Vec<u8> = (0..40_000u32).map(|i| (i * 7) as u8).collect();
        let game_mod = GameMod {
            name: "test".into(),
            hash: file_hash(&bytes),
        };
        let (mut download, request) = ModDownload::new(game_mod.clone());
        assert_eq!(request, ClientMessage::Download("mods/test.smod".into()));
        assert!(download.wants("mods/test.smod"));
        let size = bytes.len() as u32;
        assert_eq!(
            download.chunk("mods/test.smod", size, 0, &bytes[..16384]),
            None
        );
        assert_eq!(download.progress(), (16384, 40_000));
        let rest = download.chunk("mods/test.smod", size, 16384, &bytes[16384..]);
        assert_eq!(rest.as_deref(), Some(&bytes[..]));
        assert_eq!(download.failed, None);

        // another file than the server's
        let (mut download, _) = ModDownload::new(GameMod {
            hash: game_mod.hash ^ 1,
            ..game_mod.clone()
        });
        assert_eq!(download.chunk("mods/test.smod", size, 0, &bytes), None);
        assert_eq!(download.failed.as_deref(), Some("Checksum mismatch"));
        // pieces out of order
        let (mut download, _) = ModDownload::new(game_mod);
        assert_eq!(
            download.chunk("mods/test.smod", size, 16384, &bytes[..10]),
            None
        );
        assert!(download.failed.is_some());

        assert!(GameMod::valid_name("Rambo Mod-2_b.1"));
        for bad in ["", "../x", "a/b", ".hidden", "a\\b"] {
            assert!(!GameMod::valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn downloads_stay_with_what_was_asked() {
        assert!(same_file("maps/ctf_Ash.pms", "maps/ctf_ash.pms"));
        assert!(same_file("textures/rock.bmp", "textures/rock.png"));
        assert!(!same_file("textures/rock.bmp", "textures/rock.exe"));
        assert!(!same_file("textures/rock.bmp", "textures/objects/flag.png"));
        assert!(!same_file("maps/x.pms", "maps/../x.pms"));
        assert!(!same_file("configs/client.cfg", "configs/client.cfg"));
    }

    #[test]
    fn a_map_arrives_in_pieces() {
        let bytes: Vec<u8> = (0..100u8).collect();
        let mut vfs = Vfs::new();
        let (mut download, requests) = MapDownload::new(&vfs, "Test", file_hash(&bytes));
        assert!(matches!(&requests[..], [ClientMessage::Download(p)] if p == "maps/Test.pms"));
        let request = "maps/Test.pms";
        let (whole, _) = download.chunk(&mut vfs, request, request, 100, 0, &bytes[..60]);
        assert!(whole.is_none());
        assert_eq!(download.progress(), (60, 100));
        let (whole, _) = download.chunk(&mut vfs, request, request, 100, 60, &bytes[60..]);
        assert_eq!(whole, Some((request.to_string(), bytes.clone())));
        assert_eq!(vfs.read(request).unwrap(), bytes);
        // not a real map
        assert!(download.failed.is_some());
        assert!(!download.done());

        // a piece out of place spoils it
        let (mut download, _) = MapDownload::new(&Vfs::new(), "Other", 1);
        download.chunk(
            &mut vfs,
            "maps/Other.pms",
            "maps/Other.pms",
            100,
            50,
            &bytes,
        );
        assert!(download.failed.is_some());
    }

    fn soldier(num: PlayerNum) -> SoldierState {
        SoldierState {
            num,
            buttons: (Buttons::LEFT | Buttons::FIRE).bits(),
            aim: [100.5, -20.0],
            pos: [1.25, 2.5],
            velocity: [0.1, -0.2],
            health: 87.5,
            vest: 0.0,
            dead: false,
            jets: 190,
            weapons: [(2, 30), (10, 12), (19, 2)],
            active_weapon: 0,
            kills: 4,
            deaths: 1,
            flags: 0,
            bonus: 0,
            bonus_time: 0,
            ping: 85,
            stat: Some(7),
            weapon_sel: 0x3ff,
        }
    }

    fn thing(slot: u8) -> ThingState {
        ThingState {
            slot,
            kind: 1,
            points: vec![[10.0, 20.0], [12.0, 20.0], [10.0, 40.0], [12.0, 40.0]],
            holder: None,
            in_base: true,
            ammo: 0,
        }
    }

    #[test]
    fn messages_survive_encoding() {
        let message = ServerMessage::Snapshot(Snapshot {
            tick: 42,
            soldiers: vec![soldier(3)],
            things: vec![thing(1)],
            team_scores: [0, 3, 1, 0, 0, 0],
            time_left: 3600,
        });
        assert_eq!(decode::<ServerMessage>(&encode(&message)), Some(message));

        let message = ClientMessage::Control(ControlState {
            tick: 42,
            buttons: (Buttons::LEFT | Buttons::FIRE).bits(),
            aim: [100.5, -20.0],
            pos: [1.25, 2.5],
            velocity: [0.1, -0.2],
            snapshot: 40,
            resets: 3,
        });
        assert_eq!(decode::<ClientMessage>(&encode(&message)), Some(message));
        assert_eq!(decode::<ClientMessage>(&[0xff, 0x00]), None);
    }

    #[test]
    fn snapshot_deltas_carry_only_changes() {
        let base = Snapshot {
            tick: 40,
            soldiers: vec![soldier(1), soldier(2), soldier(3)],
            things: vec![thing(1), thing(2)],
            team_scores: [0, 3, 1, 0, 0, 0],
            time_left: 3600,
        };
        let mut now = base.clone();
        now.tick = 42;
        // 1 runs, 2 stands still, 3 left, 4 joined; flag 1 is taken, kit 2 gone, 5 new
        now.soldiers[0].pos = [5.0, 2.5];
        now.soldiers[0].health = 50.0;
        now.soldiers.remove(2);
        now.soldiers.push(soldier(4));
        now.things[0].holder = Some(1);
        now.things[0].in_base = false;
        now.things.remove(1);
        now.things.push(thing(5));
        now.time_left = 3598;

        let delta = SnapshotDelta::between(&base, &now);
        assert_eq!(delta.base, 40);
        assert_eq!(delta.soldiers.len(), 1);
        let changed = &delta.soldiers[0];
        assert_eq!(
            (changed.pos, changed.health),
            (Some([5.0, 2.5]), Some(50.0))
        );
        assert_eq!((changed.velocity, changed.weapons), (None, None));
        assert_eq!(delta.new_soldiers, [soldier(4)]);
        assert_eq!(delta.gone_soldiers, [3]);
        assert_eq!(delta.things[0].holder, Some(Some(1)));
        assert_eq!(delta.things[0].points, None);
        assert_eq!(delta.new_things, [thing(5)]);
        assert_eq!(delta.gone_things, [2]);
        assert_eq!((delta.team_scores, delta.time_left), (None, Some(3598)));
        assert_eq!(delta.apply(&base), now);

        // a soldier running is a fraction of the whole, nothing changed next to nothing
        let mut next = Snapshot {
            tick: 44,
            ..now.clone()
        };
        let size = |delta| encode(&ServerMessage::SnapshotDelta(delta)).len();
        assert!(size(SnapshotDelta::between(&now, &next)) < 16);
        next.soldiers[0].pos = [6.0, 2.5];
        next.soldiers[0].velocity = [1.0, 0.0];
        let full = encode(&ServerMessage::Snapshot(next.clone())).len();
        let running = size(SnapshotDelta::between(&now, &next));
        assert!(running * 5 < full, "{running} vs {full}");
    }

    #[test]
    fn a_client_expands_deltas_from_what_it_has() {
        let mut net = NetClient::default();
        let first = Snapshot {
            tick: 10,
            soldiers: vec![soldier(1)],
            things: Vec::new(),
            team_scores: [0; 6],
            time_left: 100,
        };
        let mut second = first.clone();
        second.tick = 12;
        second.soldiers[0].pos = [9.0, 9.0];
        let delta = ServerMessage::SnapshotDelta(SnapshotDelta::between(&first, &second));

        // a delta from a snapshot it never got: dropped
        assert_eq!(net.expand(delta.clone()), None);
        let full = ServerMessage::Snapshot(first.clone());
        assert_eq!(net.expand(full.clone()), Some(full.clone()));
        assert_eq!(
            net.expand(delta.clone()),
            Some(ServerMessage::Snapshot(second))
        );
        // late ones too
        assert_eq!(net.expand(full), None);
        assert_eq!(net.expand(delta), None);
        assert_eq!(net.snapshots.back().map(|s| s.tick), Some(12));
        // a map change forgets them
        net.reset();
        assert!(net.snapshots.is_empty());
    }

    #[test]
    fn weapon_and_team_numbers_round_trip() {
        for &kind in WeaponKind::values() {
            assert_eq!(weapon_kind(weapon_index(kind)), kind);
        }
        for n in 0..=5 {
            assert_eq!(team_from_num(n) as u8, n);
        }
    }
}
