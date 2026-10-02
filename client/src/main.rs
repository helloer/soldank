macro_rules! iif(
    ($cond:expr, $then:expr, $else:expr) => (if $cond { $then } else { $else })
);

mod camera;
mod input;
mod render;

use camera::*;
use input::*;
use render::*;

use anyhow::{Context, bail};
use clap::Parser;
use gfx2d::mq::{self, EventHandler, KeyCode, KeyMods, MouseButton, conf, window};
use soldank_core::assets::Vfs;
use soldank_core::config::{Console, Cvar, Cvars};
use soldank_core::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const DT: f64 = 1.0 / TICKS_PER_SECOND as f64;

#[derive(Parser)]
#[command(name = "Soldank", version)]
#[command(about = "open source clone of Soldat engine written in rust")]
struct Cli {
    /// name of map to load
    #[arg(short, long, default_value = "ctf_Ash")]
    map: String,

    /// asset directory or soldat.smod archive [default: ./assets or ./soldat.smod]
    #[arg(long, env = "SOLDANK_ASSETS")]
    assets: Option<PathBuf>,

    /// mod directory or .smod archive mounted over the base assets (repeatable)
    #[arg(long = "mod", value_name = "PATH")]
    mods: Vec<PathBuf>,

    /// directory holding autoexec.cfg; `exec` paths are relative to it
    #[arg(long, default_value = ".")]
    config_dir: PathBuf,

    /// set a cvar after config files are loaded, e.g. --set sv_gravity 0.03 (repeatable)
    #[arg(long, num_args = 2, value_names = ["CVAR", "VALUE"])]
    set: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let vfs = mount_assets(&cli)?;
    let mut console = init_console(&cli);

    let data = Arc::new(GameData::load(&vfs).context("cannot load game data")?);
    let map = MapFile::load(&vfs, &cli.map).context("cannot load map")?;

    let fullscreen = console.cvars.int("r_fullscreen") != 0;
    let width = console.cvars.int("r_screenwidth") as i32;
    let height = console.cvars.int("r_screenheight") as i32;
    log_console_output(&mut console);

    let conf = conf::Conf {
        window_title: "Soldank".to_owned(),
        window_width: width,
        window_height: height,
        fullscreen,
        window_resizable: false,
        platform: conf::Platform {
            linux_backend: conf::LinuxBackend::X11WithWaylandFallback,
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };

    mq::start(conf, move || Box::new(Game::new(vfs, console, data, map)));
    Ok(())
}

fn mount_assets(cli: &Cli) -> anyhow::Result<Vfs> {
    let base = match &cli.assets {
        Some(path) => path.clone(),
        None => match ["assets", "soldat.smod"]
            .map(PathBuf::from)
            .into_iter()
            .find(|p| p.exists())
        {
            Some(path) => path,
            None => bail!(
                "no game assets found: put Soldat's assets in ./assets or ./soldat.smod, \
                 or point --assets / SOLDANK_ASSETS at them (see README)"
            ),
        },
    };

    let mut vfs = Vfs::new();
    vfs.mount(&base)
        .with_context(|| format!("cannot mount assets from {}", base.display()))?;

    for path in &cli.mods {
        vfs.mount(path)
            .with_context(|| format!("cannot mount mod {}", path.display()))?;
    }

    Ok(vfs)
}

fn register_client_cvars(cvars: &mut Cvars) {
    use soldank_core::config::CvarFlags;
    let client = CvarFlags::CLIENT | CvarFlags::INIT_ONLY;

    cvars.register(
        Cvar::int("r_fullscreen", 0, "Set mode of fullscreen")
            .flags(client)
            .range(0.0, 2.0),
    );
    cvars.register(
        Cvar::int("r_screenwidth", 1280, "Window width")
            .flags(client)
            .range(320.0, 16384.0),
    );
    cvars.register(
        Cvar::int("r_screenheight", 720, "Window height")
            .flags(client)
            .range(240.0, 16384.0),
    );
}

fn init_console(cli: &Cli) -> Console {
    let mut cvars = Cvars::new();
    soldank_core::register_cvars(&mut cvars);
    register_client_cvars(&mut cvars);

    let mut console = Console::new(cvars);
    console.config_dir = cli.config_dir.clone();
    console.register_command("quit", "quit: exit the game");
    console.register_command("map", "map <name>: load a map");
    console.register_command(
        "cycleweapon",
        "cycleweapon: debug, switch to the next weapon",
    );

    let _ = console.execute_script(DEFAULT_BINDINGS);

    if cli.config_dir.join("autoexec.cfg").exists() {
        let _ = console.execute("exec autoexec.cfg");
    }

    for pair in cli.set.chunks(2) {
        if let Err(error) = console.cvars.set(&pair[0], &pair[1]) {
            console.print(error.to_string());
        }
    }

    console.cvars.mark_started();
    console
}

fn log_console_output(console: &mut Console) {
    for line in console.take_output() {
        tracing::info!(target: "console", "{line}");
    }
}

struct Game {
    vfs: Vfs,
    console: Console,
    context: gfx2d::Gfx2dContext,
    graphics: GameGraphics,
    world: World,
    player: SoldierId,
    camera: Camera,
    input: InputState,
    weapons: Vec<Weapon>,
    time_start: Instant,
    timecur: f64,
    timeprv: f64,
    timeacc: f64,
}

impl Game {
    fn new(vfs: Vfs, console: Console, data: Arc<GameData>, map: MapFile) -> Game {
        let (w, h) = window::screen_size();
        let mut world = World::new(data, map, WorldConfig::from_cvars(&console.cvars));
        let player = world.spawn_soldier();
        let camera = Camera::new(world.soldiers[player].particle.pos, w * (480.0 / h), 480.0);

        let mut context = gfx2d::Gfx2dContext::new();
        window::show_mouse(false);
        window::set_cursor_grab(true);

        let mut graphics = GameGraphics::new();
        graphics.load_sprites(&mut context, &vfs);
        graphics.load_map(&mut context, &vfs, &world.map);

        let weapons: Vec<Weapon> = WeaponKind::values()
            .iter()
            .map(|k| Weapon::new(*k, false))
            .collect();

        Game {
            vfs,
            console,
            context,
            graphics,
            world,
            player,
            camera,
            input: InputState::default(),
            weapons,
            time_start: Instant::now(),
            timecur: 0.0,
            timeprv: 0.0,
            timeacc: 0.0,
        }
    }

    fn current_time(&self) -> f64 {
        self.time_start.elapsed().as_secs_f64()
    }

    fn run_command(&mut self, command: &str) {
        let _ = self.console.execute(command);
        self.run_deferred_commands();
    }

    fn run_deferred_commands(&mut self) {
        for command in self.console.take_deferred() {
            match (command.name.as_str(), command.args.as_slice()) {
                ("quit", _) => window::request_quit(),
                ("map", [name]) => self.change_map(name),
                ("cycleweapon", _) => {
                    let soldier = &mut self.world.soldiers[self.player];
                    let index = soldier.primary_weapon().kind.index();
                    let index = (index + 1) % (WeaponKind::NoWeapon.index() + 1);
                    soldier.weapons[soldier.active_weapon] = self.weapons[index];
                }
                (name, _) => self.console.print(format!("usage error in {name}")),
            }
        }

        log_console_output(&mut self.console);
    }

    fn change_map(&mut self, name: &str) {
        match MapFile::load(&self.vfs, name) {
            Ok(map) => {
                let config = WorldConfig::from_cvars(&self.console.cvars);
                self.world = World::new(self.world.data.clone(), map, config);
                self.player = self.world.spawn_soldier();
                self.camera.pos = self.world.soldiers[self.player].particle.pos;
                self.camera.pos_prev = self.camera.pos;
                self.graphics
                    .load_map(&mut self.context, &self.vfs, &self.world.map);
            }
            Err(error) => self
                .console
                .print(format!("cannot load map {name}: {error}")),
        }
    }
}

impl EventHandler for Game {
    fn update(&mut self) {
        self.timecur = self.current_time();
        self.timeacc += self.timecur - self.timeprv;
        self.timeprv = self.timecur;

        while self.timeacc >= DT {
            self.timeacc -= DT;

            self.world.config = WorldConfig::from_cvars(&self.console.cvars);

            let input = Input {
                buttons: self.input.buttons,
                aim: self.camera.aim(),
            };

            self.world.step(&[(self.player, input)]);

            let target = self.world.soldiers[self.player].particle.pos;
            self.camera.update(target, self.input.zoom_dir(), DT as f32);

            self.timecur = self.current_time();
            self.timeacc += self.timecur - self.timeprv;
            self.timeprv = self.timecur;
        }
    }

    fn draw(&mut self) {
        let p = f64::clamp(self.timeacc / DT, 0.0, 1.0);

        self.graphics.render_frame(
            &mut self.context,
            &self.world,
            &self.world.soldiers[self.player],
            &self.camera,
            self.timecur - DT * (1.0 - p),
            p as f32,
        );

        self.context.present();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _keymods: KeyMods, repeat: bool) {
        if repeat {
            return;
        }

        let key = key_name(keycode);
        if let Some(command) = self.input.handle(&self.console.bindings, &key, true) {
            self.run_command(&command);
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        let key = key_name(keycode);
        self.input.handle(&self.console.bindings, &key, false);
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        let key = mouse_button_name(button);
        if let Some(command) = self.input.handle(&self.console.bindings, &key, true) {
            self.run_command(&command);
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        let key = mouse_button_name(button);
        self.input.handle(&self.console.bindings, &key, false);
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let (w, h) = window::screen_size();
        self.camera.mouse.x = x * self.camera.game_width / w;
        self.camera.mouse.y = y * self.camera.game_height / h;
    }
}
