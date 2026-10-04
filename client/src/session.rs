//! The player's part in a session: joining a team, the loadout, bots and kicks, map changes,
//! and the local actions every change goes through ([`LocalAction`], `Game::act`).

use super::*;

impl Game {
    /// A new map: the team menu in team modes, otherwise the player joins right away.
    pub(crate) fn start_joining(&mut self) {
        self.player = None;
        // ClientHandlePlayersList: the zoom starts over
        let _ = self.console.cvars.set("r_zoom", "0");
        self.camera.zoom = 0.0;
        self.dev_zoom = 0.0;
        let mode = self.world.config.game_mode;
        // cl_player_team skips the team menu (teams the mode doesn't have are ignored;
        // 5 watches)
        let team = team_from_num(self.console.cvars.int("cl_player_team"));
        let teams = match mode {
            GameMode::Teammatch => 4,
            mode if mode.is_team_game() => 2,
            _ => 0,
        };
        if team == Team::Spectator || (team != Team::None && (team as usize) <= teams) {
            self.join(team);
        } else if mode.is_team_game() {
            self.hud.menus.show_team(mode);
        } else {
            self.join(Team::None);
        }
        // ClientHandlePlayersList: the cursor back in the middle; not playing yet, the camera
        // follows someone (`GetCameraTarget`), or looks at the map's middle
        if !self.hud.menus.esc_active {
            let middle = vec2(self.camera.game_width, self.camera.game_height) / 2.0;
            self.camera.mouse = middle;
            self.camera.mouse_prev = middle;
        }
        if self.player.is_none() {
            self.follow = self
                .world
                .soldiers
                .iter()
                .find(|(_, s)| s.active && !s.dead_meat && !s.is_spectator())
                .map(|(id, _)| id);
            if self.follow.is_none() {
                self.camera.pos = Vec2::ZERO;
                self.camera.pos_prev = Vec2::ZERO;
            }
        }
    }

    /// The weapons the player respawns with: the picked primary (or none) and the
    /// `cl_player_secwep` secondary.
    pub(crate) fn loadout(&self) -> [WeaponKind; 3] {
        let secwep = self.console.cvars.int("cl_player_secwep") as usize;
        [
            self.hud.sel_weapon.unwrap_or(WeaponKind::NoWeapon),
            WeaponKind::values()[menus::PRIMARY_WEAPONS + secwep],
            WeaponKind::FragGrenade,
        ]
    }

    /// Joins `team`, or changes to it (`ChangeTeam`): the player (re)spawns there, or
    /// watches as a spectator.
    pub(crate) fn join(&mut self, team: Team) {
        // on a server it moves us
        if let Some(connection) = &mut self.connection {
            connection.send(&soldank_core::net::ClientMessage::JoinTeam(team as u8));
            if team != Team::Spectator && self.hud.sel_weapon.is_none() {
                self.hud.menus.show_limbo(true);
            }
            return;
        }
        let cvars = &self.console.cvars;
        let join = LocalAction::Join {
            team: team as u8,
            name: cvars.string("cl_player_name").to_string(),
            looks: player_net_looks(cvars),
            loadout: self.loadout().map(weapon_index),
        };
        self.act(join);
    }

    /// Single player: the player joins `team` (or changes to it), as `name` with `looks`.
    pub(crate) fn join_local(
        &mut self,
        team: Team,
        name: String,
        looks: &NetLooks,
        loadout: [WeaponKind; 3],
    ) {
        // a demo plays it without the menus
        let watching = self.playback.is_some();
        let (id, new_player) = match self.player {
            Some(id) if self.world.soldiers.contains_key(id) => (id, false),
            _ => {
                let id = self.world.spawn_soldier();
                let soldier = &mut self.world.soldiers[id];
                soldier.local_player = true;
                soldier.name = name;
                // CreateSprite: no headgear, no helmet to lose
                looks.apply(soldier);
                self.player = Some(id);
                (id, true)
            }
        };
        if team == Team::Spectator {
            let spectators = self
                .world
                .soldiers
                .values()
                .filter(|s| s.is_spectator())
                .count();
            let max = self.console.cvars.int("sv_maxspectators") as usize;
            if spectators >= max && !self.world.soldiers[id].is_spectator() {
                self.message("Spectators are full", console_colors::WARNING);
                if new_player {
                    self.world.remove_soldier(id);
                    self.player = None;
                    if !watching {
                        self.hud.menus.show_team(self.world.config.game_mode);
                    }
                }
                return;
            }
            self.world.join_spectators(id);
            if watching {
                return;
            }
            self.hud.menus.show_limbo(false);
            self.was_dead = true;
            self.follow = None;
            self.follow = self.camera_target(false);
            return;
        }

        let soldier = &mut self.world.soldiers[id];
        soldier.team = team;
        soldier.looks.apply_team_shirt(team, &self.world.config);
        soldier.loadout = loadout;
        self.world.respawn_soldier(id);
        // survival: nobody joins a running round alive (150 for new players, 4000 on a
        // team change, like the server)
        if self.world.config.survival_mode {
            let amount = if new_player { 150.0 } else { 4000.0 };
            self.world.kill_soldier(id, amount, &mut Vec::new());
        }
        if watching {
            return;
        }
        self.was_dead = false;
        self.camera.pos = self.world.soldiers[id].particle.pos;
        self.camera.pos_prev = self.camera.pos;

        // NewPlayerWeapon: without a primary the weapons menu opens
        if self.hud.sel_weapon.is_none() {
            self.hud.menus.show_limbo(true);
        }
    }

    /// On a server: the weapons picked (`SelWeapon`, `cl_player_secwep`).
    pub(crate) fn send_loadout(&mut self) {
        let loadout = self.loadout();
        if let Some(connection) = &mut self.connection {
            connection.send(&soldank_core::net::ClientMessage::Loadout {
                primary: soldank_core::net::weapon_index(loadout[0]),
                secondary: soldank_core::net::weapon_index(loadout[1]),
            });
        }
    }

    /// A player is kicked: it leaves the game (bots don't come back for the next map).
    pub(crate) fn kick_soldier(&mut self, id: SoldierId) {
        self.act(LocalAction::Kick(soldank_core::demo::soldier_num(id)));
    }

    pub(crate) fn kick_local(&mut self, id: SoldierId) {
        let Some(name) = self.world.soldiers.get(id).map(|s| s.name.clone()) else {
            return;
        };
        if let Some(i) = self.bots.iter().position(|(_, n, _)| *n == name) {
            self.bots.remove(i);
        }
        self.world.remove_soldier(id);
        self.message(format!("{name} has been kicked"), console_colors::ENTER);
    }

    /// `bots_random_*`: random bots from `configs/bots` join the mode's teams when the game
    /// starts (the menu sets only those; Soldat's server takes them all).
    pub(crate) fn add_random_bots(&mut self) {
        let names: Vec<String> = self
            .vfs
            .list("configs/bots")
            .into_iter()
            .filter_map(|f| f.strip_suffix(".bot").map(str::to_owned))
            .collect();
        if names.is_empty() {
            return;
        }

        let teams: &[(Team, &str)] = match self.world.config.game_mode {
            GameMode::Teammatch => &[
                (Team::Alpha, "bots_random_alpha"),
                (Team::Bravo, "bots_random_bravo"),
                (Team::Charlie, "bots_random_charlie"),
                (Team::Delta, "bots_random_delta"),
            ],
            mode if mode.is_team_game() => &[
                (Team::Alpha, "bots_random_alpha"),
                (Team::Bravo, "bots_random_bravo"),
            ],
            _ => &[(Team::None, "bots_random_noteam")],
        };
        for &(team, cvar) in teams {
            for _ in 0..self.console.cvars.int(cvar) {
                // RandomBot
                let pick = self.world.rng.below(names.len() as i32) as usize;
                let name = match names[pick].as_str() {
                    "boogie man" | "dummy" => "sniper".to_string(),
                    name => name.to_string(),
                };
                self.add_bot(&name, team);
            }
        }
    }

    /// `AddBotPlayer`
    pub(crate) fn add_bot(&mut self, name: &str, team: Team) {
        self.act(LocalAction::AddBot {
            file: name.to_string(),
            team: team as u8,
        });
    }

    pub(crate) fn add_bot_local(&mut self, name: &str, team: Team) {
        match BotProfile::load(&self.vfs, name, &self.world.config.weapons) {
            Ok(profile) => {
                self.world.spawn_bot(&profile, team);
                let text = format!("{} has joined the game.", profile.name);
                self.message(text, console_colors::ENTER);
                self.bots
                    .push((name.to_string(), profile.name.clone(), team));
            }
            Err(error) => {
                let text = format!("Bot file {name} not found: {error}");
                self.message(text, console_colors::WARNING);
            }
        }
    }

    /// The map after the current one in the mod's `configs/mapslist.txt` (`NextMap`).
    pub(crate) fn next_map(&mut self) {
        let list = self
            .vfs
            .read_to_string("configs/mapslist.txt")
            .unwrap_or_default();
        let maps: Vec<&str> = list
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let current = self.map_name();
        let next = maps
            .iter()
            .position(|m| m.eq_ignore_ascii_case(&current))
            .map_or(0, |i| (i + 1) % maps.len().max(1));
        let name = maps.get(next).map_or(current.clone(), |m| m.to_string());
        self.change_map(&name);
    }

    pub(crate) fn change_map(&mut self, name: &str) {
        if self.connection.is_some() {
            self.console.print("the server changes the map");
            return;
        }
        // single-player demos start with a map: an automatic one ends with it
        let auto = self.recorder.as_ref().map(|r| r.auto);
        let wanted = self.console.cvars.bool("demo_autorecord") && self.playback.is_none();
        if auto == Some(true) || (auto.is_none() && wanted) {
            self.stop_recording();
            self.begin_local_recording(None, true, name);
        }
        if !self.act(LocalAction::Map(name.to_string())) {
            return;
        }
        // the bots stay for the next map
        for (name, _, team) in std::mem::take(&mut self.bots) {
            self.add_bot(&name, team);
        }
        self.start_joining();
        if self.player.is_some() {
            let listener = self.listener();
            self.audio.play_here(Sfx::Spawn, listener.pos);
        }
    }

    /// The current map's name.
    pub(crate) fn map_name(&self) -> String {
        // filename is "maps/<name>.pms"
        self.world
            .map
            .filename
            .trim_start_matches("maps/")
            .trim_end_matches(".pms")
            .to_string()
    }

    /// Single player: carries out what the player did to the world (live, or from a demo).
    pub(crate) fn apply_action(&mut self, action: LocalAction) -> bool {
        match action {
            LocalAction::Cvars(cvars) => {
                for (cvar, value) in cvars {
                    let _ = self.console.cvars.set_now(&cvar, &value);
                }
            }
            LocalAction::Map(name) => match MapFile::load(&self.vfs, &name) {
                Ok(map) => {
                    self.load_world(map);
                    self.world.spawn_mode_things();
                    if !self.world.config.survival_mode {
                        self.world.spawn_kits();
                    }
                    if self.console.cvars.bool("sv_stationaryguns") {
                        self.world.spawn_stationary_guns();
                    }
                }
                Err(error) => {
                    self.console
                        .print(format!("cannot load map {name}: {error}"));
                    return false;
                }
            },
            LocalAction::Join {
                team,
                name,
                looks,
                loadout,
            } => {
                let loadout = loadout.map(weapon_kind);
                self.join_local(team_from_num(i64::from(team)), name, &looks, loadout);
            }
            LocalAction::Primary(kind) => {
                let kind = weapon_kind(kind);
                let gun = self.world.config.weapons.get(kind);
                let Some(soldier) = self.player.and_then(|id| self.world.soldiers.get_mut(id))
                else {
                    return false;
                };
                soldier.loadout[0] = kind;
                // picked while alive: the weapon right away
                if !soldier.dead_meat
                    && !soldier
                        .primary_weapon()
                        .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
                {
                    soldier.weapons[soldier.active_weapon] = gun;
                }
            }
            LocalAction::Secondary(kind) => {
                let kind = weapon_kind(kind);
                let gun = self.world.config.weapons.get(kind);
                let Some(soldier) = self.player.and_then(|id| self.world.soldiers.get_mut(id))
                else {
                    return false;
                };
                soldier.loadout[1] = kind;
                soldier.weapons[(soldier.active_weapon + 1) % 2] = gun;
            }
            LocalAction::AddBot { file, team } => {
                self.add_bot_local(&file, team_from_num(i64::from(team)));
            }
            LocalAction::Kick(id) => {
                let id = soldank_core::demo::soldier_of_num(id);
                self.kick_local(id);
            }
            LocalAction::Dummy => {
                let id = self.world.spawn_soldier();
                let pos = randomize_start(&self.world.map, Team::None, &mut self.world.rng);
                let dummy = &mut self.world.soldiers[id];
                dummy.particle.pos = pos;
                dummy.particle.old_pos = pos;
                self.console.print(format!("dummy spawned at {pos}"));
            }
            LocalAction::Command(name) => {
                if let (Some(id), Some(command)) = (self.player, PlayerCommand::from_name(&name)) {
                    self.world
                        .player_command(id, command, &mut self.queued_events);
                }
            }
            LocalAction::CycleWeapon => {
                let Some(soldier) = self.player.and_then(|id| self.world.soldiers.get_mut(id))
                else {
                    return false;
                };
                let index = soldier.primary_weapon().kind.index();
                let index = (index + 1) % (WeaponKind::NoWeapon.index() + 1);
                soldier.weapons[soldier.active_weapon] = self.weapons[index];
            }
            LocalAction::Say { text, team } => {
                if let Some(id) = self.player {
                    let length = self.console.cvars.int("ui_console_length") as usize;
                    self.hud.messages.chat(&self.world, id, &text, team, length);
                }
            }
        }
        true
    }

    /// A new world on `map`: the menus, effects and stats start over.
    pub(crate) fn load_world(&mut self, map: MapFile) {
        let mut config = WorldConfig::from_cvars(&self.console.cvars, &self.world.data);
        config.client = self.connection.is_some();
        // a server's weapons there, ours here
        let remote = self.connection.is_some() || self.playback.is_some();
        let data = match &self.remote.data {
            Some(data) if remote => data.clone(),
            _ => self.base_data.clone(),
        };
        self.world = World::new(data, map, config);
        if self.connection.is_some() {
            self.world.net_bullets = Some(Vec::new());
        }
        self.player = None;
        self.follow = None;
        self.hud.menus.team_active = false;
        self.hud.menus.limbo_active = false;
        self.hud.menus.frags = false;
        self.hud.menus.stats = false;
        self.hud.stats.reset();
        self.audio.stop_all();
        self.sparks.clear();
        self.graphics.load_map(
            &mut self.context,
            &self.vfs,
            &self.world.map,
            forced_background(&self.console.cvars),
        );
        self.mode_messages();
    }

    /// The match's special modes, as a map starts (`ClientHandlePlayersList`).
    pub(crate) fn mode_messages(&mut self) {
        let config = &self.world.config;
        let modes = [
            (config.realistic_mode, "Realistic Mode ON"),
            (config.survival_mode, "Survival Mode ON"),
            (config.advance_mode, "Advance Mode ON"),
        ];
        for (on, text) in modes {
            if on {
                self.message(text, console_colors::MODE);
            }
        }
    }
}
