//! Admins, bans and the admin commands (`ServerCommands.pas`, `BanSystem.pas`). Commands come
//! from the server's console, or from players: anyone for the player commands, admins (IPs in
//! `configs/remote.txt`, or logged in with `adminlog <sv_adminpassword>`) for the rest.

use super::*;
use std::collections::HashSet;
use std::path::PathBuf;

/// A ban that doesn't run out (`PERMANENT`).
pub const PERMANENT: i64 = -1000;
pub const MINUTE: i64 = 60 * 60;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;

/// A banned IP address or mask (`TBanIP`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ban {
    /// An address, or a mask with `*` and `?` (`MatchesMask`).
    pub mask: String,
    /// Ticks left, or [`PERMANENT`].
    pub time: i64,
    pub reason: String,
}

/// The bans and admins, kept in `configs/banned.txt` and `configs/remote.txt` of the config
/// directory like Soldat's.
#[derive(Debug, Default)]
pub struct Lists {
    pub bans: Vec<Ban>,
    /// `RemoteIPs`: admins for good.
    pub remote: Vec<String>,
    /// `AdminIPs`: also those logged in since the server started.
    pub admins: HashSet<String>,
    /// `MuteList`: muted addresses, since the server started.
    pub muted: HashSet<String>,
    /// `LastBan`
    last_ban: Option<String>,
    /// Where they're kept; `None` keeps them in memory only.
    pub dir: Option<PathBuf>,
}

impl Lists {
    /// The lists in `dir/configs` (missing files are empty lists).
    pub fn load(dir: PathBuf) -> Lists {
        let read = |file: &str| std::fs::read_to_string(dir.join("configs").join(file));
        let mut lists = Lists {
            bans: parse_bans(&read("banned.txt").unwrap_or_default()),
            remote: read("remote.txt")
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect(),
            ..Lists::default()
        };
        lists.admins = lists.remote.iter().cloned().collect();
        lists.dir = Some(dir);
        lists
    }

    /// `SaveTxtLists`
    pub fn save(&self) {
        let Some(dir) = &self.dir else { return };
        let configs = dir.join("configs");
        let bans: String = self
            .bans
            .iter()
            .map(|b| format!("{}:{}:{}\r\n", b.mask, b.time, b.reason))
            .collect();
        let remote: String = self.remote.iter().map(|ip| format!("{ip}\r\n")).collect();
        let written = std::fs::create_dir_all(&configs)
            .and_then(|()| std::fs::write(configs.join("banned.txt"), bans))
            .and_then(|()| std::fs::write(configs.join("remote.txt"), remote));
        if let Err(error) = written {
            tracing::warn!(%error, "cannot save configs/banned.txt and remote.txt");
        }
    }

    /// `FindBan`
    pub fn ban_of(&self, ip: &str) -> Option<&Ban> {
        self.bans.iter().find(|b| matches_mask(ip, &b.mask))
    }

    /// `AddBannedIP` (an address already banned stays as it is).
    pub fn ban(&mut self, ip: &str, reason: &str, time: i64) {
        if self.ban_of(ip).is_some() {
            return;
        }
        self.bans.push(Ban {
            mask: ip.to_string(),
            time,
            reason: reason.to_string(),
        });
        self.last_ban = Some(ip.to_string());
    }

    /// `DelBannedIP`
    pub fn unban(&mut self, ip: &str) -> bool {
        let before = self.bans.len();
        self.bans.retain(|b| b.mask != ip);
        self.bans.len() != before
    }

    /// `UpdateIPBanList`, once a minute: the bans that ran out.
    fn minute(&mut self) -> Vec<Ban> {
        for ban in &mut self.bans {
            if ban.time > 0 {
                ban.time -= MINUTE;
                if ban.time < 0 && ban.time != PERMANENT {
                    ban.time = 0;
                }
            }
        }
        let (over, kept) = std::mem::take(&mut self.bans)
            .into_iter()
            .partition(|b| b.time == 0);
        self.bans = kept;
        over
    }

    pub fn is_admin(&self, ip: &str) -> bool {
        self.admins.contains(ip) || self.remote.iter().any(|r| r == ip)
    }
}

/// `LoadBannedList`: `ip:ticks:reason` lines (IPv6 addresses in brackets).
pub fn parse_bans(text: &str) -> Vec<Ban> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            let (mask, rest) = match line.strip_prefix('[') {
                Some(v6) => {
                    let (addr, rest) = v6.split_once(']')?;
                    (format!("[{addr}]"), rest.strip_prefix(':')?)
                }
                None => {
                    let (ip, rest) = line.split_once(':')?;
                    (ip.trim().to_string(), rest)
                }
            };
            if mask.is_empty() {
                return None;
            }
            let (time, reason) = rest.split_once(':').unwrap_or((rest, ""));
            let reason = reason.split(':').next().unwrap_or("");
            Some(Ban {
                mask,
                time: time.trim().parse().ok()?,
                reason: reason.to_string(),
            })
        })
        .collect()
}

/// `MatchesMask`: `*` any run of characters, `?` any one, case aside.
pub fn matches_mask(text: &str, mask: &str) -> bool {
    fn go(t: &[char], m: &[char]) -> bool {
        match m.split_first() {
            None => t.is_empty(),
            Some(('*', rest)) => (0..=t.len()).any(|i| go(&t[i..], rest)),
            Some((&c, rest)) => t
                .split_first()
                .is_some_and(|(&tc, tr)| (c == '?' || tc.eq_ignore_ascii_case(&c)) && go(tr, rest)),
        }
    }
    let (t, m): (Vec<char>, Vec<char>) = (text.chars().collect(), mask.chars().collect());
    go(&t, &m)
}

/// How an address is written in the lists.
pub fn ip_text(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => format!("[{v6}]"),
        },
    }
}

impl ServerGame {
    /// A command line from the server's console (`sender` `None`) or a player (`ParseInput`).
    pub fn command(&mut self, renet: &mut RenetServer, sender: Option<ClientId>, line: &str) {
        let line = line.trim().trim_start_matches('/');
        let words = tokenize(line);
        let args: Vec<&str> = words.iter().map(String::as_str).collect();
        let Some(name) = args.first().map(|n| n.to_lowercase()) else {
            return;
        };
        let arg = args.get(1).copied().unwrap_or("");
        let player = sender.and_then(|id| Some((id, self.clients.get(&id)?.num?)));
        let soldier = player.and_then(|(_, num)| self.players.get(&num).copied());
        if let Some((id, num)) = player {
            let ip = self.clients[&id].ip.map(ip_text).unwrap_or_default();
            tracing::info!("{line}({ip}[{}])", self.name_of(num));
        }
        let admin = match sender {
            None => true,
            Some(id) => self.is_admin_client(id),
        };
        // who did it, in ban reasons
        let by = match player {
            Some((_, num)) => self.name_of(num),
            None => "an admin".to_string(),
        };

        // anyone in the game (CMD_PLAYERONLY)
        if let Some(command) = PlayerCommand::from_name(&name) {
            if let Some(id) = soldier {
                let mut events = Vec::new();
                self.world.player_command(id, command, &mut events);
                self.send_events(renet, &events);
            }
            return;
        }
        match (name.as_str(), player) {
            ("adminlog", Some((id, num))) => return self.adminlog(renet, id, num, arg),
            ("info", Some((id, _))) => return self.info(renet, id),
            ("votemap", Some((id, _))) if !arg.is_empty() => {
                return self.vote(renet, id, VoteKind::Map(arg.to_string()), String::new());
            }
            ("adminlog" | "votemap" | "info", _) => return,
            _ => {}
        }

        // admins (CMD_ADMINONLY): the others are ignored
        if !admin {
            return;
        }
        let me = player.map(|(_, num)| num);
        match name.as_str() {
            "addbot" | "addbot1" | "addbot2" | "addbot3" | "addbot4" | "addbot5"
                if !arg.is_empty() =>
            {
                let team = name[6..].parse().map_or(Team::None, team_from_num);
                if let Err(error) = self.add_bot(renet, arg, team) {
                    self.reply(renet, sender, &format!("cannot add bot {arg}: {error}"));
                }
            }
            "nextmap" => {
                let next = self.listed_next_map();
                self.prepare_map_change(renet, &next);
            }
            "map" if !arg.is_empty() => {
                if self.vfs.exists(&format!("maps/{arg}.pms")) {
                    self.prepare_map_change(renet, arg);
                } else {
                    self.reply(renet, sender, &format!("Map not found ({arg})"));
                }
            }
            "restart" => {
                let current = self.map_name();
                self.prepare_map_change(renet, &current);
            }
            "loadwep" => self.load_weapons(renet, sender, arg),
            "loadcon" => self.load_config(renet, sender, arg),
            "record" => self.start_recording(renet, (!arg.is_empty()).then_some(arg), false),
            "stop" => self.stop_recording(renet),
            "kick" if !arg.is_empty() => {
                for num in self.targets(arg, me) {
                    self.kick_player(renet, num, LeaveReason::Kicked, None);
                }
            }
            "kicklast" => {
                if let Some(num) = self.last_player {
                    self.kick_player(renet, num, LeaveReason::Kicked, None);
                }
            }
            "ban" if !arg.is_empty() => {
                for num in self.targets(arg, me) {
                    let ban = Some((format!("Banned by {by}"), DAY * 30));
                    self.kick_player(renet, num, LeaveReason::Kicked, ban);
                }
            }
            "banip" if !arg.is_empty() => {
                self.lists.ban(arg, &format!("Banned by {by}"), DAY * 30);
                self.lists.save();
                self.reply(renet, sender, &format!("IP number {arg} banned"));
            }
            "unban" if !arg.is_empty() => {
                if self.lists.unban(arg) {
                    self.lists.save();
                    self.reply(renet, sender, &format!("IP number {arg} unbanned"));
                }
            }
            "unbanlast" => {
                if let Some(ip) = self.lists.last_ban.clone()
                    && self.lists.unban(&ip)
                {
                    self.lists.save();
                    self.reply(renet, sender, &format!("IP number {ip} unbanned"));
                }
            }
            "adm" if !arg.is_empty() => {
                for num in self.targets(arg, me) {
                    let ip = self.ip_of(num);
                    if let Some(ip) = ip.filter(|ip| !self.lists.remote.contains(ip)) {
                        self.lists.remote.push(ip.clone());
                        self.lists.save();
                        self.reply(
                            renet,
                            sender,
                            &format!("IP number {ip} added to Remote Admins"),
                        );
                    }
                }
            }
            "admip" if !arg.is_empty() => {
                if !self.lists.remote.iter().any(|ip| ip == arg) {
                    self.lists.remote.push(arg.to_string());
                    self.lists.save();
                    self.reply(
                        renet,
                        sender,
                        &format!("IP number {arg} added to Remote Admins"),
                    );
                }
            }
            "unadm" if !arg.is_empty() => {
                if let Some(i) = self.lists.remote.iter().position(|ip| ip == arg) {
                    self.lists.remote.remove(i);
                    self.lists.admins.remove(arg);
                    self.lists.save();
                    self.reply(
                        renet,
                        sender,
                        &format!("IP number {arg} removed from Remote Admins"),
                    );
                }
            }
            "setteam1" | "setteam2" | "setteam3" | "setteam4" | "setteam5" if !arg.is_empty() => {
                let team = name[7..].parse().unwrap_or(1);
                for num in self.targets(arg, me) {
                    if let Some(&id) = self.players.get(&num) {
                        self.join_team(renet, id, team, true);
                    }
                }
            }
            "say" if args.len() > 1 => {
                let text = args[1..].join(" ");
                tracing::info!("*SERVER*: {text}");
                self.broadcast(renet, channel::RELIABLE, &ServerMessage::ServerText(text));
            }
            "pkill" if !arg.is_empty() => {
                for num in self.targets(arg, me) {
                    let Some(&id) = self.players.get(&num) else {
                        continue;
                    };
                    let mut events = Vec::new();
                    self.world.suicide(id, 3430.0, &mut events);
                    self.send_events(renet, &events);
                    let text = format!("{} killed by admin", self.name_of(num));
                    self.reply(renet, sender, &text);
                }
            }
            "listplayers" => self.list_players(renet, sender),
            // a weapon by its menu number (1-10 primaries, 11-14 secondaries), for the humans
            "weaponon" | "weaponoff" => {
                let Some(bit) = arg.parse::<u32>().ok().filter(|n| (1..=14).contains(n)) else {
                    return;
                };
                let on = name == "weaponon";
                for (num, &id) in &self.players {
                    let Some(soldier) = self.world.soldiers.get_mut(id) else {
                        continue;
                    };
                    if soldier.brain.is_some() || self.clients.values().all(|c| c.num != Some(*num))
                    {
                        continue;
                    }
                    if on {
                        soldier.weapon_sel |= 1 << (bit - 1);
                    } else {
                        soldier.weapon_sel &= !(1 << (bit - 1));
                    }
                }
            }
            "pause" => {
                self.world.game.pause();
                self.broadcast(renet, channel::RELIABLE, &ServerMessage::Paused(true));
            }
            "unpause" => {
                self.world.game.unpause();
                self.broadcast(renet, channel::RELIABLE, &ServerMessage::Paused(false));
            }
            "pm" if args.len() > 2 => {
                // (Soldat sends only the next word: quote longer messages)
                let text = args[2..].join(" ");
                for num in self.targets(arg, me) {
                    let to = self.name_of(num);
                    self.reply(renet, sender, &format!("Private Message sent to {to}"));
                    let from = me.map_or("Server".to_string(), |n| self.name_of(n));
                    tracing::info!("(PM) To: {to} From: {from} Message: {text}");
                    if let Some(client) = self.client_of(num) {
                        let pm = ServerMessage::ServerText(format!("(PM) {text}"));
                        Self::send(renet, client, channel::RELIABLE, &pm);
                    }
                }
            }
            "gmute" | "ungmute" if !arg.is_empty() => {
                let mute = name == "gmute";
                for num in self.targets(arg, me) {
                    if let Some(ip) = self.ip_of(num) {
                        if mute {
                            self.lists.muted.insert(ip);
                        } else {
                            self.lists.muted.remove(&ip);
                        }
                    }
                    let what = if mute { "muted" } else { "unmuted" };
                    let text = format!("{} has been {what}.", self.name_of(num));
                    self.reply(renet, sender, &text);
                }
            }
            "addmap" if !arg.is_empty() => {
                self.maps.push(arg.to_string());
                self.reply(
                    renet,
                    sender,
                    &format!("{arg} has been added to the map list."),
                );
                self.save_map_list();
            }
            "delmap" if !arg.is_empty() => {
                if let Some(i) = self.maps.iter().position(|m| m.eq_ignore_ascii_case(arg)) {
                    let text = format!("{arg} has been removed from the map list.");
                    self.reply(renet, sender, &text);
                    self.maps.remove(i);
                }
                self.save_map_list();
            }
            "loadlist" => {
                if !arg.is_empty() {
                    let _ = self.cvars.set("sv_maplist", arg);
                }
                self.load_map_list();
            }
            "banlist" => {
                self.reply(
                    renet,
                    sender,
                    &format!("{:<15} | {:<9} | Reason", "IP", "Duration"),
                );
                let lines: Vec<String> = self
                    .lists
                    .bans
                    .iter()
                    .map(|b| {
                        format!(
                            "{:<15} | {:<9} | {}",
                            b.mask,
                            duration_text(b.time),
                            b.reason
                        )
                    })
                    .collect();
                for line in lines {
                    self.reply(renet, sender, &line);
                }
            }
            _ => self.cvar_command(renet, sender, &args),
        }
    }

    /// `adminlog <password>`: the player's address joins the admins.
    fn adminlog(&mut self, renet: &mut RenetServer, id: ClientId, num: PlayerNum, password: &str) {
        let wanted = self.cvars.string("sv_adminpassword").to_string();
        if wanted.is_empty() || password.is_empty() {
            return;
        }
        let name = self.name_of(num);
        if password == wanted {
            if let Some(ip) = self.clients[&id].ip.map(ip_text) {
                self.lists.admins.insert(ip);
            }
            self.reply(renet, Some(id), &format!("{name} added to Game Admins"));
        } else {
            let text = format!("{name} tried to login as Game Admin with bad password");
            self.reply(renet, Some(id), &text);
        }
    }

    /// `info`: the greetings, and what server this is.
    fn info(&mut self, renet: &mut RenetServer, id: ClientId) {
        let mut lines: Vec<String> = ["sv_greeting", "sv_greeting2", "sv_greeting3"]
            .iter()
            .map(|cvar| self.cvars.string(cvar).to_string())
            .filter(|text| !text.is_empty())
            .collect();
        let mode = match self.world.config.game_mode {
            GameMode::Deathmatch => "Deathmatch",
            GameMode::Pointmatch => "Pointmatch",
            GameMode::Teammatch => "Teammatch",
            GameMode::CaptureTheFlag => "Capture the Flag",
            GameMode::Rambo => "Rambomatch",
            GameMode::Infiltration => "Infiltration",
            GameMode::HoldTheFlag => "Hold the Flag",
        };
        lines.extend([
            format!("Server: {}", self.cvars.string("sv_hostname")),
            format!(
                "Address: {}",
                self.address.map_or(String::new(), |a| a.to_string())
            ),
            format!("Version: {}", env!("CARGO_PKG_VERSION")),
            format!("Gamemode: {mode}"),
            format!("Timelimit: {}", self.cvars.int("sv_timelimit") / 3600),
            format!("Nextmap: {}", self.listed_next_map()),
        ]);
        for text in lines {
            Self::send(
                renet,
                id,
                channel::RELIABLE,
                &ServerMessage::ServerText(text),
            );
        }
    }

    /// `SaveMapList`
    fn save_map_list(&self) {
        let Some(dir) = &self.lists.dir else { return };
        let configs = dir.join("configs");
        let path = configs.join(self.cvars.string("sv_maplist"));
        let text: String = self.maps.iter().map(|m| format!("{m}\r\n")).collect();
        let saved = std::fs::create_dir_all(&configs).and_then(|()| std::fs::write(&path, text));
        if let Err(error) = saved {
            tracing::warn!(%error, path = %path.display(), "cannot save the map list");
        }
    }

    pub(crate) fn is_muted(&self, id: ClientId) -> bool {
        let ip = self.clients.get(&id).and_then(|c| c.ip).map(ip_text);
        ip.is_some_and(|ip| self.lists.muted.contains(&ip))
    }

    /// `loadcon [file]`: the server's settings from `configs/<file>` (server.cfg) again, and
    /// the match starts over (Soldat's also drops everyone; here they stay); not in
    /// `sv_lockedmode`.
    fn load_config(&mut self, renet: &mut RenetServer, sender: Option<ClientId>, name: &str) {
        if self.cvars.bool("sv_lockedmode") {
            let text = "Locked Mode is enabled. Settings can't be changed mid-game.";
            self.reply(renet, sender, text);
            return;
        }
        let name = if name.is_empty() { "server.cfg" } else { name };
        if name.contains(['/', '\\']) || name.contains("..") {
            return;
        }
        let path = self.config_dir.join("configs").join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.reply(renet, sender, &format!("Cannot read {}", path.display()));
            return;
        };
        for line in text.lines().map(str::trim) {
            if !line.is_empty() && !line.starts_with("//") {
                self.command(renet, None, line);
            }
        }
        let map = self.map_name();
        if let Err(error) = self.change_map(renet, &map) {
            tracing::warn!(%error, map, "cannot restart the map");
        }
        self.reply(renet, sender, &format!("Config reloaded {name}"));
    }

    /// `loadwep [name]`: the mode's weapons from `configs/<name>.ini` (weapons.ini or
    /// weapons_realistic.ini by `sv_realisticmode`), the defaults without it; everyone's
    /// weapon in hand takes the new stats, and the clients get them (`ServerVars`).
    fn load_weapons(&mut self, renet: &mut RenetServer, sender: Option<ClientId>, name: &str) {
        let realistic = self.cvars.bool("sv_realisticmode");
        let name = match name {
            "" if realistic => "weapons_realistic",
            "" => "weapons",
            name => name,
        };
        if name.contains(['/', '\\']) || name.contains("..") {
            return;
        }
        let path = format!("configs/{name}.ini");
        let text = self
            .vfs
            .read_to_string(&path)
            .ok()
            .filter(|text| WeaponTable::new(realistic, Some(text)).is_ok());
        let reply = match &text {
            Some(_) => format!("Loaded weapons mod \"{name}\""),
            None => "Using default weapons mod".to_string(),
        };
        let mut mods = self.data.weapons_mods.clone();
        mods[usize::from(realistic)] = text;
        self.data = Arc::new(self.data.with_weapons_mods(&mods));
        self.world.data = self.data.clone();
        self.world
            .set_rules(WorldConfig::from_cvars(&self.cvars, &self.data));
        self.world.reapply_weapons();
        let bytes = encode(&ServerMessage::Weapons(mods));
        for (id, client) in &self.clients {
            if client.num.is_some() && client.refused_until.is_none() {
                renet.send_message(*id, channel::RELIABLE, bytes.clone());
            }
        }
        self.reply(renet, sender, &reply);
    }

    /// A server cvar: shown, or set (`ParseInput`'s cvar part).
    fn cvar_command(&mut self, renet: &mut RenetServer, sender: Option<ClientId>, args: &[&str]) {
        let name = args[0];
        let Some(cvar) = self.cvars.get(name) else {
            if sender.is_none() {
                self.reply(renet, sender, &format!("unknown command {name}"));
            }
            return;
        };
        let (shown, description) = (cvar.value.to_string(), cvar.description.clone());
        let text = match args.get(1) {
            None => format!("{name} is \"{shown}\" ({description})"),
            Some(value) => match self.cvars.set(name, value) {
                Ok(()) => {
                    let now = self.cvars.get(name).map(|c| c.value.to_string());
                    let now = now.unwrap_or_default();
                    format!("{name} is now set to: \"{now}\"")
                }
                Err(error) => format!("Unable to set {name}: {error}"),
            },
        };
        self.reply(renet, sender, &text);
    }

    /// `listplayers`
    fn list_players(&mut self, renet: &mut RenetServer, sender: Option<ClientId>) {
        let mut lines = vec![
            "[ TYPE] Player name              (K/D) [Team]".to_string(),
            "---------------------------------------------".to_string(),
        ];
        let mut nums: Vec<PlayerNum> = self.players.keys().copied().collect();
        nums.sort_unstable();
        for num in nums {
            let Some(soldier) = self.world.soldiers.get(self.players[&num]) else {
                continue;
            };
            let kind = if soldier.brain.is_some() {
                " BOT "
            } else if soldier.is_spectator() {
                "SPECT"
            } else {
                "HUMAN"
            };
            lines.push(format!(
                "[{kind:<5}] {:<24} ({}/{}) [{}]",
                soldier.name, soldier.kills, soldier.deaths, soldier.team as u8
            ));
        }
        for line in lines {
            self.reply(renet, sender, &line);
        }
    }

    /// `CommandTarget`: a player number, a name, or `@all`, `@bots`, `@humans`, `@alive`,
    /// `@dead`, `@me`, `@!me`, `@none`, `@alpha`, `@bravo`, `@charlie`, `@delta`, `@spec`.
    fn targets(&self, target: &str, me: Option<PlayerNum>) -> Vec<PlayerNum> {
        let mut nums: Vec<PlayerNum> = self.players.keys().copied().collect();
        nums.sort_unstable();
        let soldier = |num: &PlayerNum| self.world.soldiers.get(self.players[num]);
        if let Some(num) = target
            .parse::<PlayerNum>()
            .ok()
            .filter(|n| nums.contains(n))
        {
            return vec![num];
        }
        if let Some(num) = nums
            .iter()
            .find(|n| soldier(n).is_some_and(|s| s.name == target))
        {
            return vec![*num];
        }
        nums.into_iter()
            .filter(|n| soldier(n).is_some_and(|s| is_target(target, *n, me, s)))
            .collect()
    }

    /// Takes a player out of the game (`KickPlayer`), banning its address for `ban`'s ticks
    /// with its reason. Everyone hears why; its client goes a little later, once that got there.
    /// Not for a cheat without `sv_anticheatkick`, nor an admin's ping, flooding or vote kick;
    /// `true` if the player is out.
    pub(crate) fn kick_player(
        &mut self,
        renet: &mut RenetServer,
        num: PlayerNum,
        why: LeaveReason,
        ban: Option<(String, i64)>,
    ) -> bool {
        if !self.players.contains_key(&num) {
            return false;
        }
        if why == LeaveReason::Cheat && !self.cvars.bool("sv_anticheatkick") {
            return false;
        }
        let spared = [
            LeaveReason::Ping,
            LeaveReason::Flooding,
            LeaveReason::VoteKicked,
        ];
        if spared.contains(&why) && self.is_admin_num(num) {
            tracing::info!("{} is admin and cannot be kicked.", self.name_of(num));
            return false;
        }
        let ip = self.ip_of(num);
        let Some(id) = self.players.remove(&num) else {
            return false;
        };
        self.keep_tk_warnings(num, ip.clone());
        let (bot, team) = self
            .world
            .soldiers
            .get(id)
            .map_or((true, 0), |s| (s.brain.is_some(), s.team as u8));
        if let Some(soldier) = self.world.soldiers.get(id) {
            let name = soldier.name.clone();
            match (&ban, &ip) {
                // the bot balance's (`KICK_LEFTGAME`)
                (None, _) if why == LeaveReason::Left => {
                    tracing::info!("{name} has left the game.");
                }
                (Some((reason, time)), Some(ip)) => {
                    self.lists.ban(ip, reason, *time);
                    self.lists.save();
                    tracing::info!(
                        "{name} has been kicked and banned for {} ({reason})",
                        duration_text(*time)
                    );
                }
                _ => tracing::info!("{name} has been kicked.({})", ip.unwrap_or_default()),
            }
            if soldier.brain.is_some()
                && let Some(i) = self
                    .bots
                    .iter()
                    .position(|(bot, _)| bot.eq_ignore_ascii_case(&name))
            {
                self.bots.remove(i);
            }
        }
        self.world.remove_soldier(id);
        self.broadcast(
            renet,
            channel::RELIABLE,
            &ServerMessage::PlayerLeft { num, why },
        );
        self.vote_target_left(renet, num);
        if !bot {
            self.balance_bots(renet, true, team);
        }
        let until = self.uptime + REFUSED_TICKS;
        if let Some(client) = self.clients.values_mut().find(|c| c.num == Some(num)) {
            client.ready = false;
            client.refused_until = Some(until);
        }
        true
    }

    /// The bans run out, once a minute.
    pub(crate) fn ban_timer(&mut self) {
        if !self.uptime.is_multiple_of(MINUTE as u64) {
            return;
        }
        let over = self.lists.minute();
        for ban in &over {
            tracing::info!("IP number {} ({}) unbanned", ban.mask, ban.reason);
        }
        if !over.is_empty() {
            self.lists.save();
        }
    }

    /// Says `text` to the console, and to the player who gave the command.
    fn reply(&self, renet: &mut RenetServer, sender: Option<ClientId>, text: &str) {
        tracing::info!("{text}");
        if let Some(id) = sender {
            Self::send(
                renet,
                id,
                channel::RELIABLE,
                &ServerMessage::ServerText(text.into()),
            );
        }
    }

    fn ip_of(&self, num: PlayerNum) -> Option<String> {
        let client = self.clients.values().find(|c| c.num == Some(num))?;
        client.ip.map(ip_text)
    }

    fn is_admin_client(&self, id: ClientId) -> bool {
        let ip = self.clients.get(&id).and_then(|c| c.ip).map(ip_text);
        ip.is_some_and(|ip| self.lists.is_admin(&ip))
    }

    pub(crate) fn is_admin_num(&self, num: PlayerNum) -> bool {
        self.ip_of(num).is_some_and(|ip| self.lists.is_admin(&ip))
    }

    /// The address's ban, if any.
    pub(crate) fn banned(&self, ip: Option<IpAddr>) -> Option<String> {
        let ban = self.lists.ban_of(&ip_text(ip?))?;
        Some(ban.reason.clone())
    }
}

/// Whether player `num` is one of the `@` targets (`me` gave the command).
fn is_target(target: &str, num: PlayerNum, me: Option<PlayerNum>, soldier: &Soldier) -> bool {
    match target {
        "@all" => true,
        "@bots" => soldier.brain.is_some(),
        "@humans" => soldier.brain.is_none(),
        "@alive" => !soldier.dead_meat,
        "@dead" => soldier.dead_meat,
        "@me" => Some(num) == me,
        "@!me" => me.is_some() && Some(num) != me,
        "@none" => soldier.team == Team::None,
        "@alpha" => soldier.team == Team::Alpha,
        "@bravo" => soldier.team == Team::Bravo,
        "@charlie" => soldier.team == Team::Charlie,
        "@delta" => soldier.team == Team::Delta,
        "@spec" => soldier.team == Team::Spectator,
        _ => false,
    }
}

/// A command line's words; double quotes keep spaces in one (`DelimitedText`).
pub fn tokenize(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                word.get_or_insert_with(String::new);
            }
            c if c.is_whitespace() && !quoted => words.extend(word.take()),
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    words.extend(word);
    words
}

/// How long a ban is left, like `banlist`'s.
fn duration_text(time: i64) -> String {
    if time == PERMANENT {
        return "PERMANENT".into();
    }
    let minutes = time.max(0) / MINUTE;
    format!(
        "{}d{}h{}m",
        minutes / (24 * 60),
        minutes / 60 % 24,
        minutes % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bans_read_like_soldats() {
        let bans = parse_bans(
            "1.2.3.4:-1000:cheating\r\n10.0.*:216000:Banned by Admin\r\n\r\n[::1]:3600:\r\n",
        );
        assert_eq!(bans.len(), 3);
        assert_eq!(bans[0].time, PERMANENT);
        assert_eq!(bans[1].reason, "Banned by Admin");
        assert_eq!(bans[2].mask, "[::1]");
        let lists = Lists {
            bans,
            ..Lists::default()
        };
        assert!(lists.ban_of("10.0.3.7").is_some());
        assert!(lists.ban_of("10.1.3.7").is_none());
        assert!(matches_mask("192.168.1.10", "192.168.1.?0"));
        assert!(!matches_mask("192.168.1.10", "192.168.1.?"));
    }

    #[test]
    fn quotes_keep_words_together() {
        assert_eq!(
            tokenize(r#"pm 3 "hello there"  x"#),
            ["pm", "3", "hello there", "x"]
        );
        assert_eq!(tokenize(r#"kick "Major Tom""#), ["kick", "Major Tom"]);
        assert_eq!(tokenize("  say  hi "), ["say", "hi"]);
    }

    #[test]
    fn bans_run_out() {
        let mut lists = Lists::default();
        lists.ban("1.2.3.4", "a while", 2 * MINUTE);
        lists.ban("5.6.7.8", "for good", PERMANENT);
        assert!(lists.minute().is_empty());
        let over = lists.minute();
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].mask, "1.2.3.4");
        assert!(lists.ban_of("5.6.7.8").is_some());
    }
}
