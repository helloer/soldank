//! soldank-server: runs a match for network clients at 60 ticks a second.

use anyhow::{Context, bail};
use clap::Parser;
use renet::{ConnectionConfig, RenetServer};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use soldank_core::assets::Vfs;
use soldank_core::config::{Console, Cvars};
use soldank_core::net::{DEFAULT_PORT, PROTOCOL_ID, team_from_num};
use soldank_core::*;
use soldank_server::rcon::{Rcon, Request};
use soldank_server::{MAX_PLAYERS, ServerGame, admin, log};
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant, SystemTime};
use tracing_subscriber::prelude::*;

#[derive(Parser, Debug)]
#[command(version, about = "soldank dedicated server")]
struct Cli {
    /// the first map
    #[arg(short, long, default_value = "ctf_Ash")]
    map: String,

    /// UDP address to listen on
    #[arg(long, default_value_t = SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)))]
    bind: SocketAddr,

    /// asset directory or soldat.smod archive [default: ./assets or ./soldat.smod]
    #[arg(long, env = "SOLDANK_ASSETS")]
    assets: Option<PathBuf>,

    /// mod directory or .smod archive mounted over the base assets (repeatable)
    #[arg(long = "mod", value_name = "PATH")]
    mods: Vec<PathBuf>,

    /// directory holding server.cfg, and configs/banned.txt and remote.txt; `exec` paths are
    /// relative to it
    #[arg(long, default_value = ".")]
    config_dir: PathBuf,

    /// add a bot: NAME or NAME:TEAM (repeatable)
    #[arg(long = "bot", value_name = "NAME[:TEAM]")]
    bots: Vec<String>,

    /// set a cvar after config files are loaded, e.g. --set sv_gamemode 3 (repeatable)
    #[arg(long, num_args = 2, value_names = ["CVAR", "VALUE"])]
    set: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    // the console: the log, also for remote admins
    let (console_tx, console_lines) = std::sync::mpsc::channel();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(ConsoleTap(console_tx))
        .init();
    let cli = Cli::parse();

    let mut vfs = mount_assets(&cli)?;
    let cvars = load_cvars(&cli, &vfs);
    let game_mod = mount_game_mod(&cli, &mut vfs, cvars.string("fs_mod"))?;
    let data = Arc::new(GameData::load(&vfs).context("cannot load game data")?);

    let admin_password = cvars.string("sv_adminpassword").to_string();
    let mut game = ServerGame::new(vfs, data, cvars, &cli.map).context("cannot load map")?;
    game.lists = admin::Lists::load(cli.config_dir.clone());
    game.load_map_list();
    game.address = Some(cli.bind);
    game.game_mod = game_mod;
    if game.cvars.bool("log_enable") && game.cvars.int("log_level") > 0 {
        game.logs = Some(log::Logs::new(&cli.config_dir));
    }
    let mut renet = RenetServer::new(ConnectionConfig::default());
    let socket = UdpSocket::bind(cli.bind).with_context(|| format!("cannot bind {}", cli.bind))?;
    let config = ServerConfig {
        current_time: unix_time(),
        max_clients: MAX_PLAYERS,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![cli.bind],
        authentication: ServerAuthentication::Unsecure,
    };
    let mut transport = NetcodeServerTransport::new(config, socket)?;
    tracing::info!(addr = %cli.bind, map = game.map_name(), "listening");

    for bot in &cli.bots {
        let (name, team) = bot.split_once(':').unwrap_or((bot, "0"));
        let team = team_from_num(team.parse().unwrap_or(0));
        if let Err(error) = game.add_bot(&mut renet, name, team) {
            tracing::warn!(%error, name, "bot");
        }
    }

    // remote admin, on the game's port over TCP (with an admin password only)
    let rcon = if admin_password.is_empty() {
        tracing::info!("Admin password not set: no remote admin (sv_adminpassword)");
        None
    } else {
        let rcon = Rcon::start(cli.bind, admin_password)
            .with_context(|| format!("cannot listen for admins on {} (TCP)", cli.bind))?;
        tracing::info!(addr = %rcon.addr, "remote admin");
        Some(rcon)
    };

    // the console: admin commands and server cvars, one a line
    let (stdin_tx, stdin) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if stdin_tx.send(line).is_err() {
                break;
            }
        }
    });

    // fixed ticks (slower in bullet time), sleeping in between
    let tick_time =
        |game: &ServerGame| Duration::from_secs_f64(1.0 / f64::from(game.world.goal_ticks()));
    let mut last = Instant::now();
    let mut next = last + tick_time(&game);
    loop {
        let now = Instant::now();
        let elapsed = now - last;
        last = now;
        renet.update(elapsed);
        transport.update(elapsed, &mut renet)?;
        game.receive(&mut renet, |id| {
            transport.client_addr(id).map(|addr| addr.ip())
        });
        while let Ok(line) = stdin.try_recv() {
            game.command(&mut renet, None, &line);
        }
        if let Some(rcon) = &rcon {
            for (id, ip, request) in rcon.poll() {
                match request {
                    Request::Refresh => rcon.send(id, game.refresh_x()),
                    Request::Shutdown => {
                        tracing::info!("[RCON] SHUTDOWN ({ip}).");
                        if let Some(logs) = &mut game.logs {
                            logs.write();
                        }
                        transport.disconnect_all(&mut renet);
                        transport.send_packets(&mut renet);
                        return Ok(());
                    }
                    Request::Line(line) => {
                        tracing::info!("{line} ({ip})");
                        if let Some(command) = line.strip_prefix('/') {
                            game.command(&mut renet, None, command);
                        }
                    }
                }
            }
        }
        for line in console_lines.try_iter() {
            game.log_console(&line);
            if let Some(rcon) = &rcon {
                rcon.broadcast(&line);
            }
        }
        game.tick(&mut renet);
        transport.send_packets(&mut renet);

        next += tick_time(&game);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            // far behind: don't try to catch up
            next = now;
        }
    }
}

/// The server's console lines (its log at `info` and up) for the remote admins.
struct ConsoleTap(Sender<String>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for ConsoleTap {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() > tracing::Level::INFO || !meta.target().starts_with("soldank") {
            return;
        }
        let mut line = Line::default();
        event.record(&mut line);
        let _ = self.0.send(line.0);
    }
}

/// An event as a line: the message, then the other fields.
#[derive(Default)]
struct Line(String);

impl tracing::field::Visit for Line {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.0.insert_str(0, value),
            name => self.0.push_str(&format!(" {name}={value}")),
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.0.insert_str(0, &format!("{value:?}")),
            name => self.0.push_str(&format!(" {name}={value:?}")),
        }
    }
}

fn unix_time() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
}

fn mount_assets(cli: &Cli) -> anyhow::Result<Vfs> {
    let base = match cli
        .assets
        .clone()
        .or_else(soldank_core::assets::find_game_files)
    {
        Some(path) => path,
        None => bail!(
            "no game assets found: use ./assets or ./soldat.smod (or next to the program), or \
             --assets"
        ),
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

/// The server's mod (`fs_mod`): `mods/<name>.smod` of the config directory over the game's
/// files, and its archive for the clients.
fn mount_game_mod(
    cli: &Cli,
    vfs: &mut Vfs,
    name: &str,
) -> anyhow::Result<Option<(net::GameMod, Arc<Vec<u8>>)>> {
    if name.is_empty() {
        return Ok(None);
    }
    if !net::GameMod::valid_name(name) {
        bail!("fs_mod: {name:?} is not a mod's file name");
    }
    let path = cli.config_dir.join("mods").join(format!("{name}.smod"));
    let bytes = std::fs::read(&path)
        .with_context(|| format!("Could not load mod archive ({})", path.display()))?;
    vfs.mount(&path)
        .with_context(|| format!("Could not load mod archive ({})", path.display()))?;
    if bytes.len() > net::MAX_DOWNLOAD as usize {
        tracing::warn!(
            name,
            size = bytes.len(),
            "the mod is too large for clients to download"
        );
    }
    let game_mod = net::GameMod {
        name: name.to_string(),
        hash: net::file_hash(&bytes),
    };
    tracing::info!(name, size = bytes.len(), "server mod");
    Ok(Some((game_mod, Arc::new(bytes))))
}

/// The server's cvars: Soldat's `configs/server.cfg` (the config directory's or the game's),
/// then `--set`.
fn load_cvars(cli: &Cli, vfs: &Vfs) -> Cvars {
    let mut cvars = Cvars::new();
    register_cvars(&mut cvars);
    let mut console = Console::new(cvars);
    console.config_dir = cli.config_dir.clone();
    if let Ok(text) = vfs.read_to_string("configs/server.cfg") {
        console
            .fallback_files
            .insert("configs/server.cfg".into(), text);
    }
    if let Err(error) = console.execute("exec configs/server.cfg") {
        tracing::debug!(%error, "server.cfg");
    }
    for line in console.take_output() {
        tracing::debug!("{line}");
    }
    for pair in cli.set.chunks(2) {
        if let Err(error) = console.cvars.set(&pair[0], &pair[1]) {
            tracing::warn!(%error, "--set");
        }
    }
    // `StartServer`: a LAN game has colliding guns and kits, a game on the internet doesn't
    let lan = if console.cvars.int("net_lan") == 1 {
        "1"
    } else {
        "0"
    };
    for cvar in ["sv_guns_collide", "sv_kits_collide"] {
        let _ = console.cvars.set(cvar, lan);
    }
    console.cvars
}
