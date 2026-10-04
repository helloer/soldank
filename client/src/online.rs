//! Playing on a server: its messages into the world and the HUD, the controls and bullets out,
//! and getting its map and mod first.

use super::*;

/// The game of a server (or of a demo of one): what it plays with, and what's on its way.
#[derive(Default)]
pub(crate) struct Remote {
    /// Its mod, in use.
    pub(crate) game_mod: Option<GameMod>,
    /// Its weapons mods (from `Welcome`), and the game data with them.
    pub(crate) weapons: [Option<String>; 2],
    pub(crate) data: Option<Arc<GameData>>,
    /// Its cvars (from `Welcome`), set again with each of its maps.
    pub(crate) cvars: Vec<(String, String)>,
    /// Its maps, for the map menu.
    pub(crate) maps: Vec<String>,
    /// Getting its map before joining it.
    pub(crate) download: Option<soldank_core::net::MapDownload>,
    /// Getting its mod, and the map to get after it.
    pub(crate) mod_download: Option<(ModDownload, String, u64)>,
    /// `RenderGameInfo`: what's left to show once the server is gone (refused, disconnected).
    pub(crate) game_info: Option<String>,
}

/// `TDownloadThread.SetStatus`: "Downloading ctf_Ash.pms - 45% (120 Kb/266 Kb)".
pub(crate) fn downloading(path: &str, got: usize, of: usize) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let progress = match of {
        0 => 0.0,
        _ => (got as f64 / of as f64 * 100.0).round_ties_even(),
    };
    format!(
        "Downloading {name} - {progress}% ({}/{})",
        file_size(got),
        file_size(of)
    )
}

/// `GetSize`: bytes, Kb, Mb or Gb, rounded down.
pub(crate) fn file_size(bytes: usize) -> String {
    let kb = bytes / 1024;
    match kb {
        0 => format!("{bytes} B"),
        1025.. if kb / 1024 > 1024 => format!("{} Gb", kb / 1024 / 1024),
        1025.. => format!("{} Mb", kb / 1024),
        _ => format!("{kb} Kb"),
    }
}

impl Game {
    /// The server's map (on joining, and when it changes): its world, then the players.
    pub(crate) fn load_server_map(&mut self, name: &str, cvars: &[(String, String)]) {
        for (cvar, value) in cvars {
            let _ = self.console.cvars.set_now(cvar, value);
        }
        match MapFile::load(&self.vfs, name) {
            Ok(map) => self.load_world(map),
            Err(error) => {
                tracing::warn!(%error, name, "map");
                self.leave_server(format!("Could not load map: {name}"));
                return;
            }
        }
        if let Some(connection) = &mut self.connection {
            connection.net.reset();
        }
        if let Some(playback) = &mut self.playback {
            playback.net.reset();
            return;
        }
        // a team game without cl_player_team: the team menu
        let mode = self.world.config.game_mode;
        let team = self.console.cvars.int("cl_player_team");
        if mode.is_team_game() && !(1..=5).contains(&team) {
            self.hud.menus.show_team(mode);
        }
    }

    /// The server's mod (`ClientHandlePlayersList`), with `cl_servermods`: from the config
    /// directory's `mods/` if it's there, else downloaded there first; then the map.
    pub(crate) fn server_mod(&mut self, game_mod: Option<GameMod>, map: String, map_hash: u64) {
        let wanted = game_mod.filter(|_| self.console.cvars.bool("cl_servermods"));
        if wanted != self.remote.game_mod {
            if let Some(game_mod) = wanted.clone().filter(|m| !self.assets.has_mod(m)) {
                self.message(
                    format!("Downloading server mod: {}", game_mod.name),
                    console_colors::GAME,
                );
                let (download, request) = ModDownload::new(game_mod);
                if let Some(connection) = &mut self.connection {
                    connection.send(&request);
                }
                self.remote.mod_download = Some((download, map, map_hash));
                return;
            }
            if !self.use_mod(wanted) {
                return;
            }
        }
        let data = self.base_data.with_weapons_mods(&self.remote.weapons);
        self.remote.data = Some(Arc::new(data));
        self.get_map(&map, map_hash);
    }

    /// The game's files again with `game_mod` (or the player's own mods without one), and
    /// everything loaded from them: data, sprites, fonts, sounds, radio texts.
    pub(crate) fn use_mod(&mut self, game_mod: Option<GameMod>) -> bool {
        let loaded = self.assets.mount(game_mod.as_ref()).and_then(|vfs| {
            let data = GameData::load(&vfs)?;
            Ok((vfs, data))
        });
        let (vfs, data) = match loaded {
            Ok(loaded) => loaded,
            Err(error) => {
                tracing::warn!(%error, "mod");
                let name = game_mod.map_or(String::new(), |m| m.name);
                self.leave_server(format!("Could not load mod archive ({name})."));
                return false;
            }
        };
        self.vfs = vfs;
        self.base_data = Arc::new(data);
        let cvars = &self.console.cvars;
        let ui_style = cvars.string("ui_style").to_string();
        self.graphics
            .load_sprites(&mut self.context, &self.vfs, &ui_style);
        self.graphics.load_fonts(
            &mut self.context,
            &self.vfs,
            cvars.string("font_1_filename"),
        );
        self.audio.stop_all();
        self.audio.load_samples(&self.vfs);
        self.hud.radio_texts = load_radio_texts(&self.vfs);
        if let Some(game_mod) = &game_mod {
            let text = format!("Loading server mod: {}", game_mod.name);
            self.message(text, console_colors::MODE);
        }
        self.remote.game_mod = game_mod;
        true
    }

    /// Gives up on the server, saying why on the game info screen.
    pub(crate) fn leave_server(&mut self, reason: String) {
        self.message(reason.clone(), console_colors::WARNING);
        self.remote.game_info = Some(reason);
        if let Some(connection) = &mut self.connection {
            connection.disconnect();
        }
    }

    /// `RenderGameInfo`'s text instead of the game: connecting, downloading, or why the
    /// server's gone.
    pub(crate) fn game_info(&self) -> Option<String> {
        if let Some(text) = &self.remote.game_info {
            return Some(text.clone());
        }
        let connection = self.connection.as_ref()?;
        if let Some((download, ..)) = &self.remote.mod_download {
            let (got, of) = download.progress();
            return Some(downloading(&download.game_mod.path(), got, of));
        }
        if let Some(download) = &self.remote.download {
            let (got, of) = download.progress();
            return Some(downloading(download.current()?, got, of));
        }
        // before the welcome
        if self.remote.data.is_none() {
            return Some(format!("Connecting to {}", connection.server));
        }
        None
    }

    /// The server's map: loaded right away if it's here, else downloaded first.
    pub(crate) fn get_map(&mut self, map: &str, map_hash: u64) {
        let (download, requests) = soldank_core::net::MapDownload::new(&self.vfs, map, map_hash);
        if self.playback.is_some() && !requests.is_empty() {
            self.message(
                format!("The demo's map {map} is missing"),
                console_colors::WARNING,
            );
            self.playback = None;
            return;
        }
        // a recording goes on onto the next map; an automatic one ends with this
        if self.recorder.as_ref().is_some_and(|r| r.auto) {
            self.stop_recording();
        }
        if !requests.is_empty() {
            self.message(
                format!("Downloading {map} from the server"),
                console_colors::GAME,
            );
        }
        if let Some(connection) = &mut self.connection {
            for request in &requests {
                connection.send(request);
            }
        }
        self.remote.download = Some(download);
        self.check_download();
    }

    /// A piece of a downloaded file; whole files are kept in `downloads/` for next time.
    pub(crate) fn file_chunk(
        &mut self,
        request: &str,
        path: &str,
        size: u32,
        offset: u32,
        data: &[u8],
    ) {
        if let Some((download, ..)) = &mut self.remote.mod_download
            && download.wants(request)
        {
            let whole = download.chunk(path, size, offset, data);
            if let Some(reason) = download.failed.clone() {
                self.remote.mod_download = None;
                self.leave_server(format!("Download error: {reason}"));
            } else if let Some(bytes) = whole {
                let (download, map, map_hash) = self.remote.mod_download.take().unwrap();
                let file = self.assets.mod_path(&download.game_mod);
                let saved = file
                    .parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| std::fs::write(&file, &bytes));
                match saved {
                    Ok(()) => self.server_mod(Some(download.game_mod), map, map_hash),
                    Err(error) => self.leave_server(format!("Download error: {error}")),
                }
            }
            return;
        }
        let Some(download) = &mut self.remote.download else {
            return;
        };
        let (whole, requests) = download.chunk(&mut self.vfs, request, path, size, offset, data);
        if let Some(connection) = &mut self.connection {
            for request in &requests {
                connection.send(request);
            }
        }
        if let Some((path, bytes)) = whole {
            let file = self
                .console
                .config_dir
                .join("downloads")
                .join(soldank_core::assets::normalize(&path));
            let saved = file
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&file, &bytes));
            if let Err(error) = saved {
                tracing::warn!(%error, file = %file.display(), "cannot keep the download");
            }
            self.message(
                format!("Downloaded {path} ({} KB)", bytes.len().div_ceil(1024)),
                console_colors::GAME,
            );
        }
        self.check_download();
    }

    /// A download that's over: the map loads and the client says it's ready, or it gives up.
    pub(crate) fn check_download(&mut self) {
        let Some(download) = &self.remote.download else {
            return;
        };
        if let Some(reason) = download.failed.clone() {
            self.remote.download = None;
            self.leave_server(format!("Download error: {reason}"));
        } else if download.done() {
            let map = self
                .remote
                .download
                .take()
                .map(|d| d.map)
                .unwrap_or_default();
            let cvars = self.remote.cvars.clone();
            self.load_server_map(&map, &cvars);
            if let Some(connection) = &mut self.connection {
                let mod_hash = match &self.remote.game_mod {
                    Some(game_mod) => game_mod.hash,
                    None if self.assets.mods.is_empty() => 0,
                    None => 1,
                };
                connection.send(&soldank_core::net::ClientMessage::Ready { mod_hash });
                connection.send(&soldank_core::net::ClientMessage::MapList);
            }
            if self.connection.is_some()
                && self.recorder.is_none()
                && self.console.cvars.bool("demo_autorecord")
            {
                self.start_recording(None, true);
            }
        }
    }

    /// Network: packets in, the server's messages into the world; the events they make
    /// for the HUD and sounds.
    pub(crate) fn net_receive(&mut self) -> Vec<GameEvent> {
        let Some(connection) = &mut self.connection else {
            return Vec::new();
        };
        let tick_time = 1.0 / f64::from(self.world.goal_ticks());
        let updated = connection.update(std::time::Duration::from_secs_f64(tick_time));
        if updated.is_err() || connection.is_disconnected() {
            if let Err(error) = updated {
                tracing::info!(%error, "network");
            }
            self.connection = None;
            self.message("Disconnected from the server", console_colors::WARNING);
            self.remote
                .game_info
                .get_or_insert_with(|| "Server disconnected".to_string());
            self.stop_recording();
            return Vec::new();
        }
        if connection.is_connected() && !connection.greeted {
            connection.greeted = true;
            let cvars = &self.console.cvars;
            let team = cvars.int("cl_player_team");
            let hello = soldank_core::net::ClientMessage::Hello {
                version: soldank_core::net::PROTOCOL_VERSION,
                password: connection.password.clone(),
                name: cvars.string("cl_player_name").to_string(),
                looks: player_net_looks(cvars),
                team: (1..=5).contains(&team).then_some(team as u8),
            };
            connection.send(&hello);
        }

        let mut events = Vec::new();
        for message in connection.receive() {
            let Some(connection) = &mut self.connection else {
                break;
            };
            // deltas made whole (and recorded so)
            let Some(message) = connection.net.expand(message) else {
                continue;
            };
            if let Some(recorder) = &mut self.recorder {
                recorder.message(&message);
            }
            let Some(connection) = &mut self.connection else {
                break;
            };
            let Some(notice) = connection.net.apply(&mut self.world, message) else {
                continue;
            };
            if let Some(event) = self.notice(notice) {
                events.push(event);
            }
        }
        if let Some(connection) = &self.connection {
            self.player = connection.net.own();
        }
        events
    }

    /// What the game does about a server notice; the game event it amounts to, if any.
    pub(crate) fn notice(&mut self, notice: soldank_core::net::Notice) -> Option<GameEvent> {
        use soldank_core::net::Notice;
        let name = |world: &World, id: SoldierId| {
            world
                .soldiers
                .get(id)
                .map_or(String::new(), |s| s.name.clone())
        };
        match notice {
            Notice::Welcome {
                map,
                map_hash,
                cvars,
                weapons_mods,
                game_mod,
            } => {
                self.remote.cvars = cvars;
                self.remote.weapons = weapons_mods;
                self.server_mod(game_mod, map, map_hash);
            }
            Notice::MapChange { map, map_hash } => match &mut self.remote.mod_download {
                // the mod first, then this map
                Some((_, next, hash)) => (*next, *hash) = (map, map_hash),
                None => self.get_map(&map, map_hash),
            },
            Notice::FileChunk {
                request,
                path,
                size,
                offset,
                data,
            } => self.file_chunk(&request, &path, size, offset, &data),
            Notice::NoFile(request)
                if self
                    .remote
                    .mod_download
                    .as_ref()
                    .is_some_and(|(download, ..)| download.wants(&request)) =>
            {
                let reason = format!("the server can't give out {request}");
                self.remote.mod_download = None;
                self.leave_server(format!("Download error: {reason}"));
            }
            Notice::NoFile(request) => {
                if let Some(download) = &mut self.remote.download {
                    download.no_file(&request);
                    self.message(
                        format!("The server has no {request}"),
                        console_colors::WARNING,
                    );
                }
                self.check_download();
            }
            Notice::Refused(reason) => {
                self.message(reason.to_string(), console_colors::WARNING);
                self.remote.game_info = Some(reason.to_string());
            }
            Notice::Joined { id, .. } => {
                let text = format!("{} has joined the game.", name(&self.world, id));
                self.message(text, console_colors::ENTER);
            }
            Notice::Left { name, why } => {
                let text = match why {
                    soldank_core::net::LeaveReason::Left => format!("{name} has left the game."),
                    soldank_core::net::LeaveReason::VoteKicked => {
                        format!("{name} has been voted to leave the game")
                    }
                    soldank_core::net::LeaveReason::Kicked => {
                        format!("{name} has been kicked from console")
                    }
                    soldank_core::net::LeaveReason::Cheat => {
                        format!("{name} has been kicked for possible cheat")
                    }
                    soldank_core::net::LeaveReason::Ping => {
                        format!("{name} has been ping kicked (for 15 minutes)")
                    }
                    soldank_core::net::LeaveReason::Flooding => {
                        format!("{name} has been flood kicked (for 5 minutes)")
                    }
                };
                self.message(text, console_colors::ENTER);
            }
            Notice::VoteOn {
                kind,
                starter,
                reason,
            } => self.vote_on(kind, starter, reason),
            Notice::VoteOff => self.hud.vote = None,
            Notice::MapList(maps) => self.remote.maps = maps,
            Notice::ServerText(text) => {
                self.message(format!("*SERVER*: {text}"), console_colors::SERVER);
            }
            Notice::TeamChanged { id } => {
                if Some(id) == self.own_soldier() {
                    let spectating = self
                        .world
                        .soldiers
                        .get(id)
                        .is_some_and(|s| s.is_spectator());
                    if spectating {
                        self.follow = self.camera_target(false);
                    }
                }
            }
            Notice::Killed {
                victim,
                killer,
                how,
                weapon,
                headshot,
            } => {
                return Some(GameEvent::Killed {
                    victim,
                    killer,
                    how,
                    weapon,
                    headshot,
                });
            }
            Notice::Respawned { id } => {
                if Some(id) == self.player || self.own_soldier() == Some(id) {
                    self.was_dead = false;
                    self.camera.pos = self.world.soldiers[id].particle.pos;
                    self.camera.pos_prev = self.camera.pos;
                }
            }
            Notice::Chat { who, text, team } => {
                let length = self.console.cvars.int("ui_console_length") as usize;
                self.hud
                    .messages
                    .chat(&self.world, who, &text, team, length);
            }
            Notice::ThingTaken { kind, who, pos } => {
                return Some(GameEvent::ThingTaken { kind, who, pos });
            }
            Notice::FlagCaptured { team, who } => {
                return Some(GameEvent::FlagCaptured { team, who });
            }
            Notice::MatchEnded => return Some(GameEvent::MatchEnded),
        }
        None
    }

    /// Network: this tick's controls out, then the packets.
    pub(crate) fn net_send(&mut self, input: &Input) {
        let Some(connection) = &mut self.connection else {
            return;
        };
        if let Some(message) = connection.net.control(&self.world, input) {
            connection.send(&message);
        }
        // the player's slow shots and grenades, for the server to make and pass on
        for bullet in self.world.net_bullets.iter_mut().flat_map(std::mem::take) {
            connection.send(&soldank_core::net::ClientMessage::Bullet(bullet.state()));
        }
        connection.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_status_like_soldat() {
        assert_eq!(file_size(1000), "1000 B");
        assert_eq!(file_size(266 * 1024 + 500), "266 Kb");
        assert_eq!(file_size(1024 * 1024), "1024 Kb");
        assert_eq!(file_size(20 * 1024 * 1024 + 1), "20 Mb");
        assert_eq!(file_size(1000 << 20), "1000 Mb");
        assert_eq!(file_size(3 << 30), "3 Gb");
        assert_eq!(file_size((1025 << 20) + (1 << 30)), "2 Gb");
        assert_eq!(
            downloading("mods/test.smod", 120 * 1024, 266 * 1024),
            "Downloading test.smod - 45% (120 Kb/266 Kb)"
        );
        // FPC's Round: halves to even
        assert_eq!(downloading("x", 1, 8), "Downloading x - 12% (1 B/8 B)");
        assert_eq!(downloading("x", 0, 0), "Downloading x - 0% (0 B/0 B)");
    }
}
