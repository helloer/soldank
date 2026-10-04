mod app;
mod audio;
mod camera;
mod chat;
mod clock;
mod commands;
mod config;
mod controls;
mod demo;
mod events;
mod files;
mod front;
mod game;
mod input;
mod menus;
mod net;
mod online;
#[cfg(feature = "dev")]
mod overlay;
mod platform;
mod render;
mod session;
mod settings;
mod sparks;
mod stats;
mod view;
mod vote;
#[cfg(target_arch = "wasm32")]
mod web;

use camera::*;
use chat::{Chat, ChatKey, ChatKind, ChatOutcome, Completions};
use config::*;
use files::*;
use input::*;
use menus::{MenuAction, Menus};
use render::console::colors as console_colors;
use render::*;

use anyhow::Context;
use clap::Parser;
use gfx2d::mq::{self, EventHandler, KeyCode, KeyMods, MouseButton, conf, window};
use soldank_core::assets::Vfs;
use soldank_core::config::{Console, Cvar, Cvars};
use soldank_core::demo::LocalAction;
use soldank_core::net::{GameMod, ModDownload, NetLooks, file_hash, weapon_index, weapon_kind};
use soldank_core::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DT: f64 = 1.0 / TICKS_PER_SECOND as f64;

#[derive(Parser)]
#[command(name = "Soldank", version)]
#[command(about = "open source clone of Soldat engine written in rust")]
struct Cli {
    /// play a local game on this map right away (else the menus open)
    #[arg(short, long)]
    map: Option<String>,

    /// asset directory or soldat.smod archive [default: ./assets or ./soldat.smod]
    #[arg(long, env = "SOLDANK_ASSETS")]
    assets: Option<PathBuf>,

    /// mod directory or .smod archive mounted over the base assets (repeatable)
    #[arg(long = "mod", value_name = "PATH")]
    mods: Vec<PathBuf>,

    /// directory holding autoexec.cfg, configs/, demos/, screens/ and downloads; `exec` paths
    /// are relative to it [default: where the game's files are, like Soldat's portable mode]
    #[arg(long)]
    config_dir: Option<PathBuf>,

    /// set a cvar after config files are loaded, e.g. --set sv_gravity 0.03 (repeatable)
    #[arg(long, num_args = 2, value_names = ["CVAR", "VALUE"])]
    set: Vec<String>,

    /// play on a server (host or host:port, port 23073 by default)
    #[arg(long, value_name = "ADDRESS")]
    connect: Option<String>,

    /// the server's password (sv_password)
    #[arg(long, default_value = "")]
    password: String,

    /// play a demo: a file, or a name in the config directory's demos/
    #[arg(long, value_name = "DEMO")]
    demo: Option<String>,
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let assets = Assets::new(&cli)?;
    let vfs = assets.mount(None)?;
    let console = init_console(&cli, &assets.config_dir, &vfs);
    // the command line can ask for a session; else the menus
    let start = match (&cli.demo, &cli.connect, &cli.map) {
        (Some(name), ..) => Some(app::Session::Demo {
            path: demo::demo_path(&assets.config_dir, name),
        }),
        (None, Some(address), _) => Some(app::Session::Join {
            address: address.clone(),
            password: cli.password.clone(),
        }),
        (None, None, Some(map)) => Some(app::Session::Local { map: map.clone() }),
        (None, None, None) => None,
    };

    let fullscreen = console.cvars.int("r_fullscreen") != 0;
    // 0 (Soldat's configs: the desktop's size) keeps a 1280x720 window
    let size = |name: &str, default: i32| match console.cvars.int(name) as i32 {
        0 => default,
        size => size,
    };
    let (width, height) = (size("r_screenwidth", 1280), size("r_screenheight", 720));

    let conf = conf::Conf {
        window_title: "Soldank".to_owned(),
        window_width: width,
        window_height: height,
        fullscreen,
        // fixed like opensoldat's window; resizes (tiling window managers, fullscreen on any
        // shape) are letterboxed
        window_resizable: false,
        // full resolution where the platform scales windows (macOS, Windows, Wayland); the
        // game draws in pixels whatever their size, and X11 takes the size as pixels
        high_dpi: true,
        platform: conf::Platform {
            linux_backend: conf::LinuxBackend::X11WithWaylandFallback,
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };
    mq::start(conf, move || {
        Box::new(app::App::new(console, assets, vfs, start))
    });
    Ok(())
}

/// The radio menu's choices: enemy flagger, friendly flagger, enemy spotted; up, middle,
/// down.
const RADIO_SUBJECTS: [&str; 3] = ["EFC", "FFC", "ES"];
const RADIO_WHERE: [&str; 3] = ["U", "M", "D"];

/// `MENU_TIME`: after dying the camera stays on the corpse this long.
const MENU_TIME: i32 = 60;

/// A session: a local game, a game on a server, or a demo.
struct Game {
    vfs: Vfs,
    console: Console,
    /// Playing on a server (`--connect`).
    connection: Option<net::Connection>,
    /// The server's (or the demo's) game: its mod, weapons, cvars and maps, and downloads.
    remote: online::Remote,
    /// Where the game's files come from.
    assets: Assets,
    /// The escape menu asked for the front end's menus, or for the settings over the game.
    pub to_menu: bool,
    /// The console asked for a server (`connect`, `joinurl`; `None` inside: `retry` the
    /// last one).
    pub join: Option<Option<app::Session>>,
    action_snap: view::ActionSnap,
    pub wants_settings: bool,
    /// The game data as loaded (the server's or the demo's weapons mods change it).
    base_data: Arc<GameData>,
    /// Events from outside the tick (player commands), handled with the next one's.
    queued_events: Vec<GameEvent>,
    /// Recording a demo of the game on a server.
    recorder: Option<demo::Recorder>,
    /// Watching a demo (`--demo`).
    playback: Option<demo::Playback>,
    /// The dev overlay (F12).
    #[cfg(feature = "dev")]
    overlay: overlay::Overlay,
    context: gfx2d::Gfx2dContext,
    graphics: GameGraphics,
    world: World,
    /// The local player; in team modes nobody until a team is picked.
    player: Option<SoldierId>,
    /// The player was dead last tick (the weapons menu opens on death).
    was_dead: bool,
    /// The menus, messages, weapon stats, radio and vote.
    hud: view::Hud,
    audio: audio::Audio,
    sparks: sparks::Sparks,
    /// A screenshot to save after the next frame is drawn (`ScreenshotPath`).
    screenshot: Option<PathBuf>,
    /// `ScreenTaken`: the end of the match gets a screenshot (`cl_endscreenshot`).
    end_screenshot: bool,
    camera: Camera,
    /// `CameraFollowSprite`: `None` is the free camera.
    follow: Option<SoldierId>,
    /// `FreeCamPressed`: switching the camera waits for the key to come up.
    free_cam_pressed: bool,
    /// The numpad zoom (a dev extra) on top of `r_zoom`.
    dev_zoom: f32,
    chat: Chat,
    input: InputState,
    /// Each weapon as it comes, for `cycleweapon`.
    weapons: Vec<Weapon>,
    clock: clock::Clock,
    /// Bots in the game (file name, team); they come back after a map change.
    bots: Vec<(String, String, Team)>,
}

/// `r_forcebg`: the sky in two colours of the player's choice instead of the map's.
fn forced_background(cvars: &Cvars) -> Option<[(u8, u8, u8); 2]> {
    cvars.bool("r_forcebg").then(|| {
        [
            cvars.color("r_forcebg_color1"),
            cvars.color("r_forcebg_color2"),
        ]
    })
}

/// `MIN_FOV`, `MAX_FOV`: the game's width/height ratio stays within these, the rest of the
/// window is black.
const MIN_FOV: f32 = 1.25;
const MAX_FOV: f32 = 1.78;

/// `GameWidth` for a window `screen` pixels large (the game is 480 high).
fn game_width((w, h): (f32, f32)) -> f32 {
    (480.0 * (w / h).clamp(MIN_FOV, MAX_FOV)).round_ties_even()
}

fn team_from_num(n: i64) -> Team {
    match n {
        1 => Team::Alpha,
        2 => Team::Bravo,
        3 => Team::Charlie,
        4 => Team::Delta,
        5 => Team::Spectator,
        _ => Team::None,
    }
}

/// What a session (a local game, a server's, a demo) is made of.
pub(crate) struct SessionParts {
    pub vfs: Vfs,
    pub console: Console,
    pub data: Arc<GameData>,
    pub map: MapFile,
    pub connection: Option<net::Connection>,
    pub playback: Option<demo::Playback>,
    pub assets: Assets,
    pub context: gfx2d::Gfx2dContext,
}
