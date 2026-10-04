//! The server's guards against cheats and abuse: packet and chat floods and pings out of
//! bounds (`AppOnIdle`, `UpdateFrame`), mass flag captures (`sv_antimassflag`), team killers
//! (`sv_punishtk`, in `TSprite.Die`) and knife throwers (`ServerHandleBulletSnapshot`).

use super::*;

/// `SECOND`
const SECOND: u64 = 60;
/// The bans: `FIVE_MINUTES` for a chat flood, `SIXTY_MINUTES div 4` for a bad ping, a packet
/// flood or team killing (`3600 * 15`), `TWENTY_MINUTES` for an overlong chat line.
const FIVE_MINUTES: i64 = 5 * admin::MINUTE;
const QUARTER_HOUR: i64 = 15 * admin::MINUTE;
const TWENTY_MINUTES: i64 = 20 * admin::MINUTE;
/// More chat lines than this, not worn off yet, is a chat flood.
const CHAT_FLOOD: u32 = 5;
/// Soldat's longest chat line; a longer one is an attack ("DoS Exploit").
const MAX_CHAT: usize = 100;
/// `sv_warnings_knifecheat` turns on the knife throw check at this value only (Soldat's
/// check counts every thrown knife: it was never meant for play).
const KNIFE_CHECK: i64 = 69;
/// The farthest a soldier moves in a tick: `MAX_VELOCITY` each way (15.6), with room for jets,
/// gravity and polygons pushing it out (bots fighting for long never pass 11.2).
const MOVE_STEP: f32 = 20.0;
/// A client's move may make up for this many ticks without a word from it (a lag spike); after
/// a longer silence, more than that is put back.
const STALL_TICKS: u64 = 30;
/// A player's corrections go to the log at most this often.
const LOG_TICKS: u64 = 5 * SECOND;

/// What the server believes of a client's positions (`sv_movecheck`). Soldat takes a client's
/// word for where its soldier is; so does soldank, as far as the soldier can have gone.
#[derive(Default)]
pub(crate) struct Moves {
    /// The last position taken from the client (or where the server put the soldier), and
    /// when (`uptime`).
    last: Option<(Vec2, u64)>,
    /// The client's control last looked at (its tick and position), and whether it was taken.
    seen: Option<(u64, Vec2, bool)>,
    /// The server's resets of the soldier's position the client was told of (respawns,
    /// corrections): its controls say how many it has had ([`ControlState::resets`]).
    resets: u8,
    /// Corrections since the last log line, and when that was.
    corrections: u32,
    logged: Option<u64>,
}

/// What becomes of the position in a client's control.
pub(crate) enum Move {
    Take,
    /// From before the server's last reset, or not a number.
    Ignore,
    /// Farther than the soldier can have gone.
    PutBack,
}

impl Moves {
    /// The server put the soldier at `pos` and told the client (a respawn, a correction).
    pub(crate) fn reset(&mut self, pos: Vec2, now: u64) {
        self.resets = self.resets.wrapping_add(1);
        self.last = Some((pos, now));
        self.seen = None;
    }

    /// A new map: no position to go by.
    pub(crate) fn new_map(&mut self) {
        self.last = None;
        self.seen = None;
    }

    /// Judges a control's position (`check`: against where the soldier can have gone).
    pub(crate) fn judge(&mut self, control: &ControlState, now: u64, check: bool) -> Move {
        let pos = Vec2::from(control.pos);
        let finite = pos.is_finite() && Vec2::from(control.velocity).is_finite();
        if !finite || control.resets != self.resets {
            return Move::Ignore;
        }
        // the same control again: no newer one came
        if let Some((tick, at, taken)) = self.seen
            && tick == control.tick
            && at == pos
        {
            return if taken { Move::Take } else { Move::Ignore };
        }
        let fits = match self.last {
            Some((from, at)) if check => {
                let ticks = now.saturating_sub(at).clamp(1, STALL_TICKS);
                distance(pos, from) <= MOVE_STEP * ticks as f32
            }
            _ => true,
        };
        self.seen = Some((control.tick, pos, fits));
        if fits {
            self.last = Some((pos, now));
            Move::Take
        } else {
            Move::PutBack
        }
    }
}

/// A client's counts against flooding, lag and cheating.
#[derive(Default)]
pub(crate) struct Warnings {
    /// Its messages this second (`MessagesASecNum`), and its controls apart: they come every
    /// tick, where Soldat's sprite snapshots came about every third, so they count a third.
    messages: u32,
    controls: u32,
    /// `FloodWarnings`, `PingWarnings`, `ChatWarnings`, `KnifeWarnings`
    flood: u32,
    ping: u32,
    chat: u32,
    knife: u32,
}

impl ServerGame {
    /// A move the server doesn't believe: the client hears where its soldier is, and its
    /// positions count again once it has.
    pub(crate) fn put_back(&mut self, renet: &mut RenetServer, client_id: ClientId, id: SoldierId) {
        let Some(soldier) = self.world.soldiers.get(id) else {
            return;
        };
        let (pos, velocity) = (soldier.particle.pos, soldier.particle.velocity);
        let name = soldier.name.clone();
        let now = self.uptime;
        let Some(client) = self.clients.get_mut(&client_id) else {
            return;
        };
        self.corrections += 1;
        let moves = &mut client.moves;
        moves.reset(pos, now);
        moves.corrections += 1;
        if moves.logged.is_none_or(|at| now >= at + LOG_TICKS) {
            let times = std::mem::take(&mut moves.corrections);
            tracing::info!("{name} moved too far: put back ({times} times)");
            moves.logged = Some(now);
        }
        let message = ServerMessage::ForcePosition {
            pos: pos.into(),
            velocity: velocity.into(),
        };
        Self::send(renet, client_id, channel::RELIABLE, &message);
    }

    /// The server put player `num`'s soldier at `pos` and told it so (a respawn): its client's
    /// positions count again once it has heard.
    pub(crate) fn position_reset(&mut self, num: PlayerNum, pos: Vec2) {
        let now = self.uptime;
        if let Some(client) = self
            .clients
            .values_mut()
            .find(|c| c.num == Some(num) && c.ready)
        {
            client.moves.reset(pos, now);
        }
    }

    /// The total corrections of moves so far, for tests and admins.
    pub fn corrections(&self) -> u64 {
        self.corrections
    }

    /// A joined client's message, as the guards count it.
    pub(crate) fn watch_message(&mut self, client_id: ClientId, message: &ClientMessage) {
        let Some(client) = self.clients.get_mut(&client_id) else {
            return;
        };
        let warnings = &mut client.warnings;
        match message {
            ClientMessage::Control(control) => {
                warnings.controls += 1;
                // the knife warnings go with the throw key up
                if !control.input().buttons.contains(Buttons::DROP) {
                    warnings.knife = 0;
                }
            }
            ClientMessage::Bullet(_) => warnings.messages += 1,
            ClientMessage::Chat { .. } => warnings.messages += 1,
            _ => {}
        }
    }

    /// A chat line: one more chat warning, or a 20 minute ban if it's longer than any
    /// client sends. `false` if the player is out.
    pub(crate) fn watch_chat(
        &mut self,
        renet: &mut RenetServer,
        client_id: ClientId,
        num: PlayerNum,
        text: &str,
    ) -> bool {
        if text.chars().count() > MAX_CHAT {
            let ban = ("DoS Exploit".to_string(), TWENTY_MINUTES);
            return !self.kick_player(renet, num, LeaveReason::Flooding, Some(ban));
        }
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.warnings.chat += 1;
        }
        true
    }

    /// Before the world's tick: warnings wear off (a ping and a flood warning every 5
    /// minutes, the knife warnings every 1000 ticks).
    pub(crate) fn guard_timers(&mut self) {
        let five_minutes = self.uptime.is_multiple_of(5 * admin::MINUTE as u64);
        let knives = self.uptime.is_multiple_of(1000);
        for client in self.clients.values_mut() {
            let warnings = &mut client.warnings;
            if five_minutes {
                warnings.ping = warnings.ping.saturating_sub(1);
                warnings.flood = warnings.flood.saturating_sub(1);
            }
            if knives {
                warnings.knife = 0;
            }
        }
    }

    /// After the world's tick, once a second: what came of the game's second (not while
    /// bullet time stops its clocks), and the network's.
    pub(crate) fn guard(&mut self, renet: &mut RenetServer) {
        if !self.uptime.is_multiple_of(SECOND) {
            return;
        }
        if self.world.clocks_ran {
            self.anti_mass_flag(renet);
            self.anti_chat_flood(renet);
        }
        if !self.world.game.ended() && self.uptime.is_multiple_of(6 * SECOND) {
            self.ping_warnings(renet);
        }
        self.packet_flood(renet);
    }

    /// `sv_antimassflag`: a flag grabbed in its base and brought home within the same second
    /// starts the server's vote to kick the player (banned a day if it passes, `CheatTag`).
    fn anti_mass_flag(&mut self, renet: &mut RenetServer) {
        if !self.cvars.bool("sv_antimassflag") {
            return;
        }
        let mut players: Vec<(PlayerNum, SoldierId)> =
            self.players.iter().map(|(n, s)| (*n, *s)).collect();
        players.sort_unstable_by_key(|(num, _)| *num);
        for (num, id) in players {
            let Some(soldier) = self.world.soldiers.get_mut(id).filter(|s| s.active) else {
                continue;
            };
            let suspect = soldier.grabs_per_second > 0
                && soldier.scores_per_second > 0
                && soldier.grabbed_in_base;
            soldier.grabs_per_second = 0;
            soldier.scores_per_second = 0;
            soldier.grabbed_in_base = false;
            if suspect {
                let name = soldier.name.clone();
                self.cheat_tags.insert(num);
                let reason = "Server: Possible cheating".to_string();
                self.start_vote(renet, VoteKind::Kick(num), 0, reason);
                tracing::info!("** Detected possible Mass-Flag cheating from {name}");
            }
        }
    }

    /// More chat lines than wore off (one a second) is a chat flood: banned 5 minutes.
    fn anti_chat_flood(&mut self, renet: &mut RenetServer) {
        let flooding: Vec<PlayerNum> = self
            .clients
            .values()
            .filter(|c| c.warnings.chat > CHAT_FLOOD)
            .filter_map(|c| c.num)
            .collect();
        for num in flooding {
            let ban = ("Chat Flood".to_string(), FIVE_MINUTES);
            self.kick_player(renet, num, LeaveReason::Flooding, Some(ban));
        }
        for client in self.clients.values_mut() {
            client.warnings.chat = client.warnings.chat.saturating_sub(1);
        }
    }

    /// Every 6 seconds: a player with a ping over `sv_maxping` (or measured under
    /// `sv_minping`) gets a warning, and past `sv_warnings_ping` of them a ping kick (banned
    /// 15 minutes).
    fn ping_warnings(&mut self, renet: &mut RenetServer) {
        let (min, max) = (self.cvars.int("sv_minping"), self.cvars.int("sv_maxping"));
        let most = self.cvars.int("sv_warnings_ping");
        let mut kicked = Vec::new();
        for client in self.clients.values_mut() {
            let soldier = client
                .num
                .and_then(|num| self.players.get(&num))
                .and_then(|id| self.world.soldiers.get(*id));
            let Some(soldier) = soldier.filter(|s| s.active) else {
                continue;
            };
            let ping = i64::from(soldier.ping);
            if ping > max || (ping < min && ping > 0) {
                tracing::info!("{} gets a ping warning", soldier.name);
                client.warnings.ping += 1;
                if i64::from(client.warnings.ping) > most {
                    kicked.extend(client.num);
                }
            }
        }
        for num in kicked {
            let ban = ("Ping Kick".to_string(), QUARTER_HOUR);
            self.kick_player(renet, num, LeaveReason::Ping, Some(ban));
        }
    }

    /// More messages in a second than `net_floodingpackets{lan,internet}` (by `net_lan`): a
    /// flood warning, and past `sv_warnings_flood` of them a flood kick (banned 15 minutes).
    fn packet_flood(&mut self, renet: &mut RenetServer) {
        let limit = if self.cvars.int("net_lan") == 1 {
            self.cvars.int("net_floodingpacketslan")
        } else {
            self.cvars.int("net_floodingpacketsinternet")
        };
        let most = self.cvars.int("sv_warnings_flood");
        let mut kicked = Vec::new();
        for client in self.clients.values_mut() {
            let warnings = &mut client.warnings;
            let count = warnings.messages + warnings.controls / 3;
            warnings.messages = 0;
            warnings.controls = 0;
            let soldier = client
                .num
                .and_then(|num| self.players.get(&num))
                .and_then(|id| self.world.soldiers.get(*id));
            let Some(soldier) = soldier.filter(|s| s.active) else {
                continue;
            };
            if i64::from(count) > limit {
                tracing::info!("{} is flooding the server", soldier.name);
                warnings.flood += 1;
                if i64::from(warnings.flood) > most {
                    kicked.extend(client.num);
                }
            }
        }
        for num in kicked {
            let ban = ("Flood Kicked".to_string(), QUARTER_HOUR);
            self.kick_player(renet, num, LeaveReason::Flooding, Some(ban));
        }
    }

    /// `sv_punishtk`, in a team game: a team kill is a warning to the killer; past half of
    /// `sv_warnings_tk` the killer dies for it, and at `sv_warnings_tk` it's banned 15
    /// minutes. The warnings stay with its address until the map changes.
    pub(crate) fn punish_team_kill(
        &mut self,
        renet: &mut RenetServer,
        victim: SoldierId,
        killer: SoldierId,
    ) {
        if !self.cvars.bool("sv_punishtk") {
            return;
        }
        let mode = self.world.config.game_mode;
        let (Some(v), Some(k)) = (
            self.world.soldiers.get(victim),
            self.world.soldiers.get(killer),
        ) else {
            return;
        };
        let team_kill = v.team == k.team
            && !matches!(mode, GameMode::Deathmatch | GameMode::Rambo)
            && v.name != k.name;
        let Some(num) = self.num_of(killer).filter(|_| team_kill) else {
            return;
        };
        let (victim_name, killer_name) = (v.name.clone(), k.name.clone());
        let most = self.cvars.int("sv_warnings_tk");
        let warnings = self.tk_warnings.entry(num).or_default();
        *warnings += 1;
        let warnings = i64::from(*warnings);
        tracing::info!("{killer_name} Team Killed {victim_name} (Warning #{warnings})");
        if let Some(client_id) = self.client_of(num) {
            let text = format!("TK Warning #{warnings}. Max Warnings: {most}");
            Self::send(
                renet,
                client_id,
                channel::RELIABLE,
                &ServerMessage::ServerText(text),
            );
        }
        if warnings > most / 2 {
            // the killer's own 200 damage, no vest (a dead one's body takes it)
            if self
                .world
                .soldiers
                .get(killer)
                .is_some_and(|k| !k.dead_meat)
            {
                let mut events = Vec::new();
                self.world.suicide(killer, 200.0, &mut events);
                self.send_events(renet, &events);
            }
            let text =
                format!("{killer_name} has been punished for TeamKilling. ({warnings}/{most})");
            self.broadcast(renet, channel::RELIABLE, &ServerMessage::ServerText(text));
        }
        if warnings > most - 1 {
            let ban = ("Team Killing".to_string(), QUARTER_HOUR);
            self.kick_player(renet, num, LeaveReason::Kicked, Some(ban));
        }
    }

    /// The team kill warnings of a player who leaves stay with its address (`TKList`).
    pub(crate) fn keep_tk_warnings(&mut self, num: PlayerNum, ip: Option<String>) {
        let warnings = self.tk_warnings.remove(&num).unwrap_or(0);
        if let Some(ip) = ip {
            self.tk_list.insert(ip, warnings);
        }
    }

    /// With `sv_warnings_knifecheat` at 69 (and `sv_anticheatkick`): a third knife thrown
    /// with the throw key held all along, no new weapon in between, is a cheat (banned a day,
    /// "Knife Throw Cheat"). `true` if the player is out.
    pub(crate) fn knife_throw_cheat(
        &mut self,
        renet: &mut RenetServer,
        client_id: ClientId,
        num: PlayerNum,
        bullet: &BulletState,
    ) -> bool {
        let thrown = WeaponKind::values()
            .get(usize::from(bullet.weapon))
            .is_some_and(|&kind| {
                self.world.config.weapons.get(kind).bullet_style == BulletStyle::ThrownKnife
            });
        if !thrown || self.cvars.int("sv_warnings_knifecheat") != KNIFE_CHECK {
            return false;
        }
        let Some(client) = self.clients.get_mut(&client_id) else {
            return false;
        };
        client.warnings.knife += 1;
        if client.warnings.knife != 3 {
            return false;
        }
        tracing::info!("** DETECTED KNIFE CHEATING FROM {} **", self.name_of(num));
        let ban = ("Knife Throw Cheat".to_string(), admin::DAY);
        self.kick_player(renet, num, LeaveReason::Cheat, Some(ban))
    }

    /// A new weapon in hand (`ApplyWeaponByNum`): a knife warning less.
    pub(crate) fn new_weapon(&mut self, num: PlayerNum) {
        if let Some(client) = self.clients.values_mut().find(|c| c.num == Some(num)) {
            client.warnings.knife = client.warnings.knife.saturating_sub(1);
        }
    }
}
