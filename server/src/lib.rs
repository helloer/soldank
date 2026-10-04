//! The dedicated server's game: the match, its players and what goes over the network.
//! `main.rs` drives it with renet's UDP transport; the tests drive it with renet's in-memory
//! clients.

use renet2::{ClientId, RenetServer, ServerEvent};
use soldank_core::assets::Vfs;
use soldank_core::config::{CvarFlags, Cvars};
use soldank_core::net::*;
use soldank_core::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;

pub mod admin;
mod demo;
mod guard;
pub mod log;
pub mod rcon;
mod vote;
pub mod web;

/// Players at most (`MAX_PLAYERS`).
pub const MAX_PLAYERS: usize = 32;

/// Snapshots go out every this many ticks (30 a second).
const SNAPSHOT_TICKS: u64 = 2;
/// The snapshots kept for deltas (a second's worth): a client that acknowledged none of them
/// gets a whole one.
const SERVER_SNAPSHOTS: usize = 30;

/// A connected human.
#[derive(Default)]
struct Client {
    /// `None` until its `Hello`.
    num: Option<PlayerNum>,
    /// Who it said it is, for when it's ready.
    hello: Option<(String, NetLooks, Option<u8>)>,
    /// It has the map loaded: in the match, hearing about it.
    ready: bool,
    control: Option<ControlState>,
    /// The newest snapshot it said it has (`ControlState::snapshot`).
    acked: Option<u64>,
    /// Turned away: disconnected at this tick, once the reason got there.
    refused_until: Option<u64>,
    /// Files on their way: what was asked for, the file, its bytes, how far along.
    downloads: VecDeque<(String, String, Arc<Vec<u8>>, usize)>,
    shots: Shots,
    /// No starting a vote before this tick (`VoteCooldown`).
    vote_cooldown: u64,
    /// Where it connects from (`Player.IP`): bans and admins go by it.
    ip: Option<IpAddr>,
    warnings: guard::Warnings,
    moves: guard::Moves,
    /// The server's own demo recorder: it watches (no soldier), and nobody sees it.
    demo: bool,
}

/// What the server checks a client's bullets against (`BulletTime`, `BulletWarningCount`,
/// `GrenadeTime`, `OldBulletSnapshotMsg`).
#[derive(Default)]
struct Shots {
    last_tick: u64,
    warnings: u32,
    last_grenade: Option<u64>,
    last: Option<BulletState>,
}

/// Twice `STAT_RADIUS`: how near a stationary gun its bullets come from.
const STAT_REACH: f32 = 30.0;

/// Downloads go out in pieces this big, a couple each tick.
const CHUNK_BYTES: usize = 16 * 1024;
const CHUNKS_PER_TICK: usize = 2;

/// A refused client stays connected this long, for the reason to arrive.
const REFUSED_TICKS: u64 = 30;

pub struct ServerGame {
    pub world: World,
    pub cvars: Cvars,
    vfs: Vfs,
    data: Arc<GameData>,
    clients: HashMap<ClientId, Client>,
    /// Player numbers to soldiers, humans and bots.
    players: HashMap<PlayerNum, SoldierId>,
    /// Bots by file name and team, kept for the next map.
    bots: Vec<(String, Team)>,
    /// Soldiers dead last tick, to tell respawns.
    dead: HashMap<SoldierId, bool>,
    /// [`file_hash`] of the map file.
    map_hash: u64,
    vote: Option<vote::Vote>,
    /// The voted next map, instead of the map list's.
    next_map: Option<String>,
    /// Ticks since the server started (the world's start over with each map).
    uptime: u64,
    /// Bans, admins, and where they're kept.
    pub lists: admin::Lists,
    /// The last player to join (`LastPlayer`, for `kicklast`).
    last_player: Option<PlayerNum>,
    /// The console and kill logs, when `log_enable` and `log_level` want them.
    pub logs: Option<log::Logs>,
    /// The map list.
    maps: Vec<String>,
    /// Where players connect, for `info`.
    pub address: Option<std::net::SocketAddr>,
    /// The server's mod (`fs_mod`) and its archive, for clients to download.
    pub game_mod: Option<(GameMod, Arc<Vec<u8>>)>,
    /// The latest snapshots, newest last: what deltas start from.
    snapshots: VecDeque<Snapshot>,
    /// Players the server started a kick vote against (`CheatTag`, `sv_antimassflag`).
    cheat_tags: HashSet<PlayerNum>,
    /// Team kills of the players (`TKWarnings`), and of those who left by their address
    /// (`TKList`), until the map changes.
    tk_warnings: HashMap<PlayerNum, u32>,
    tk_list: HashMap<String, u32>,
    /// Moves put back so far (`sv_movecheck`).
    corrections: u64,
    /// When the things' points last went out (`ServerThingSnapshot`), and the dead's
    /// respawn counters (`ServerSkeletonSnapshot`).
    thing_snapshot_tick: Option<u64>,
    dead_snapshot_tick: Option<u64>,
    /// The cvars' generation the match has, and the synced ones clients have.
    cvars_seen: u64,
    cvars_sent: Vec<(String, String)>,
    /// Where `demos/` is (the config directory).
    pub config_dir: std::path::PathBuf,
    /// Recording a demo (`record`, `demo_autorecord`).
    recorder: Option<demo::Recorder>,
}

impl ServerGame {
    pub fn new(
        vfs: Vfs,
        data: Arc<GameData>,
        cvars: Cvars,
        map: &str,
    ) -> anyhow::Result<ServerGame> {
        let world = new_world(&vfs, &data, &cvars, map)?;
        let map_hash = map_hash(&vfs, &map_name(&world.map));
        let mut game = ServerGame {
            world,
            cvars,
            vfs,
            data,
            clients: HashMap::new(),
            players: HashMap::new(),
            bots: Vec::new(),
            dead: HashMap::new(),
            map_hash,
            vote: None,
            next_map: None,
            uptime: 0,
            lists: admin::Lists::default(),
            last_player: None,
            logs: None,
            maps: Vec::new(),
            address: None,
            game_mod: None,
            snapshots: VecDeque::new(),
            cheat_tags: HashSet::new(),
            tk_warnings: HashMap::new(),
            tk_list: HashMap::new(),
            corrections: 0,
            thing_snapshot_tick: None,
            dead_snapshot_tick: None,
            cvars_seen: 0,
            cvars_sent: Vec::new(),
            config_dir: std::path::PathBuf::from("."),
            recorder: None,
        };
        game.load_map_list();
        game.cvars_seen = game.cvars.generation();
        game.cvars_sent = game.synced_cvars();
        Ok(game)
    }

    /// The map's name, as clients load it.
    pub fn map_name(&self) -> String {
        map_name(&self.world.map)
    }

    fn num_of(&self, id: SoldierId) -> Option<PlayerNum> {
        self.players
            .iter()
            .find(|(_, s)| **s == id)
            .map(|(num, _)| *num)
    }

    fn free_num(&self) -> Option<PlayerNum> {
        let taken = |n: &PlayerNum| {
            self.players.contains_key(n) || self.clients.values().any(|c| c.num == Some(*n))
        };
        (1..=MAX_PLAYERS as PlayerNum).find(|n| !taken(n))
    }

    /// `sv_pauseonidle`: no human is here (bots and the demo recorder don't count), the
    /// match waits.
    pub fn idle(&self) -> bool {
        self.cvars.bool("sv_pauseonidle")
            && !self.clients.values().any(|c| c.num.is_some() && !c.demo)
    }

    /// Cvars that changed: the match plays by them now, and so do the clients (the synced
    /// ones, `ServerSyncCvars`; also those still getting the map, who had the old ones).
    fn sync_cvars(&mut self, renet: &mut RenetServer) {
        if self.cvars.generation() == self.cvars_seen {
            return;
        }
        self.cvars_seen = self.cvars.generation();
        self.world
            .set_rules(WorldConfig::from_cvars(&self.cvars, &self.data));
        let synced = self.synced_cvars();
        let changed: Vec<(String, String)> = synced
            .iter()
            .filter(|cvar| !self.cvars_sent.contains(cvar))
            .cloned()
            .collect();
        self.cvars_sent = synced;
        if changed.is_empty() {
            return;
        }
        let bytes = encode(&ServerMessage::Cvars(changed));
        for (id, client) in &self.clients {
            if client.num.is_some() && client.refused_until.is_none() {
                renet.send_message(*id, channel::RELIABLE, bytes.clone());
            }
        }
    }

    /// Server cvars clients play by (`CVAR_SYNC`).
    fn synced_cvars(&self) -> Vec<(String, String)> {
        self.cvars
            .iter()
            .filter(|c| c.flags.contains(CvarFlags::SYNC))
            .map(|c| (c.name.clone(), c.value.to_string()))
            .collect()
    }

    fn player_info(&self, num: PlayerNum) -> Option<PlayerInfo> {
        let soldier = self.world.soldiers.get(*self.players.get(&num)?)?;
        Some(PlayerInfo {
            num,
            name: soldier.name.clone(),
            team: soldier.team as u8,
            looks: NetLooks::of(soldier),
            bot: soldier.brain.is_some(),
        })
    }

    /// The clients in the match (welcomed and with the map loaded).
    fn joined(&self) -> impl Iterator<Item = ClientId> + '_ {
        self.clients
            .iter()
            .filter(|(_, c)| c.num.is_some() && c.ready)
            .map(|(id, _)| *id)
    }

    fn broadcast(&self, renet: &mut RenetServer, channel: u8, message: &ServerMessage) {
        let bytes = encode(message);
        for id in self.joined() {
            renet.send_message(id, channel, bytes.clone());
        }
    }

    fn send(renet: &mut RenetServer, id: ClientId, channel: u8, message: &ServerMessage) {
        renet.send_message(id, channel, encode(message));
    }

    /// Adds a bot from `configs/bots/<name>.bot`.
    pub fn add_bot(
        &mut self,
        renet: &mut RenetServer,
        name: &str,
        team: Team,
    ) -> anyhow::Result<()> {
        let num = self
            .free_num()
            .ok_or_else(|| anyhow::anyhow!("server full"))?;
        let profile = BotProfile::load(&self.vfs, name, &self.world.config.weapons)
            .map_err(|e| anyhow::anyhow!(e))?;
        let id = self.world.spawn_bot(&profile, team);
        self.players.insert(num, id);
        self.bots.push((name.to_string(), team));
        tracing::info!("{} has joined the game.", profile.name);
        if let Some(info) = self.player_info(num) {
            self.broadcast(renet, channel::RELIABLE, &ServerMessage::PlayerJoined(info));
        }
        Ok(())
    }

    /// Connections (from the address `addr` gives for each), disconnections and the
    /// clients' messages.
    pub fn receive(&mut self, renet: &mut RenetServer, addr: impl Fn(ClientId) -> Option<IpAddr>) {
        while let Some(event) = renet.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    let demo = self.recorder.as_ref().is_some_and(|r| r.id == client_id);
                    let client = Client {
                        ip: addr(client_id).filter(|_| !demo),
                        demo,
                        ..Client::default()
                    };
                    self.clients.insert(client_id, client);
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    tracing::info!(client_id, %reason, "disconnected");
                    self.leave(renet, client_id);
                }
            }
        }

        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            for channel in [channel::RELIABLE, channel::UNRELIABLE] {
                while let Some(bytes) = renet.receive_message(id, channel) {
                    match decode::<ClientMessage>(&bytes) {
                        Some(message) => self.handle(renet, id, message),
                        None => tracing::warn!(id, "bad message"),
                    }
                }
            }
        }
    }

    fn leave(&mut self, renet: &mut RenetServer, client_id: ClientId) {
        let Some(client) = self.clients.remove(&client_id) else {
            return;
        };
        // not in the match yet (still getting the map): nobody to tell
        let Some(num) = client.num else { return };
        let Some(id) = self.players.remove(&num) else {
            return;
        };
        self.keep_tk_warnings(num, client.ip.map(admin::ip_text));
        let team = self.world.soldiers.get(id).map_or(0, |s| s.team as u8);
        if let Some(soldier) = self.world.soldiers.get(id) {
            tracing::info!("{} has left the game.", soldier.name);
        }
        self.world.remove_soldier(id);
        let why = LeaveReason::Left;
        self.broadcast(
            renet,
            channel::RELIABLE,
            &ServerMessage::PlayerLeft { num, why },
        );
        self.vote_target_left(renet, num);
        self.balance_bots(renet, true, team);
    }

    /// `DoBalanceBots` (`sv_botbalance`, in the flag modes): as a human joins `team` (or
    /// leaves), a bot leaves the bigger team, or else a random one joins the smaller.
    fn balance_bots(&mut self, renet: &mut RenetServer, left: bool, team: u8) {
        use GameMode::*;
        let flags = matches!(
            self.world.config.game_mode,
            CaptureTheFlag | HoldTheFlag | Infiltration
        );
        if !self.cvars.bool("sv_botbalance") || !flags {
            return;
        }
        let mut teams = [0usize; 6];
        for soldier in self
            .world
            .soldiers
            .values()
            .filter(|s| s.active && !s.is_spectator())
        {
            teams[soldier.team as usize] += 1;
        }
        let mut bots: Vec<(PlayerNum, u8)> = self
            .players
            .iter()
            .filter_map(|(&num, &id)| {
                let soldier = self.world.soldiers.get(id)?;
                soldier.brain.is_some().then_some((num, soldier.team as u8))
            })
            .collect();
        bots.sort_unstable();
        let (alpha, bravo) = (teams[1], teams[2]);
        if left {
            // a bot leaves the bigger team
            let leaves = bots
                .iter()
                .find(|&&(_, t)| (alpha > bravo && t == 1) || (bravo > alpha && t == 2));
            if let Some(&(num, _)) = leaves {
                self.kick_player(renet, num, LeaveReason::Left, None);
                return;
            }
        } else if alpha != bravo
            && let Some(&(num, _)) = bots.iter().find(|&&(_, t)| t == team)
        {
            // one of the joined team's bots makes room, and a bigger team gives up one too
            self.kick_player(renet, num, LeaveReason::Left, None);
            self.balance_bots(renet, true, if alpha > bravo { 2 } else { 1 });
            return;
        }
        let smaller = match alpha.cmp(&bravo) {
            std::cmp::Ordering::Greater => Team::Bravo,
            std::cmp::Ordering::Less => Team::Alpha,
            std::cmp::Ordering::Equal => return,
        };
        let bot = self.random_bot();
        match self.add_bot(renet, &bot, smaller) {
            Ok(()) => {
                let team = format!("{smaller:?}").to_lowercase();
                tracing::info!("{bot} has joined {team} team. (Bot Balance)");
            }
            Err(error) => tracing::warn!(%error, bot, "bot balance"),
        }
    }

    /// `RandomBot`: one of `configs/bots`.
    fn random_bot(&mut self) -> String {
        let bots: Vec<String> = self
            .vfs
            .list("configs/bots")
            .into_iter()
            .filter_map(|f| f.strip_suffix(".bot").map(str::to_string))
            .collect();
        let bot = match bots.len() {
            0 => "Dummy".to_string(),
            n => bots[self.world.rng.below(n as i32) as usize].clone(),
        };
        if bot.eq_ignore_ascii_case("boogie man") || bot.eq_ignore_ascii_case("dummy") {
            "Sniper".to_string()
        } else {
            bot
        }
    }

    fn handle(&mut self, renet: &mut RenetServer, client_id: ClientId, message: ClientMessage) {
        // turned away: nothing more from it
        if self
            .clients
            .get(&client_id)
            .is_some_and(|c| c.refused_until.is_some())
        {
            return;
        }
        let num = self.clients.get(&client_id).and_then(|c| c.num);
        let soldier = num.and_then(|n| self.players.get(&n)).copied();
        if soldier.is_some() {
            self.watch_message(client_id, &message);
        }
        match (message, soldier) {
            (
                ClientMessage::Hello {
                    version,
                    password,
                    name,
                    looks,
                    team,
                },
                None,
            ) if num.is_none() => {
                let wanted = self.cvars.string("sv_password");
                if !wanted.is_empty() && password != wanted {
                    self.refuse(renet, client_id, Refusal::WrongPassword);
                    return;
                }
                self.hello(renet, client_id, version, name, looks, team)
            }
            (ClientMessage::Ready { mod_hash }, _) if num.is_some() => {
                self.ready(renet, client_id, mod_hash);
            }
            (ClientMessage::Download(request), _) if num.is_some() => {
                self.download(renet, client_id, request);
            }
            (ClientMessage::JoinTeam(team), Some(id)) => self.join_team(renet, id, team, false),
            // the team menu's first pick: the player joins with it
            (ClientMessage::JoinTeam(team), None) if num.is_some() => {
                let ready = self
                    .clients
                    .get(&client_id)
                    .is_some_and(|c| c.ready && c.hello.is_some());
                if let (true, Some(num)) = (ready, num) {
                    let team = self.fix_team(team);
                    match self.team_refusal(Team::Spectator, team) {
                        Some(text) => {
                            let text = ServerMessage::ServerText(text);
                            Self::send(renet, client_id, channel::RELIABLE, &text);
                        }
                        None => self.spawn_player(renet, client_id, num, team),
                    }
                }
            }
            (ClientMessage::Loadout { primary, secondary }, Some(id)) => {
                let weapons = self.world.config.weapons.clone();
                if let Some(soldier) = self.world.soldiers.get_mut(id) {
                    // only what the player may pick (advance mode, `weaponoff`)
                    let allowed = |kind: WeaponKind| {
                        if soldier.may_use(kind) {
                            kind
                        } else {
                            WeaponKind::NoWeapon
                        }
                    };
                    let (primary, secondary) = (
                        allowed(weapon_kind(primary)),
                        allowed(weapon_kind(secondary)),
                    );
                    soldier.loadout = [primary, secondary, WeaponKind::FragGrenade];
                    // picked while alive: the weapons right away (not over a bow)
                    let bow = soldier
                        .primary_weapon()
                        .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow]);
                    if !soldier.dead_meat && !bow {
                        let active = soldier.active_weapon;
                        soldier.weapons[active] = weapons.get(primary);
                        soldier.weapons[(active + 1) % 2] = weapons.get(secondary);
                    }
                }
            }
            (ClientMessage::Control(control), soldier) => {
                // a snapshot the server still has, newer than the one it had
                let acked = self.snapshots.iter().any(|s| s.tick == control.snapshot);
                if let Some(client) = self.clients.get_mut(&client_id).filter(|c| c.ready) {
                    if acked && client.acked.is_none_or(|tick| control.snapshot > tick) {
                        client.acked = Some(control.snapshot);
                    }
                    // (picking a team, watching: the snapshot it has, nothing to control)
                    if soldier.is_none() {
                        return;
                    }
                    let newer = client
                        .control
                        .as_ref()
                        .is_none_or(|c| control.tick >= c.tick);
                    if newer {
                        client.control = Some(control);
                    }
                }
            }
            (ClientMessage::Chat { text, team, radio }, Some(id)) => {
                let Some(from) = num else { return };
                if !self.watch_chat(renet, client_id, from, &text) {
                    return;
                }
                // the radio menu's two digits, 1 to 3 each; for the team, like team chat
                let radio =
                    radio.filter(|r| (1..=3).contains(&(r / 10)) && (1..=3).contains(&(r % 10)));
                let team = team || radio.is_some();
                let sender_team = self.world.soldiers.get(id).map(|s| s.team);
                let muted = self.is_muted(client_id);
                let line = format!(
                    "{}{}[{}] {text}",
                    match (radio, team) {
                        (Some(_), _) => "(RADIO) ",
                        (None, true) => "(TEAM) ",
                        (None, false) => "",
                    },
                    if muted { "(MUTED) " } else { "" },
                    self.name_of(from)
                );
                tracing::info!("{line}");
                // a muted player's chat reaches everyone as "(Muted)"
                let (text, team, radio) = if muted {
                    ("(Muted)".to_string(), false, None)
                } else {
                    (text, team, radio)
                };
                let message = encode(&ServerMessage::Chat {
                    from,
                    text,
                    team,
                    radio,
                });
                for other in self.joined().collect::<Vec<_>>() {
                    let other_team = self.clients[&other]
                        .num
                        .and_then(|n| self.players.get(&n))
                        .and_then(|s| self.world.soldiers.get(*s))
                        .map(|s| s.team);
                    if !team || other_team == sender_team {
                        renet.send_message(other, channel::RELIABLE, message.clone());
                    }
                }
            }
            (ClientMessage::Bullet(bullet), Some(id)) => {
                if self.knife_cheat(renet, num, id, &bullet) {
                    return;
                }
                if let Some(num) = num
                    && self.knife_throw_cheat(renet, client_id, num, &bullet)
                {
                    return;
                }
                if self.bullet_allowed(client_id, id, &bullet) {
                    self.world.receive_bullet(id, &bullet, 0);
                    self.send_bullets(renet);
                }
            }
            (ClientMessage::Vote { kind, reason }, Some(_)) => {
                self.vote(renet, client_id, kind, reason);
            }
            (ClientMessage::MapList, _) if num.is_some() => {
                let maps = ServerMessage::MapList(self.maps_list());
                Self::send(renet, client_id, channel::RELIABLE, &maps);
            }
            (ClientMessage::Command(line), _) if num.is_some() => {
                self.command(renet, Some(client_id), &line);
            }
            (message, _) => tracing::debug!(client_id, ?message, "ignored"),
        }
    }

    /// `ServerHandleBulletSnapshot`: a thrown knife from a player who may not have one
    /// (`WeaponActive[KNIFE]`; ours is each player's, `weaponoff`) kicks them and bans them for
    /// a day with `sv_anticheatkick` ("Knife-Spawn Cheat"); without it the knife flies, like
    /// Soldat's. `true` if they're out.
    fn knife_cheat(
        &mut self,
        renet: &mut RenetServer,
        num: Option<PlayerNum>,
        id: SoldierId,
        bullet: &BulletState,
    ) -> bool {
        let thrown = WeaponKind::values()
            .get(usize::from(bullet.weapon))
            .is_some_and(|&kind| {
                self.world.config.weapons.get(kind).bullet_style == BulletStyle::ThrownKnife
            });
        let allowed = self
            .world
            .soldiers
            .get(id)
            .is_none_or(|s| s.may_use(WeaponKind::Knife));
        let (true, false, true, Some(num)) =
            (thrown, allowed, self.cvars.bool("sv_anticheatkick"), num)
        else {
            return false;
        };
        let ban = Some(("Knife-Spawn Cheat".to_string(), admin::DAY));
        self.kick_player(renet, num, LeaveReason::Cheat, ban);
        true
    }

    /// `ServerHandleBulletSnapshot`'s checks: a gun the player has, at a believable speed,
    /// from near its hands, not faster than the gun fires; grenades not too often. (Soldat
    /// also takes a grenade here; ours went with the throw the player's controls made.)
    fn bullet_allowed(&mut self, client_id: ClientId, id: SoldierId, bullet: &BulletState) -> bool {
        let Some(&kind) = WeaponKind::values().get(usize::from(bullet.weapon)) else {
            return false;
        };
        let (Some(soldier), Some(client)) = (
            self.world.soldiers.get(id),
            self.clients.get_mut(&client_id),
        ) else {
            return false;
        };
        let (pos, velocity) = (Vec2::from(bullet.pos), Vec2::from(bullet.velocity));
        if soldier.is_spectator() || !pos.is_finite() || !velocity.is_finite() {
            return false;
        }
        let gun = self.world.config.weapons.get(kind);
        let style = gun.bullet_style;
        let grenade = matches!(
            style,
            BulletStyle::FragGrenade | BulletStyle::ClusterGrenade
        );
        let special = grenade
            || matches!(
                style,
                BulletStyle::Cluster | BulletStyle::ThrownKnife | BulletStyle::M2Bullet
            );

        if style == BulletStyle::M2Bullet {
            let at = soldier.skeleton.pos(1);
            let near = self.world.things.iter().any(|thing| {
                thing.active
                    && thing.kind == ThingKind::StationaryGun
                    && distance(at, thing.skeleton.pos(1)) < STAT_REACH
            });
            if !near {
                return false;
            }
        }
        // one of its guns (Soldat compares the style with the gun the client last reported)
        let has = soldier.weapons[..2].iter().any(|w| w.kind == kind);
        if !special && style != BulletStyle::Fist && !has {
            return false;
        }
        if client.shots.last.as_ref() == Some(bullet) {
            return false;
        }
        if !special && velocity.length() > gun.speed + 10.0 * gun.inherited_velocity {
            return false;
        }
        if !special && style != BulletStyle::Flame {
            let hands = soldier.skeleton.pos(15) - velocity / 1.33 - vec2(0.0, 2.0);
            if distance(hands, pos) > 366.0 {
                return false;
            }
        }

        let now = self.uptime;
        if !special && style != BulletStyle::Fist {
            let since = now.saturating_sub(client.shots.last_tick) as f32;
            let too_soon = match gun.ammo {
                0 => false,
                1 => since < f32::from(gun.reload_time) * 0.9,
                _ => since < f32::from(gun.fire_interval) * 0.85,
            };
            client.shots.warnings = if too_soon {
                client.shots.warnings + 1
            } else {
                0
            };
            if client.shots.warnings > 2 {
                return false;
            }
            client.shots.last_tick = now;
        }
        if grenade {
            if client
                .shots
                .last_grenade
                .is_some_and(|t| now.saturating_sub(t) < 6)
            {
                return false;
            }
            client.shots.last_grenade = Some(now);
        }
        client.shots.last = Some(bullet.clone());
        true
    }

    /// The slow shots to the other players (`ServerBulletSnapshot`, unreliable like Soldat's).
    fn send_bullets(&mut self, renet: &mut RenetServer) {
        let Some(sent) = self.world.net_bullets.as_mut() else {
            return;
        };
        let bullets = std::mem::take(sent);
        let joined: Vec<ClientId> = self.joined().collect();
        for bullet in bullets {
            let Some(owner) = self.num_of(bullet.owner) else {
                continue;
            };
            let message = encode(&ServerMessage::Bullet {
                owner,
                bullet: bullet.state(),
            });
            for client_id in &joined {
                if self.clients[client_id].num != Some(owner) {
                    renet.send_message(*client_id, channel::UNRELIABLE, message.clone());
                }
            }
        }
    }

    /// Tells a client why it can't join, and lets it go a little later.
    fn refuse(&mut self, renet: &mut RenetServer, client_id: ClientId, reason: Refusal) {
        tracing::info!(client_id, %reason, "refused");
        let refused = ServerMessage::Refused(reason);
        Self::send(renet, client_id, channel::RELIABLE, &refused);
        let until = self.uptime + REFUSED_TICKS;
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.refused_until = Some(until);
        }
    }

    fn hello(
        &mut self,
        renet: &mut RenetServer,
        client_id: ClientId,
        version: u32,
        name: String,
        looks: NetLooks,
        team: Option<u8>,
    ) {
        if version != PROTOCOL_VERSION {
            self.refuse(renet, client_id, Refusal::WrongVersion(PROTOCOL_VERSION));
            return;
        }
        let ip = self.clients.get(&client_id).and_then(|c| c.ip);
        if let Some(reason) = self.banned(ip) {
            self.refuse(renet, client_id, Refusal::Banned(reason));
            return;
        }
        let humans = self.clients.values().filter(|c| c.num.is_some()).count();
        let full = humans >= self.cvars.int("sv_maxplayers") as usize;
        let Some(num) = self.free_num().filter(|_| !full) else {
            self.refuse(renet, client_id, Refusal::ServerFull);
            return;
        };

        if let Some(client) = self.clients.get_mut(&client_id) {
            client.num = Some(num);
            client.hello = Some((name.chars().take(24).collect(), looks, team));
        }
        Self::send(
            renet,
            client_id,
            channel::RELIABLE,
            &ServerMessage::Welcome {
                you: num,
                map: self.map_name(),
                map_hash: self.map_hash,
                cvars: self.synced_cvars(),
                weapons_mods: self.data.weapons_mods.clone(),
                game_mod: self.game_mod.as_ref().map(|(game_mod, _)| game_mod.clone()),
            },
        );
    }

    /// The client has the map: its soldier joins (the first time), and it hears of everyone.
    fn ready(&mut self, renet: &mut RenetServer, client_id: ClientId, mod_hash: u64) {
        // `sv_pure`: the server's mod, or none if it has none (`WRONG_CHECKSUM`)
        let wanted = self
            .game_mod
            .as_ref()
            .map_or(0, |(game_mod, _)| game_mod.hash);
        if self.cvars.bool("sv_pure") && mod_hash != wanted {
            self.refuse(renet, client_id, Refusal::WrongChecksum);
            return;
        }
        let Some(client) = self.clients.get_mut(&client_id) else {
            return;
        };
        let Some(num) = client.num else { return };
        if client.ready {
            return;
        }
        client.ready = true;
        if self.world.game.paused() {
            Self::send(
                renet,
                client_id,
                channel::RELIABLE,
                &ServerMessage::Paused(true),
            );
        }

        let mut nums: Vec<PlayerNum> = self.players.keys().copied().collect();
        nums.sort_unstable();
        for other in nums {
            if let Some(info) = self.player_info(other) {
                let joined = ServerMessage::PlayerJoined(info);
                Self::send(renet, client_id, channel::RELIABLE, &joined);
            }
        }

        if let Some(&id) = self.players.get(&num) {
            // back from a map change: where its soldier is now
            if let Some(soldier) = self.world.soldiers.get(id).filter(|s| !s.dead_meat) {
                let pos = soldier.particle.pos;
                let respawned = ServerMessage::Respawned {
                    num,
                    pos: pos.into(),
                };
                Self::send(renet, client_id, channel::RELIABLE, &respawned);
                self.position_reset(num, pos);
            }
            return;
        }
        // the demo recorder only watches
        if self.clients[&client_id].demo {
            return;
        }
        let Some((_, _, team)) = self.clients[&client_id].hello.clone() else {
            return;
        };
        // a team game without a team: the player picks one from the team menu first (Soldat's
        // client sends its player info then), watching meanwhile
        let team = match team {
            Some(team) => self.fix_team(team),
            None if self.world.config.game_mode.is_team_game() => return,
            None => Team::None as u8,
        };
        self.spawn_player(renet, client_id, num, team);
    }

    /// The client's player joins the match in `team` (`ServerHandlePlayerInfo`).
    fn spawn_player(
        &mut self,
        renet: &mut RenetServer,
        client_id: ClientId,
        num: PlayerNum,
        team: u8,
    ) {
        let Some((name, looks, _)) = self.clients[&client_id].hello.clone() else {
            return;
        };
        let id = self.world.spawn_soldier();
        let soldier = &mut self.world.soldiers[id];
        soldier.name = name;
        soldier.remote = true;
        looks.apply(soldier);
        self.players.insert(num, id);
        self.last_player = Some(num);
        self.cheat_tags.remove(&num);
        let ip = self.clients[&client_id].ip.map(admin::ip_text);
        let tk_warnings = ip.and_then(|ip| self.tk_list.get(&ip)).copied();
        self.tk_warnings.insert(num, tk_warnings.unwrap_or(0));
        let cooldown = self.uptime + vote::VOTE_COOLDOWN_TICKS;
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.vote_cooldown = cooldown;
        }

        self.join_team(renet, id, team, true);
        // its spawn now, for everyone (and its positions count from here: none from before)
        if let Some(soldier) = self.world.soldiers.get(id) {
            let (alive, pos) = (!soldier.dead_meat, soldier.particle.pos);
            self.dead.insert(id, !alive);
            if alive {
                let respawned = ServerMessage::Respawned {
                    num,
                    pos: pos.into(),
                };
                self.broadcast(renet, channel::RELIABLE, &respawned);
                self.new_weapon(num);
                self.position_reset(num, pos);
            }
        }
        let team = self.world.soldiers.get(id).map_or(0, |s| s.team as u8);
        self.balance_bots(renet, false, team);

        // the greetings
        for cvar in ["sv_greeting", "sv_greeting2", "sv_greeting3"] {
            let text = self.cvars.string(cvar).to_string();
            if !text.is_empty() {
                Self::send(
                    renet,
                    client_id,
                    channel::RELIABLE,
                    &ServerMessage::ServerText(text),
                );
            }
        }
    }

    /// Queues a file for a client, or says there's none.
    fn download(&mut self, renet: &mut RenetServer, client_id: ClientId, request: String) {
        // the server's mod, from its archive
        if let Some((game_mod, bytes)) = &self.game_mod
            && request == game_mod.path()
        {
            tracing::info!(client_id, path = request, size = bytes.len(), "download");
            if let Some(client) = self.clients.get_mut(&client_id) {
                let bytes = bytes.clone();
                client
                    .downloads
                    .push_back((request.clone(), request, bytes, 0));
            }
            return;
        }
        // images may be shipped with another extension (`stopa.bmp` as `stopa.png`)
        let found = if !downloadable(&request) {
            None
        } else if self.vfs.exists(&request) {
            Some(request.clone())
        } else {
            self.vfs
                .find_with_extensions(&request, &["png", "jpg", "gif", "bmp"])
        };
        let bytes = found.and_then(|path| Some((path.clone(), self.vfs.read(&path).ok()?)));
        match bytes {
            Some((path, bytes)) => {
                tracing::info!(client_id, path, size = bytes.len(), "download");
                if let Some(client) = self.clients.get_mut(&client_id) {
                    client
                        .downloads
                        .push_back((request, path, Arc::new(bytes), 0));
                }
            }
            None => Self::send(
                renet,
                client_id,
                channel::RELIABLE,
                &ServerMessage::NoFile(request),
            ),
        }
    }

    /// The match now, to each client as what changed since the newest snapshot it has (whole
    /// if the server no longer has that one).
    fn send_snapshots(&mut self, renet: &mut RenetServer) {
        // the dead's respawn counters only every so often (`ServerSkeletonSnapshot`)
        let dead_snapshot = self.dead_snapshot_due();
        let last = self.snapshots.back();
        let mut soldiers: Vec<SoldierState> = self
            .players
            .iter()
            .filter_map(|(num, id)| Some(SoldierState::of(*num, self.world.soldiers.get(*id)?)))
            .map(|mut state| {
                let was = last.and_then(|s| s.soldiers.iter().find(|o| o.num == state.num));
                if let Some(was) = was.filter(|_| !dead_snapshot) {
                    state.respawn_counter = was.respawn_counter;
                }
                state
            })
            .collect();
        soldiers.sort_by_key(|s| s.num);
        // the things' points only every so often, but a new thing's (parachutes are the
        // clients' own)
        let thing_snapshot = self.thing_snapshot_due();
        let last = self.snapshots.back();
        let things = self
            .world
            .things
            .iter()
            .enumerate()
            .filter(|(_, t)| t.active && t.kind != ThingKind::Parachute)
            .map(|(slot, t)| {
                let mut state = ThingState::of(slot, t, |id| self.num_of(id));
                let was = last.and_then(|s| {
                    s.things
                        .iter()
                        .find(|o| o.slot == state.slot && o.kind == state.kind)
                });
                if let Some(was) = was.filter(|_| !thing_snapshot) {
                    state.points.clone_from(&was.points);
                    state.old_points.clone_from(&was.old_points);
                }
                state
            })
            .collect();
        let snapshot = Snapshot {
            tick: self.uptime,
            soldiers,
            things,
            thing_snapshot,
            dead_snapshot,
            team_scores: self.world.game.team_scores,
            time_left: self.world.game.time_left,
        };

        // clients that have the same snapshot get the same bytes
        let mut encoded: HashMap<Option<u64>, Vec<u8>> = HashMap::new();
        for client_id in self.joined().collect::<Vec<_>>() {
            let acked = self.clients[&client_id].acked;
            let base = acked.and_then(|tick| self.snapshots.iter().find(|s| s.tick == tick));
            let bytes = encoded
                .entry(base.map(|b| b.tick))
                .or_insert_with(|| match base {
                    Some(base) => encode(&ServerMessage::SnapshotDelta(SnapshotDelta::between(
                        base, &snapshot,
                    ))),
                    None => encode(&ServerMessage::Snapshot(snapshot.clone())),
                });
            renet.send_message(client_id, channel::UNRELIABLE, bytes.clone());
        }

        if self.snapshots.len() == SERVER_SNAPSHOTS {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    /// Whether the things' points go out (`ServerThingSnapshot`): every `net_t1_thingsnapshot`
    /// ticks (12 on a LAN), sooner with fewer players in the match.
    fn thing_snapshot_due(&mut self) -> bool {
        let every = self.send_every("net_t1_thingsnapshot", 12);
        let due = self
            .thing_snapshot_tick
            .is_none_or(|last| self.uptime >= last + every);
        if due {
            self.thing_snapshot_tick = Some(self.uptime);
        }
        due
    }

    /// Whether the dead's respawn counters go out (`ServerSkeletonSnapshot`): every
    /// `net_t1_deadsnapshot` ticks (20 on a LAN), sooner with fewer players in the match.
    fn dead_snapshot_due(&mut self) -> bool {
        let every = self.send_every("net_t1_deadsnapshot", 20);
        let due = self
            .dead_snapshot_tick
            .is_none_or(|last| self.uptime >= last + every);
        if due {
            self.dead_snapshot_tick = Some(self.uptime);
        }
        due
    }

    /// Ticks between sends of a kind: the `net_t1_*` cvar's (`lan` on a LAN), times
    /// `Adjust` (fewer players in the match, more often).
    fn send_every(&self, cvar: &str, lan: i64) -> u64 {
        let playing = self
            .world
            .soldiers
            .values()
            .filter(|s| s.active && !s.is_spectator())
            .count();
        let adjust = match playing {
            0..5 => 0.66,
            5..9 => 0.75,
            _ => 1.0,
        };
        let ticks = if self.cvars.int("net_lan") == 1 {
            lan
        } else {
            self.cvars.int(cvar)
        };
        (ticks as f32 * adjust).round().max(1.0) as u64
    }

    /// Downloads go on: a few pieces per client per tick.
    fn send_downloads(&mut self, renet: &mut RenetServer) {
        for (client_id, client) in &mut self.clients {
            for _ in 0..CHUNKS_PER_TICK {
                let Some((request, path, bytes, offset)) = client.downloads.front_mut() else {
                    break;
                };
                let end = (*offset + CHUNK_BYTES).min(bytes.len());
                let chunk = ServerMessage::FileChunk {
                    request: request.clone(),
                    path: path.clone(),
                    size: bytes.len() as u32,
                    offset: *offset as u32,
                    data: bytes[*offset..end].to_vec(),
                };
                let encoded = encode(&chunk);
                if !renet.can_send_message(*client_id, channel::RELIABLE, encoded.len()) {
                    break;
                }
                renet.send_message(*client_id, channel::RELIABLE, encoded);
                *offset = end;
                if end >= bytes.len() {
                    client.downloads.pop_front();
                }
            }
        }
    }

    /// `TSprite.ChangeTeam`: the team if the player may (`admin` may always), else why not.
    fn join_team(&mut self, renet: &mut RenetServer, id: SoldierId, team: u8, admin: bool) {
        let mode = self.world.config.game_mode;
        let (Some(num), Some(soldier)) = (self.num_of(id), self.world.soldiers.get(id)) else {
            return;
        };
        if team > Team::Spectator as u8 {
            return;
        }
        let refusal = (!admin)
            .then(|| self.team_refusal(soldier.team, team))
            .flatten();
        // a mode's teams only (`FixTeam` does this for joining players)
        let fits = match team {
            0 => !mode.is_team_game(),
            1 | 2 => mode.is_team_game(),
            3 | 4 => mode == GameMode::Teammatch,
            _ => true,
        };
        if let Some(text) = refusal {
            if let Some(client_id) = self.client_of(num) {
                Self::send(
                    renet,
                    client_id,
                    channel::RELIABLE,
                    &ServerMessage::ServerText(text),
                );
            }
            return;
        }
        if !admin && !fits {
            return;
        }
        self.change_team(renet, id, num, team);
    }

    /// Why a player in `current` (the spectators while still picking) can't join `team`, if
    /// it can't: `sv_balanceteams`, `sv_maxspectators`.
    fn team_refusal(&self, current: Team, team: u8) -> Option<String> {
        let mode = self.world.config.game_mode;
        let mut playing = [0usize; 6];
        for s in self.world.soldiers.values().filter(|s| !s.is_spectator()) {
            playing[s.team as usize] += 1;
        }
        // (Soldat counts only who's playing, so its limit never counts the spectators)
        let spectators = self
            .world
            .soldiers
            .values()
            .filter(|s| s.is_spectator())
            .count();
        let mut refusal = None;
        if mode.is_team_game() && self.cvars.bool("sv_balanceteams") {
            // uneven teams: a spectator may join the smallest, others one smaller than theirs
            let lowest = lowest_team(&playing, mode);
            let to_lowest = current == Team::Spectator && usize::from(team) == lowest;
            let current = current as usize;
            if !to_lowest && team < 5 && playing[usize::from(team)] >= playing[current] {
                let name = ["Alpha", "Bravo", "Charlie", "Delta"][usize::from(team.max(1)) - 1];
                refusal = Some(format!("{name} team is full"));
            }
        }
        if team == 5 && spectators >= self.cvars.int("sv_maxspectators") as usize {
            refusal = Some("Spectators are full".to_string());
        }
        refusal
    }

    /// The player goes to `team` (`TSprite.ChangeTeam`), and everyone hears of it.
    fn change_team(&mut self, renet: &mut RenetServer, id: SoldierId, num: PlayerNum, team: u8) {
        let team = team_from_num(team);
        let mut respawned_alive = None;
        // (a soldier the ticks haven't seen yet is a joining player's, or one come along to a
        // new map)
        let new = !self.dead.contains_key(&id);
        if team == Team::Spectator && new {
            self.world.add_spectator(id);
        } else if team == Team::Spectator {
            self.world.join_spectators(id);
        } else {
            let config = self.world.config.clone();
            let soldier = &mut self.world.soldiers[id];
            soldier.team = team;
            soldier.looks.apply_team_shirt(team, &config);
            self.world.respawn_soldier(id);
            if self.dead.get(&id) == Some(&false) {
                respawned_alive = Some(self.world.soldiers[id].particle.pos);
            }
        }
        let name = &self.world.soldiers[id].name;
        match team {
            Team::None => tracing::info!("{name} has joined the game."),
            Team::Spectator if new => tracing::info!("{name} has joined as spectator."),
            Team::Spectator => tracing::info!("{name} has joined spectators."),
            team => tracing::info!(
                "{name} has joined {} team.",
                format!("{team:?}").to_lowercase()
            ),
        }
        if let Some(info) = self.player_info(num) {
            self.broadcast(renet, channel::RELIABLE, &ServerMessage::PlayerJoined(info));
        }
        // alive already, the tick sees no respawn: everyone hears of it here (its client too,
        // which kept its soldier where it was)
        if let Some(pos) = respawned_alive {
            let respawned = ServerMessage::Respawned {
                num,
                pos: pos.into(),
            };
            self.broadcast(renet, channel::RELIABLE, &respawned);
            self.new_weapon(num);
            self.position_reset(num, pos);
        }
    }

    /// `FixTeam`: a team the mode has (a random one if the asked one isn't).
    fn fix_team(&mut self, team: u8) -> u8 {
        use GameMode::*;
        match self.world.config.game_mode {
            Deathmatch | Pointmatch | Rambo if team != 0 && team != 5 => 0,
            Teammatch if !(1..=5).contains(&team) => self.world.rng.below(4) as u8 + 1,
            CaptureTheFlag | Infiltration | HoldTheFlag if ![1, 2, 5].contains(&team) => {
                self.world.rng.below(2) as u8 + 1
            }
            _ => team,
        }
    }

    fn client_of(&self, num: PlayerNum) -> Option<ClientId> {
        self.clients
            .iter()
            .find(|(_, c)| c.num == Some(num))
            .map(|(id, _)| *id)
    }

    /// One tick: the players' controls, the world, and what clients hear of it.
    pub fn tick(&mut self, renet: &mut RenetServer) {
        self.uptime += 1;
        let now = self.uptime;
        for (client_id, client) in &self.clients {
            if client.refused_until.is_some_and(|until| now >= until) {
                renet.disconnect(*client_id);
            }
        }
        self.guard_timers();
        self.ban_timer();
        self.sync_cvars(renet);
        // the first map's demo
        if now == 1 {
            self.auto_record(renet);
        }
        // nobody left to wait for
        if self.world.game.paused() && !self.clients.values().any(|c| c.ready) {
            self.world.game.unpause();
        }
        let every = self.cvars.int("log_filesupdate").max(1) as u64;
        if self.uptime.is_multiple_of(every)
            && let Some(logs) = &mut self.logs
        {
            logs.write();
        }

        // the clients' own soldiers are where they say (ClientSpriteSnapshot), as far as they
        // can have gone (`sv_movecheck`)
        let check = self.cvars.bool("sv_movecheck");
        let mut inputs = Vec::new();
        let mut put_back = Vec::new();
        for (client_id, client) in &mut self.clients {
            let (Some(num), Some(control)) = (client.num, &client.control) else {
                continue;
            };
            let Some(&id) = self.players.get(&num) else {
                continue;
            };
            if let Some(soldier) = self.world.soldiers.get_mut(id)
                && !soldier.dead_meat
            {
                match client.moves.judge(control, now, check) {
                    guard::Move::Take => {
                        let max = Vec2::splat(MAX_VELOCITY);
                        let velocity = Vec2::from(control.velocity);
                        soldier.particle.pos = Vec2::from(control.pos);
                        soldier.particle.velocity = if check {
                            velocity.clamp(-max, max)
                        } else {
                            velocity
                        };
                    }
                    guard::Move::Ignore => {}
                    guard::Move::PutBack => put_back.push((*client_id, id)),
                }
            }
            let mut input = control.input();
            if let Some(soldier) = self.world.soldiers.get(id) {
                // prone toggles while the client's position differs (its buttons don't say)
                let toggle = (control.position == POS_PRONE) != (soldier.position == POS_PRONE);
                input.buttons.set(Buttons::PRONE, toggle);
            }
            inputs.push((id, input));
        }
        for (client_id, id) in put_back {
            self.put_back(renet, client_id, id);
        }

        let events = self.world.step(&inputs);
        // (the vote's clock stands still in bullet time too)
        if self.world.clocks_ran {
            self.vote_timer();
        }
        self.send_events(renet, &events);
        self.send_bullets(renet);
        self.guard(renet);

        // respawns
        let mut respawned = Vec::new();
        for (num, id) in &self.players {
            let Some(soldier) = self.world.soldiers.get(*id) else {
                continue;
            };
            let was_dead = self.dead.insert(*id, soldier.dead_meat).unwrap_or(true);
            if was_dead && !soldier.dead_meat {
                respawned.push(ServerMessage::Respawned {
                    num: *num,
                    pos: soldier.particle.pos.into(),
                });
            }
        }
        for message in respawned {
            if let ServerMessage::Respawned { num, pos } = message {
                self.new_weapon(num);
                self.position_reset(num, Vec2::from(pos));
            }
            self.broadcast(renet, channel::RELIABLE, &message);
        }

        let tick = self.world.tick;
        if tick.is_multiple_of(SNAPSHOT_TICKS) {
            // the pings: renet's round trip runs from the tick a packet left to the tick its
            // answer is read, like Soldat's pongs (`PingTicks`), so it's a tick even on the
            // same machine. Without that tick it's about the network's, which Soldat shows
            // (`RealPing`, GameNetworkingSockets' ping).
            let tick_time = 1.0 / f64::from(self.world.goal_ticks());
            for (client_id, client) in &self.clients {
                let soldier = client.num.and_then(|n| self.players.get(&n));
                if let Some(soldier) = soldier.and_then(|id| self.world.soldiers.get_mut(*id)) {
                    let rtt = renet.rtt(*client_id);
                    soldier.ping_ticks = (rtt / tick_time).round().min(255.0) as u8;
                    // the packets that came (GameNetworkingSockets' `ConnectionQualityLocal`)
                    let loss = renet
                        .network_info(*client_id)
                        .map_or(1.0, |n| n.packet_loss);
                    soldier.connection_quality = ((1.0 - loss) * 100.0).clamp(0.0, 100.0) as u8;
                    soldier.ping = ((rtt - tick_time).max(0.0) * 1000.0).round().min(9999.0) as u16;
                }
            }
            self.send_snapshots(renet);
        }
        self.send_downloads(renet);
        self.record(renet);

        if events.contains(&GameEvent::ChangeMap) {
            self.next_map(renet);
        }
    }

    fn send_events(&mut self, renet: &mut RenetServer, events: &[GameEvent]) {
        for event in events {
            let message = match *event {
                GameEvent::Killed {
                    victim,
                    killer,
                    how,
                    weapon,
                    headshot,
                    hit,
                    shot,
                } => {
                    self.log_kill(killer, victim, weapon);
                    let (Some(victim_num), Some(killer_num), Some(body)) = (
                        self.num_of(victim),
                        self.num_of(killer),
                        self.world.soldiers.get(victim),
                    ) else {
                        continue;
                    };
                    let message = ServerMessage::Killed {
                        victim: victim_num,
                        killer: killer_num,
                        how: death_num(how),
                        weapon: weapon.map(weapon_index),
                        headshot,
                        death: DeathState::of(body, hit, shot),
                    };
                    self.broadcast(renet, channel::RELIABLE, &message);
                    self.punish_team_kill(renet, victim, killer);
                    continue;
                }
                GameEvent::Chat { who, ref text } => {
                    let Some(from) = self.num_of(who) else {
                        continue;
                    };
                    ServerMessage::Chat {
                        from,
                        text: text.clone(),
                        team: false,
                        radio: None,
                    }
                }
                GameEvent::IdleAnimation { who, style } => {
                    let Some(num) = self.num_of(who) else {
                        continue;
                    };
                    ServerMessage::IdleAnimation { num, style }
                }
                GameEvent::ThingTaken {
                    slot,
                    kind,
                    who,
                    pos,
                } => {
                    let Some(by) = self.num_of(who) else { continue };
                    if kind.is_gun() {
                        self.new_weapon(by);
                    }
                    ServerMessage::ThingTaken {
                        slot: slot as u8,
                        kind: kind as u8,
                        by,
                        pos: pos.into(),
                    }
                }
                GameEvent::FlagCaptured { team, who } => {
                    let Some(by) = self.num_of(who) else { continue };
                    ServerMessage::FlagCaptured {
                        team: team as u8,
                        by,
                    }
                }
                GameEvent::MatchEnded => ServerMessage::MatchEnded,
                _ => continue,
            };
            self.broadcast(renet, channel::RELIABLE, &message);
        }
    }

    /// A kill to the console (`sv_echokills`) and the kill log.
    fn log_kill(&mut self, killer: SoldierId, victim: SoldierId, weapon: Option<WeaponKind>) {
        let soldier = |id| self.world.soldiers.get(id);
        let (Some(k), Some(v)) = (soldier(killer), soldier(victim)) else {
            return;
        };
        // the bullet's weapon, by Soldat's names
        let with = match weapon {
            None => "Selfkill",
            Some(WeaponKind::FragGrenade) => "Grenade",
            Some(WeaponKind::ClusterGrenade) => "Cluster Grenades",
            Some(WeaponKind::M2) => "Stationary gun",
            Some(kind) => self.world.config.weapons.get(kind).name,
        };
        if self.cvars.bool("sv_echokills") && k.name != v.name {
            let (kt, vt) = (k.team as u8, v.team as u8);
            tracing::info!("({kt}) {} killed ({vt}) {} with {with}", k.name, v.name);
        }
        let (killer, victim) = (k.name.clone(), v.name.clone());
        if let Some(logs) = &mut self.logs {
            logs.kill(&killer, &victim, with);
        }
    }

    /// A console line to the console log.
    pub fn log_console(&mut self, line: &str) {
        if let Some(logs) = &mut self.logs {
            logs.console(line);
        }
    }

    /// The map list (`MapsList`).
    fn maps_list(&self) -> Vec<String> {
        self.maps.clone()
    }

    /// `LoadMapsList`: `configs/<sv_maplist>` of the config directory, else the game's
    /// `configs/mapslist.txt`.
    pub fn load_map_list(&mut self) {
        let file = format!("configs/{}", self.cvars.string("sv_maplist"));
        let text = self
            .lists
            .dir
            .as_ref()
            .and_then(|dir| std::fs::read_to_string(dir.join(&file)).ok())
            .or_else(|| self.vfs.read_to_string(&file).ok())
            .or_else(|| self.vfs.read_to_string("configs/mapslist.txt").ok())
            .unwrap_or_default();
        self.maps = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
    }

    /// The map after this one in `configs/mapslist.txt` (this one without a list).
    pub fn listed_next_map(&self) -> String {
        let maps = self.maps_list();
        let current = self.map_name();
        let next = maps
            .iter()
            .position(|m| m.eq_ignore_ascii_case(&current))
            .map_or(0, |i| (i + 1) % maps.len().max(1));
        maps.get(next).cloned().unwrap_or(current)
    }

    /// The voted map, or the next of `configs/mapslist.txt`: everyone stays, bots too.
    fn next_map(&mut self, renet: &mut RenetServer) {
        let name = self
            .next_map
            .take()
            .unwrap_or_else(|| self.listed_next_map());
        if let Err(error) = self.change_map(renet, &name) {
            tracing::warn!(%error, name, "cannot change map");
        }
    }

    pub fn change_map(&mut self, renet: &mut RenetServer, name: &str) -> anyhow::Result<()> {
        let world = new_world(&self.vfs, &self.data, &self.cvars, name)?;
        // an automatic demo ends with its map (another one starts with the next)
        if self.recorder.as_ref().is_some_and(|r| r.auto) {
            self.stop_recording(renet);
        }
        let old = std::mem::replace(&mut self.world, world);
        // the old map's snapshots: nobody starts from them
        self.snapshots.clear();
        // team kills are forgiven
        self.tk_warnings.values_mut().for_each(|w| *w = 0);
        self.tk_list.clear();
        self.map_hash = map_hash(&self.vfs, &self.map_name());
        tracing::info!(name, "map");
        // everyone loads (or downloads) it and says when they're ready
        let change = encode(&ServerMessage::MapChange {
            map: self.map_name(),
            map_hash: self.map_hash,
        });
        for (client_id, client) in &mut self.clients {
            if client.num.is_some() && client.refused_until.is_none() {
                client.ready = false;
                client.control = None;
                client.acked = None;
                client.moves.new_map();
                // the old map's files: no longer wanted (and asked for again if they are);
                // the mod still is
                let game_mod = self.game_mod.as_ref().map(|(game_mod, _)| game_mod.path());
                client
                    .downloads
                    .retain(|(request, ..)| Some(request) == game_mod.as_ref());
                renet.send_message(*client_id, channel::RELIABLE, change.clone());
            }
        }

        // the players come along with their names, looks and teams
        let players = std::mem::take(&mut self.players);
        self.dead.clear();
        let mut nums: Vec<PlayerNum> = players.keys().copied().collect();
        nums.sort_unstable();
        let bots = std::mem::take(&mut self.bots);
        for num in nums {
            let Some(old_soldier) = old.soldiers.get(players[&num]) else {
                continue;
            };
            if old_soldier.brain.is_some() {
                continue;
            }
            let id = self.world.spawn_soldier();
            let soldier = &mut self.world.soldiers[id];
            soldier.name = old_soldier.name.clone();
            soldier.remote = true;
            soldier.looks = old_soldier.looks;
            soldier.head_cap = old_soldier.head_cap;
            soldier.loadout = old_soldier.loadout;
            self.players.insert(num, id);
            let team = old_soldier.team as u8;
            self.join_team(renet, id, team, true);
        }
        for (name, team) in bots {
            if let Err(error) = self.add_bot(renet, &name, team) {
                tracing::warn!(%error, name, "bot");
            }
        }
        self.auto_record(renet);
        Ok(())
    }

    /// `demo_autorecord`: a demo of each map, from its start.
    fn auto_record(&mut self, renet: &mut RenetServer) {
        if self.cvars.bool("demo_autorecord") && self.recorder.is_none() {
            self.start_recording(renet, None, true);
        }
    }

    /// `record [name]`: a demo from now on (`demos/<date>_<map>` by default), over the one
    /// being recorded.
    pub fn start_recording(&mut self, renet: &mut RenetServer, name: Option<&str>, auto: bool) {
        self.stop_recording(renet);
        let name = match name {
            Some(name) if !name.contains(['/', '\\']) && !name.contains("..") => name.to_string(),
            Some(_) => return,
            None => format!(
                "{}{}",
                chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_"),
                self.map_name()
            ),
        };
        // an id no netcode client is likely to have
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let id = (1 << 63) | (nanos ^ self.uptime.rotate_left(32));
        let password = self.cvars.string("sv_password").to_string();
        let mod_hash = self
            .game_mod
            .as_ref()
            .map_or(0, |(game_mod, _)| game_mod.hash);
        let recorder = demo::Recorder::start(
            renet,
            id,
            &self.config_dir,
            &name,
            auto,
            &password,
            mod_hash,
        );
        tracing::info!("Recording demo: {}", recorder.name);
        self.recorder = Some(recorder);
    }

    /// `stop`: the demo being recorded ends.
    pub fn stop_recording(&mut self, renet: &mut RenetServer) {
        let Some(recorder) = self.recorder.take() else {
            return;
        };
        let name = recorder.name.clone();
        let id = recorder.id;
        if let Err(error) = recorder.stop(renet) {
            tracing::warn!(%error, name, "demo");
        }
        self.leave(renet, id);
        tracing::info!("Demo stopped ({name})");
    }

    /// The recorder hears this tick's messages.
    fn record(&mut self, renet: &mut RenetServer) {
        let elapsed = std::time::Duration::from_secs_f64(1.0 / f64::from(self.world.goal_ticks()));
        let Some(recorder) = &mut self.recorder else {
            return;
        };
        if let Err(error) = recorder.tick(renet, elapsed) {
            tracing::warn!(%error, name = recorder.name, "demo");
            self.stop_recording(renet);
        }
    }
}

/// [`file_hash`] of `maps/<name>.pms`; 0 without one.
fn map_hash(vfs: &Vfs, name: &str) -> u64 {
    vfs.read(&format!("maps/{name}.pms"))
        .map_or(0, |bytes| file_hash(&bytes))
}

/// `FindLowestTeam`: the playing team with the fewest players.
fn lowest_team(playing: &[usize; 6], mode: GameMode) -> usize {
    let teams = if mode == GameMode::Teammatch { 4 } else { 2 };
    (1..=teams).fold(1, |lowest, team| {
        if playing[lowest] > playing[team] {
            team
        } else {
            lowest
        }
    })
}

/// `maps/<name>.pms` to `<name>`.
pub fn map_name(map: &MapFile) -> String {
    map.filename
        .trim_start_matches("maps/")
        .trim_end_matches(".pms")
        .to_string()
}

/// A world on `map`, with the mode's flags, the kits and the stationary guns.
pub fn new_world(
    vfs: &Vfs,
    data: &Arc<GameData>,
    cvars: &Cvars,
    map: &str,
) -> anyhow::Result<World> {
    let map = MapFile::load(vfs, map)?;
    let config = WorldConfig::from_cvars(cvars, data);
    let mut world = World::new(data.clone(), map, config);
    world.net_bullets = Some(Vec::new());
    world.spawn_mode_things();
    if !world.config.survival_mode {
        world.spawn_kits();
    }
    if cvars.bool("sv_stationaryguns") {
        world.spawn_stationary_guns();
    }
    Ok(world)
}
