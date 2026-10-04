//! The keyboard and the mouse: the game's keys and bindings, the chat line, the menus' keys and
//! clicks, the cursor.

use super::*;

impl Game {
    /// Carries out a menu choice (`GameMenuAction`).
    pub(crate) fn menu_action(&mut self, action: MenuAction) {
        tracing::debug!(?action, "menu choice");
        let listener = self.listener();
        self.audio.play_here(Sfx::Menuclick, listener.pos);
        match action {
            MenuAction::Team(team) => self.join(team),
            MenuAction::Primary(kind) => {
                self.hud.sel_weapon = Some(kind);
                self.send_loadout();
                if self.player.is_some() && self.connection.is_none() {
                    self.act(LocalAction::Primary(weapon_index(kind)));
                }
            }
            MenuAction::Secondary(n) => {
                let _ = self.console.cvars.set("cl_player_secwep", &n.to_string());
                self.send_loadout();
                if self.player.is_some() && self.connection.is_none() {
                    let kind = WeaponKind::values()[menus::PRIMARY_WEAPONS + n as usize];
                    self.act(LocalAction::Secondary(weapon_index(kind)));
                }
            }
            // back to the front end's menus (Soldat's leaves the game)
            MenuAction::Quit => self.to_menu = true,
            MenuAction::Settings => self.wants_settings = true,
            MenuAction::ChangeTeam => {
                if self.player.is_some() && !self.world.game.ended() {
                    self.hud.menus.show_team(self.world.config.game_mode);
                }
            }
            MenuAction::KickPrev | MenuAction::KickNext => {
                let ids: Vec<SoldierId> = self.world.soldiers.keys().collect();
                if !ids.is_empty() {
                    let current = self
                        .kick_target()
                        .and_then(|t| ids.iter().position(|&id| id == t));
                    let next = match (current, action) {
                        (Some(i), MenuAction::KickPrev) => (i + ids.len() - 1) % ids.len(),
                        (Some(i), _) => (i + 1) % ids.len(),
                        (None, _) => 0,
                    };
                    self.hud.kick = Some(ids[next]);
                }
            }
            MenuAction::Kick => {
                if let Some(id) = self.kick_target()
                    && Some(id) != self.player
                {
                    if self.connection.is_some() {
                        self.start_kick_vote(id);
                    } else {
                        self.hud.menus.show_esc(false);
                        self.kick_soldier(id);
                    }
                }
            }
            MenuAction::MapPrev => self.hud.map_index = self.hud.map_index.saturating_sub(1),
            MenuAction::MapNext => {
                if self.hud.map_index + 1 < self.map_list().len() {
                    self.hud.map_index += 1;
                }
            }
            MenuAction::MapSelect => {
                if let Some(name) = self.map_list().get(self.hud.map_index).cloned() {
                    self.hud.menus.show_esc(false);
                    if self.connection.is_some() {
                        let map = soldank_core::net::VoteKind::Map(name);
                        self.send_vote(map, "---".into());
                    } else {
                        self.change_map(&name);
                    }
                }
            }
        }
    }

    /// A choice in the radio menu: the second one sends the message to the team.
    pub(crate) fn radio_key(&mut self, digit: u8) {
        let Some(mut state) = self.hud.radio else {
            return;
        };
        if state[0].is_none() {
            state[0] = Some(digit);
            self.hud.radio = Some(state);
            return;
        }
        self.hud.radio = None;
        let first = state[0].unwrap_or(1);
        let subject = RADIO_SUBJECTS[usize::from(first - 1)];
        let place = RADIO_WHERE[usize::from(digit - 1)];
        let text = |key: String| self.hud.radio_texts.get(&key).cloned().unwrap_or_default();
        let message = format!(
            "{} {}",
            text(format!("Menu1{subject}")),
            text(format!("Menu2{subject}{place}"))
        );
        let Some(id) = self.player else { return };
        let length = self.console.cvars.int("ui_console_length") as usize;
        self.hud.messages.radio(&self.world, id, &message, length);

        // PlayRadioSound: teammates hear it (radio modes only)
        let radio_mode = matches!(
            self.world.config.game_mode,
            GameMode::CaptureTheFlag | GameMode::HoldTheFlag | GameMode::Infiltration
        );
        if self.hud.radio_cooldown == 0 && radio_mode {
            self.hud.radio_cooldown = 3;
            let sfx = Sfx::RadioEfcup.offset((first - 1) * 3 + digit - 1);
            let listener = self.listener();
            self.audio.play_here(sfx, listener.pos);
        }
    }

    /// The `weapons` key (`TAction.Weapons`): dead, it toggles the weapons menu; alive,
    /// it closes it once both weapons are there and toggles whether it opens on death.
    pub(crate) fn weapons_key(&mut self) {
        let Some(id) = self.player else { return };
        let soldier = &self.world.soldiers[id];
        if soldier.is_spectator() {
            return;
        }
        if soldier.dead_meat {
            let show = !self.hud.menus.limbo_active;
            self.hud.menus.show_limbo(show);
            self.hud.menus.limbo_lock = !show;
        } else {
            let armed = soldier.primary_weapon().kind != WeaponKind::NoWeapon
                && soldier.secondary_weapon().kind != WeaponKind::NoWeapon;
            if self.hud.menus.limbo_active && !armed {
                return;
            }
            self.hud.menus.show_limbo(false);
            self.hud.menus.limbo_lock = !self.hud.menus.limbo_lock;
        }
        self.console.print(if self.hud.menus.limbo_lock {
            "Weapons menu disabled"
        } else {
            "Weapons menu active"
        });
    }

    /// `StartChat` from the chat binds.
    pub(crate) fn start_chat(&mut self, kind: ChatKind) {
        if self.chat.active() {
            return;
        }
        // team chat needs a team
        let has_team = self
            .player
            .is_some_and(|_| self.world.config.game_mode.is_team_game());
        if kind == ChatKind::Team && !has_team {
            return;
        }
        self.chat.start(kind);
        self.chat.changed_at = self.current_time();
        // the opening key's character is still to come, unless it came first (X11)
        let came = self
            .input
            .batch_char
            .is_some_and(|c| Some(c.to_ascii_lowercase()) == self.input.key_char);
        self.input.skip_char = !came;
    }

    /// A key while typing (`ChatKeyDown`).
    pub(crate) fn chat_key(&mut self, keycode: KeyCode, keymods: KeyMods) {
        let key = match keycode {
            KeyCode::V if keymods.ctrl => {
                if let Some(text) = window::clipboard_get() {
                    self.chat.paste(&text);
                }
                None
            }
            KeyCode::Insert if keymods.shift => {
                if let Some(text) = window::clipboard_get() {
                    self.chat.paste(&text);
                }
                None
            }
            // the chat history shown while typing goes to the clipboard
            KeyCode::C if keymods.ctrl => {
                let text: String = self
                    .hud
                    .messages
                    .history
                    .lines()
                    .map(|(line, _)| format!("{line}\n"))
                    .collect();
                window::clipboard_set(&text);
                self.message("Copied chat contents to clipboard", console_colors::GAME);
                None
            }
            KeyCode::Escape => Some(ChatKey::Escape),
            KeyCode::Backspace => Some(ChatKey::Backspace),
            KeyCode::Delete => Some(ChatKey::Delete),
            KeyCode::Home => Some(ChatKey::Home),
            KeyCode::End => Some(ChatKey::End),
            KeyCode::Left if keymods.ctrl => Some(ChatKey::WordLeft),
            KeyCode::Right if keymods.ctrl => Some(ChatKey::WordRight),
            KeyCode::Left => Some(ChatKey::Left),
            KeyCode::Right => Some(ChatKey::Right),
            KeyCode::Tab => Some(ChatKey::Tab),
            KeyCode::Enter | KeyCode::KpEnter => Some(ChatKey::Enter),
            _ => None,
        };
        self.chat.changed_at = self.current_time();
        let Some(key) = key else { return };

        let players: Vec<(usize, String)> = self
            .world
            .soldiers
            .iter()
            .enumerate()
            .filter(|(_, (id, s))| s.active && Some(*id) != self.player)
            .map(|(i, (_, s))| (i + 1, s.name.clone()))
            .collect();
        let commands = self.console.names();
        let completions = Completions {
            players: &players,
            commands: &commands,
        };
        match self.chat.key(key, &completions) {
            Some(ChatOutcome::Command(command)) if self.for_the_server(&command) => {
                if let Some(connection) = &mut self.connection {
                    connection.send(&soldank_core::net::ClientMessage::Command(command));
                }
            }
            Some(ChatOutcome::Command(command)) => self.run_command(&command),
            Some(ChatOutcome::Say(ChatKind::VoteReason, text)) => self.kick_vote_reason(&text),
            Some(ChatOutcome::Say(kind, text)) => self.say(&text, kind == ChatKind::Team),
            None => {}
        }
    }

    /// The cursor moves by `pixels` on the screen times `cl_sensitivity`, like opensoldat's.
    pub(crate) fn move_cursor(&mut self, pixels: Vec2) {
        let size = vec2(self.camera.game_width, self.camera.game_height);
        let sensitivity = self.console.cvars.float("cl_sensitivity");
        self.camera.mouse = (self.camera.mouse + pixels * sensitivity).clamp(Vec2::ZERO, size);
        self.hud.menus.mouse_move(self.camera.mouse);
    }

    pub(crate) fn key_down(&mut self, keycode: KeyCode, keymods: KeyMods, repeat: bool) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.key(keycode, keymods, true);
            if self.overlay.wants_keyboard() {
                return;
            }
        }
        self.input.skip_char = false;
        self.input.key_char = key_char(keycode);
        if self.chat.active() {
            self.chat_key(keycode, keymods);
            return;
        }
        // the game info screen: Escape quits the game
        if keycode == KeyCode::Escape && self.game_info().is_some() {
            window::request_quit();
            return;
        }
        // Escape is the escape menu (`MenuKeyDown`)
        if keycode == KeyCode::Escape && !keymods.ctrl && !keymods.alt && !keymods.shift {
            if !repeat && self.hud.radio.take().is_none() {
                self.hud.menus.escape();
                if self.hud.menus.esc_active {
                    self.audio.stop_all();
                }
            }
            return;
        }
        // Page Down/Up scroll the frags menu
        let plain = !keymods.ctrl && !keymods.alt && !keymods.shift;
        if plain && self.hud.menus.frags && matches!(keycode, KeyCode::PageDown | KeyCode::PageUp) {
            let hide = self.console.cvars.bool("ui_hidespectators");
            let max = render::scoreboard::frags_scroll_max(&self.world, hide);
            let scroll = &mut self.hud.menus.frags_scroll;
            if keycode == KeyCode::PageUp {
                *scroll = scroll.saturating_sub(1);
            } else if *scroll < max {
                *scroll += 1;
            }
            return;
        }
        if repeat {
            return;
        }
        // F12/F11 vote while a vote shows
        if plain
            && matches!(keycode, KeyCode::F11 | KeyCode::F12)
            && self.vote_key(keycode == KeyCode::F12)
        {
            return;
        }

        let key = key_name(keycode);
        tracing::debug!(key, "key down");
        // Alt/Ctrl bindings ("alt+1") win over the plain key
        let modified = [(keymods.alt, "alt"), (keymods.ctrl, "ctrl")]
            .into_iter()
            .filter(|(held, _)| *held)
            .map(|(_, m)| format!("{m}+{key}"))
            .find(|k| self.console.bindings.get(k).is_some());

        // 1-3 pick from the radio menu
        if let Some(digit @ 1..=3) = key.parse::<u8>().ok().filter(|_| key.len() == 1)
            && self.hud.radio.is_some()
            && !keymods.ctrl
            && !keymods.alt
            && !self.hud.menus.any_active()
        {
            self.radio_key(digit);
            return;
        }

        // number keys pick from an open menu
        if self.hud.menus.any_active()
            && let Some(digit) = key.parse::<usize>().ok().filter(|_| key.len() == 1)
        {
            let digit = if digit == 0 { 10 } else { digit };
            if let Some(action) = self.hud.menus.key(digit, keymods.ctrl) {
                self.menu_action(action);
            }
            return;
        }

        let key = modified.unwrap_or(key);
        if let Some(command) = self.input.handle(&self.console.bindings, &key, true) {
            self.run_command(&command);
        }
    }

    pub(crate) fn key_up(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.key(keycode, _keymods, false);
        }
        // the modifier may be up already: release every binding of the key
        let key = key_name(keycode);
        for k in [format!("alt+{key}"), format!("ctrl+{key}"), key] {
            self.input.handle(&self.console.bindings, &k, false);
        }
    }

    pub(crate) fn char_typed(&mut self, character: char, _keymods: KeyMods, _repeat: bool) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.char(character);
            if self.overlay.wants_keyboard() {
                return;
            }
        }
        if !self.chat.active() {
            self.input.batch_char = Some(character);
            return;
        }
        if std::mem::take(&mut self.input.skip_char) {
            return;
        }
        self.chat.input(&character.to_string());
        self.chat.changed_at = self.current_time();
    }

    pub(crate) fn mouse_down(&mut self, button: MouseButton, _x: f32, _y: f32) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.mouse_button(button, _x, _y, true);
            if self.overlay.wants_pointer() {
                return;
            }
        }
        tracing::debug!(?button, "mouse down");
        // clicking while typing puts the line away for later
        if button == MouseButton::Left {
            self.chat.stash();
        }
        if button == MouseButton::Left && self.hud.menus.any_active() {
            let (action, consumed) = self.hud.menus.click(self.hud.sel_weapon.is_some());
            if let Some(action) = action {
                self.menu_action(action);
            }
            if consumed {
                return;
            }
        }

        let key = mouse_button_name(button);
        if let Some(command) = self.input.handle(&self.console.bindings, &key, true) {
            self.run_command(&command);
        }
    }

    pub(crate) fn mouse_up(&mut self, button: MouseButton, _x: f32, _y: f32) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.mouse_button(button, _x, _y, false);
        }
        let key = mouse_button_name(button);
        self.input.handle(&self.console.bindings, &key, false);
    }

    pub(crate) fn mouse_wheel(&mut self, _dx: f32, _dy: f32) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.mouse_wheel(_dx, _dy);
        }
    }

    pub(crate) fn mouse_motion(&mut self, x: f32, y: f32) {
        #[cfg(feature = "dev")]
        if self.overlay.open {
            self.overlay.mouse_motion(x, y);
            if self.overlay.wants_pointer() {
                self.input.last_mouse = None;
                return;
            }
        }
        // in the browser the cursor follows the pointer's moves (`raw_mouse_motion`): a locked
        // pointer stays where it is
        if cfg!(target_arch = "wasm32") {
            return;
        }
        // the cursor moves by screen pixels times cl_sensitivity, like opensoldat's
        let vp = self.context.viewport();
        let pixel = vec2(x - vp.x, y - vp.y);
        // (the first move only says where the pointer is: the cursor stays, like Soldat's
        // relative mouse)
        if let Some(last) = self.input.last_mouse {
            self.move_cursor(pixel - last);
        }
        self.input.last_mouse = Some(pixel);
    }

    pub(crate) fn raw_mouse(&mut self, dx: f32, dy: f32) {
        // (the browser's are CSS pixels)
        if cfg!(target_arch = "wasm32") {
            self.move_cursor(vec2(dx, dy) * window::dpi_scale());
        }
    }
}
