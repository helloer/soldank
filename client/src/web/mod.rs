//! The browser build (wasm32): the game's files come over HTTP before the menus open (next to
//! the page: `soldat.smod` and the interface font), the log goes to the browser's console, sound
//! goes through the page's Web Audio, and servers are played on over WebTransport.

pub mod sound;
pub mod webtransport;

use super::*;
use std::cell::RefCell;
use std::rc::Rc;

/// The files fetched next to the page: the game's archive, the interface font. (The page's
/// `loading.js` has them already, fetched while it showed the progress, and hands them over.)
const FILES: [&str; 2] = ["soldat.smod", "play-regular.ttf"];

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// Tells the page the game is up (1), or couldn't start (0): its loading screen goes, or
    /// says so (`loading.js`).
    fn soldank_started(ok: i32);
}

/// The version of `loading.js` this game goes with (miniquad's loader compares them).
#[unsafe(no_mangle)]
pub extern "C" fn soldank_files_crate_version() -> u32 {
    1
}

pub fn start() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_writer(|| ConsoleLine(Vec::new()))
        .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
        .init();
    let conf = conf::Conf {
        window_title: "Soldank".to_owned(),
        high_dpi: true,
        // the batches' indices are 32-bit, which WebGL 1 can't draw
        platform: conf::Platform {
            webgl_version: conf::WebGLVersion::WebGL2,
            ..Default::default()
        },
        ..Default::default()
    };
    mq::start(conf, || Box::new(Web::new()));
}

/// A log line for the browser's console.
struct ConsoleLine(Vec<u8>);

impl std::io::Write for ConsoleLine {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for ConsoleLine {
    fn drop(&mut self) {
        let line = String::from_utf8_lossy(&self.0);
        let level = if line.contains("WARN") || line.contains("ERROR") {
            mq::log::Level::Warn
        } else {
            mq::log::Level::Info
        };
        mq::log::__private_api_log_lit(line.trim_end(), level, &("soldank", "", "", 0));
    }
}

type Fetched = Rc<RefCell<Vec<Option<Result<Vec<u8>, String>>>>>;

/// Fetching the files, then the program.
enum Web {
    Loading(Fetched),
    Running(Box<app::App>),
    Failed,
}

impl Web {
    fn new() -> Web {
        let fetched: Fetched = Rc::new(RefCell::new(vec![None; FILES.len()]));
        for (i, file) in FILES.iter().enumerate() {
            let fetched = fetched.clone();
            mq::fs::load_file(file, move |result| {
                fetched.borrow_mut()[i] = Some(result.map_err(|error| format!("{error:?}")));
            });
        }
        Web::Loading(fetched)
    }

    fn app(&mut self) -> Option<&mut app::App> {
        match self {
            Web::Running(app) => Some(app),
            _ => None,
        }
    }
}

/// The program from the fetched files, like `main` does from the command line.
fn build(files: Vec<Result<Vec<u8>, String>>) -> anyhow::Result<app::App> {
    let mut files = files.into_iter();
    let smod = files.next().unwrap_or_else(|| Err("missing".into()));
    let smod = smod.map_err(|error| anyhow::anyhow!("cannot fetch soldat.smod: {error}"))?;
    let font = match files.next() {
        Some(Ok(font)) => Some(Arc::<[u8]>::from(font)),
        _ => {
            tracing::warn!("cannot fetch play-regular.ttf: no text");
            None
        }
    };
    let assets = Assets {
        base: PathBuf::from("soldat.smod"),
        archive: Some(Arc::from(smod)),
        font,
        mods: Vec::new(),
        config_dir: PathBuf::from("."),
    };
    let vfs = assets.mount(None)?;
    let cli = Cli::parse_from(["soldank"]);
    let console = init_console(&cli, Path::new("."), &vfs);
    Ok(app::App::new(console, assets, vfs, None))
}

impl EventHandler for Web {
    fn update(&mut self) {
        if let Web::Loading(fetched) = self {
            if fetched.borrow().iter().any(Option::is_none) {
                return;
            }
            let files = fetched.borrow_mut().drain(..).flatten().collect();
            *self = match build(files) {
                Ok(app) => Web::Running(Box::new(app)),
                Err(error) => {
                    tracing::error!("{error:#}");
                    Web::Failed
                }
            };
            let ok = matches!(self, Web::Running(_));
            // SAFETY: a call with a number
            unsafe { soldank_started(i32::from(ok)) };
        }
        if let Some(app) = self.app() {
            app.update();
        }
        sound::mix();
    }

    fn draw(&mut self) {
        match self {
            Web::Running(app) => app.draw(),
            // the game info screen's colour while the files come, reddish if they didn't
            Web::Loading(_) | Web::Failed => {
                let color = match self {
                    Web::Failed => gfx2d::rgb(79, 49, 49),
                    _ => gfx2d::rgb(49, 61, 79),
                };
                let mut context = gfx2d::Gfx2dContext::new();
                context.clear(color);
                context.present();
            }
        }
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        if let Some(app) = self.app() {
            app.resize_event(width, height);
        }
    }

    fn key_down_event(&mut self, keycode: KeyCode, keymods: KeyMods, repeat: bool) {
        if let Some(app) = self.app() {
            app.key_down_event(keycode, keymods, repeat);
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, keymods: KeyMods) {
        if let Some(app) = self.app() {
            app.key_up_event(keycode, keymods);
        }
    }

    fn char_event(&mut self, character: char, keymods: KeyMods, repeat: bool) {
        if let Some(app) = self.app() {
            app.char_event(character, keymods, repeat);
        }
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if let Some(app) = self.app() {
            // in a game a click may lock the pointer to the page (Esc lets it go); the menus
            // want it free
            window::set_cursor_grab(app.playing());
            app.mouse_button_down_event(button, x, y);
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if let Some(app) = self.app() {
            app.mouse_button_up_event(button, x, y);
        }
    }

    fn mouse_wheel_event(&mut self, dx: f32, dy: f32) {
        if let Some(app) = self.app() {
            app.mouse_wheel_event(dx, dy);
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        if let Some(app) = self.app() {
            app.mouse_motion_event(x, y);
        }
    }

    fn raw_mouse_motion(&mut self, dx: f32, dy: f32) {
        if let Some(app) = self.app() {
            app.raw_mouse_motion(dx, dy);
        }
    }
}
