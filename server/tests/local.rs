//! The server with in-memory clients (no sockets): joining, snapshots, deaths and flags reach
//! the clients' worlds, also over a slow, lossy link. Needs the game assets (`SOLDANK_ASSETS`
//! or `../assets`), else the tests are skipped.

mod common;

use common::*;
use soldank_core::net::*;
use soldank_core::*;

const WALK: Input = Input {
    buttons: Buttons::RIGHT,
    aim: Vec2::ZERO,
};

/// Two players and a bot for 30 seconds over `link`s; then everyone knows everyone, Bob sees
/// Alice near where the server has her, and every death reached both clients.
fn fight(link: impl Fn() -> Link, tolerance: f32, ping: std::ops::Range<u16>) {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    game.game
        .add_bot(&mut game.server, "Dutch", Team::None)
        .unwrap();
    let alice = game.join(hello("Alice"), link());
    let bob = game.join(hello("Bob"), link());
    for tick in 0..1800 {
        // deaths for sure, besides the bot's: Alice and Bob kill themselves
        if tick == 600 {
            game.clients[alice]
                .0
                .send(&ClientMessage::Command("kill".into()));
        }
        if tick == 1200 {
            game.clients[bob]
                .0
                .send(&ClientMessage::Command("brutalkill".into()));
        }
        game.step(&[Some(WALK), Some(Input::default())]);
    }

    let deaths_on_server: i32 = game.game.world.soldiers.values().map(|s| s.deaths).sum();
    let alice_num = game.client(alice).net.you.unwrap();
    let on_server = game
        .game
        .world
        .soldiers
        .values()
        .find(|s| s.name == "Alice")
        .unwrap();
    let a = on_server.particle.pos;
    let bob_client = game.client(bob);
    let on_bob =
        &bob_client.world.as_ref().unwrap().soldiers[bob_client.net.soldier(alice_num).unwrap()];
    let b = on_bob.particle.pos;
    assert!(a.distance(b) < tolerance, "server {a}, bob {b}");
    // honest moves are never put back (`sv_movecheck`)
    assert_eq!(game.game.corrections(), 0);
    // Bob knows Alice's ping
    assert!(
        ping.contains(&on_bob.ping),
        "ping {} not in {ping:?}",
        on_bob.ping
    );

    // what's still on its way arrives (resent if lost)
    game.settle(120);
    for client in [game.client(alice), game.client(bob)] {
        assert!(
            client
                .notices
                .iter()
                .any(|n| matches!(n, Notice::Welcome { .. }))
        );
        let world = client.world.as_ref().unwrap();
        let mut names: Vec<_> = world.soldiers.values().map(|s| s.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["Alice", "Bob", "Dutch"]);

        let killed = client
            .notices
            .iter()
            .filter(|n| matches!(n, Notice::Killed { .. }))
            .count() as i32;
        let deaths: i32 = world.soldiers.values().map(|s| s.deaths).sum();
        assert!(deaths_on_server >= 2);
        assert_eq!(killed, deaths_on_server);
        assert_eq!(deaths, deaths_on_server);
    }
}

#[test]
fn clients_join_and_see_the_match() {
    // snapshots are at most two ticks old
    fight(Link::perfect, 20.0, 0..50);
}

#[test]
fn a_slow_lossy_link_still_gets_everything() {
    // 100 ms each way, a tenth of the packets lost: positions lag, nothing goes missing
    fight(|| Link::lossy(6, 0.1), 80.0, 150..300);
}

#[test]
fn flags_reach_the_clients() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    let mut alpha = hello("Alice");
    if let ClientMessage::Hello { team, .. } = &mut alpha {
        *team = Some(1);
    }
    let alice = game.join(alpha, Link::perfect());
    for _ in 0..120 {
        game.step(&[Some(Input::default())]);
    }
    game.settle(10);

    let flags = |world: &World| {
        let mut flags: Vec<(ThingKind, Vec2)> = world
            .things
            .iter()
            .filter(|t| t.active && t.kind.is_flag())
            .map(|t| (t.kind, t.skeleton.pos(1)))
            .collect();
        flags.sort_by_key(|(kind, _)| *kind as u8);
        flags
    };
    let on_server = flags(&game.game.world);
    let on_alice = flags(game.client(alice).world.as_ref().unwrap());
    assert_eq!(on_server.len(), 2);
    assert_eq!(on_alice.len(), 2);
    for ((k1, p1), (k2, p2)) in on_server.iter().zip(&on_alice) {
        assert_eq!(k1, k2);
        assert!(p1.distance(*p2) < 5.0, "{k1:?}: server {p1}, client {p2}");
    }
    let world = game.client(alice).world.as_ref().unwrap();
    let me = &world.soldiers[game.client(alice).net.own().unwrap()];
    assert_eq!(me.team, Team::Alpha);
}

#[test]
fn a_password_keeps_strangers_out() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_password", "secret")]) else {
        return;
    };
    let stranger = game.join(hello("Mallory"), Link::perfect());
    let mut with_password = hello("Alice");
    if let ClientMessage::Hello { password, .. } = &mut with_password {
        *password = "secret".into();
    }
    let friend = game.join(with_password, Link::perfect());
    for _ in 0..30 {
        game.step(&[None, None]);
    }
    let refused = |n: &Notice| *n == Notice::Refused(Refusal::WrongPassword);
    assert!(game.client(stranger).notices.iter().any(refused));
    assert!(game.client(stranger).world.is_none());
    assert!(
        game.client(friend)
            .notices
            .iter()
            .any(|n| matches!(n, Notice::Welcome { .. }))
    );
    let names: Vec<_> = game
        .game
        .world
        .soldiers
        .values()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, ["Alice"]);
}

#[test]
fn a_map_the_client_lacks_is_downloaded() {
    let Some(vfs) = vfs() else { return };
    // ctf_Ash with its texture renamed: a map and an image only the server has
    let mut map = vfs.read("maps/ctf_Ash.pms").unwrap();
    let texture = MapFile::parse("maps/ctf_Ash.pms", &map)
        .unwrap()
        .texture_name;
    let image = vfs
        .find_with_extensions(&format!("textures/{texture}"), IMAGE_EXTENSIONS)
        .unwrap();
    let image = vfs.read(&image).unwrap();
    let name = b"downloaded.png";
    // `string[24]` after the version and `string[38]` map name
    map[43] = name.len() as u8;
    map[44..44 + name.len()].copy_from_slice(name);
    let files = vec![
        ("maps/ctf_Fresh.pms", map.clone()),
        ("textures/downloaded.png", image.clone()),
    ];
    let Some(mut game) = TestMatch::with_files("ctf_Ash", &[], files) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.05));
    for _ in 0..60 {
        game.step(&[None]);
    }
    // the first map is the client's own
    assert!(game.client(alice).downloaded.is_empty());
    assert!(game.client(alice).world.is_some());

    game.game.change_map(&mut game.server, "ctf_Fresh").unwrap();
    for _ in 0..300 {
        game.step(&[Some(WALK)]);
    }
    game.settle(30);

    let client = game.client(alice);
    let mut downloaded = client.downloaded.clone();
    downloaded.sort();
    assert_eq!(
        downloaded,
        ["maps/ctf_Fresh.pms", "textures/downloaded.png"]
    );
    assert_eq!(client.vfs.read("maps/ctf_Fresh.pms").unwrap(), map);
    assert_eq!(client.vfs.read("textures/downloaded.png").unwrap(), image);
    let world = client.world.as_ref().expect("the new map loaded");
    assert_eq!(world.map.texture_name, "downloaded.png");
    // Alice came along and plays on it
    let me = &world.soldiers[client.net.own().unwrap()];
    assert_eq!(me.name, "Alice");
    let on_server = game.game.world.soldiers.values().next().unwrap();
    assert!(on_server.particle.pos.distance(me.particle.pos) < 40.0);
}

/// Alice's bullets (owner, velocity) in a world, by velocity.
fn bullets_of(world: &World, owner: SoldierId) -> Vec<Vec2> {
    let mut bullets: Vec<Vec2> = world
        .bullets
        .iter()
        .filter(|b| b.active && b.owner == owner)
        .map(|b| b.particle.velocity)
        .collect();
    bullets.sort_by(|a, b| a.x.total_cmp(&b.x));
    bullets
}

#[test]
fn a_shotgun_blast_is_the_same_everywhere() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.0));
    let bob = game.join(hello("Bob"), Link::lossy(3, 0.0));
    for _ in 0..60 {
        game.step(&[None, None]);
    }
    let loadout = ClientMessage::Loadout {
        primary: weapon_index(WeaponKind::Spas12),
        secondary: weapon_index(WeaponKind::USSOCOM),
    };
    game.clients[alice].0.send(&loadout);
    for _ in 0..60 {
        game.step(&[Some(Input::default()), None]);
    }

    // one blast, aimed to the right
    let num = game.client(alice).net.you.unwrap();
    let client = game.client(alice);
    let me = &client.world.as_ref().unwrap().soldiers[client.net.own().unwrap()];
    assert_eq!(me.primary_weapon().kind, WeaponKind::Spas12);
    let fire = Input {
        buttons: Buttons::FIRE,
        aim: me.particle.pos + vec2(200.0, -20.0),
    };
    let on_server = game
        .game
        .world
        .soldiers
        .values()
        .position(|s| s.name == "Alice");
    let server_id = game
        .game
        .world
        .soldiers
        .keys()
        .nth(on_server.unwrap())
        .unwrap();
    let (mut at_alice, mut on_server, mut at_bob) = (Vec::new(), Vec::new(), Vec::new());
    for tick in 0..40 {
        game.step(&[Some(if tick < 2 { fire } else { Input::default() }), None]);
        let client = game.client(alice);
        if at_alice.is_empty() {
            at_alice = bullets_of(client.world.as_ref().unwrap(), client.net.own().unwrap());
        }
        // the server's, tick by tick from the first
        let bullets = bullets_of(&game.game.world, server_id);
        if !bullets.is_empty() || !on_server.is_empty() {
            on_server.push(bullets);
        }
        let client = game.client(bob);
        if at_bob.is_empty() {
            let id = client.net.soldier(num).unwrap();
            at_bob = bullets_of(client.world.as_ref().unwrap(), id);
        }
    }
    let same = |a: &[Vec2], b: &[Vec2]| {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.distance(*b) < 0.001)
    };
    // Alice sees the pellets that count
    assert_eq!(at_alice.len(), 6);
    assert!(
        same(&at_alice, &on_server[0]),
        "{at_alice:?}\n{:?}",
        on_server[0]
    );
    // Bob gets them later and moves them on by both pings: as the server's were then
    assert!(!at_bob.is_empty());
    let age = on_server.iter().position(|bullets| same(bullets, &at_bob));
    assert!(age.is_some_and(|age| age > 2), "{at_bob:?}\n{on_server:?}");
}

#[test]
fn everyone_sees_a_taunt() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.0));
    let bob = game.join(hello("Bob"), Link::lossy(3, 0.0));
    // standing still a while: Bob lands
    for _ in 0..120 {
        game.step(&[Some(Input::default()), Some(Input::default())]);
    }
    let num = game.client(bob).net.you.unwrap();
    game.clients[bob]
        .0
        .send(&ClientMessage::Command("victory".into()));
    let (mut seen_by_alice, mut seen_by_bob) = (0, 0);
    let mut was = [false; 2];
    for _ in 0..60 {
        game.step(&[Some(Input::default()), Some(Input::default())]);
        for (i, client) in [alice, bob].into_iter().enumerate() {
            let client = game.client(client);
            let world = client.world.as_ref().unwrap();
            let victory = world.soldiers[client.net.soldier(num).unwrap()]
                .body_animation
                .id
                == Anim::Victory;
            // count the starts
            if victory && !was[i] {
                *[&mut seen_by_alice, &mut seen_by_bob][i] += 1;
            }
            was[i] = victory;
        }
    }
    assert_eq!((seen_by_alice, seen_by_bob), (1, 1));
}

/// Two minutes and a bit: players can't start votes right after joining.
const VOTE_COOLDOWN: usize = 2 * 60 * 60 + 60;

fn server_texts(client: &TestClient) -> Vec<String> {
    client
        .notices
        .iter()
        .filter_map(|n| match n {
            Notice::ServerText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_map_vote_changes_the_map() {
    let maps = ("configs/mapslist.txt", b"ctf_Ash\nctf_Run\n".to_vec());
    let Some(mut game) = TestMatch::with_files("ctf_Ash", &[], vec![maps]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    let vote = |map: &str| ClientMessage::Vote {
        kind: VoteKind::Map(map.into()),
        reason: String::new(),
    };
    for _ in 0..60 {
        game.step(&[None, None]);
    }
    // too soon, and a map the server doesn't have
    game.clients[bob].0.send(&vote("ctf_Run"));
    game.clients[alice].0.send(&vote("ctf_Nowhere"));
    for _ in 0..VOTE_COOLDOWN {
        game.step(&[None, None]);
    }
    assert_eq!(
        server_texts(game.client(bob)),
        [
            "Welcome",
            "Can't vote for 2:00 minutes after joining game or last vote"
        ]
    );
    assert_eq!(
        server_texts(game.client(alice)),
        ["Welcome", "Map not found (ctf_Nowhere)"]
    );

    // Alice starts it (the map menu's Select), then both say yes (F12)
    game.clients[alice].0.send(&vote("ctf_run"));
    game.step(&[None, None]);
    for client in [alice, bob] {
        assert!(game.client(client).notices.iter().any(|n| matches!(
            n,
            Notice::VoteOn { kind: VoteKind::Map(map), .. } if map == "ctf_Run"
        )));
    }
    game.clients[alice].0.send(&vote("ctf_Run"));
    game.step(&[None, None]);
    assert_eq!(game.game.map_name(), "ctf_Ash", "half isn't 60%");
    game.clients[bob].0.send(&vote("ctf_Run"));
    // the match ends, and a few seconds later the map changes
    for _ in 0..600 {
        game.step(&[None, None]);
    }
    assert_eq!(game.game.map_name(), "ctf_Run");
    for client in [alice, bob] {
        let client = game.client(client);
        assert!(client.notices.contains(&Notice::VoteOff));
        assert!(client.notices.contains(&Notice::MatchEnded));
        let world = client.world.as_ref().expect("the new map loaded");
        assert!(
            world.map.filename.ends_with("ctf_Run.pms"),
            "{}",
            world.map.filename
        );
        assert_eq!(world.soldiers.len(), 2);
    }
}

#[test]
fn a_kick_vote_removes_a_player() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    let carol = game.join(hello("Carol"), Link::perfect());
    for _ in 0..VOTE_COOLDOWN {
        game.step(&[None, None, None]);
    }
    let target = game.client(carol).net.you.unwrap();
    let kick = |reason: &str| ClientMessage::Vote {
        kind: VoteKind::Kick(target),
        reason: reason.into(),
    };
    // Alice starts it; her game says yes for her right away
    game.clients[alice].0.send(&kick(" camping"));
    game.step(&[None, None, None]);
    game.clients[alice].0.send(&kick(""));
    game.clients[carol].0.send(&kick(""));
    game.step(&[None, None, None]);
    assert_eq!(
        server_texts(game.client(carol)),
        [
            "Welcome",
            "A vote has been cast against you. You can not vote."
        ]
    );
    assert!(game.game.world.soldiers.values().any(|s| s.name == "Carol"));
    // two of three
    game.clients[bob].0.send(&kick(""));
    for _ in 0..60 {
        game.step(&[None, None, None]);
    }
    assert!(!game.game.world.soldiers.values().any(|s| s.name == "Carol"));
    for client in [alice, bob, carol] {
        let client = game.client(client);
        let voted = |n: &Notice| matches!(n, Notice::Left { name, why: LeaveReason::VoteKicked } if name == "Carol");
        assert!(client.notices.iter().any(voted), "{:?}", client.notices);
    }
    for client in [alice, bob] {
        let world = game.client(client).world.as_ref().unwrap();
        assert!(!world.soldiers.values().any(|s| s.name == "Carol"));
    }
    // and her client is let go (renet's in-memory clients don't hear it)
    assert!(!game.server.is_connected(carol as u64 + 1));
    // for an hour (no permanent bans by votes)
    let ban = game.game.lists.ban_of("10.0.0.3").unwrap();
    assert_eq!(
        (ban.reason.as_str(), ban.time),
        ("Vote Kicked", 60 * 60 * 60)
    );
}

#[test]
fn a_demo_replays_the_game() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    game.game
        .add_bot(&mut game.server, "Dutch", Team::None)
        .unwrap();
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.05));
    game.clients[alice].0.demo = Some(Vec::new());
    for tick in 0..1200 {
        let input = Input {
            buttons: if tick % 120 < 60 {
                Buttons::RIGHT | Buttons::FIRE
            } else {
                Buttons::LEFT
            },
            aim: Vec2::new(300.0, -50.0),
        };
        game.step(&[Some(input)]);
    }
    let client = game.client(alice);
    let frames = client.demo.as_ref().unwrap();
    // through a demo file and back (snapshots kept as deltas there)
    let header = soldank_core::demo::DemoHeader {
        protocol: PROTOCOL_VERSION,
        map: "ctf_Ash".into(),
        map_hash: 0,
        cvars: Vec::new(),
        weapons_mods: [None, None],
        game_mod: None,
        you: None,
        start: Vec::new(),
        date: 0,
        local: false,
    };
    let mut writer = soldank_core::demo::DemoWriter::new(Vec::new(), &header).unwrap();
    for frame in frames {
        writer.frame(frame).unwrap();
    }
    let file = writer.finish().unwrap();
    let demo = soldank_core::demo::read_demo(file.as_slice()).unwrap();
    assert_eq!(&demo.frames, frames);
    let data = client.world.as_ref().unwrap().data.clone();
    let replayed = replay(&demo.frames, &vfs().unwrap(), &data).unwrap();

    // the same world, to the bit: soldiers, bullets, things
    let state = |world: &World| {
        let soldiers: Vec<_> = world
            .soldiers
            .values()
            .map(|s| {
                (
                    s.name.clone(),
                    s.particle.pos,
                    s.particle.velocity,
                    s.health,
                    s.dead_meat,
                )
            })
            .collect();
        let bullets: Vec<_> = world
            .bullets
            .iter()
            .filter(|b| b.active)
            .map(|b| (b.particle.pos, b.particle.velocity))
            .collect();
        let things: Vec<_> = world
            .things
            .iter()
            .filter(|t| t.active)
            .map(|t| t.skeleton.pos(1))
            .collect();
        format!("{soldiers:?}\n{bullets:?}\n{things:?}")
    };
    let live = client.world.as_ref().unwrap();
    assert!(live.bullets.iter().any(|b| b.active) || live.tick > 0);
    assert_eq!(state(&replayed), state(live));
    assert_eq!(replayed.tick, live.tick);
}

/// A mod only the server has (`fs_mod`), large enough to take a few ticks to download.
fn with_mod(game: &mut TestMatch) -> GameMod {
    let mut rng = 0x2545_f491_4f6c_dd1d_u64;
    let noise: Vec<u8> = (0..300_000)
        .map(|_| {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng as u8
        })
        .collect();
    let bytes = smod(&[
        ("txt/marker.txt", b"from the server's mod"),
        ("noise.bin", &noise),
    ]);
    let game_mod = GameMod {
        name: "test".into(),
        hash: file_hash(&bytes),
    };
    game.game.game_mod = Some((game_mod.clone(), std::sync::Arc::new(bytes)));
    game_mod
}

#[test]
fn clients_get_the_servers_mod() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_pure", "1")]) else {
        return;
    };
    let game_mod = with_mod(&mut game);
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.05));
    // a map change while the mod is on its way: it keeps coming, then the new map
    while game
        .client(alice)
        .mod_progress()
        .is_none_or(|(got, _)| got == 0)
    {
        game.step(&[None]);
    }
    let (got, of) = game.client(alice).mod_progress().unwrap();
    assert!(got < of, "{got} of {of}");
    game.game.change_map(&mut game.server, "Arena").unwrap();
    for _ in 0..240 {
        game.step(&[None]);
    }

    let client = game.client(alice);
    assert_eq!(client.downloaded, [game_mod.path()]);
    assert_eq!(client.game_mod.as_ref(), Some(&game_mod));
    let marker = client.vfs.read_to_string("txt/marker.txt").unwrap();
    assert_eq!(marker, "from the server's mod");
    let world = client.world.as_ref().expect("the map loaded");
    assert_eq!(world.map.filename, "maps/Arena.pms");
    // the pure server took Alice: she plays
    assert!(client.net.own().is_some());
    let names: Vec<_> = game.game.world.soldiers.values().map(|s| &s.name).collect();
    assert_eq!(names, ["Alice"]);
}

#[test]
fn a_pure_server_wants_its_mod() {
    for pure in ["0", "1"] {
        let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_pure", pure)]) else {
            return;
        };
        with_mod(&mut game);
        let alice = game.join(hello("Alice"), Link::perfect());
        let bob = game.join(hello("Bob"), Link::perfect());
        // Bob doesn't take server mods (`cl_servermods 0`)
        game.client_mut(bob).servermods = false;
        for _ in 0..120 {
            game.step(&[None, None]);
        }
        let refused = |n: &Notice| *n == Notice::Refused(Refusal::WrongChecksum);
        assert_eq!(game.client(bob).notices.iter().any(refused), pure == "1");
        assert!(!game.client(alice).notices.iter().any(refused));
        let mut names: Vec<_> = game.game.world.soldiers.values().map(|s| &s.name).collect();
        names.sort();
        match pure {
            "1" => assert_eq!(names, ["Alice"]),
            _ => assert_eq!(names, ["Alice", "Bob"]),
        }
    }
}

#[test]
fn snapshots_come_as_deltas() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    for (bot, team) in [("Dutch", 1), ("Kruger", 2), ("Sniper", 1), ("Danko", 2)] {
        game.game
            .add_bot(&mut game.server, bot, team_from_num(team))
            .unwrap();
    }
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::lossy(5, 0.1));
    for _ in 0..1800 {
        game.step(&[Some(WALK), Some(WALK)]);
    }
    for (who, client) in [("perfect", alice), ("lossy", bob)] {
        let bytes = game.client(client).snapshots;
        eprintln!("{who}: {bytes:?}");
        // whole only until the first acknowledgement (and once in a while on the lossy link)
        assert!(bytes.full <= 20, "{who}: {bytes:?}");
        assert!(bytes.deltas > 600, "{who}: {bytes:?}");
        // and a fraction of the bytes
        assert!(
            bytes.delta_bytes * 3 < bytes.expanded_bytes,
            "{who}: {bytes:?}"
        );
    }
}
