//! A session's game: made from its parts and taken apart again, a tick's events for the HUD and
//! sounds, the clock, and the window's events.

use super::*;

impl Game {
    pub(crate) fn new(parts: SessionParts) -> Game {
        let SessionParts {
            vfs,
            console,
            data,
            map,
            connection,
            playback,
            assets,
            mut context,
        } = parts;
        let remote = connection.is_some() || playback.is_some();
        let mut config = WorldConfig::from_cvars(&console.cvars, &data);
        config.client = remote;
        let base_data = data.clone();
        let mut world = World::new(data, map, config);
        // on a server (or in a demo of one) the things come from it
        if !remote {
            world.spawn_mode_things();
            if !world.config.survival_mode {
                world.spawn_kits();
            }
            if console.cvars.bool("sv_stationaryguns") {
                world.spawn_stationary_guns();
            }
        }
        let start = world
            .map
            .spawnpoints
            .first()
            .map_or(Vec2::ZERO, |s| vec2(s.x as f32, s.y as f32));
        let camera = Camera::new(start, game_width(window::screen_size()), 480.0);
        let menus = Menus::new(&world.config.weapons, camera.game_width);

        context.set_aspect_range(MIN_FOV, MAX_FOV);
        window::show_mouse(false);
        // (a browser grabs the pointer only after a click; the game needs no grab there)
        #[cfg(not(target_arch = "wasm32"))]
        window::set_cursor_grab(true);

        let mut graphics = GameGraphics::new();
        graphics.load_sprites(&mut context, &vfs, console.cvars.string("ui_style"));
        graphics.load_map(
            &mut context,
            &vfs,
            &world.map,
            forced_background(&console.cvars),
        );
        graphics.load_fonts(&mut context, &vfs, console.cvars.string("font_1_filename"));

        let weapons: Vec<Weapon> = WeaponKind::values()
            .iter()
            .map(|k| Weapon::new(*k, false))
            .collect();

        let audio = audio::Audio::new(&vfs);
        let radio_texts = load_radio_texts(&vfs);
        #[cfg(feature = "dev")]
        let overlay = overlay::Overlay::new(&mut context);
        let mut game = Game {
            vfs,
            console,
            connection,
            remote: Default::default(),
            assets,
            to_menu: false,
            join: None,
            action_snap: Default::default(),
            wants_settings: false,
            base_data,
            queued_events: Vec::new(),
            recorder: None,
            playback,
            #[cfg(feature = "dev")]
            overlay,
            context,
            graphics,
            world,
            player: None,
            was_dead: false,
            hud: view::Hud::new(menus, radio_texts),
            audio,
            sparks: Default::default(),
            screenshot: None,
            end_screenshot: false,
            camera,
            follow: None,
            free_cam_pressed: false,
            dev_zoom: 0.0,
            chat: Chat::default(),
            input: InputState::default(),
            weapons,
            clock: clock::Clock::new(),
            bots: Vec::new(),
        };

        // on a server the player joins when it says hello
        if !remote {
            game.add_random_bots();
            game.start_joining();
            game.mode_messages();
        }

        // game commands of the config files (addbot, minimap, ...)
        game.run_deferred_commands();
        game.flush_console();
        // single player with demo_autorecord: the map starts over, recorded
        if !remote && game.console.cvars.bool("demo_autorecord") {
            let map = game.map_name();
            game.change_map(&map);
        }
        game
    }

    /// A line in the console (and the log).
    pub(crate) fn message(&mut self, text: impl Into<String>, color: u32) {
        let text = text.into();
        tracing::info!(target: "console", "{text}");
        let length = self.console.cvars.int("ui_console_length") as usize;
        self.hud.messages.console(text, color, length);
    }

    /// Where sounds are heard from: the followed soldier, or the camera.
    /// `screens/<date>_<map>_<kind>.png` in the config directory (Soldat's user directory).
    pub(crate) fn screenshot_path(&self, kind: &str) -> PathBuf {
        let date = platform::local_time("%Y-%m-%d_%H-%M-%S_");
        let map = std::path::Path::new(&self.world.map.filename)
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        self.console
            .config_dir
            .join("screens")
            .join(format!("{date}{map}_{kind}.png"))
    }

    pub(crate) fn listener(&self) -> audio::Listener {
        let followed = self.follow.and_then(|id| self.world.soldiers.get(id));
        let me = self.player.and_then(|id| self.world.soldiers.get(id));
        audio::Listener {
            pos: followed.map_or(self.camera.pos, |s| s.particle.pos),
            player: self.player,
            team: me.map_or(Team::None, |s| s.team),
            follow: self.follow,
        }
    }

    /// The client's side of the tick's events: sounds, messages.
    pub(crate) fn handle_events(&mut self, events: &[GameEvent], scores_before: &[i32; 6]) {
        let cvars = &self.console.cvars;
        self.audio.volume = audio::scale_volume(cvars.int("snd_volume"));
        self.audio.battle_effects = cvars.bool("snd_effects_battle");
        self.audio.explosion_effects = cvars.bool("snd_effects_explosions");
        let console_length = cvars.int("ui_console_length") as usize;
        self.hud.messages.kill_length = cvars.int("ui_killconsole_length") as usize;
        let listener = self.listener();

        if self.world.clocks_ran {
            self.hud.messages.tick();
        }
        // the action snap's offer runs out
        if self.world.tick.is_multiple_of(60)
            && let Some(left) = self.action_snap.counter
        {
            self.action_snap.counter = left.checked_sub(1);
        }
        // a kill of the player's, or by another's bullet, on screen: caught in a moment
        for event in events {
            if let GameEvent::Killed {
                victim,
                killer,
                weapon: Some(_),
                ..
            } = *event
                && (Some(killer) == self.player || Some(victim) == self.player)
                && victim != killer
                && let Some(chest) = self.world.soldiers.get(victim).map(|s| s.skeleton.pos(9))
                && self.camera.on_screen(chest)
            {
                self.action_snap.counter = Some(5);
                self.action_snap.capture_in = Some(4);
            }
        }
        // RadioCooldown
        if self.world.tick.is_multiple_of(60)
            && self.hud.radio_cooldown > 0
            && cvars.bool("sv_radio")
        {
            self.hud.radio_cooldown -= 1;
        }
        let mut new_sparks = Vec::new();
        let mut ctx = events::EventCtx {
            world: &self.world,
            audio: &mut self.audio,
            messages: &mut self.hud.messages,
            listener: &listener,
            console_length,
            sparks: &mut new_sparks,
        };
        for event in events {
            match *event {
                GameEvent::Killed {
                    victim,
                    killer,
                    weapon,
                    shot,
                    ..
                } => {
                    ctx.messages
                        .killed(ctx.world, self.player, victim, killer, weapon, shot);
                    if Some(victim) == self.player {
                        ctx.audio.play_here(Sfx::Playerdeath, listener.pos);
                    }
                    ctx.survivors(self.player, victim);
                }
                GameEvent::Sound { soldier, ref sound } => {
                    ctx.audio.event(soldier, sound, &listener);
                }
                GameEvent::BulletHit {
                    kind: HitKind::Explode | HitKind::FragGrenade | HitKind::Cluster | HitKind::Flak,
                    pos,
                    ..
                } => ctx.audio.explosion(pos, ctx.world, &listener),
                GameEvent::ThingTaken { kind, who, pos, .. } => ctx.thing_taken(kind, who, pos),
                GameEvent::FlagCaptured { team, who } => ctx.flag_captured(team, who),
                GameEvent::Chat { who, ref text } => {
                    tracing::debug!(?who, text, "chat");
                    ctx.messages
                        .chat(ctx.world, who, text, false, ctx.console_length);
                }
                GameEvent::MatchEnded => ctx.audio.stop_all(),
                _ => {}
            }
        }
        ctx.team_points(scores_before);
        ctx.time_left();
        self.audio.tick(&self.world, &listener);
        self.fire_shake(events);

        // sparks: the new ones, then all of them move (`TSpark.Update`)
        let mut spark_ctx = sparks::SparkCtx {
            world: &self.world,
            audio: &mut self.audio,
            listener: &listener,
            follow: self.follow,
            max_sparks: self.console.cvars.int("r_maxsparks") as usize,
            game_width: self.camera.game_width,
        };
        for event in events {
            if let GameEvent::Spark(spawn) = event {
                self.sparks.create(spawn, &spark_ctx);
            }
        }
        for spawn in &new_sparks {
            self.sparks.create(spawn, &spark_ctx);
        }
        if self.console.cvars.bool("r_weathereffects") {
            let half = vec2(self.camera.game_width, self.camera.game_height) / 2.0;
            let tick = self.world.tick;
            self.sparks.weather(self.camera.pos, half, tick, &spark_ctx);
        }
        let shake = self.sparks.update(&mut spark_ctx);
        self.camera.pos += shake;
    }

    pub(crate) fn current_time(&self) -> f64 {
        self.clock.now()
    }

    /// The interface style (`ui_style`) changed: its sprites and layout.
    pub fn reload_interface(&mut self) {
        let style = self.console.cvars.string("ui_style").to_string();
        self.graphics
            .load_sprites(&mut self.context, &self.vfs, &style);
    }

    /// The session's over: a recording ends, the server hears we're gone, the GPU is cleared;
    /// what the next session needs comes back.
    pub fn into_parts(mut self) -> (Console, Assets, gfx2d::Gfx2dContext) {
        self.stop_recording();
        if let Some(connection) = &mut self.connection {
            connection.disconnect();
            connection.flush();
        }
        self.audio.stop_all();
        self.graphics.delete(&mut self.context);
        (self.console, self.assets, self.context)
    }

    /// Seconds a tick takes: longer in bullet time (`GOALTICKS`).
    pub(crate) fn tick_time(&self) -> f64 {
        1.0 / f64::from(self.world.goal_ticks())
    }

    /// Time passed since the last look; a demo goes at `demo_speed`.
    pub(crate) fn advance_clock(&mut self) {
        let speed = match self.playback {
            Some(_) => f64::from(self.console.cvars.float("demo_speed")),
            None => 1.0,
        };
        self.clock.advance(speed);
    }
}

impl EventHandler for Game {
    /// The interface stays 480 high and gets as wide as the window's shape allows.
    fn resize_event(&mut self, width: f32, height: f32) {
        if width < 1.0 || height < 1.0 {
            return;
        }
        let game_width = game_width((width, height));
        self.camera.mouse.x *= game_width / self.camera.game_width;
        self.camera.mouse_prev.x *= game_width / self.camera.game_width;
        self.camera.game_width = game_width;
        self.hud.menus.resize(game_width);
        self.input.last_mouse = None;
        // the minimap is drawn for the new pixel size
        self.graphics
            .rebuild_minimap(&mut self.context, &self.world.map);
    }

    fn update(&mut self) {
        self.advance_clock();
        // the keys' events came before this frame
        self.input.batch_char = None;

        while self.clock.take_tick(self.tick_time()) {
            // a demo stands in for the server, and for the player
            let playing = self.playback.is_some();
            if self.playback.as_ref().is_some_and(|p| !p.started) {
                self.begin_playback();
            }
            let (net_events, played) = if playing {
                match self.playback_frame(false) {
                    Some((events, inputs)) => (events, Some(inputs)),
                    None => {
                        self.playback_follow();
                        self.update_camera();
                        self.advance_clock();
                        continue;
                    }
                }
            } else {
                (self.net_receive(), None)
            };
            // (single player: changed server cvars go into the demo first)
            self.record_cvars();
            let rules = WorldConfig::from_cvars(&self.console.cvars, &self.world.data);
            self.world.set_rules(rules);
            self.world.config.now.sparks_count = self.sparks.count as i32;
            let replaying = self.playback.as_ref().is_some_and(|p| p.local());
            self.world.config.client = self.connection.is_some() || (playing && !replaying);

            // the weapons the player may pick (advance mode, an admin's `weaponoff`)
            let weapon_sel = self
                .player
                .and_then(|id| self.world.soldiers.get(id))
                .map_or(soldank_core::ALL_WEAPONS, |s| s.weapon_sel);
            self.hud.menus.allow_weapons(weapon_sel);

            // no controls while typing or picking from a menu
            let busy = self.chat.active() || self.hud.menus.any_active();
            let input = Input {
                buttons: if busy {
                    Buttons::default()
                } else {
                    self.input.buttons
                },
                aim: self.camera.aim(),
            };

            let inputs: Vec<_> =
                played.unwrap_or_else(|| self.player.map(|id| (id, input)).into_iter().collect());
            let scores_before = self.world.game.team_scores;
            let mut events = self.world.step(&inputs);
            self.record_frame(&input);
            if playing {
                self.keep_keyframe();
            }
            events.append(&mut self.queued_events);
            events.extend(net_events);
            // (in bullet time the vote's and the messages' clocks stand still)
            if self.world.clocks_ran {
                self.vote_timer();
            }
            self.net_send(&input);
            self.handle_events(&events, &scores_before);
            self.hud.stats.count(self.player, &events);
            // ShowMapChangeScoreboard
            if events.contains(&GameEvent::MatchEnded) {
                self.hud.menus.show_limbo(false);
                self.hud.menus.frags = true;
                self.hud.menus.stats = false;
                self.end_screenshot = self.console.cvars.bool("cl_endscreenshot");
            }
            // the scoreboard a while into the map change
            let counter = self.world.game.map_change_counter as f32;
            if self.end_screenshot && counter < MAP_CHANGE_TIME as f32 / 3.0 {
                self.end_screenshot = false;
                self.screenshot = Some(self.screenshot_path("endgame"));
            }
            // (on a server it says which map; in a demo, the demo)
            if events.contains(&GameEvent::ChangeMap)
                && self.connection.is_none()
                && self.playback.is_none()
            {
                self.next_map();
                continue;
            }

            if let Some(id) = self.player {
                let soldier = &self.world.soldiers[id];
                if soldier.recoil_kick != 0.0 {
                    let (pos, kick) = (soldier.particle.pos, soldier.recoil_kick);
                    self.camera.apply_recoil(pos, kick);
                }

                // killed: the weapons menu opens, and the camera stays a moment
                if soldier.dead_meat && !self.was_dead {
                    if !self.hud.menus.limbo_lock {
                        self.hud.menus.show_limbo(true);
                    }
                    self.hud.menu_timer = MENU_TIME;
                }
                self.was_dead = soldier.dead_meat;
            }
            if events.contains(&GameEvent::MatchEnded) && !playing {
                self.follow_winner();
            }
            if playing {
                self.playback_follow();
            } else {
                self.update_follow();
            }
            self.update_camera();
            self.advance_clock();
        }
    }

    fn draw(&mut self) {
        if let Some(text) = self.game_info() {
            let width = self.camera.game_width;
            self.graphics
                .render_game_info(&mut self.context, &text, width);
            return;
        }
        let tick_time = self.tick_time();
        let p = f64::clamp(self.clock.acc / tick_time, 0.0, 1.0);
        self.clock.frame();

        // the action snap shown: only it (`ShowScreen`)
        if self.action_snap.show
            && let Some(image) = &self.action_snap.image
        {
            let width = self.camera.game_width;
            let elapsed = self.clock.cur;
            self.graphics
                .render_snap(&mut self.context, image, width, elapsed);
            self.take_screenshot();
            if std::mem::take(&mut self.action_snap.close_after_shot) {
                self.action_snap.show = false;
            }
            return;
        }
        // its moment comes (`CapScreen`)
        let grab = match self.action_snap.capture_in {
            Some(0) => {
                self.action_snap.capture_in = None;
                self.console.cvars.bool("cl_actionsnap")
            }
            Some(n) => {
                self.action_snap.capture_in = Some(n - 1);
                false
            }
            None => false,
        };

        let shown = self.shown();
        let interface = self.hud.interface_state(
            shown,
            &self.chat,
            &self.world,
            self.player,
            &self.console.cvars,
            (self.camera.mouse, self.follow),
        );
        let caught = self.graphics.render_frame(
            &mut self.context,
            &self.world,
            &self.camera,
            self.clock.cur - tick_time * (1.0 - p),
            p as f32,
            &interface,
            &self.sparks,
            grab,
        );
        if let Some(image) = caught {
            let texture = gfx2d::Texture::from_image(
                &mut self.context,
                image,
                gfx2d::FilterMethod::Scale,
                gfx2d::WrapMode::Clamp,
                None,
            );
            if let Some(old) = self.action_snap.image.replace(texture) {
                self.context.delete_texture(old);
            }
        }

        #[cfg(feature = "dev")]
        let overlay_open = self.overlay.open;
        self.take_screenshot();
        #[cfg(feature = "dev")]
        if overlay_open {
            let (world, console) = (&self.world, &mut self.console);
            self.overlay
                .draw(&mut self.context, world, console, &mut self.hud.debug_draw);
            self.flush_console();
        }
    }

    fn key_down_event(&mut self, keycode: KeyCode, keymods: KeyMods, repeat: bool) {
        self.key_down(keycode, keymods, repeat);
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        self.key_up(keycode, _keymods);
    }

    fn char_event(&mut self, character: char, _keymods: KeyMods, _repeat: bool) {
        self.char_typed(character, _keymods, _repeat);
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        self.mouse_down(button, _x, _y);
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        self.mouse_up(button, _x, _y);
    }

    fn mouse_wheel_event(&mut self, _dx: f32, _dy: f32) {
        self.mouse_wheel(_dx, _dy);
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.mouse_motion(x, y);
    }

    fn raw_mouse_motion(&mut self, dx: f32, dy: f32) {
        self.raw_mouse(dx, dy);
    }
}

impl Game {
    /// A screenshot asked for, of what's on the screen now.
    fn take_screenshot(&mut self) {
        if let Some(path) = self.screenshot.take() {
            let image = self.context.read_screen();
            // saved asynchronously, like GfxSaveScreen
            platform::spawn(move || {
                let saved = path
                    .parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .map_err(gfx2d::image::ImageError::IoError)
                    .and_then(|()| image.save(&path));
                if let Err(error) = saved {
                    tracing::warn!(%error, path = %path.display(), "cannot save screenshot");
                }
            });
        }
    }
}
