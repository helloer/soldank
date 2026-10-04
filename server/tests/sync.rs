//! The match as the server has it, on the clients: cvars and weapons that change, deaths (the
//! body, its torn joints, the respawn counter), stances, helmets and radio messages; and the
//! server's own demos. Needs the game assets (`SOLDANK_ASSETS` or `../assets`), else the
//! tests are skipped.

mod common;

use common::*;
use soldank_core::demo::*;
use soldank_core::net::*;
use soldank_core::*;

fn hello_team(name: &str, team: u8) -> ClientMessage {
    let mut hello = hello(name);
    if let ClientMessage::Hello { team: wanted, .. } = &mut hello {
        *wanted = Some(team);
    }
    hello
}

fn soldier<'a>(world: &'a World, name: &str) -> &'a Soldier {
    world.soldiers.values().find(|s| s.name == name).unwrap()
}

#[test]
fn changed_cvars_reach_the_match_and_the_clients() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    let before = game.game.world.config.gravity;

    game.game.command(&mut game.server, None, "sv_gravity 0.1");
    game.settle_steps(5);
    let gravity = game.game.world.config.gravity;
    assert_ne!(gravity, before, "the match plays by it at once");
    let on_alice = |game: &TestMatch| game.client(alice).world.as_ref().unwrap().config.gravity;
    assert_eq!(on_alice(&game), gravity);

    // and on the next map
    game.game.change_map(&mut game.server, "ctf_Run").unwrap();
    game.settle_steps(30);
    assert_eq!(game.game.world.config.gravity, gravity);
    assert_eq!(on_alice(&game), gravity);
}

#[test]
fn loaded_weapons_reach_the_clients() {
    let Some(vfs) = vfs() else {
        return;
    };
    let ini = vfs.read_to_string("configs/weapons.ini").unwrap();
    let hard = ini.replace(
        "[Desert Eagles]\nDamage=1.81",
        "[Desert Eagles]\nDamage=3.5",
    );
    assert_ne!(hard, ini);
    let Some(mut game) =
        TestMatch::with_files("ctf_Ash", &[], vec![("configs/hard.ini", hard.into())])
    else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    let deagles = |world: &World| {
        world
            .config
            .weapons
            .get(WeaponKind::DesertEagles)
            .hit_multiply
    };
    assert_eq!(deagles(&game.game.world), 1.81);

    game.game.command(&mut game.server, None, "loadwep hard");
    game.settle_steps(5);
    assert_eq!(deagles(&game.game.world), 3.5);
    assert_eq!(deagles(game.client(alice).world.as_ref().unwrap()), 3.5);

    // newcomers get them with the welcome
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);
    assert_eq!(deagles(game.client(bob).world.as_ref().unwrap()), 3.5);
}

#[test]
fn bodies_fall_on_the_clients_as_on_the_server() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    for bot in ["Dutch", "Kruger", "Danko"] {
        game.game
            .add_bot(&mut game.server, bot, Team::None)
            .unwrap();
    }
    let alice = game.join(hello_team("Alice", 5), Link::perfect());
    let torn_of = |s: &Soldier| -> Vec<bool> {
        [2, 4, 20, 21, 23]
            .iter()
            .map(|c| s.skeleton.constraints()[c - 1].active)
            .collect()
    };
    let mut deaths = 0;
    for _ in 0..7200 {
        game.step(&[]);
        let notices = std::mem::take(&mut game.client_mut(alice).notices);
        for notice in notices {
            let Notice::Killed { victim, shot, .. } = notice else {
                continue;
            };
            deaths += 1;
            let world = game.client(alice).world.as_ref().unwrap();
            let on_alice = &world.soldiers[victim];
            let on_server = soldier(&game.game.world, &on_alice.name);
            // the same body, torn the same, respawning as soon
            assert!(on_server.dead_meat && on_alice.dead_meat);
            assert_eq!(torn_of(on_alice), torn_of(on_server), "{}", on_alice.name);
            let (a, b) = (on_alice.skeleton.pos(12), on_server.skeleton.pos(12));
            assert!(a.distance(b) < 10.0, "head: client {a}, server {b}");
            let counters = (on_alice.respawn_counter, on_server.respawn_counter);
            assert!(counters.0.abs_diff(counters.1) <= 2, "{counters:?}");
            // the killer's screen shows the server's shot
            assert_eq!(shot, game.game.world.shot);
        }
        if deaths >= 5 {
            break;
        }
    }
    assert!(deaths >= 5, "{deaths} deaths");
}

#[test]
fn a_head_shot_off_is_seen_everywhere() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    for bot in ["Dutch", "Kruger"] {
        game.game
            .add_bot(&mut game.server, bot, Team::None)
            .unwrap();
    }
    let alice = game.join(hello_team("Alice", 5), Link::perfect());
    game.settle_steps(60);
    let id_of = |world: &World, name: &str| {
        world
            .soldiers
            .iter()
            .find(|(_, s)| s.name == name)
            .map(|(id, _)| id)
            .unwrap()
    };
    let (dutch, kruger) = (
        id_of(&game.game.world, "Dutch"),
        id_of(&game.game.world, "Kruger"),
    );
    // (out of his spawn protection)
    let target = &mut game.game.world.soldiers[dutch];
    assert!(!target.dead_meat);
    target.vest = 0.0;
    target.health = 150.0;
    target.ceasefire_counter = -1;
    // Kruger's sniper round through Dutch's head, hard enough to take it off (the health
    // ends between the head chop's and the brutal death's)
    let gun = game.game.world.config.weapons.get(WeaponKind::Barrett);
    let head = game.game.world.soldiers[dutch].skeleton.pos(12);
    let speed = 40.0;
    let params = BulletParams {
        style: BulletStyle::Bullet,
        weapon: WeaponKind::Barrett,
        position: head - vec2(speed, 0.0),
        velocity: vec2(speed, 0.0),
        timeout: 60,
        hit_multiply: 350.0 / (speed * gun.modifier_head),
        team: Team::None,
        sprite: None,
        seed: Some(1),
        must_create: true,
        net: false,
        owner_immune: false,
    };
    assert!(game.game.world.create_bullet(&params, kruger));
    let mut death = None;
    for _ in 0..5 {
        game.step(&[]);
        death = death.or(game.client(alice).notices.iter().find_map(|n| match n {
            Notice::Killed { how, hit, .. } => Some((*how, *hit)),
            _ => None,
        }));
    }
    assert_eq!(death, Some((DeathKind::Headchop, 12)));
    let head_on =
        |world: &World| world.soldiers[id_of(world, "Dutch")].skeleton.constraints()[19].active;
    assert!(!head_on(&game.game.world));
    assert!(
        !head_on(game.client(alice).world.as_ref().unwrap()),
        "the head stays on"
    );
}

#[test]
fn respawn_counters_stay_with_the_server() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    // enough players for long waves, which go on a while before she comes: her client's
    // count starts from the server's
    for (bot, team) in [("Dutch", 1), ("Kruger", 2), ("Sniper", 1), ("Danko", 2)] {
        game.game
            .add_bot(&mut game.server, bot, team_from_num(team))
            .unwrap();
    }
    game.settle_steps(400);
    let alice = game.join(hello_team("Alice", 1), Link::perfect());
    game.settle_steps(60);
    game.clients[alice]
        .0
        .send(&ClientMessage::Command("kill".into()));
    game.settle_steps(30);
    let world = game.client(alice).world.as_ref().unwrap();
    let on_alice = soldier(world, "Alice");
    let on_server = soldier(&game.game.world, "Alice");
    assert!(on_server.dead_meat && on_alice.dead_meat);
    let counters = (on_alice.respawn_counter, on_server.respawn_counter);
    assert!(counters.0.abs_diff(counters.1) <= 2, "{counters:?}");
}

#[test]
fn going_prone_is_seen_everywhere() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::lossy(2, 0.0));
    let bob = game.join(hello("Bob"), Link::lossy(2, 0.0));
    let still = Input::default();
    let prone = Input {
        buttons: Buttons::PRONE,
        ..still
    };
    // on her feet first
    for _ in 0..180 {
        game.step(&[Some(still), Some(still)]);
    }
    let stances = |game: &TestMatch| {
        let on_bob = soldier(game.client(bob).world.as_ref().unwrap(), "Alice").position;
        let on_server = soldier(&game.game.world, "Alice").position;
        let on_alice = soldier(game.client(alice).world.as_ref().unwrap(), "Alice").position;
        (on_alice, on_server, on_bob)
    };
    // a tap of the key: down she goes, for everyone
    game.step(&[Some(prone), Some(still)]);
    for _ in 0..90 {
        game.step(&[Some(still), Some(still)]);
    }
    assert_eq!(stances(&game), (POS_PRONE, POS_PRONE, POS_PRONE));
    // and up again
    game.step(&[Some(prone), Some(still)]);
    for _ in 0..90 {
        game.step(&[Some(still), Some(still)]);
    }
    assert_eq!(stances(&game), (POS_STAND, POS_STAND, POS_STAND));
}

#[test]
fn a_lost_helmet_is_seen_everywhere() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let mut alpha = hello("Alice");
    if let ClientMessage::Hello { looks, .. } = &mut alpha {
        looks.head_cap = 1;
    }
    let _alice = game.join(alpha, Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(120);
    let helmet =
        |game: &TestMatch| soldier(game.client(bob).world.as_ref().unwrap(), "Alice").wear_helmet;
    assert_eq!(helmet(&game), 1);
    let id = game
        .game
        .world
        .soldiers
        .iter()
        .find(|(_, s)| s.name == "Alice")
        .map(|(id, _)| id)
        .unwrap();
    game.game.world.soldiers[id].wear_helmet = 0;
    game.settle_steps(5);
    assert_eq!(helmet(&game), 0);
}

#[test]
fn radio_messages_reach_the_team() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    let alice = game.join(hello_team("Alice", 1), Link::perfect());
    let bob = game.join(hello_team("Bob", 1), Link::perfect());
    let carol = game.join(hello_team("Carol", 2), Link::perfect());
    game.settle_steps(60);
    for i in [alice, bob, carol] {
        game.client_mut(i).notices.clear();
    }
    game.clients[alice].0.send(&ClientMessage::Chat {
        text: "Enemy flagger up".into(),
        team: true,
        radio: Some(11),
    });
    game.settle_steps(5);
    let radio = |game: &TestMatch, i: usize| {
        game.client(i).notices.iter().find_map(|n| match n {
            Notice::Chat { text, radio, .. } => Some((text.clone(), *radio)),
            _ => None,
        })
    };
    let heard = Some(("Enemy flagger up".to_string(), Some(11)));
    // the team hears it, the sender too (that's how it shows), the others not
    assert_eq!(radio(&game, bob), heard);
    assert_eq!(radio(&game, alice), heard);
    assert_eq!(radio(&game, carol), None);
}

#[test]
fn the_server_records_demos() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    game.game.config_dir = dir.path().to_path_buf();
    game.game
        .add_bot(&mut game.server, "Dutch", Team::None)
        .unwrap();
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);

    game.game.command(&mut game.server, None, "record test");
    game.settle_steps(300);
    game.game.command(&mut game.server, None, "stop");
    game.settle_steps(5);

    // nobody saw the recorder
    let world = game.client(alice).world.as_ref().unwrap();
    assert!(world.soldiers.values().all(|s| s.name != "Demo Recorder"));
    assert!(
        game.client(alice)
            .notices
            .iter()
            .all(|n| !matches!(n, Notice::Joined { .. }) || world.soldiers.len() == 2)
    );

    let file = std::fs::File::open(dir.path().join("demos").join("test.sdemo")).unwrap();
    let demo = read_demo(file).unwrap();
    assert_eq!(demo.header.map, "ctf_Ash");
    assert!(demo.frames.len() >= 290, "{} frames", demo.frames.len());
    // it plays like the game's: everyone's there, where the server had them
    let vfs = vfs().unwrap();
    let data = std::sync::Arc::new(GameData::load(&vfs).unwrap());
    let mut world = client_world(&vfs, &data, &demo.header.map, &demo.header.cvars);
    let mut net = NetClient::default();
    net.you = demo.header.you;
    for frame in &demo.frames {
        for message in &frame.messages {
            net.apply(&mut world, message.clone());
        }
        world.step(&[]);
    }
    let names: Vec<&str> = world.soldiers.values().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Dutch") && names.contains(&"Alice"),
        "{names:?}"
    );
}

/// Alice and Bob in a deathmatch, Bob out of his spawn protection everywhere; Bob's id on
/// Alice's client and on the server.
fn alice_and_bob() -> Option<(TestMatch, usize, SoldierId, SoldierId)> {
    let mut game = TestMatch::new("ctf_Ash", &[])?;
    let alice = game.join(hello("Alice"), Link::perfect());
    game.join(hello("Bob"), Link::perfect());
    game.settle_steps(120);
    let id_of = |world: &World| {
        world
            .soldiers
            .iter()
            .find(|(_, s)| s.name == "Bob")
            .map(|(id, _)| id)
            .unwrap()
    };
    let on_alice = id_of(game.client(alice).world.as_ref().unwrap());
    let on_server = id_of(&game.game.world);
    game.client_mut(alice).world.as_mut().unwrap().soldiers[on_alice].ceasefire_counter = -1;
    game.game.world.soldiers[on_server].ceasefire_counter = -1;
    Some((game, alice, on_alice, on_server))
}

/// Alice shoots through `at` from the left, `late` ticks late (`OwnerPingTick`).
fn shoot(world: &mut World, owner: SoldierId, at: Vec2, late: u8) {
    let params = BulletParams {
        style: BulletStyle::Bullet,
        weapon: WeaponKind::Ruger77,
        position: at - vec2(15.0, 0.0),
        velocity: vec2(30.0, 0.0),
        timeout: 30,
        hit_multiply: 0.01,
        team: Team::None,
        sprite: None,
        seed: Some(1),
        must_create: true,
        net: false,
        owner_immune: false,
    };
    assert!(world.create_bullet(&params, owner));
    if let Some(bullet) = world.bullets.iter_mut().rfind(|b| b.active) {
        bullet.owner_ping_tick = late;
    }
}

/// Where knock-back waits in a soldier's queue: the ticks to it.
fn pushes(soldier: &Soldier) -> Vec<usize> {
    let queued = soldier.next_push.iter().enumerate();
    queued
        .filter(|(_, p)| **p != Vec2::ZERO)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn a_client_takes_shots_where_their_owner_saw_them() {
    let Some((mut game, alice, bob, _)) = alice_and_bob() else {
        return;
    };
    let me = game.client(alice).net.own().unwrap();
    let world = game.client_mut(alice).world.as_mut().unwrap();
    let head = |s: &Soldier| s.skeleton.pos(12) - s.particle.pos;
    // Bob was 80 higher a while ago
    let soldier = &mut world.soldiers[bob];
    soldier.ping_ticks = 10;
    let was = soldier.particle.pos - vec2(0.0, 80.0);
    soldier.old_positions = [was; MAX_OLDPOS + 1];
    let through = was + head(soldier);

    // a shot 5 ticks late hits him there, and pushes him when the server's push comes:
    // half his ping, the shot's lateness and the tick after
    shoot(world, me, through, 5);
    world.step(&[]);
    assert_eq!(pushes(&world.soldiers[bob]), [10 / 2 + 5 + 1]);

    // a shot on time goes by where he is: nothing there
    world.soldiers[bob].next_push = [Vec2::ZERO; MAX_PUSHTICK + 1];
    world.soldiers[bob].old_positions = [was; MAX_OLDPOS + 1];
    shoot(world, me, through, 0);
    world.step(&[]);
    assert_eq!(pushes(&world.soldiers[bob]), Vec::<usize>::new());

    // nor does a bot's place lag
    world.soldiers[bob].bot = true;
    world.soldiers[bob].old_positions = [was; MAX_OLDPOS + 1];
    shoot(world, me, through, 5);
    world.step(&[]);
    assert_eq!(pushes(&world.soldiers[bob]), Vec::<usize>::new());
}

#[test]
fn the_server_pushes_at_once() {
    let Some((mut game, _, _, bob)) = alice_and_bob() else {
        return;
    };
    let world = &mut game.game.world;
    let me = world
        .soldiers
        .iter()
        .find(|(_, s)| s.name == "Alice")
        .map(|(id, _)| id)
        .unwrap();
    world.soldiers[bob].ping_ticks = 10;
    let through = world.soldiers[bob].skeleton.pos(12);
    shoot(world, me, through, 5);
    world.step(&[]);
    assert_eq!(pushes(&world.soldiers[bob]), [0]);
}

#[test]
fn a_late_shot_knows_how_late_it_is() {
    let Some((mut game, alice, bob, _)) = alice_and_bob() else {
        return;
    };
    let world = game.client_mut(alice).world.as_mut().unwrap();
    world.soldiers[bob].ping_ticks = 7;
    let pos = world.soldiers[bob].particle.pos;
    let shot = BulletState {
        weapon: weapon_index(WeaponKind::Barrett),
        pos: (pos + vec2(0.0, -300.0)).into(),
        velocity: [0.0, -10.0],
        seed: 5,
    };
    world.receive_bullet(bob, &shot, 0);
    let bullet = world.bullets.iter().rfind(|b| b.active).unwrap();
    // his ping and `PingTicksAdd`
    assert_eq!(bullet.owner_ping_tick, 7 + 2);
}

#[test]
fn a_hurt_soldier_stays_where_the_client_has_it() {
    let Some((mut game, alice, bob, _)) = alice_and_bob() else {
        return;
    };
    let world = game.client_mut(alice).world.as_mut().unwrap();
    let here = world.soldiers[bob].particle.pos;
    let mut state = SoldierState::of(2, &world.soldiers[bob]);
    state.pos = (here + vec2(100.0, 0.0)).into();
    // the snapshot that says he got hurt: not where it says
    state.health -= 20.0;
    world.apply_soldier_state(bob, &state, false, false);
    assert_eq!(world.soldiers[bob].particle.pos, here);
    // the next one: there
    world.apply_soldier_state(bob, &state, false, false);
    assert_eq!(world.soldiers[bob].particle.pos, here + vec2(100.0, 0.0));
}

#[test]
fn the_scoreboard_knows_connections_and_bots() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    game.game
        .add_bot(&mut game.server, "Dutch", Team::None)
        .unwrap();
    let alice = game.join(hello("Alice"), Link::perfect());
    game.join(hello("Bob"), Link::lossy(2, 0.3));
    game.settle_steps(600);
    let world = game.client(alice).world.as_ref().unwrap();
    let quality = |name: &str| soldier(world, name).connection_quality;
    assert!(quality("Alice") >= 95, "{}", quality("Alice"));
    assert!((1..90).contains(&quality("Bob")), "{}", quality("Bob"));
    assert_eq!(quality("Dutch"), 0);
    assert!(soldier(world, "Dutch").is_bot());
    assert!(!soldier(world, "Bob").is_bot());
}
