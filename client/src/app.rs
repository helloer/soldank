//! The program: the front end's menus (egui, `front`), or a session — a local game, a game on a
//! server, a demo — which ends back in the menus. The in-match settings are the front end's,
//! over the game.

use super::*;
use egui_miniquad::EguiMq;

/// A session the menus (or the command line) start.
#[derive(Debug, Clone, PartialEq)]
pub enum Session {
    /// A local game on this map (the cvars say the rest).
    Local {
        map: String,
    },
    Join {
        address: String,
        password: String,
    },
    Demo {
        path: PathBuf,
    },
}

/// Between sessions: what the next one takes.
struct Idle {
    console: Console,
    assets: Assets,
    context: gfx2d::Gfx2dContext,
}

pub struct App {
    egui: EguiMq,
    front: front::FrontEnd,
    idle: Option<Idle>,
    game: Option<Box<Game>>,
    /// The game's files as the menus see them (maps, demos' maps).
    vfs: Vfs,
    /// A session to start once the loading screen has been drawn.
    pending: Option<Session>,
    loading_drawn: bool,
}

impl Session {
    /// What the loading screen says.
    fn loading_text(&self) -> String {
        match self {
            Session::Local { map } => format!("Loading {map}"),
            Session::Join { address, .. } => format!("Connecting to {address}"),
            Session::Demo { path } => {
                let name = path.file_stem().map(|s| s.to_string_lossy());
                format!("Loading {}", name.as_deref().unwrap_or("the demo"))
            }
        }
    }
}

impl App {
    /// The menus, or `start`'s session right away (the command line asked for one).
    pub fn new(console: Console, assets: Assets, vfs: Vfs, start: Option<Session>) -> App {
        let mut context = gfx2d::Gfx2dContext::new();
        let egui = EguiMq::new(&mut *context.ctx);
        front::theme::apply(egui.egui_ctx(), vfs.read(FONT_FILE).ok());
        let front = front::FrontEnd::new(&vfs, &assets.config_dir, assets.archive.is_some());
        let mut app = App {
            egui,
            front,
            idle: Some(Idle {
                console,
                assets,
                context,
            }),
            game: None,
            vfs,
            pending: None,
            loading_drawn: false,
        };
        match start {
            Some(session) => app.start(session),
            None => free_cursor(),
        }
        app
    }

    /// A session starts (or the menus say why it can't).
    fn start(&mut self, session: Session) {
        self.front.loading = None;
        let Some(mut idle) = self.idle.take() else {
            return;
        };
        // the session draws its own
        self.front.previews.clear(&mut idle.context);
        match build(idle, session) {
            Ok(game) => {
                self.front.error = None;
                self.game = Some(Box::new(game));
            }
            Err(failed) => {
                let (idle, error) = *failed;
                tracing::warn!("{error:#}");
                self.front.error = Some(format!("{error:#}"));
                self.idle = Some(idle);
                free_cursor();
            }
        }
    }

    /// The session's over: back to the menus.
    fn leave(&mut self) {
        let Some(game) = self.game.take() else {
            return;
        };
        let (console, assets, context) = game.into_parts();
        // the menus' files again (a server's mod is gone)
        match assets.mount(None) {
            Ok(vfs) => self.vfs = vfs,
            Err(error) => tracing::warn!("{error:#}"),
        }
        self.front.back_from_session(&self.vfs, &assets.config_dir);
        self.idle = Some(Idle {
            console,
            assets,
            context,
        });
        free_cursor();
    }

    /// What the menus asked for.
    fn act(&mut self) {
        match self.front.take_action() {
            // the loading screen first: making the session takes a moment
            Some(front::Action::Start(session)) => {
                self.front.loading = Some(session.loading_text());
                self.pending = Some(session);
                self.loading_drawn = false;
            }
            Some(front::Action::Quit) => window::request_quit(),
            Some(front::Action::CloseOverlay) => {
                if let Some(game) = &mut self.game {
                    // back to the escape menu the settings came from, the cursor the game's
                    game.hud.menus.show_esc(true);
                    window::show_mouse(false);
                }
            }
            None => {}
        }
    }

    /// A session is being played (no menus over it).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn playing(&self) -> bool {
        self.game.is_some() && !self.front.overlay
    }

    /// The menus or the settings overlay are up: egui gets the input.
    fn menus_up(&self) -> bool {
        self.game.is_none() || self.front.overlay
    }

    /// A key or button for a binding being set (it takes the next one).
    fn capture(&mut self, key: &str) -> bool {
        let Some(command) = self.front.capturing.take() else {
            return false;
        };
        if key != "escape" {
            let console = match (&mut self.game, &mut self.idle) {
                (Some(game), _) => &mut game.console,
                (None, Some(idle)) => &mut idle.console,
                (None, None) => return true,
            };
            settings::bind(console, command, key);
            self.front.changed(1);
        }
        true
    }
}

/// The menus show the system's cursor.
fn free_cursor() {
    window::show_mouse(true);
    window::set_cursor_grab(false);
}

/// The first map of the mod's map list (a placeholder until a server's map comes).
pub(crate) fn first_map(vfs: &Vfs) -> String {
    vfs.read_to_string("configs/mapslist.txt")
        .ok()
        .and_then(|list| {
            list.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "ctf_Ash".to_string())
}

/// A demo from its file.
fn read_playback(path: &Path) -> anyhow::Result<demo::Playback> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open demo {}", path.display()))?;
    let demo = soldank_core::demo::read_demo(std::io::BufReader::new(file))
        .with_context(|| format!("cannot read demo {}", path.display()))?;
    let name = path
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    Ok(demo::Playback::new(demo, name))
}

/// The session's game, or why not (with the parts back).
fn build(idle: Idle, session: Session) -> Result<Game, Box<(Idle, anyhow::Error)>> {
    let prepared = (|| {
        let (playback, connection) = match &session {
            Session::Demo { path } => (Some(read_playback(path)?), None),
            Session::Join { address, password } => {
                let connection = net::Connection::connect(address)
                    .with_context(|| format!("cannot connect to {address}"))?;
                tracing::info!(server = %connection.server, "connecting");
                (None, Some(connection.with_password(password)))
            }
            Session::Local { .. } => (None, None),
        };
        // a demo of a modded server plays with the mod, if it's here
        let demo_mod = playback
            .as_ref()
            .and_then(|p| p.demo.header.game_mod.clone())
            .filter(|game_mod| {
                let here = idle.assets.has_mod(game_mod);
                if !here {
                    tracing::warn!(name = game_mod.name, "the demo's mod is missing");
                }
                here
            });
        let vfs = idle.assets.mount(demo_mod.as_ref())?;
        let data = Arc::new(GameData::load(&vfs).context("cannot load game data")?);
        let map_name = match (&session, &playback) {
            (Session::Local { map }, _) => map.clone(),
            (_, Some(playback)) => playback.demo.header.map.clone(),
            _ => first_map(&vfs),
        };
        let map = MapFile::load(&vfs, &map_name)
            .with_context(|| format!("cannot load map {map_name}"))?;
        anyhow::Ok((vfs, data, map, connection, playback, demo_mod))
    })();
    let (vfs, data, map, connection, playback, demo_mod) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return Err(Box::new((idle, error))),
    };
    let Idle {
        console,
        assets,
        context,
    } = idle;
    let mut game = Game::new(SessionParts {
        vfs,
        console,
        data,
        map,
        connection,
        playback,
        assets,
        context,
    });
    game.remote.game_mod = demo_mod;
    Ok(game)
}

impl EventHandler for App {
    fn update(&mut self) {
        if self.loading_drawn
            && let Some(session) = self.pending.take()
        {
            self.start(session);
        }
        if let Some(game) = &mut self.game {
            game.update();
            if std::mem::take(&mut game.wants_settings) {
                self.front.show_overlay();
                window::show_mouse(true);
            }
            if std::mem::take(&mut game.to_menu) {
                self.leave();
            }
        }
        self.act();
    }

    fn draw(&mut self) {
        let front = &mut self.front;
        let vfs = &self.vfs;
        match (&mut self.game, &mut self.idle) {
            (Some(game), _) => {
                if front.overlay {
                    front
                        .previews
                        .update(&mut game.context, &game.vfs, &game.console);
                }
                game.draw();
                if front.overlay {
                    let config_dir = game.console.config_dir.clone();
                    let console = &mut game.console;
                    self.egui.run(&mut *game.context.ctx, |_, ctx| {
                        front.ui(ctx, console, vfs, &config_dir);
                    });
                    self.egui.draw(&mut *game.context.ctx);
                    // a new interface style: the game's sprites again
                    if std::mem::take(&mut front.reload_interface) {
                        game.reload_interface();
                    }
                }
                game.context.present();
            }
            (None, Some(idle)) => {
                front.previews.update(&mut idle.context, vfs, &idle.console);
                idle.context.clear(front::theme::background());
                let config_dir = idle.assets.config_dir.clone();
                let console = &mut idle.console;
                self.egui.run(&mut *idle.context.ctx, |_, ctx| {
                    front.ui(ctx, console, vfs, &config_dir);
                });
                self.egui.draw(&mut *idle.context.ctx);
                idle.context.present();
                self.loading_drawn = self.front.loading.is_some();
            }
            (None, None) => {}
        }
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        if let Some(game) = &mut self.game {
            game.resize_event(width, height);
        }
    }

    fn key_down_event(&mut self, keycode: KeyCode, keymods: KeyMods, repeat: bool) {
        if !repeat && self.capture(&key_name(keycode)) {
            return;
        }
        if self.menus_up() {
            if keycode == KeyCode::Escape && !repeat {
                self.front.escape();
            } else {
                self.egui.key_down_event(keycode, keymods);
            }
        } else if let Some(game) = &mut self.game {
            game.key_down_event(keycode, keymods, repeat);
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, keymods: KeyMods) {
        self.egui.key_up_event(keycode, keymods);
        if let Some(game) = &mut self.game {
            game.key_up_event(keycode, keymods);
        }
    }

    fn char_event(&mut self, character: char, keymods: KeyMods, repeat: bool) {
        if self.menus_up() {
            self.egui.char_event(character);
        } else if let Some(game) = &mut self.game {
            game.char_event(character, keymods, repeat);
        }
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if self.front.capturing.is_some() && self.capture(&mouse_button_name(button)) {
            return;
        }
        if self.menus_up() {
            self.egui.mouse_button_down_event(button, x, y);
        } else if let Some(game) = &mut self.game {
            game.mouse_button_down_event(button, x, y);
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        self.egui.mouse_button_up_event(button, x, y);
        if let Some(game) = &mut self.game {
            game.mouse_button_up_event(button, x, y);
        }
    }

    fn mouse_wheel_event(&mut self, dx: f32, dy: f32) {
        if self.menus_up() {
            self.egui.mouse_wheel_event(dx, dy);
        } else if let Some(game) = &mut self.game {
            game.mouse_wheel_event(dx, dy);
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.egui.mouse_motion_event(x, y);
        if let Some(game) = &mut self.game
            && !self.front.overlay
        {
            game.mouse_motion_event(x, y);
        }
    }

    fn raw_mouse_motion(&mut self, dx: f32, dy: f32) {
        if let Some(game) = &mut self.game
            && !self.front.overlay
        {
            game.raw_mouse_motion(dx, dy);
        }
    }
}
