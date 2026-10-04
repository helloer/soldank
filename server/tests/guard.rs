//! The server's guards against cheats and abuse, and the rules that came with them (bullet
//! time, colliding kits). Needs the game assets, else the tests are skipped.

mod common;

use common::*;
use soldank_core::net::*;
use soldank_core::{SoldierId, WeaponKind};

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

fn soldier(game: &TestMatch, name: &str) -> SoldierId {
    game.game
        .world
        .soldiers
        .iter()
        .find(|(_, s)| s.name == name)
        .map(|(id, _)| id)
        .unwrap()
}

fn here(game: &TestMatch, name: &str) -> bool {
    game.game.world.soldiers.values().any(|s| s.name == name)
}

fn ban_reason(game: &TestMatch, ip: &str) -> Option<String> {
    game.game.lists.ban_of(ip).map(|b| b.reason.clone())
}

fn left_for(client: &TestClient, why: LeaveReason) -> bool {
    client
        .notices
        .iter()
        .any(|n| matches!(n, Notice::Left { why: w, .. } if *w == why))
}

#[test]
fn a_flag_grabbed_in_base_and_brought_home_at_once_starts_the_servers_kick_vote() {
    for antimassflag in ["1", "0"] {
        let settings = [
            ("sv_gamemode", "3"),
            ("sv_votepercent", "50"),
            ("sv_antimassflag", antimassflag),
        ];
        let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
            return;
        };
        let alice = game.join(hello("Alice"), Link::perfect());
        let bob = game.join(hello("Bob"), Link::perfect());
        game.settle_steps(30);
        game.clients[alice].0.send(&ClientMessage::JoinTeam(1));
        game.clients[bob].0.send(&ClientMessage::JoinTeam(2));
        game.settle_steps(10);
        let num = game.client(alice).net.you.unwrap();
        // Alice took the flag from its base and scored within the second
        let id = soldier(&game, "Alice");
        let alice_soldier = &mut game.game.world.soldiers[id];
        alice_soldier.grabs_per_second = 1;
        alice_soldier.scores_per_second = 1;
        alice_soldier.grabbed_in_base = true;
        game.settle_steps(61);
        let vote = game.client(bob).notices.iter().find_map(|n| match n {
            Notice::VoteOn {
                kind,
                starter,
                reason,
            } => Some((kind.clone(), *starter, reason.clone())),
            _ => None,
        });
        if antimassflag == "0" {
            assert_eq!(vote, None);
            continue;
        }
        let reason = "Server: Possible cheating".to_string();
        assert_eq!(vote, Some((VoteKind::Kick(num), None, reason)));

        // the vote passes: banned a day, as the server's suspect
        let yes = ClientMessage::Vote {
            kind: VoteKind::Kick(num),
            reason: String::new(),
        };
        game.clients[bob].0.send(&yes);
        game.settle_steps(30);
        assert!(!here(&game, "Alice"));
        let ban = game.game.lists.ban_of("10.0.0.1").unwrap();
        assert_eq!(ban.reason, "Vote Kicked by Server");
        assert!(ban.time > soldank_server::admin::HOUR);
    }
}

/// Alice kills her teammate Bob (with one shot, friendly fire on).
fn team_kill(game: &mut TestMatch) {
    for client in 0..2 {
        game.clients[client].0.send(&ClientMessage::JoinTeam(1));
    }
    // past the spawn protection
    game.settle_steps(120);
    let (alice, bob) = (soldier(game, "Alice"), soldier(game, "Bob"));
    let world = &mut game.game.world;
    world.soldiers[bob].health = 1.0;
    let head = world.soldiers[bob].skeleton.pos(12);
    let shot = BulletState {
        weapon: weapon_index(WeaponKind::DesertEagles),
        pos: (head - soldank_core::vec2(30.0, 0.0)).into(),
        velocity: [20.0, 0.0],
        seed: 0,
    };
    world.receive_bullet(alice, &shot, 0);
    game.settle_steps(5);
}

#[test]
fn a_team_kill_is_a_warning() {
    let settings = [
        ("sv_gamemode", "3"),
        ("sv_friendlyfire", "1"),
        ("sv_punishtk", "1"),
        ("sv_warnings_tk", "3"),
    ];
    let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let _bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);
    team_kill(&mut game);
    assert!(game.game.world.soldiers[soldier(&game, "Bob")].dead_meat);
    assert!(texts(game.client(alice)).contains(&"TK Warning #1. Max Warnings: 3".to_string()));
    // not yet punished
    assert!(!game.game.world.soldiers[soldier(&game, "Alice")].dead_meat);
    assert!(here(&game, "Alice"));
}

#[test]
fn team_killers_are_punished_and_banned() {
    for punish in ["1", "0"] {
        let settings = [
            ("sv_gamemode", "3"),
            ("sv_friendlyfire", "1"),
            ("sv_punishtk", punish),
            ("sv_warnings_tk", "1"),
        ];
        let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
            return;
        };
        let alice = game.join(hello("Alice"), Link::perfect());
        let bob = game.join(hello("Bob"), Link::perfect());
        game.settle_steps(30);
        team_kill(&mut game);
        assert!(game.game.world.soldiers[soldier(&game, "Bob")].dead_meat);
        if punish == "0" {
            assert!(here(&game, "Alice"));
            assert!(
                !texts(game.client(alice))
                    .iter()
                    .any(|t| t.starts_with("TK"))
            );
            continue;
        }
        assert!(texts(game.client(alice)).contains(&"TK Warning #1. Max Warnings: 1".to_string()));
        let punished = "Alice has been punished for TeamKilling. (1/1)".to_string();
        assert!(texts(game.client(bob)).contains(&punished));
        assert!(!here(&game, "Alice"));
        assert_eq!(
            ban_reason(&game, "10.0.0.1").as_deref(),
            Some("Team Killing")
        );
        assert!(left_for(game.client(bob), LeaveReason::Kicked));
    }
}

#[test]
fn a_bad_ping_is_kicked() {
    let settings = [("sv_maxping", "50"), ("sv_warnings_ping", "0")];
    let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
        return;
    };
    // about 200 ms there and back
    let alice = game.join(hello("Alice"), Link::lossy(6, 0.0));
    game.settle_steps(7 * 60);
    assert!(!here(&game, "Alice"));
    assert_eq!(ban_reason(&game, "10.0.0.1").as_deref(), Some("Ping Kick"));
    assert!(left_for(game.client(alice), LeaveReason::Ping));
}

#[test]
fn a_good_ping_stays() {
    let settings = [("sv_maxping", "400"), ("sv_warnings_ping", "0")];
    let Some(mut game) = TestMatch::new("ctf_Ash", &settings) else {
        return;
    };
    let _alice = game.join(hello("Alice"), Link::lossy(6, 0.0));
    game.settle_steps(7 * 60);
    assert!(here(&game, "Alice"));
}

#[test]
fn chat_floods_are_kicked() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let bob = game.join(hello("Bob"), Link::perfect());
    game.settle_steps(30);
    // five lines a second wear off
    for _ in 0..5 {
        let line = ClientMessage::Chat {
            text: "hi".into(),
            team: false,
            radio: None,
        };
        game.clients[bob].0.send(&line);
        game.settle_steps(60);
    }
    assert!(here(&game, "Bob"));
    for i in 0..7 {
        let line = ClientMessage::Chat {
            text: format!("spam {i}"),
            team: false,
            radio: None,
        };
        game.clients[alice].0.send(&line);
    }
    game.settle_steps(70);
    assert!(!here(&game, "Alice"));
    assert_eq!(ban_reason(&game, "10.0.0.1").as_deref(), Some("Chat Flood"));
    assert!(left_for(game.client(bob), LeaveReason::Flooding));
}

#[test]
fn packet_floods_are_kicked_but_not_admins() {
    for admin in [false, true] {
        let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_warnings_flood", "0")]) else {
            return;
        };
        let alice = game.join(hello("Alice"), Link::perfect());
        game.settle_steps(30);
        if admin {
            game.game.command(&mut game.server, None, "adm Alice");
        }
        let junk = ClientMessage::Bullet(BulletState {
            weapon: weapon_index(WeaponKind::DesertEagles),
            pos: [0.0, 0.0],
            velocity: [0.0, 0.0],
            seed: 0,
        });
        for _ in 0..50 {
            game.clients[alice].0.send(&junk);
        }
        game.settle_steps(70);
        assert_eq!(here(&game, "Alice"), admin);
        if !admin {
            assert_eq!(
                ban_reason(&game, "10.0.0.1").as_deref(),
                Some("Flood Kicked")
            );
            assert!(left_for(game.client(alice), LeaveReason::Flooding));
        }
    }
}

#[test]
fn the_controls_alone_are_no_flood() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_warnings_flood", "0")]) else {
        return;
    };
    let _alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    let input = soldank_core::Input {
        buttons: soldank_core::Buttons::RIGHT,
        aim: soldank_core::vec2(500.0, 0.0),
    };
    for _ in 0..3 * 60 {
        game.step(&[Some(input)]);
    }
    assert!(here(&game, "Alice"));
}

#[test]
fn a_lone_kill_is_bullet_time() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_bullettime", "1")]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    assert_eq!(game.game.world.goal_ticks(), 60);
    // nobody far from the killer (nobody else at all)
    game.game.command(&mut game.server, None, "pkill Alice");
    assert_eq!(game.game.world.goal_ticks(), 20);
    let time_left = game.game.world.game.time_left;
    game.settle_steps(5);
    // the clocks stand still, on the client too
    assert_eq!(game.game.world.game.time_left, time_left);
    assert_eq!(game.client(alice).world.as_ref().unwrap().goal_ticks(), 20);
    game.settle_steps(30);
    assert_eq!(game.game.world.goal_ticks(), 60);
    assert!(game.game.world.game.time_left < time_left);
}

#[test]
fn no_bullet_time_without_it() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let _alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    game.game.command(&mut game.server, None, "pkill Alice");
    assert_eq!(game.game.world.goal_ticks(), 60);
}

#[test]
fn kits_collide_with_bullets_by_the_cvar() {
    for collide in ["0", "1"] {
        let Some(game) = TestMatch::new("ctf_Ash", &[("sv_kits_collide", collide)]) else {
            return;
        };
        let kits: Vec<bool> = game
            .game
            .world
            .things
            .iter()
            .filter(|t| t.active && t.kind == soldank_core::ThingKind::MedicalKit)
            .map(|t| t.collide_with_bullets)
            .collect();
        assert!(!kits.is_empty());
        assert!(kits.iter().all(|&c| c == (collide == "1")));
    }
}

#[test]
fn grabbing_the_enemy_flag_in_its_base_counts_for_the_check() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    game.settle_steps(30);
    game.clients[alice].0.send(&ClientMessage::JoinTeam(1));
    game.settle_steps(120);
    // early in a second: the server looks (and starts over) at its end
    while game.game.world.tick % 60 != 5 {
        game.step(&[]);
    }
    let id = soldier(&game, "Alice");
    let world = &mut game.game.world;
    let flag = world
        .things
        .iter()
        .position(|t| t.active && t.kind == soldank_core::ThingKind::BravoFlag)
        .unwrap();
    let at = world.things[flag].skeleton.pos(1);
    let particle = &mut world.soldiers[id].particle;
    particle.pos = at;
    particle.old_pos = at;
    game.settle_steps(3);
    let world = &game.game.world;
    assert_eq!(world.things[flag].holding, Some(id));
    let alice_soldier = &world.soldiers[id];
    assert_eq!(alice_soldier.grabs_per_second, 1);
    assert!(alice_soldier.grabbed_in_base);
    assert_eq!(alice_soldier.scores_per_second, 0);
}

/// Client `i`'s control for now with its soldier somewhere else (what a cheat sends).
fn moved_control(game: &TestMatch, i: usize, pos: soldank_core::Vec2) -> ClientMessage {
    let client = game.client(i);
    let world = client.world.as_ref().unwrap();
    let Some(ClientMessage::Control(mut control)) =
        client.net.control(world, &soldank_core::Input::default())
    else {
        panic!("no control");
    };
    control.pos = pos.into();
    ClientMessage::Control(control)
}

fn server_pos(game: &TestMatch, name: &str) -> soldank_core::Vec2 {
    game.game.world.soldiers[soldier(game, name)].particle.pos
}

fn own_pos(game: &TestMatch, i: usize) -> soldank_core::Vec2 {
    let client = game.client(i);
    let world = client.world.as_ref().unwrap();
    world.soldiers[client.net.own().unwrap()].particle.pos
}

#[test]
fn a_teleport_is_put_back() {
    for check in ["1", "0"] {
        let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_movecheck", check)]) else {
            return;
        };
        let alice = game.join(hello("Alice"), Link::perfect());
        let stand = Some(soldank_core::Input::default());
        for _ in 0..120 {
            game.step(&[stand]);
        }
        let before = server_pos(&game, "Alice");
        let far = before + soldank_core::vec2(600.0, -100.0);
        let teleport = moved_control(&game, alice, far);
        game.clients[alice].0.send(&teleport);
        game.settle_steps(3);
        if check == "0" {
            // Soldat's way: the client's word
            assert!(server_pos(&game, "Alice").distance(far) < 30.0);
            continue;
        }
        assert!(server_pos(&game, "Alice").distance(before) < 30.0);
        assert_eq!(game.game.corrections(), 1);
        // the client was told where it is, and its moves count again
        assert!(own_pos(&game, alice).distance(server_pos(&game, "Alice")) < 30.0);
        for _ in 0..60 {
            game.step(&[stand]);
        }
        assert_eq!(game.game.corrections(), 1);
        assert!(own_pos(&game, alice).distance(server_pos(&game, "Alice")) < 5.0);
    }
}

#[test]
fn a_speed_hack_is_put_back() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::perfect());
    let stand = Some(soldank_core::Input::default());
    for _ in 0..120 {
        game.step(&[stand]);
    }
    let start = server_pos(&game, "Alice");
    // twice as fast as anything can go, for half a second
    for tick in 1..=30 {
        let pos = start + soldank_core::vec2(40.0 * tick as f32, 0.0);
        let control = moved_control(&game, alice, pos);
        game.clients[alice].0.send(&control);
        game.step(&[]);
    }
    assert!(game.game.corrections() >= 1);
    assert!(server_pos(&game, "Alice").distance(start) < 100.0);
}

#[test]
fn changing_team_alive_respawns_the_player_on_its_client_too() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.0));
    let stand = Some(soldank_core::Input::default());
    game.settle_steps(30);
    game.clients[alice].0.send(&ClientMessage::JoinTeam(1));
    for _ in 0..150 {
        game.step(&[stand]);
    }
    let in_alpha = server_pos(&game, "Alice");
    game.clients[alice].0.send(&ClientMessage::JoinTeam(2));
    for _ in 0..60 {
        game.step(&[stand]);
    }
    // in Bravo's base, on the server and on the client alike
    let in_bravo = server_pos(&game, "Alice");
    assert!(
        in_bravo.distance(in_alpha) > 300.0,
        "{in_alpha} -> {in_bravo}"
    );
    assert!(own_pos(&game, alice).distance(in_bravo) < 5.0);
    assert_eq!(game.game.corrections(), 0);
}

#[test]
fn moves_count_again_after_a_map_change() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let alice = game.join(hello("Alice"), Link::lossy(3, 0.05));
    let walk = Some(soldank_core::Input {
        buttons: soldank_core::Buttons::RIGHT,
        aim: soldank_core::vec2(500.0, 0.0),
    });
    for _ in 0..200 {
        game.step(&[walk]);
    }
    game.game.change_map(&mut game.server, "ctf_Run").unwrap();
    for _ in 0..400 {
        game.step(&[walk]);
    }
    assert_eq!(game.game.corrections(), 0);
    assert!(own_pos(&game, alice).distance(server_pos(&game, "Alice")) < 30.0);
}
