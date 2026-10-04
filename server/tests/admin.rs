//! Admin commands and bans: from the server's console, and from players logged in with
//! `adminlog`. Needs the game assets, else the tests are skipped.

mod common;

use common::*;
use soldank_core::net::*;
use soldank_server::admin;

fn texts(client: &TestClient) -> Vec<String> {
    client
        .notices
        .iter()
        .filter_map(|n| match n {
            Notice::ServerText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn command(line: &str) -> ClientMessage {
    ClientMessage::Command(line.into())
}

fn names(game: &TestMatch) -> Vec<String> {
    let mut names: Vec<String> = game
        .game
        .world
        .soldiers
        .values()
        .map(|s| s.name.clone())
        .collect();
    names.sort();
    names
}

#[test]
fn an_admin_bans_and_the_console_unbans() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_adminpassword", "boss")]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);

    // not an admin yet: ignored
    game.clients[bob].0.send(&command("kick Alice"));
    game.clients[bob].0.send(&command("adminlog guess"));
    game.settle_steps(10);
    assert_eq!(names(&game), ["Alice", "Bob"]);
    game.clients[bob].0.send(&command("adminlog boss"));
    game.clients[bob].0.send(&command("ban Alice"));
    game.settle_steps(60);
    assert_eq!(
        texts(game.client(bob)),
        [
            "Welcome",
            "Bob tried to login as Game Admin with bad password",
            "Bob added to Game Admins"
        ]
    );
    assert_eq!(names(&game), ["Bob"]);
    let kicked = Notice::Left {
        name: "Alice".into(),
        why: LeaveReason::Kicked,
    };
    assert!(game.client(alice).notices.contains(&kicked));
    assert!(game.client(bob).notices.contains(&kicked));
    let ban = game.game.lists.ban_of("10.0.0.1").unwrap();
    assert_eq!(
        (ban.reason.as_str(), ban.time),
        ("Banned by Bob", 30 * admin::DAY)
    );

    // back from the same address: turned away, until the console unbans her
    let again = game.join_from(hello("Alice"), [10, 0, 0, 1]);
    game.settle_steps(10);
    let banned = Notice::Refused(Refusal::Banned("Banned by Bob".into()));
    assert!(game.client(again).notices.contains(&banned));
    game.game.command(&mut game.server, None, "unban 10.0.0.1");
    let back = game.join_from(hello("Alice"), [10, 0, 0, 1]);
    game.settle_steps(30);
    assert!(game.client(back).world.is_some());
    assert_eq!(names(&game), ["Alice", "Bob"]);
}

#[test]
fn console_commands() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);
    let console = |game: &mut TestMatch, line: &str| {
        game.game.command(&mut game.server, None, line);
        game.settle_steps(10);
    };

    console(&mut game, "say hello all");
    console(&mut game, "pkill Bob");
    console(&mut game, "addbot Dutch");
    console(&mut game, "setteam5 @bots");
    console(&mut game, "sv_votepercent 50");
    assert!(texts(game.client(alice)).contains(&"hello all".to_string()));
    let died = |n: &Notice| matches!(n, Notice::Killed { .. });
    assert!(game.client(alice).notices.iter().any(died));
    assert_eq!(names(&game), ["Alice", "Bob", "Dutch"]);
    let dutch = game
        .game
        .world
        .soldiers
        .values()
        .find(|s| s.name == "Dutch")
        .unwrap();
    assert!(dutch.is_spectator());
    assert_eq!(game.game.cvars.int("sv_votepercent"), 50);

    // the console's admins: Bob's address
    console(&mut game, "admip 10.0.0.2");
    game.clients[bob].0.send(&command("sv_votepercent"));
    game.clients[alice].0.send(&command("kick @!me"));
    game.settle_steps(30);
    assert_eq!(
        texts(game.client(bob)),
        [
            "Welcome",
            "hello all",
            "sv_votepercent is \"50\" (Percentage of players in favor of a map/kick vote to let it pass)"
        ]
    );
    assert_eq!(names(&game), ["Alice", "Bob", "Dutch"]);
    game.clients[bob].0.send(&command("kick @!me"));
    game.settle_steps(30);
    assert_eq!(names(&game), ["Bob"]);
}

#[test]
fn admins_cant_be_voted_out() {
    // Alice alone is enough
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_votepercent", "50")]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(2 * 60 * 60 + 60);
    game.game.command(&mut game.server, None, "adm Bob");
    let num = game.client(bob).net.you.unwrap();
    let kick = ClientMessage::Vote {
        kind: VoteKind::Kick(num),
        reason: " admin abuse".into(),
    };
    game.clients[alice].0.send(&kick);
    game.settle_steps(2);
    game.clients[alice].0.send(&kick);
    game.settle_steps(30);
    // the vote passed but Bob stays
    assert!(game.client(alice).notices.contains(&Notice::VoteOff));
    assert_eq!(names(&game), ["Alice", "Bob"]);
    assert_eq!(game.game.lists.remote, ["10.0.0.2"]);
}

#[test]
fn refreshx_is_soldats_record() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    game.game
        .add_bot(&mut game.server, "Dutch", soldank_core::Team::Alpha)
        .unwrap();
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    // (in the game once a team is picked)
    game.clients[alice].0.send(&ClientMessage::JoinTeam(2));
    game.settle_steps(5);
    let packet = game.game.refresh_x();
    assert_eq!(packet.len(), soldank_server::rcon::REFRESHX_SIZE);
    assert_eq!(&packet[..10], b"REFRESHX\r\n");
    // names, in the frags order: string[24] each
    let name = |i: usize| {
        let at = 10 + i * 25;
        String::from_utf8_lossy(&packet[at + 1..at + 1 + packet[at] as usize]).into_owned()
    };
    let mut names = [name(0), name(1)];
    names.sort();
    assert_eq!(names, ["Alice", "Dutch"]);
    assert_eq!(name(2), "");
    // the map near the end: string[16], then the limits and settings, then the next map
    let map_at = packet.len() - 17 - 4 - 2 - 8 - 17;
    assert_eq!(
        &packet[map_at + 1..map_at + 1 + packet[map_at] as usize],
        b"ctf_Ash"
    );
    assert_eq!(packet[packet.len() - 17 - 4], 3, "sv_gamemode");
}

#[test]
fn kills_go_to_the_kill_log() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("log_filesupdate", "60")]) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    game.game.logs = Some(soldank_server::log::Logs::new(dir.path()));
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    game.clients[alice].0.send(&command("kill"));
    game.game.command(&mut game.server, None, "pkill Alice");
    game.settle_steps(120);

    let kills = std::fs::read_dir(dir.path().join("logs/kills")).unwrap();
    let file = kills.map(|f| f.unwrap().path()).next().unwrap();
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("killlog-") && name.ends_with("-01.txt"),
        "{name}"
    );
    let text = std::fs::read_to_string(file).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].ends_with("   Kill Log Started"));
    assert!(lines[1].starts_with("--- "));
    assert_eq!(&lines[2..5], ["Alice", "Alice", "Selfkill"]);
}

#[test]
fn pause_mute_pm_and_info() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);

    game.game.command(&mut game.server, None, "pause");
    game.settle_steps(10);
    let tick = game.game.world.tick;
    game.settle_steps(10);
    assert!(game.game.world.game.paused());
    assert!(game.client(alice).world.as_ref().unwrap().game.paused());
    assert_eq!(game.game.world.tick, tick + 10, "the ticks go on");
    game.game.command(&mut game.server, None, "unpause");
    game.settle_steps(10);
    assert!(!game.client(alice).world.as_ref().unwrap().game.paused());

    game.game.command(&mut game.server, None, "gmute Bob");
    game.game
        .command(&mut game.server, None, r#"pm Alice "meet me at the flag""#);
    let say = ClientMessage::Chat {
        text: "admin sucks".into(),
        team: false,
        radio: None,
    };
    game.clients[bob].0.send(&say);
    game.clients[alice].0.send(&command("info"));
    game.settle_steps(10);
    let heard: Vec<&str> = game
        .client(alice)
        .notices
        .iter()
        .filter_map(|n| match n {
            Notice::Chat { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(heard, ["(Muted)"]);
    let alice_texts = texts(game.client(alice));
    assert!(alice_texts.contains(&"(PM) meet me at the flag".to_string()));
    assert!(alice_texts.contains(&"Gamemode: Deathmatch".to_string()));
    assert!(alice_texts.iter().any(|t| t.starts_with("Nextmap: ")));
}

#[test]
fn teams_stay_even() {
    let settings = [
        ("sv_gamemode", "3"),
        ("sv_balanceteams", "1"),
        ("sv_maxspectators", "1"),
    ];
    let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
        return;
    };
    let with_team = |name: &str, t: u8| {
        let mut hello = hello(name);
        if let ClientMessage::Hello { team, .. } = &mut hello {
            *team = Some(t);
        }
        hello
    };
    game.join(with_team("Alice", 1), Link::perfect());
    // no charlie in CTF: Carol gets alpha or bravo (`FixTeam`)
    game.join(with_team("Carol", 3), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);
    let team_of = |game: &TestMatch, name: &str| {
        game.game
            .world
            .soldiers
            .values()
            .find(|s| s.name == name)
            .map(|s| s.team)
    };
    assert!(matches!(
        team_of(&game, "Carol"),
        Some(soldank_core::Team::Alpha | soldank_core::Team::Bravo)
    ));

    // Bob watches, not playing yet (no team asked for: the server waits for the team menu's
    // pick); the teams are 2:0, or 1:1 where the lowest is alpha
    assert_eq!(team_of(&game, "Bob"), None);
    let (alpha, bravo) = (soldank_core::Team::Alpha, soldank_core::Team::Bravo);
    let carol_alpha = team_of(&game, "Carol") == Some(alpha);
    let (fuller, smaller) = if carol_alpha {
        (alpha, bravo)
    } else {
        (bravo, alpha)
    };
    game.clients[bob]
        .0
        .send(&ClientMessage::JoinTeam(fuller as u8));
    game.settle_steps(10);
    assert_eq!(team_of(&game, "Bob"), None);
    let full = if fuller == alpha {
        "Alpha team is full"
    } else {
        "Bravo team is full"
    };
    assert!(texts(game.client(bob)).contains(&full.to_string()));
    game.clients[bob]
        .0
        .send(&ClientMessage::JoinTeam(smaller as u8));
    game.settle_steps(10);
    assert_eq!(team_of(&game, "Bob"), Some(smaller));

    // one spectator at most (Soldat never counts them: its limit only works at 0)
    game.clients[0].0.send(&ClientMessage::JoinTeam(5));
    game.settle_steps(5);
    game.clients[bob].0.send(&ClientMessage::JoinTeam(5));
    game.settle_steps(10);
    assert_eq!(team_of(&game, "Alice"), Some(soldank_core::Team::Spectator));
    assert_eq!(team_of(&game, "Bob"), Some(smaller));
    assert!(texts(game.client(bob)).contains(&"Spectators are full".to_string()));
}

#[test]
fn the_map_list_is_edited_and_kept() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    game.game.lists.dir = Some(dir.path().to_path_buf());
    let mut console = |line: &str| game.game.command(&mut game.server, None, line);
    console("addmap ctf_Ash");
    console("addmap ctf_Run");
    console("addmap ctf_Voland");
    console("delmap CTF_RUN");
    let saved = std::fs::read_to_string(dir.path().join("configs/mapslist.txt")).unwrap();
    assert!(saved.ends_with("ctf_Ash\r\nctf_Voland\r\n"), "{saved:?}");
    std::fs::write(dir.path().join("configs/short.txt"), "ctf_Run\nctf_Ash\n").unwrap();
    console("loadlist short.txt");
    assert_eq!(game.game.listed_next_map(), "ctf_Run");
}

#[test]
fn weapons_can_be_switched_off() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    // the USSOCOM is number 11
    game.game.command(&mut game.server, None, "weaponoff 11");
    let loadout = ClientMessage::Loadout {
        primary: weapon_index(soldank_core::WeaponKind::Ak74),
        secondary: weapon_index(soldank_core::WeaponKind::USSOCOM),
    };
    game.clients[alice].0.send(&loadout);
    game.settle_steps(10);
    let on_server = game
        .game
        .world
        .soldiers
        .values()
        .find(|s| s.name == "Alice")
        .unwrap();
    assert_eq!(
        on_server.loadout[..2],
        [
            soldank_core::WeaponKind::Ak74,
            soldank_core::WeaponKind::NoWeapon
        ]
    );
    // the player's game hears of it (the weapons menu leaves it out)
    let client = game.client(alice);
    let me = &client.world.as_ref().unwrap().soldiers[client.net.own().unwrap()];
    assert_eq!(me.weapon_sel, soldank_core::ALL_WEAPONS & !(1 << 10));
    game.game.command(&mut game.server, None, "weaponon 11");
    game.settle_steps(10);
    let client = game.client(alice);
    let me = &client.world.as_ref().unwrap().soldiers[client.net.own().unwrap()];
    assert_eq!(me.weapon_sel, soldank_core::ALL_WEAPONS);
}

#[test]
fn advance_mode_gives_weapons_for_kills() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_advancemode", "1")]) else {
        return;
    };
    for bot in ["Dutch", "Kruger", "Sniper"] {
        game.game
            .add_bot(&mut game.server, bot, soldank_core::Team::None)
            .unwrap();
    }
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    // nobody has a primary to begin with: Alice's pick leaves her hands
    let primaries = |s: &soldank_core::Soldier| s.weapon_sel & soldank_core::PRIMARIES;
    assert!(game.game.world.soldiers.values().all(|s| primaries(s) == 0));
    let loadout = ClientMessage::Loadout {
        primary: weapon_index(soldank_core::WeaponKind::Spas12),
        secondary: weapon_index(soldank_core::WeaponKind::USSOCOM),
    };
    game.clients[alice].0.send(&loadout);
    game.settle_steps(10);
    let alice_soldier = game
        .game
        .world
        .soldiers
        .values()
        .find(|s| s.name == "Alice")
        .unwrap();
    assert_eq!(alice_soldier.loadout[0], soldank_core::WeaponKind::NoWeapon);

    // the bots fight: kills win primaries, deaths lose them
    for _ in 0..7200 {
        game.step(&[]);
    }
    let soldiers: Vec<_> = game.game.world.soldiers.values().collect();
    assert!(soldiers.iter().any(|s| s.kills >= 2));
    assert!(soldiers.iter().any(|s| primaries(s) != 0));
    // whoever holds a primary may
    for s in &soldiers {
        let gun = s.primary_weapon().kind;
        assert!(s.may_use(gun), "{} holds {gun:?}", s.name);
    }
}

#[test]
fn bots_even_the_teams() {
    let settings = [("sv_gamemode", "3"), ("sv_botbalance", "1")];
    let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
        return;
    };
    let with_team = |name: &str, t: u8| {
        let mut hello = hello(name);
        if let ClientMessage::Hello { team, .. } = &mut hello {
            *team = Some(t);
        }
        hello
    };
    let teams = |game: &TestMatch| {
        let mut teams: Vec<(String, soldank_core::Team, bool)> = game
            .game
            .world
            .soldiers
            .values()
            .map(|s| (s.name.clone(), s.team, s.brain.is_some()))
            .collect();
        teams.sort_by_key(|(_, team, _)| *team as u8);
        teams
    };
    use soldank_core::Team::{Alpha, Bravo};

    // Alice alone on alpha: a bot comes for bravo
    let alice = game.join(with_team("Alice", 1), Link::perfect());
    game.settle_steps(30);
    let now = teams(&game);
    assert_eq!(now.len(), 2, "{now:?}");
    assert_eq!((now[0].1, now[0].2), (Alpha, false));
    assert_eq!((now[1].1, now[1].2), (Bravo, true));

    // Bob joins bravo: the bot makes room
    game.join(with_team("Bob", 2), Link::perfect());
    game.settle_steps(30);
    let now = teams(&game);
    assert_eq!(
        now,
        [
            ("Alice".to_string(), Alpha, false),
            ("Bob".to_string(), Bravo, false)
        ]
    );

    // Alice leaves: a bot takes alpha
    game.leave(alice);
    game.settle_steps(30);
    let now = teams(&game);
    assert_eq!(now.len(), 2, "{now:?}");
    assert_eq!((now[0].1, now[0].2), (Alpha, true));
    assert_eq!(now[1].0, "Bob");
}

#[test]
fn clients_play_with_the_servers_weapons() {
    // the server's weapons mod: stronger Desert Eagles
    let weapons = "[Info]\nName=Strong\nVersion=1\n[Desert Eagles]\nDamage=3.5\n";
    let files = vec![("configs/weapons.ini", weapons.as_bytes().to_vec())];
    let Some(mut game) = TestMatch::with_files("ctf_Ash", &[], files) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    let eagles = soldank_core::WeaponKind::DesertEagles;
    let on_server = game.game.world.config.weapons.get(eagles).hit_multiply;
    let world = game.client(alice).world.as_ref().unwrap();
    assert_eq!(on_server, 3.5);
    assert_eq!(world.config.weapons.get(eagles).hit_multiply, 3.5);
    let default = soldank_core::WeaponTable::new(false, None).unwrap();
    assert_ne!(default.get(eagles).hit_multiply, 3.5);
}

#[test]
fn a_thrown_knife_without_the_knife_is_a_cheat() {
    for kick in ["0", "1"] {
        let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_anticheatkick", kick)]) else {
            return;
        };
        let alice = game.join(hello("Alice"), Link::perfect());
        game.settle_steps(30);
        // the Combat Knife is number 12
        game.game.command(&mut game.server, None, "weaponoff 12");
        game.settle_steps(5);
        let hands = game
            .game
            .world
            .soldiers
            .values()
            .next()
            .unwrap()
            .skeleton
            .pos(15);
        let knife = ClientMessage::Bullet(BulletState {
            weapon: weapon_index(soldank_core::WeaponKind::ThrownKnife),
            pos: hands.into(),
            velocity: [10.0, 0.0],
            seed: 0,
        });
        game.clients[alice].0.send(&knife);
        game.settle_steps(10);
        // Soldat kicks for it only with sv_anticheatkick (off by default)
        let here = game.game.world.soldiers.values().any(|s| s.name == "Alice");
        assert_eq!(here, kick == "0");
        if kick == "1" {
            let ban = game
                .game
                .lists
                .ban_of("10.0.0.1")
                .map(|b| b.reason.as_str());
            assert_eq!(ban, Some("Knife-Spawn Cheat"));
            let told = game.client(alice).notices.iter().any(|n| {
                matches!(
                    n,
                    Notice::Left {
                        why: LeaveReason::Cheat,
                        ..
                    }
                )
            });
            assert!(told);
        }
    }
}
