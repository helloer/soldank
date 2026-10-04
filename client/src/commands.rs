//! Console commands: what the player types or binds, run at the next tick; what's for the
//! server goes there.

use super::*;

impl Game {
    /// The console's output goes to the screen too.
    pub(crate) fn flush_console(&mut self) {
        for line in self.console.take_output() {
            self.message(line, console_colors::DEFAULT);
        }
    }

    /// The player says `text` (`ClientSendStringMessage`).
    pub(crate) fn say(&mut self, text: &str, team: bool) {
        // on a server everyone hears it from the server, us too
        if let Some(connection) = &mut self.connection {
            if !text.is_empty() {
                connection.send(&soldank_core::net::ClientMessage::Chat {
                    text: text.to_string(),
                    team,
                    radio: None,
                });
            }
            return;
        }
        if self.player.is_some() && !text.is_empty() {
            self.act(LocalAction::Say {
                text: text.to_string(),
                team,
            });
        }
    }

    /// On a server, a typed command the server runs (what a Soldat client can't parse goes
    /// there): the admin commands, server cvars, and whatever else the client doesn't know.
    pub(crate) fn for_the_server(&self, line: &str) -> bool {
        const ON_THE_SERVER: &[&str] = &[
            "kick", "map", "addbot", "addbot1", "addbot2", "addbot3", "addbot4", "addbot5",
        ];
        let Some(name) = line.split_whitespace().next().map(str::to_lowercase) else {
            return false;
        };
        if self.connection.is_none() {
            return false;
        }
        if ON_THE_SERVER.contains(&name.as_str()) {
            return true;
        }
        match self.console.cvars.get(&name) {
            Some(cvar) => cvar.flags.contains(soldank_core::config::CvarFlags::SERVER),
            None => !self.console.names().contains(&name),
        }
    }

    pub(crate) fn run_command(&mut self, command: &str) {
        let zoom = self.console.cvars.float("r_zoom");
        let _ = self.console.execute(command);
        self.check_zoom(zoom);
        self.run_deferred_commands();
    }

    /// `r_zoomChange`: only spectators zoom; a refused value goes back to `old`.
    pub(crate) fn check_zoom(&mut self, old: f32) {
        let zoom = self.console.cvars.float("r_zoom");
        if zoom == old {
            return;
        }
        let error = match self.player.and_then(|id| self.world.soldiers.get(id)) {
            None => Some("You need to be in-game to set zoom."),
            Some(me) if !me.is_spectator() && zoom != 0.0 => {
                Some("You need to be in the spectators team")
            }
            _ => None,
        };
        if let Some(error) = error {
            let _ = self.console.cvars.set("r_zoom", &old.to_string());
            self.console.print(error);
        }
    }

    pub(crate) fn run_deferred_commands(&mut self) {
        for command in self.console.take_deferred() {
            match (command.name.as_str(), command.args.as_slice()) {
                ("quit", _) => window::request_quit(),
                ("connect", args) => match join_address(args) {
                    Some((address, password)) => {
                        self.join = Some(Some(app::Session::Join { address, password }));
                    }
                    None => self.console.print("Usage: connect ip port password"),
                },
                ("joinurl", [url]) => match join_url(url) {
                    Some((address, password)) => {
                        self.join = Some(Some(app::Session::Join { address, password }));
                    }
                    None => self
                        .console
                        .print("Usage: joinurl soldat://ip:port/password"),
                },
                ("retry", _) => self.join = Some(None),
                // (`ExitToMenu`)
                ("disconnect" | "shutdown", _) => self.to_menu = true,
                ("mute", [target]) if target == "all" => {
                    let all = !self.hud.messages.mute_all;
                    self.hud.messages.mute_all = all;
                    let text = if all {
                        "Everyone is muted"
                    } else {
                        "Everyone is unmuted"
                    };
                    self.message(text, console_colors::CLIENT);
                }
                (mute @ ("mute" | "unmute"), [target]) => {
                    let muted = mute == "mute";
                    for id in self.command_targets(target) {
                        let soldier = &mut self.world.soldiers[id];
                        soldier.muted = muted;
                        let text = format!("{} is {mute}d", soldier.name);
                        self.message(text, console_colors::CLIENT);
                    }
                }
                ("map", [name]) => self.change_map(name),
                ("record", args) => self.start_recording(args.first().map(String::as_str), false),
                ("stop", _) => self.stop_recording(),
                ("demo_tick" | "demo_tick_r", [ticks]) => match ticks.parse::<i64>() {
                    Ok(ticks) => {
                        let from = if command.name == "demo_tick_r" {
                            self.playback_tick().unwrap_or(0) as i64
                        } else {
                            0
                        };
                        self.seek((from + ticks).max(0) as usize);
                    }
                    Err(_) => self
                        .console
                        .print(format!("Usage: {} \"tick\"", command.name)),
                },
                ("votemap", [name]) => match self.connection {
                    Some(_) => {
                        self.send_vote(soldank_core::net::VoteKind::Map(name.clone()), "---".into())
                    }
                    None => self.change_map(name),
                },
                ("dummy", _) if self.connection.is_some() => {
                    self.console.print("not on a server");
                }
                ("dummy", _) => {
                    self.act(LocalAction::Dummy);
                }
                (addbot, words) if addbot.starts_with("addbot") && !words.is_empty() => {
                    let team = team_from_num(addbot[6..].parse().unwrap_or(0));
                    // bot names can have spaces ("Boogie Man")
                    self.add_bot(&words.join(" "), team);
                }
                ("weapons", _) => self.weapons_key(),
                ("fragslist", _) => self.hud.menus.toggle_frags(),
                ("statsmenu", _) => self.hud.menus.toggle_stats(),
                ("minimap", _) => self.hud.menus.minimap = !self.hud.menus.minimap,
                ("screenshot", _) => {
                    let path = self.screenshot_path("screenshot");
                    self.message(
                        format!("Screenshot saved to {}", path.display()),
                        console_colors::DEBUG,
                    );
                    self.screenshot = Some(path);
                    let listener = self.listener();
                    self.audio.play_here(Sfx::Snapshot, listener.pos);
                    // (the shown action snap is what's kept)
                    if self.action_snap.show {
                        self.action_snap.close_after_shot = true;
                    }
                }
                ("snap", _) => {
                    let snap = &mut self.action_snap;
                    let offered = snap.counter.is_some() && snap.image.is_some();
                    if self.console.cvars.bool("cl_actionsnap") && offered {
                        snap.show = !snap.show;
                        if snap.show {
                            let listener = self.listener();
                            self.audio.play_here(Sfx::Snapshot, listener.pos);
                        } else {
                            snap.counter = None;
                        }
                    } else {
                        snap.counter = None;
                        snap.show = false;
                    }
                }
                ("playername", _) => self.hud.menus.player_names = !self.hud.menus.player_names,
                ("sniperline", _) => {
                    let cvars = &mut self.console.cvars;
                    if cvars.bool("sv_sniperline") {
                        let on = !cvars.bool("ui_sniperline");
                        let _ = cvars.set("ui_sniperline", if on { "1" } else { "0" });
                    } else {
                        self.message(
                            "Sniper Line disabled on this server",
                            console_colors::WARNING,
                        );
                    }
                }
                ("radio", _) => {
                    let playing = self
                        .player
                        .and_then(|id| self.world.soldiers.get(id))
                        .is_some_and(|s| !s.is_spectator());
                    let ok = !self.chat.active() && self.console.cvars.bool("sv_radio") && playing;
                    if ok {
                        self.hud.radio = match self.hud.radio {
                            Some(_) => None,
                            None => Some([None, None]),
                        };
                    }
                }
                ("kick", words) if !words.is_empty() => {
                    let who = words.join(" ");
                    let found = self.world.soldiers.iter().enumerate().find(|(i, (_, s))| {
                        s.name.eq_ignore_ascii_case(&who) || (i + 1).to_string() == who
                    });
                    match found {
                        Some((_, (id, _))) if Some(id) != self.player => self.kick_soldier(id),
                        Some(_) => self.console.print("You can't kick yourself"),
                        None => self.console.print(format!("No player {who}")),
                    }
                }
                (say @ ("say" | "say_team"), words) => {
                    if words.is_empty() {
                        self.message(format!("Usage: {say} \"text\""), console_colors::GAME);
                    } else {
                        self.say(&words.join(" "), say == "say_team");
                    }
                }
                ("switchcam", [n]) => {
                    let me = self.player.and_then(|id| self.world.soldiers.get(id));
                    let target = n.parse::<usize>().ok().and_then(|n| {
                        n.checked_sub(1)
                            .and_then(|i| self.world.soldiers.keys().nth(i))
                    });
                    match (me, target) {
                        (Some(me), _) if !me.is_spectator() => {
                            self.message("You are not a spectator", console_colors::DEBUG);
                        }
                        (_, None) if n != "0" => {
                            self.message("Invalid id value", console_colors::DEBUG);
                        }
                        (_, target) => self.follow = target,
                    }
                }
                ("switchcamflag", [style]) => {
                    let spectating = self
                        .player
                        .and_then(|id| self.world.soldiers.get(id))
                        .is_some_and(|s| s.is_spectator());
                    let style = style.parse::<u8>().ok();
                    let flag = self
                        .world
                        .things
                        .iter()
                        .rfind(|t| t.active && Some(t.kind as u8) == style);
                    if let (true, Some(flag)) = (spectating, flag) {
                        self.follow = None;
                        self.camera.pos = flag.skeleton.pos(1);
                    }
                }
                (volume @ ("volumeup" | "volumedown"), _) => {
                    let old = self.console.cvars.int("snd_volume");
                    let new = if volume == "volumeup" {
                        (old + 10).min(100)
                    } else {
                        (old - 10).max(0)
                    };
                    if new != old {
                        let _ = self.console.cvars.set("snd_volume", &new.to_string());
                        self.message(format!("Volume: {new}%"), console_colors::MUSIC);
                    }
                }
                (sens @ ("mousesensitivityup" | "mousesensitivitydown"), _) => {
                    let step = if sens == "mousesensitivityup" { 5 } else { -5 };
                    let old = (100.0 * self.console.cvars.float("cl_sensitivity")).floor() as i64;
                    let new = (old + step).max(0);
                    let _ = self
                        .console
                        .cvars
                        .set("cl_sensitivity", &(new as f32 / 100.0).to_string());
                    self.message(format!("Sensitivity: {new}%"), console_colors::MUSIC);
                }
                ("gamestats", _) => self.hud.menus.con_info = !self.hud.menus.con_info,
                ("recorddemo", _) => self.record_demo_key(),
                ("debugdraw", parts) => {
                    let mut draw = render::debug::DebugDraw::default();
                    for part in parts {
                        match part.as_str() {
                            "polygons" => draw.polygons = true,
                            "colliders" => draw.colliders = true,
                            "waypoints" => draw.waypoints = true,
                            "spawns" => draw.spawns = true,
                            "skeletons" => draw.skeletons = true,
                            other => self.console.print(format!("no debug drawing \"{other}\"")),
                        }
                    }
                    self.hud.debug_draw = draw;
                }
                ("debug", _) => {
                    #[cfg(feature = "dev")]
                    self.overlay.toggle();
                    #[cfg(not(feature = "dev"))]
                    self.console
                        .print("the dev overlay needs soldank built with --features dev");
                }
                ("chat", _) => {
                    // survival: a spectator talks to the spectators till the round's over
                    let spectating = self
                        .player
                        .and_then(|id| self.world.soldiers.get(id))
                        .is_some_and(|s| s.is_spectator());
                    let cvars = &self.console.cvars;
                    let antispy = cvars.bool("sv_survivalmode")
                        && spectating
                        && !self.world.game.survival_end_round
                        && cvars.bool("sv_survivalmode_antispy");
                    self.start_chat(if antispy {
                        ChatKind::Team
                    } else {
                        ChatKind::Public
                    });
                }
                ("teamchat", _) => self.start_chat(ChatKind::Team),
                ("cmd", _) => self.start_chat(ChatKind::Command),
                (name, []) if PlayerCommand::from_name(name).is_some() => {
                    if let (Some(id), Some(command)) = (self.player, PlayerCommand::from_name(name))
                    {
                        // on a server it's the server's to do (it tells of the taunt)
                        match &mut self.connection {
                            Some(connection) => connection
                                .send(&soldank_core::net::ClientMessage::Command(name.to_string())),
                            None => {
                                let _ = (id, command);
                                self.act(LocalAction::Command(name.to_string()));
                            }
                        }
                    }
                }
                ("changeteam", _) => {
                    if self.world.config.game_mode.is_team_game() {
                        self.hud.menus.show_team(self.world.config.game_mode);
                    }
                }
                ("cycleweapon", _) => {
                    if self.player.is_some() && self.connection.is_none() {
                        self.act(LocalAction::CycleWeapon);
                    }
                }
                (name, _) => self.console.print(format!("usage error in {name}")),
            }
        }

        self.flush_console();
    }
}

/// `connect ip [port] [password]`'s server (port 23073 by default, or in the address) and
/// password.
fn join_address(args: &[String]) -> Option<(String, String)> {
    let ip = args.first()?;
    let address = match args.get(1) {
        Some(port) => format!("{ip}:{port}"),
        None => ip.clone(),
    };
    let password = args.get(2).cloned().unwrap_or_default();
    Some((address, password))
}

/// `joinurl soldat://ip:port/password`'s server and password.
fn join_url(url: &str) -> Option<(String, String)> {
    let rest = url.split("//").nth(1)?;
    let (address, password) = rest.split_once('/').unwrap_or((rest, ""));
    if address.is_empty() {
        return None;
    }
    Some((address.to_string(), password.to_string()))
}

impl Game {
    /// `CommandTarget`: the soldiers `target` names: a player number or an exact name, or a
    /// group (`@all`, `@bots`, `@humans`, `@alive`, `@dead`, `@me`, `@!me`, `@none`,
    /// `@alpha`, `@bravo`, `@charlie`, `@delta`, `@spec`).
    pub(crate) fn command_targets(&self, target: &str) -> Vec<SoldierId> {
        let soldiers: Vec<(usize, SoldierId)> = match &self.connection {
            Some(connection) => connection
                .net
                .players
                .iter()
                .map(|(num, id)| (usize::from(*num), *id))
                .collect(),
            None => self
                .world
                .soldiers
                .keys()
                .enumerate()
                .map(|(i, id)| (i + 1, id))
                .collect(),
        };
        let soldiers: Vec<(usize, SoldierId)> = soldiers
            .into_iter()
            .filter(|(_, id)| self.world.soldiers.get(*id).is_some_and(|s| s.active))
            .collect();
        let by_number = target.parse::<usize>().ok().filter(|&n| n > 0);
        if let Some(&(_, id)) = soldiers
            .iter()
            .find(|(num, id)| Some(*num) == by_number || self.world.soldiers[*id].name == target)
        {
            return vec![id];
        }
        let me = self.player;
        soldiers
            .into_iter()
            .map(|(_, id)| id)
            .filter(|&id| {
                let s = &self.world.soldiers[id];
                match target {
                    "@all" => true,
                    "@bots" => s.is_bot(),
                    "@humans" => !s.is_bot(),
                    "@alive" => !s.dead_meat,
                    "@dead" => s.dead_meat,
                    "@me" => Some(id) == me,
                    "@!me" => Some(id) != me,
                    "@none" => s.team == Team::None,
                    "@alpha" => s.team == Team::Alpha,
                    "@bravo" => s.team == Team::Bravo,
                    "@charlie" => s.team == Team::Charlie,
                    "@delta" => s.team == Team::Delta,
                    "@spec" => s.team == Team::Spectator,
                    _ => false,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod join_tests {
    use super::*;

    #[test]
    fn servers_from_commands_and_urls() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            join_address(&args(&["10.0.0.1"])),
            Some(("10.0.0.1".into(), String::new()))
        );
        assert_eq!(
            join_address(&args(&["10.0.0.1", "23074", "secret"])),
            Some(("10.0.0.1:23074".into(), "secret".into()))
        );
        assert_eq!(join_address(&[]), None);
        assert_eq!(
            join_url("soldat://example.org:23073/pass"),
            Some(("example.org:23073".into(), "pass".into()))
        );
        assert_eq!(
            join_url("soldat://[::1]:23073"),
            Some(("[::1]:23073".into(), String::new()))
        );
        assert_eq!(join_url("nonsense"), None);
    }
}
