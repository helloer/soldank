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
pub const PROTOCOL_VERSION: u32 = 10;

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
    /// Standing, crouching or prone (`Position`): the server's soldier goes prone or gets up
    /// when its own differs.
    pub position: u8,
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
    Ready { mod_hash: u64 },
    /// Asks for a file the client lacks: the map, its textures and scenery
    /// ([`downloadable`] paths).
    Download(String),
    /// Changes to a team (`ChangeTeam`): 0 no team, 1-4, 5 spectators.
    JoinTeam(u8),
    /// The weapons to respawn with, by `WeaponKind` index (`SelWeapon`, `SecWep`).
    Loadout { primary: u8, secondary: u8 },
    /// Sent every tick (unreliable).
    Control(ControlState),
    Chat {
        text: String,
        team: bool,
        /// A radio message (`MSGTYPE_RADIO`, for the team): the radio menu's two digits.
        radio: Option<u8>,
    },
    /// A command line (without the `/`): player commands (`kill`, `smoke`, ...), `adminlog`,
    /// `votemap`, and an admin's commands and server cvars (`ParseInput` from MSGTYPE_CMD).
    Command(String),
    /// A shot of the player's slow weapon, or a grenade (`ClientSendBullet`).
    Bullet(BulletState),
    /// Starts a vote, or says yes to the running one (`ClientVoteKick`, `votemap`).
    Vote { kind: VoteKind, reason: String },
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
    /// Milliseconds to the server and back (0 for bots), and the same in the server's ticks.
    pub ping: u16,
    pub ping_ticks: u8,
    /// The stationary gun's thing slot, while on one.
    pub stat: Option<u8>,
    /// The weapons the player may pick (`WeaponSel`, [`Soldier::weapon_sel`]).
    pub weapon_sel: u16,
    /// Standing, crouching or prone (`Position`): a client's copy goes prone or gets up when
    /// its own differs.
    pub position: u8,
    /// The helmet and the cigar (`Look`: bit 0 no helmet, 1 an unlit cigar, 2 a lit one, 3
    /// helmet style 2).
    pub look: u8,
    /// A dead soldier's ticks to its respawn (`RespawnCounter`), new in dead snapshots.
    pub respawn_counter: i32,
    /// Of the packets from the player, the share that came (`ConnectionQuality`, 0 to 100).
    pub quality: u8,
}
}

state_with_delta! {
/// A thing for clients (`ServerThingSnapshot`).
pub struct ThingState / ThingDelta {
    pub slot: u8,
    pub kind: u8,
    /// The skeleton's points (two to four), and where they were a tick before.
    pub points: Vec<[f32; 2]>,
    pub old_points: Vec<[f32; 2]>,
    pub holder: Option<PlayerNum>,
    pub in_base: bool,
    pub ammo: u8,
    /// Lying still (`StaticType`): thing snapshots pass it by (but flags and guns on stands).
    pub static_type: bool,
    /// Its owner faced left (the images it's drawn with).
    pub flip: bool,
}
}

/// The match as clients see it, 30 times a second: the soldiers (by number) and the active
/// things (by slot, but parachutes: every client makes its own).
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Snapshot {
    /// Counts the server's ticks (across maps too): newer snapshots have larger ones.
    pub tick: u64,
    pub soldiers: Vec<SoldierState>,
    pub things: Vec<ThingState>,
    /// The things' points are new (`ServerThingSnapshot`, every `net_t1_thingsnapshot`
    /// ticks): clients move theirs that are far off. In between they're the last ones (but a
    /// new thing's).
    pub thing_snapshot: bool,
    /// The dead soldiers' respawn counters are new (`ServerSkeletonSnapshot`, every
    /// `net_t1_deadsnapshot` ticks); in between they're the last ones.
    pub dead_snapshot: bool,
    pub team_scores: [i32; 6],
    pub time_left: i32,
}

/// How a soldier died (`ServerSpriteDeath`): what clients need to have the same body fall.
#[derive(Debug, Clone, Default, PartialEq, Encode, Decode)]
pub struct DeathState {
    /// The skeleton point the killing hit struck (`Where`).
    pub hit: u8,
    /// The skeleton's points 1 to 16, and where they were a tick before.
    pub skeleton: Vec<([f32; 2], [f32; 2])>,
    /// The torn joints: constraints 2, 4, 20, 21 and 23 (bits 0 to 4).
    pub torn: u8,
    pub on_fire: u8,
    pub respawn_counter: i32,
    /// The last killing shot, for the killer's screen.
    pub shot: Shot,
}

/// The constraints a death can tear, in [`DeathState::torn`]'s bit order.
const TORN_CONSTRAINTS: [usize; 5] = [2, 4, 20, 21, 23];

impl DeathState {
    /// `soldier` just died from a hit on point `hit`.
    pub fn of(soldier: &Soldier, hit: u8, shot: Shot) -> DeathState {
        let skeleton = &soldier.skeleton;
        let constraints = skeleton.constraints();
        let torn = TORN_CONSTRAINTS
            .iter()
            .enumerate()
            .filter(|(_, c)| constraints.get(**c - 1).is_some_and(|c| !c.active))
            .fold(0, |bits, (i, _)| bits | (1 << i));
        DeathState {
            hit,
            skeleton: (1..=skeleton.len().min(16))
                .map(|i| (skeleton.pos(i).into(), skeleton.old_pos(i).into()))
                .collect(),
            torn,
            on_fire: soldier.on_fire,
            respawn_counter: soldier.respawn_counter,
            shot,
        }
    }
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
    pub thing_snapshot: bool,
    pub dead_snapshot: bool,
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
            thing_snapshot: snapshot.thing_snapshot,
            dead_snapshot: snapshot.dead_snapshot,
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
            thing_snapshot: self.thing_snapshot,
            dead_snapshot: self.dead_snapshot,
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
        /// The body as it fell, for clients to have the same one.
        death: DeathState,
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
        /// A radio message from the team: the radio menu's two digits.
        radio: Option<u8>,
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
    /// Synced server cvars that changed (`ServerSyncCvars`): clients play by them at once.
    Cvars(Vec<(String, String)>),
    /// The server loaded other weapons (`loadwep`, `ServerVars`): weapons.ini and
    /// weapons_realistic.ini, like [`ServerMessage::Welcome`]'s.
    Weapons([Option<String>; 2]),
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

/// `Look`'s bits: no helmet, an unlit cigar, a lit one, helmet style 2.
const LOOK_NO_HELMET: u8 = 1;
const LOOK_CIGAR: u8 = 2;
const LOOK_LIT_CIGAR: u8 = 4;
const LOOK_HELMET_2: u8 = 8;

/// A soldier's helmet and cigar as `ServerSpriteSnapshot`'s `Look`.
fn look_of(soldier: &Soldier) -> u8 {
    let mut look = 0;
    if soldier.wear_helmet == 0 {
        look |= LOOK_NO_HELMET;
    }
    if soldier.has_cigar == 5 {
        look |= LOOK_CIGAR;
    }
    if soldier.has_cigar == 10 {
        look |= LOOK_LIT_CIGAR;
    }
    if soldier.wear_helmet == 2 {
        look |= LOOK_HELMET_2;
    }
    look
}

impl Soldier {
    /// `Look` on a client: the helmet (another's knocked off with a spark and its sound, like
    /// `Delta_Helmet`) and the cigar (not while it's smoking one).
    fn apply_look(&mut self, look: u8, own: bool) {
        let mut helmet = 1;
        if look & LOOK_NO_HELMET != 0 {
            helmet = 0;
        }
        if look & LOOK_HELMET_2 != 0 {
            helmet = 2;
        }
        if helmet == 0 && self.wear_helmet != 0 && !own {
            let head = self.skeleton.pos(12);
            self.spark(head, self.particle.velocity, 6, 198);
            self.play_sound(Sound::new(Sfx::Headchop).at(head));
        }
        self.wear_helmet = helmet;
        let smoking = matches!(self.body_animation.id, Anim::Cigar | Anim::Smoke)
            || (self.idle_random == 1 && self.body_animation.id == Anim::Stand);
        if !smoking {
            self.has_cigar = 0;
            if look & LOOK_CIGAR != 0 {
                self.has_cigar = 5;
            }
            if look & LOOK_LIT_CIGAR != 0 {
                self.has_cigar = 10;
            }
        }
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
            // (held while the animation lasts, for others to see it; prone goes by the
            // position: `EncodeKeys`)
            (
                c.change_weapon || soldier.body_animation.id == Anim::Change,
                Buttons::CHANGE_WEAPON,
            ),
            (c.throw_nade, Buttons::THROW),
            (
                c.throw_weapon || soldier.body_animation.id == Anim::ThrowWeapon,
                Buttons::DROP,
            ),
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
            ping_ticks: soldier.ping_ticks,
            stat: soldier.stat.map(|slot| slot as u8),
            weapon_sel: soldier.weapon_sel,
            position: soldier.position,
            look: look_of(soldier),
            respawn_counter: soldier.respawn_counter,
            quality: soldier.connection_quality,
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
    /// whose movement and controls the client knows better; `dead_snapshot`: the respawn
    /// counter is new.
    pub fn apply_soldier_state(
        &mut self,
        id: SoldierId,
        state: &SoldierState,
        own: bool,
        dead_snapshot: bool,
    ) {
        let weapons = self.config.weapons.clone();
        // a death or respawn whose message hasn't come (or got here first)
        match self.soldiers.get(id).map(|s| s.dead_meat) {
            Some(false) if state.dead => self.apply_death(id, None, DeathKind::Normal, None, None),
            Some(true) if !state.dead => self.apply_respawn(id, Vec2::from(state.pos)),
            None => return,
            _ => {}
        }
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        if !own {
            soldier.apply_input(&state.input());
            // not where the server has it if it got hurt meanwhile (its knock-back is
            // coming here too)
            if soldier.health == state.health {
                soldier.particle.pos = Vec2::from(state.pos);
                soldier.particle.velocity = Vec2::from(state.velocity);
            }
            soldier.jets_count = state.jets;
            // prone toggles when it was activated or deactivated
            soldier.control.prone =
                (state.position == POS_PRONE) != (soldier.position == POS_PRONE);
        }
        soldier.apply_look(state.look, own);
        // (`ServerSkeletonSnapshot`)
        if dead_snapshot && state.dead {
            soldier.respawn_counter = state.respawn_counter;
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
        soldier.ping_ticks = state.ping_ticks;
        soldier.connection_quality = state.quality;
        soldier.stat = state.stat.map(usize::from);
        soldier.weapon_sel = state.weapon_sel;
    }

    /// A client: the server says `id` died (`ClientHandleSpriteDeath`): with `death`, the
    /// body falls from where the server's did, torn the same.
    pub fn apply_death(
        &mut self,
        id: SoldierId,
        killer: Option<SoldierId>,
        how: DeathKind,
        weapon: Option<WeaponKind>,
        death: Option<&DeathState>,
    ) {
        let Some(soldier) = self.soldiers.get_mut(id) else {
            return;
        };
        let was_alive = !soldier.dead_meat;
        if was_alive && let Some(death) = death {
            let skeleton = &mut soldier.skeleton;
            for (i, (pos, old)) in death.skeleton.iter().enumerate() {
                // (Soldat's skips points at 0)
                let set = [pos[0], pos[1], old[0], old[1]]
                    .iter()
                    .all(|v| v.round() != 0.0);
                if set && i < skeleton.len() {
                    *skeleton.pos_mut(i + 1) = Vec2::from(*pos);
                    *skeleton.old_pos_mut(i + 1) = Vec2::from(*old);
                }
            }
            // the points that double others
            if skeleton.len() >= 20 && death.skeleton.len() >= 16 {
                for (point, from) in [(17, 0), (18, 1), (19, 14), (20, 15)] {
                    let (pos, old) = death.skeleton[from];
                    *skeleton.pos_mut(point) = Vec2::from(pos);
                    *skeleton.old_pos_mut(point) = Vec2::from(old);
                }
            }
        }
        if was_alive {
            let berserk = killer
                .and_then(|k| self.soldiers.get(k))
                .is_some_and(|k| k.bonus_style == Bonus::Berserker);
            let by = HitBy {
                killer,
                other: killer.is_some_and(|k| k != id),
                weapon,
                ..Default::default()
            };
            let hit = death.map_or(1, |d| usize::from(d.hit.max(1)));
            self.bullet_time_kill(killer.unwrap_or(id));
            let soldier = &mut self.soldiers[id];
            soldier.die(
                how,
                hit,
                Vec2::ZERO,
                &self.config,
                berserk,
                by,
                &mut self.rng,
            );
            self.process_drops();
        }
        if let Some(death) = death {
            let soldier = &mut self.soldiers[id];
            for (i, c) in TORN_CONSTRAINTS.iter().enumerate() {
                soldier
                    .skeleton
                    .set_constraint_active(*c, death.torn & (1 << i) == 0);
            }
            soldier.respawn_counter = death.respawn_counter;
            soldier.on_fire = death.on_fire;
        }
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
            old_points: (1..=thing.skeleton.len())
                .map(|i| thing.skeleton.old_pos(i).into())
                .collect(),
            holder: thing.holding.and_then(num),
            in_base: thing.in_base,
            ammo: thing.ammo_count,
            static_type: thing.static_type,
            flip: thing.flip,
        }
    }
}

impl World {
    /// A client takes the server's things, slot by slot; the others are gone (but its own
    /// parachutes). Like Soldat's (`ClientHandleServerThingSnapshot`) it moves its things
    /// itself: a new one comes where the server has it, an other only when a thing snapshot
    /// finds it far off (both ends 10 apart, or a held one 330 from its holder).
    pub fn apply_things(
        &mut self,
        states: &[ThingState],
        thing_snapshot: bool,
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
            let new = !self.things[slot].active || self.things[slot].kind != kind;
            if new {
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
                self.things[slot].flip = state.flip;
            }

            let holding = state.holder.and_then(&soldier);
            // not held anymore
            if holding.is_none() {
                for soldier in self.soldiers.values_mut() {
                    if soldier.holded_thing == Some(slot) {
                        soldier.holded_thing = None;
                        soldier.holds_flag = false;
                    }
                }
            }

            let thing = &self.things[slot];
            let off = |i: usize, by: f32| {
                state
                    .points
                    .get(i)
                    .is_some_and(|p| distance(thing.pos(i + 1), Vec2::from(*p)) > by)
            };
            let far = match holding {
                None => kind != ThingKind::StationaryGun && off(0, 10.0) && off(1, 10.0),
                Some(holder) => {
                    kind != ThingKind::Parachute
                        && self
                            .soldiers
                            .get(holder)
                            .is_some_and(|h| distance(thing.pos(1), h.particle.pos) > 330.0)
                }
            };
            // Soldat's thing snapshots leave out things lying still (but flags and guns)
            let snapshotted = thing_snapshot
                && (kind.is_flag() || kind == ThingKind::StationaryGun || !state.static_type);
            let thing = &mut self.things[slot];
            if new || (snapshotted && far) {
                let points = state.points.len().min(thing.skeleton.len());
                for i in 0..points {
                    let pos = Vec2::from(state.points[i]);
                    let old = state.old_points.get(i).map_or(pos, |p| Vec2::from(*p));
                    *thing.skeleton.pos_mut(i + 1) = pos;
                    *thing.skeleton.old_pos_mut(i + 1) = old;
                }
            }
            thing.holding = holding;
            thing.in_base = state.in_base;
            thing.ammo_count = state.ammo;
            if new || snapshotted {
                thing.static_type = false;
            }
        }
        for (slot, thing) in self.things.iter_mut().enumerate() {
            if thing.active && !seen[slot] && thing.kind != ThingKind::Parachute {
                thing.kill();
            }
        }
    }

    /// A client: the server says `who` took the flag in `slot` (`ClientHandleThingTaken`):
    /// it's theirs, or back home if it's their team's (CTF and Infiltration).
    pub fn apply_flag_taken(&mut self, slot: usize, kind: ThingKind, who: SoldierId) {
        let Some(team) = self.soldiers.get(who).map(|s| s.team) else {
            return;
        };
        let Some(thing) = self
            .things
            .get_mut(slot)
            .filter(|t| t.active && t.kind == kind)
        else {
            return;
        };
        thing.holding = Some(who);
        thing.static_type = false;
        let returned = team == thing.flag_team()
            && matches!(
                self.config.game_mode,
                GameMode::CaptureTheFlag | GameMode::Infiltration
            );
        if returned {
            self.respawn_thing(slot);
        }
    }

    /// A client: the server sent `slot`'s thing home (a flag returned or captured), which
    /// Soldat's clients do themselves (`TThing.Respawn` on `ThingTaken` and `FlagInfo`).
    pub fn respawn_thing(&mut self, slot: usize) {
        if !self.things.get(slot).is_some_and(|t| t.active) {
            return;
        }
        let mut thing = std::mem::take(&mut self.things[slot]);
        let others = std::mem::take(&mut self.things);
        thing.respawn(slot, &mut self.thing_ctx());
        self.things = others;
        self.things[slot] = thing;
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
        hit: u8,
        shot: Shot,
    },
    Respawned {
        id: SoldierId,
    },
    Chat {
        who: SoldierId,
        text: String,
        team: bool,
        radio: Option<u8>,
    },
    ThingTaken {
        slot: usize,
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
    /// Server cvars to play by now (`ClientHandleSyncCvars`).
    Cvars(Vec<(String, String)>),
    /// The server's weapons mods now.
    Weapons([Option<String>; 2]),
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

    /// The control message for this tick: the input, and where the own soldier is; without
    /// one yet (picking a team), the newest snapshot only.
    pub fn control(&self, world: &World, input: &Input) -> Option<ClientMessage> {
        self.you?;
        let soldier = self.own().and_then(|id| world.soldiers.get(id));
        let (buttons, pos, velocity, position) = match soldier {
            Some(s) => (
                input.buttons.bits(),
                s.particle.pos,
                s.particle.velocity,
                s.position,
            ),
            None => (0, Vec2::ZERO, Vec2::ZERO, POS_STAND),
        };
        Some(ClientMessage::Control(ControlState {
            tick: world.tick,
            buttons,
            aim: input.aim.into(),
            pos: pos.into(),
            velocity: velocity.into(),
            snapshot: self.snapshots.back().map_or(0, |s| s.tick),
            resets: self.resets,
            position,
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
            ServerMessage::Cvars(cvars) => Notice::Cvars(cvars),
            ServerMessage::Weapons(mods) => Notice::Weapons(mods),
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
                soldier.bot = info.bot;
                info.looks.apply(soldier);
                let changed = soldier.team != team;
                if team == Team::Spectator {
                    // a newcomer drops nothing; a player changing to the spectators does
                    if new {
                        world.add_spectator(id);
                    } else if !soldier.is_spectator() {
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
                        world.apply_soldier_state(id, state, own, snapshot.dead_snapshot);
                    }
                }
                world.apply_things(&snapshot.things, snapshot.thing_snapshot, |num| {
                    self.soldier(num)
                });
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
                death,
            } => {
                let victim = self.soldier(victim)?;
                let killer = self.soldier(killer)?;
                let how = death_kind(how);
                let weapon = weapon.map(weapon_kind);
                world.apply_death(victim, Some(killer), how, weapon, Some(&death));
                return Some(Notice::Killed {
                    victim,
                    killer,
                    how,
                    weapon,
                    headshot,
                    hit: death.hit,
                    shot: death.shot,
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
            ServerMessage::Chat {
                from,
                text,
                team,
                radio,
            } => {
                let who = self.soldier(from)?;
                return Some(Notice::Chat {
                    who,
                    text,
                    team,
                    radio,
                });
            }
            ServerMessage::ThingTaken {
                slot,
                kind,
                by,
                pos,
            } => {
                let kind = ThingKind::from_num(kind)?;
                let who = self.soldier(by)?;
                if kind.is_flag() {
                    world.apply_flag_taken(usize::from(slot), kind, who);
                }
                return Some(Notice::ThingTaken {
                    slot: usize::from(slot),
                    kind,
                    who,
                    pos: Vec2::from(pos),
                });
            }
            ServerMessage::FlagCaptured { team, by } => {
                let team = team_from_num(team);
                // the other team's flag goes home
                let flag = match team {
                    Team::Alpha => Some(ThingKind::BravoFlag),
                    Team::Bravo => Some(ThingKind::AlphaFlag),
                    _ => None,
                };
                if let Some(slot) =
                    flag.and_then(|f| world.things.iter().position(|t| t.active && t.kind == f))
                {
                    world.respawn_thing(slot);
                }
                return Some(Notice::FlagCaptured {
                    team,
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
                let advance = u32::from(own_ping.map_or(0, |s| s.ping_ticks))
                    + u32::from(world.soldiers.get(id)?.ping_ticks)
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
            | ServerMessage::ServerText(_)
            | ServerMessage::Cvars(_)
            | ServerMessage::Weapons(_) => unreachable!("session messages are applied above"),
        }
        None
    }
}

/// A client moves others' bullets on by this many ticks more than the pings (`PingTicksAdd`).
const PING_TICKS_ADD: u32 = 2;

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
        let soldier_ping_ticks = soldier.ping_ticks;
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

        let first_slot = self.create_bullet_in_slot(&first, owner);
        // the shot is late by the owner's ping (its other pellets aren't told so)
        if self.config.client
            && let Some(slot) = first_slot
        {
            let late = u32::from(soldier_ping_ticks) + PING_TICKS_ADD;
            let bullet = &mut self.bullets[slot];
            bullet.owner_ping_tick = late.min(255) as u8;
            bullet.ping_add = advance.min(i16::MAX as u32) as i16;
            bullet.ping_add_start = bullet.ping_add;
        }
        let mut slots: Vec<usize> = first_slot.into_iter().collect();
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
            ping_ticks: 5,
            stat: Some(7),
            weapon_sel: 0x3ff,
            position: 1,
            look: 0,
            respawn_counter: 0,
            quality: 100,
        }
    }

    fn thing(slot: u8) -> ThingState {
        ThingState {
            slot,
            kind: 1,
            points: vec![[10.0, 20.0], [12.0, 20.0], [10.0, 40.0], [12.0, 40.0]],
            old_points: vec![[10.0, 20.0], [12.0, 20.0], [10.0, 40.0], [12.0, 40.0]],
            holder: None,
            in_base: true,
            ammo: 0,
            static_type: false,
            flip: false,
        }
    }

    #[test]
    fn messages_survive_encoding() {
        let message = ServerMessage::Snapshot(Snapshot {
            tick: 42,
            soldiers: vec![soldier(3)],
            things: vec![thing(1)],
            thing_snapshot: true,
            dead_snapshot: true,
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
            position: 3,
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
            thing_snapshot: true,
            dead_snapshot: true,
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
            thing_snapshot: true,
            dead_snapshot: true,
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
