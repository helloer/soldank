//! The client's cvars and console at start: Soldat's configs, the bindings, the player's looks.

use super::*;

pub(crate) fn register_client_cvars(cvars: &mut Cvars) {
    use soldank_core::config::CvarFlags;
    let client = CvarFlags::CLIENT | CvarFlags::INIT_ONLY;

    cvars.register(
        Cvar::int("r_fullscreen", 0, "Set mode of fullscreen")
            .flags(client)
            .range(0.0, 2.0),
    );
    cvars.register(
        Cvar::int("r_screenwidth", 1280, "Window width (0: 1280)")
            .flags(client)
            .range(0.0, 16384.0),
    );
    cvars.register(
        Cvar::int("r_screenheight", 720, "Window height (0: 720)")
            .flags(client)
            .range(0.0, 16384.0),
    );
    cvars.register(
        Cvar::string("font_1_filename", "play-regular.ttf", "First font filename").flags(client),
    );
    cvars.register(
        Cvar::int("ui_status_transparency", 200, "Transparency of ui")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 255.0),
    );
    cvars.register(
        Cvar::int(
            "cl_player_secwep",
            1,
            "Player secondary weapon (0 USSOCOM, 1 knife, 2 chainsaw, 3 LAW)",
        )
        .flags(CvarFlags::CLIENT)
        .range(0.0, 3.0),
    );
    cvars.register(
        Cvar::int("snd_volume", 50, "Sets sound volume")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 100.0),
    );
    cvars.register(
        Cvar::bool("snd_effects_battle", false, "Enables battle sound effects")
            .flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::bool(
            "snd_effects_explosions",
            false,
            "Enables sound explosions effects",
        )
        .flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::bool("r_weathereffects", true, "Enables weather effects").flags(CvarFlags::CLIENT),
    );
    cvars.register(Cvar::bool("ui_console", true, "Enables chat").flags(CvarFlags::CLIENT));
    for name in ["r_forcebg_color1", "r_forcebg_color2"] {
        cvars.register(
            Cvar::color(name, 0xFF0000, "Forced background colour").flags(CvarFlags::CLIENT),
        );
    }
    cvars.register(
        Cvar::int(
            "r_maxfps",
            60,
            "Frame rate limit (frames follow the display already)",
        )
        .flags(CvarFlags::CLIENT),
    );
    // Soldat's settings without an effect here: accepted so its configs load quietly
    for name in [
        "r_renderwidth",
        "r_renderheight",
        "r_scaleinterface",
        "r_smoothedges",
        "r_swapeffect",
        "r_dithering",
    ] {
        cvars.register(
            Cvar::string(name, "0", "(Soldat setting without an effect in soldank)")
                .flags(CvarFlags::CLIENT),
        );
    }
    cvars.register(
        Cvar::bool("cl_servermods", true, "Enables server mods feature").flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::bool("cl_actionsnap", false, "Enables action snap").flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::bool("r_renderui", true, "Enables interface rendering").flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::bool(
            "demo_showcrosshair",
            true,
            "Enables rendering crosshair in demos",
        )
        .flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::float("cl_sensitivity", 1.0, "Mouse sensitivity")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 100.0),
    );
    cvars.register(
        Cvar::string(
            "ui_style",
            "Default",
            "The interface (custom-interfaces/<name>)",
        )
        .flags(CvarFlags::CLIENT),
    );
    cvars.register(
        Cvar::int("cl_runs", 0, "Game runs")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 32.0),
    );
    cvars.register(
        Cvar::float("demo_speed", 1.0, "Demo speed")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 10.0),
    );
    cvars.register(
        Cvar::float("r_zoom", 0.0, "Sets rendering zoom (only for spectators)")
            .flags(CvarFlags::CLIENT)
            .range(-5.0, 5.0),
    );
    for (name, value, help) in [
        (
            "cl_screenshake",
            false,
            "Enables screen shake from enemy fire",
        ),
        ("r_renderbackground", true, "Draws the scenery in the back"),
        (
            "r_forcebg",
            false,
            "Forces the background colours r_forcebg_color1/2",
        ),
        (
            "r_fpslimit",
            true,
            "Limits the frame rate (frames follow the display already)",
        ),
        ("cl_endscreenshot", false, "Take screenshot when game ends"),
        (
            "ui_hidespectators",
            false,
            "Hides spectators from the fragsmenu",
        ),
        ("ui_playerindicator", true, "Enables player indicator"),
        (
            "ui_bonuscolors",
            true,
            "Tints the screen while a bonus lasts",
        ),
        ("ui_killconsole", true, "Enables kill console"),
        (
            "ui_sniperline",
            false,
            "Draws a line between the player and the cursor",
        ),
    ] {
        cvars.register(Cvar::bool(name, value, help).flags(CvarFlags::CLIENT));
    }
    for (name, value, help, max) in [
        (
            "ui_killconsole_length",
            15,
            "Sets length of kill console",
            50.0,
        ),
        (
            "ui_minimap_transparency",
            230,
            "Transparency of minimap",
            255.0,
        ),
        (
            "ui_minimap_posx",
            285,
            "Horizontal position of minimap",
            640.0,
        ),
        ("ui_minimap_posy", 5, "Vertical position of minimap", 480.0),
    ] {
        cvars.register(
            Cvar::int(name, value, help)
                .flags(CvarFlags::CLIENT)
                .range(0.0, max),
        );
    }
    cvars.register(
        Cvar::int("ui_console_length", 6, "Sets length of main console")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 50.0),
    );
    cvars.register(
        Cvar::int("cl_player_team", 0, "Player team ID")
            .flags(CvarFlags::CLIENT)
            .range(0.0, 5.0),
    );
    cvars.register(
        Cvar::string("cl_player_name", "Major", "Player nickname")
            .flags(CvarFlags::CLIENT)
            .range(1.0, 24.0),
    );
    for (name, default, description) in [
        ("cl_player_shirt", 0x304289, "Player shirt color"),
        ("cl_player_pants", 0xFF0000, "Player pants color"),
        ("cl_player_hair", 0x000000, "Player hair color"),
        ("cl_player_jet", 0x00008B, "Player jet color"),
        ("cl_player_skin", 0xE6B478, "Player skin color"),
    ] {
        cvars.register(Cvar::color(name, default, description).flags(CvarFlags::CLIENT));
    }
    for (name, max, description) in [
        ("cl_player_hairstyle", 4.0, "Player hair style"),
        ("cl_player_headstyle", 2.0, "Player head style"),
        ("cl_player_chainstyle", 2.0, "Player chain style"),
    ] {
        cvars.register(
            Cvar::int(name, 0, description)
                .flags(CvarFlags::CLIENT)
                .range(0.0, max),
        );
    }
}

/// The player's looks from the settings (`cl_player_*`), as they go over the network.
pub(crate) fn player_net_looks(cvars: &Cvars) -> NetLooks {
    let looks = player_looks(cvars);
    NetLooks {
        shirt: looks.shirt,
        pants: looks.pants,
        skin: looks.skin,
        hair: looks.hair,
        jet: looks.jet,
        hair_style: looks.hair_style,
        chain: looks.chain,
        head_cap: cvars.int("cl_player_headstyle") as u8,
    }
}

/// `txt/radiomenu-default.ini`: the radio menu's texts.
pub(crate) fn load_radio_texts(vfs: &Vfs) -> HashMap<String, String> {
    vfs.read_to_string("txt/radiomenu-default.ini")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| {
            (
                k.trim().to_string(),
                v.trim_end_matches(['\r', '\n']).to_string(),
            )
        })
        .collect()
}

/// The local player's looks from the `cl_player_*` cvars.
pub(crate) fn player_looks(cvars: &Cvars) -> Looks {
    let color = |name| {
        let (r, g, b) = cvars.color(name);
        u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
    };
    Looks {
        shirt: color("cl_player_shirt"),
        pants: color("cl_player_pants"),
        skin: color("cl_player_skin"),
        hair: color("cl_player_hair"),
        jet: color("cl_player_jet"),
        hair_style: cvars.int("cl_player_hairstyle") as u8,
        chain: cvars.int("cl_player_chainstyle") as u8,
    }
}

/// Bots in a local game by default, for each team the mode has.
const LOCAL_BOTS: [(&str, &str); 5] = [
    ("bots_random_noteam", "5"),
    ("bots_random_alpha", "3"),
    ("bots_random_bravo", "3"),
    ("bots_random_charlie", "2"),
    ("bots_random_delta", "2"),
];

pub(crate) fn init_console(cli: &Cli, config_dir: &Path, vfs: &Vfs) -> Console {
    let mut cvars = Cvars::new();
    soldank_core::register_cvars(&mut cvars);
    register_client_cvars(&mut cvars);
    // a local game has bots to play against (a server has none of these unless it says so)
    for (cvar, bots) in LOCAL_BOTS {
        let _ = cvars.set_default(cvar, bots);
    }

    let mut console = Console::new(cvars);
    console.config_dir = config_dir.to_path_buf();
    console.register_command("quit", "quit: exit the game");
    console.register_command("map", "map <name>: load a map");
    console.register_command(
        "votemap",
        "votemap <name>: vote for a map (or load it alone)",
    );
    console.register_command("dummy", "dummy: spawn an idle soldier to shoot at");
    console.register_command(
        "record",
        "record [name]: record a demo of the game on a server",
    );
    console.register_command("stop", "stop: stop recording the demo");
    console.register_command("demo_tick", "demo_tick <tick>: skip to a tick in the demo");
    console.register_command("demo_tick_r", "demo_tick_r <ticks>: skip ticks in the demo");
    console.register_command("weapons", "weapons: weapons menu (or lock it while alive)");
    console.register_command("fragslist", "fragslist: show or hide the scoreboard");
    console.register_command("statsmenu", "statsmenu: show or hide your weapon stats");
    console.register_command("minimap", "minimap: show or hide the minimap");
    console.register_command("screenshot", "screenshot: save the screen to screens/");
    console.register_command("playername", "playername: show or hide teammates' names");
    console.register_command("sniperline", "sniperline: toggle the sniper line");
    console.register_command(
        "kick",
        "kick <name or number>: remove a player from the game",
    );
    console.register_command("radio", "radio: the radio menu (team games with flags)");
    console.register_command("chat", "chat: say something");
    console.register_command("teamchat", "teamchat: say something to your team");
    console.register_command("cmd", "cmd: type a console command");
    console.register_command("say", "say <text>: send chat message");
    console.register_command(
        "debug",
        "debug: the dev overlay (soldank built with --features dev)",
    );
    console.register_command(
        "debugdraw",
        "debugdraw [polygons colliders waypoints spawns skeletons]: draw these over the world",
    );
    console.register_command("say_team", "say_team <text>: send team chat message");
    console.register_command("switchcam", "switchcam <id>: (spectators) follow player id");
    console.register_command(
        "switchcamflag",
        "switchcamflag <style>: (spectators) look at a flag",
    );
    for (name, help) in [
        ("volumeup", "volumeup: sound volume up"),
        ("volumedown", "volumedown: sound volume down"),
        (
            "mousesensitivityup",
            "mousesensitivityup: mouse sensitivity up",
        ),
        (
            "mousesensitivitydown",
            "mousesensitivitydown: mouse sensitivity down",
        ),
        (
            "gamestats",
            "gamestats: show or hide the frame rate, ping and connection",
        ),
        (
            "recorddemo",
            "recorddemo: record a demo, or stop (a new one with demo_autorecord)",
        ),
    ] {
        console.register_command(name, help);
    }
    for (name, help) in [
        ("tabac", "tabac: chew tobacco"),
        ("smoke", "smoke: light a cigar"),
        ("takeoff", "takeoff: take the helmet off or put it on"),
        ("victory", "victory: cheer"),
        ("piss", "piss: ..."),
        ("mercy", "mercy: shoot yourself"),
        ("pwn", "pwn: taunt"),
        ("kill", "kill: suicide"),
        ("brutalkill", "brutalkill: suicide, messily"),
    ] {
        console.register_command(name, help);
    }
    console.register_command("changeteam", "changeteam: team menu");
    for (name, help) in [
        (
            "connect",
            "connect <ip> [port] [password]: play on a server",
        ),
        (
            "joinurl",
            "joinurl soldat://<ip>:<port>/<password>: play on a server",
        ),
        ("disconnect", "disconnect: leave the server"),
        ("retry", "retry: connect to the last server again"),
        ("shutdown", "shutdown: leave the game for the menus"),
        ("snap", "snap: show the action snap, or put it away"),
        ("mute", "mute <player or id, @group, all>: hide their chat"),
        (
            "unmute",
            "unmute <player or id, @group>: show their chat again",
        ),
    ] {
        console.register_command(name, help);
    }
    console.register_command(
        "addbot",
        "addbot <name>: add a bot (configs/bots/<name>.bot)",
    );
    for (name, help) in [
        ("addbot1", "addbot1 <name>: add a bot to alpha team"),
        ("addbot2", "addbot2 <name>: add a bot to bravo team"),
        ("addbot3", "addbot3 <name>: add a bot to charlie team"),
        ("addbot4", "addbot4 <name>: add a bot to delta team"),
    ] {
        console.register_command(name, help);
    }
    console.register_command(
        "cycleweapon",
        "cycleweapon: debug, switch to the next weapon",
    );

    let _ = console.execute_script(DEFAULT_BINDINGS);

    // Soldat's configs: configs/client.cfg runs the others (controls, taunt bindings,
    // player, sound, graphics, game); the config directory's own, else the game's
    for file in vfs.list("configs") {
        if file.ends_with(".cfg")
            && let Ok(text) = vfs.read_to_string(&format!("configs/{file}"))
        {
            console
                .fallback_files
                .insert(format!("configs/{file}"), text);
        }
    }
    if let Err(error) = console.execute("exec configs/client.cfg") {
        tracing::warn!(%error, "Soldat's configs");
    }
    // soldank's extras where the configs left the keys free
    for (key, command) in [
        ("m", "changeteam"),
        ("f12", "debug"),
        ("kpadd", "+zoomin"),
        ("kpsubtract", "+zoomout"),
    ] {
        if console.bindings.get(key).is_none() {
            console.bindings.insert(key, command.to_string());
        }
    }

    if config_dir.join("autoexec.cfg").exists() {
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
