//! Tests against the real game assets.
//!
//! Assets are taken from `SOLDANK_ASSETS` or the workspace `assets/` directory. When none
//! are available the tests are skipped, unless `SOLDANK_REQUIRE_ASSETS` is set (CI).
//!
//! Snapshots are regression baselines of the current behaviour. Update them deliberately
//! with `INSTA_UPDATE=always cargo test -p soldank-core` and review the diff.

use soldank_core::assets::Vfs;
use soldank_core::*;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;

fn vfs() -> Option<Vfs> {
    let path = std::env::var_os("SOLDANK_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets"));

    if !path.exists() {
        if std::env::var_os("SOLDANK_REQUIRE_ASSETS").is_some() {
            panic!("assets required but not found at {}", path.display());
        }
        eprintln!("skipping: no assets at {}", path.display());
        return None;
    }

    let mut vfs = Vfs::new();
    vfs.mount(&path).unwrap();
    Some(vfs)
}

#[test]
fn parses_ctf_ash() {
    let Some(vfs) = vfs() else { return };
    let map = MapFile::load(&vfs, "ctf_Ash").unwrap();

    let mut out = String::new();
    writeln!(out, "name: {}", map.mapname).unwrap();
    writeln!(out, "texture: {}", map.texture_name).unwrap();
    writeln!(out, "polygons: {}", map.polygons.len()).unwrap();
    writeln!(
        out,
        "sectors: {} x {}",
        map.sectors_num, map.sectors_division
    )
    .unwrap();
    writeln!(out, "props: {}", map.props.len()).unwrap();
    writeln!(out, "scenery: {}", map.scenery.len()).unwrap();
    writeln!(out, "colliders: {}", map.colliders.len()).unwrap();
    writeln!(out, "spawnpoints: {}", map.spawnpoints.len()).unwrap();
    for s in map.spawnpoints.iter().take(4) {
        writeln!(out, "  ({}, {}) team {}", s.x, s.y, s.team).unwrap();
    }

    insta::assert_snapshot!(out);
}

#[test]
fn loads_every_bundled_map() {
    let Some(vfs) = vfs() else { return };
    let maps: Vec<String> = vfs
        .list("maps")
        .into_iter()
        .filter(|m| m.ends_with(".pms"))
        .collect();
    assert!(!maps.is_empty());

    for file in maps {
        let name = file.trim_end_matches(".pms");
        if let Err(e) = MapFile::load(&vfs, name) {
            panic!("{file}: {e}");
        }
    }
}

#[test]
fn loads_game_data() {
    let Some(vfs) = vfs() else { return };
    let data = GameData::load(&vfs).unwrap();

    let mut out = String::new();
    writeln!(
        out,
        "skeleton particles: {}",
        data.soldier_skeleton.particles().len()
    )
    .unwrap();
    writeln!(
        out,
        "skeleton constraints: {}",
        data.soldier_skeleton.constraints().len()
    )
    .unwrap();
    for anim in [
        Anim::Stand,
        Anim::Run,
        Anim::Jump,
        Anim::Prone,
        Anim::Roll,
        Anim::Change,
    ] {
        writeln!(
            out,
            "{anim:?}: {} frames",
            data.anims.get(anim).num_frames()
        )
        .unwrap();
    }

    insta::assert_snapshot!(out);
}

/// Runs a golden-trace input script (see `tests/golden/*.script`) through the simulation and
/// prints the soldier state in the same format as the opensoldat trace harness
/// (`tools/opensoldat-trace`).
fn iif<T>(c: bool, a: T, b: T) -> T {
    if c { a } else { b }
}

fn run_script(vfs: &Vfs, script: &str) -> String {
    run_script_with(vfs, &Arc::new(GameData::load(vfs).unwrap()), script)
}

/// Runs a trace script with already loaded game data (loading takes a while in debug).
fn run_script_with(vfs: &Vfs, data: &Arc<GameData>, script: &str) -> String {
    let mut ticks = 420u64;
    let mut every = 15u64;
    let mut precision = 2usize;
    let mut detail = false;
    let mut bullet_detail = false;
    let mut target: Option<Vec2> = None;
    let mut weapons = (WeaponKind::DesertEagles, WeaponKind::Chainsaw);
    let mut grenades = None;
    let mut realistic = false;
    let mut map_name = "ctf_Ash".to_string();
    let mut start: Option<Vec2> = None;
    let mut health: Option<f32> = None;
    let mut kits = false;
    let mut thing_detail = false;
    let mut thing_old = false;
    let mut thing_spawns: Vec<(u8, Vec2)> = Vec::new();
    let mut cvar_lines: Vec<(String, String)> = Vec::new();
    let mut target_weapon = WeaponKind::NoWeapon;
    let (mut team, mut target_team) = (Team::None, Team::None);
    let mut score_detail = false;
    let mut bots: Vec<(String, Team)> = Vec::new();
    let mut commands: Vec<(u64, PlayerCommand)> = Vec::new();
    let mut steps: Vec<(u64, Buttons, Vec2)> = Vec::new();

    for line in script.lines().map(str::trim) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            [] => {}
            [comment, ..] if comment.starts_with('#') => {}
            ["ticks", n] => ticks = n.parse().unwrap(),
            ["every", n] => every = n.parse().unwrap(),
            ["precision", n] => precision = n.parse().unwrap(),
            ["detail", n] => detail = *n == "1",
            ["bulletdetail", n] => bullet_detail = *n == "1",
            ["weapon", p, s] => {
                // Soldat's Guns indices start at 1, in WeaponKind order
                let kind = |n: &str| WeaponKind::values()[n.parse::<usize>().unwrap() - 1];
                weapons = (kind(p), kind(s));
            }
            ["map", name] => map_name = name.to_string(),
            ["cvar", name, value] => cvar_lines.push((name.to_string(), value.to_string())),
            ["team", n] => team = team_from_num(n),
            ["targetteam", n] => target_team = team_from_num(n),
            ["scoredetail", n] => score_detail = *n == "1",
            ["bot", name, n] => bots.push((name.to_string(), team_from_num(n))),
            ["command", tick, name] => commands.push((
                tick.parse().unwrap(),
                PlayerCommand::from_name(name).unwrap(),
            )),
            ["targetweapon", n] => {
                target_weapon = WeaponKind::values()[n.parse::<usize>().unwrap() - 1];
            }
            ["kits", n] => kits = *n == "1",
            ["thingdetail", n] => {
                thing_detail = *n == "1" || *n == "2";
                thing_old = *n == "2";
            }
            ["thing", style, x, y] => thing_spawns.push((
                style.parse().unwrap(),
                vec2(x.parse().unwrap(), y.parse().unwrap()),
            )),
            ["health", n] => health = Some(n.parse().unwrap()),
            ["start", x, y] => start = Some(vec2(x.parse().unwrap(), y.parse().unwrap())),
            ["realistic", n] => realistic = *n == "1",
            ["grenades", n] => grenades = Some(n.parse::<u8>().unwrap()),
            ["target", x, y] => target = Some(vec2(x.parse().unwrap(), y.parse().unwrap())),
            [tick, mask, x, y] => steps.push((
                tick.parse().unwrap(),
                Buttons::from_bits(mask.parse().unwrap()).unwrap(),
                vec2(x.parse().unwrap(), y.parse().unwrap()),
            )),
            _ => panic!("bad script line: {line}"),
        }
    }

    let data = data.clone();
    let map = MapFile::load(vfs, &map_name).unwrap();
    // server options, like run.sh passes them
    let mut cvars = config::Cvars::new();
    register_cvars(&mut cvars);
    cvars
        .set("sv_realisticmode", if realistic { "1" } else { "0" })
        .unwrap();
    // run.sh starts the server in deathmatch (`-sv_gamemode 0`)
    cvars.set("sv_gamemode", "0").unwrap();
    // run.sh starts the server with `-bots_chat 0` (scripts may turn it on)
    cvars.set("bots_chat", "0").unwrap();
    for (name, value) in &cvar_lines {
        cvars.set(name, value).unwrap();
    }
    let mut world = World::new(data.clone(), map, WorldConfig::from_cvars(&cvars, &data));
    let id = world.spawn_soldier();
    let soldier = &mut world.soldiers[id];
    soldier.team = team;
    soldier.bullet_count = 0; // pinned like the opensoldat harness
    if let Some(h) = health {
        soldier.health = h;
    }
    if let Some(pos) = start {
        soldier.particle.pos = pos;
        soldier.particle.old_pos = pos;
    }
    soldier.weapons[0] = world.config.weapons.get(weapons.0);
    soldier.weapons[1] = world.config.weapons.get(weapons.1);
    soldier.loadout = [weapons.0, weapons.1, WeaponKind::FragGrenade];
    if let Some(n) = grenades {
        soldier.weapons[2].ammo_count = n;
    }

    // idle second soldier to shoot at, set up like GoldenTrace.pas does
    let target = target.map(|pos| {
        let tid = world.spawn_soldier();
        let t = &mut world.soldiers[tid];
        t.team = target_team;
        t.particle.pos = pos;
        t.particle.old_pos = pos;
        t.particle.velocity = Vec2::ZERO;
        t.particle.force = Vec2::ZERO;
        t.weapons[0] = world.config.weapons.get(target_weapon);
        t.weapons[1] = Weapon::new(WeaponKind::NoWeapon, false);
        t.loadout = [target_weapon, WeaponKind::NoWeapon, WeaponKind::FragGrenade];
        t.ceasefire_counter = -1;
        t.bullet_count = 0;
        t.vest = 0.0;
        (tid, vec2(pos.x.round() - 100.0, pos.y.round()))
    });
    // like GoldenTrace.pas: the match starts now (wave counter at the full wave time),
    // RandSeed := 1, then kits and script things
    world.game.wave_respawn_counter = world.game.wave_respawn_time;
    world.rng.rand_seed = 1;
    let bots: Vec<SoldierId> = bots
        .iter()
        .map(|(name, team)| {
            let profile = BotProfile::load(vfs, name, &world.config.weapons).unwrap();
            world.spawn_bot(&profile, *team)
        })
        .collect();
    if kits {
        world.spawn_kits();
    }
    for (style, pos) in thing_spawns {
        world.create_thing(ThingKind::from_num(style).unwrap(), pos);
    }

    let mut out = String::new();
    let mut fired = 0;

    for tick in 0..ticks {
        let &(_, buttons, aim) = steps
            .iter()
            .rev()
            .find(|(start, ..)| *start <= tick)
            .unwrap();
        // same reseeding as the opensoldat harness (GoldenTrace.pas, TraceControl)
        world.rng.rand_seed = 1000 + tick as u32;
        let mut inputs = vec![(id, Input { buttons, aim })];
        if let Some((tid, target_aim)) = target {
            inputs.push((
                tid,
                Input {
                    buttons: Buttons::empty(),
                    aim: target_aim,
                },
            ));
        }
        for &(_, command) in commands.iter().filter(|(t, _)| *t == tick) {
            world.player_command(id, command, &mut Vec::new());
        }
        let events = world.step(&inputs);
        fired += events
            .iter()
            .filter(|e| matches!(e, GameEvent::BulletFired { owner, .. } if *owner == id))
            .count();

        if tick % every == 0 {
            let s = &world.soldiers[id];
            writeln!(
                out,
                "{tick:3}: pos ({:8.p$}, {:8.p$}) vel ({:6.p$}, {:6.p$}) legs {}/{} body {}/{} ground {} bullets {} ammo {}/{}/{}",
                P(s.particle.pos.x),
                P(s.particle.pos.y),
                P(s.particle.velocity.x),
                P(s.particle.velocity.y),
                s.legs_animation.id as usize,
                s.legs_animation.frame,
                s.body_animation.id as usize,
                s.body_animation.frame,
                s.on_ground,
                world.bullets.iter().filter(|b| b.active).count(),
                s.primary_weapon().ammo_count,
                s.primary_weapon().fire_interval_count,
                s.primary_weapon().reload_time_count,
                p = precision,
            )
            .unwrap();

            if detail {
                writeln!(
                    out,
                    "     force ({:.p$}, {:.p$}) old ({:.p$}, {:.p$}) permanent {} health {:.p$} jets {} bonus {}/{}",
                    P(s.particle.force.x),
                    P(s.particle.force.y),
                    P(s.particle.old_pos.x),
                    P(s.particle.old_pos.y),
                    s.on_ground_permanent,
                    P(s.health),
                    s.jets_count,
                    // BONUS_* numbers
                    match s.bonus_style {
                        Bonus::None => 0,
                        Bonus::Flamegod => 18,
                        Bonus::Predator => 19,
                        Bonus::Berserker => 21,
                    },
                    s.bonus_time,
                    p = precision,
                )
                .unwrap();
                let sk = |i: usize| s.skeleton.pos(i);
                writeln!(
                    out,
                    "     skeleton 1 ({:.p$}, {:.p$}) 12 ({:.p$}, {:.p$}) 15 ({:.p$}, {:.p$}) 16 ({:.p$}, {:.p$})",
                    P(sk(1).x),
                    P(sk(1).y),
                    P(sk(12).x),
                    P(sk(12).y),
                    P(sk(15).x),
                    P(sk(15).y),
                    P(sk(16).x),
                    P(sk(16).y),
                    p = precision,
                )
                .unwrap();
            }

            if let Some((tid, _)) = target {
                let t = &world.soldiers[tid];
                writeln!(
                    out,
                    "     target pos ({:.p$}, {:.p$}) vel ({:.p$}, {:.p$}) health {:.p$} dead {} legs {}/{} body {}/{}",
                    P(t.particle.pos.x),
                    P(t.particle.pos.y),
                    P(t.particle.velocity.x),
                    P(t.particle.velocity.y),
                    P(t.health),
                    t.dead_meat,
                    t.legs_animation.id as usize,
                    t.legs_animation.frame,
                    t.body_animation.id as usize,
                    t.body_animation.frame,
                    p = precision,
                )
                .unwrap();

                if detail {
                    let sk = |i: usize| t.skeleton.pos(i);
                    writeln!(
                        out,
                        "     target skeleton 1 ({:.p$}, {:.p$}) 9 ({:.p$}, {:.p$}) 12 ({:.p$}, {:.p$}) 21 ({:.p$}, {:.p$}) 22 ({:.p$}, {:.p$})",
                        P(sk(1).x), P(sk(1).y), P(sk(9).x), P(sk(9).y), P(sk(12).x), P(sk(12).y),
                        P(sk(21).x), P(sk(21).y), P(sk(22).x), P(sk(22).y),
                        p = precision,
                    )
                    .unwrap();
                }
            }

            for (i, &bid) in bots.iter().enumerate() {
                let b = &world.soldiers[bid];
                let c = &b.control;
                let keys = [
                    c.left,
                    c.right,
                    c.up,
                    c.down,
                    c.fire,
                    c.jets,
                    c.change_weapon,
                    c.throw_nade,
                    c.throw_weapon,
                    c.prone,
                    c.flag_throw,
                    c.reload,
                ]
                .iter()
                .enumerate()
                .fold(0, |mask, (bit, &on)| mask | (u32::from(on) << bit));
                let weapon = b.primary_weapon().kind;
                writeln!(
                    out,
                    "     bot {} pos ({:.p$}, {:.p$}) vel ({:.p$}, {:.p$}) health {:.p$} dead {} legs {}/{} body {}/{} weapon {} ammo {} keys {} aim {} {} waypoint {}",
                    i + 1,
                    P(b.particle.pos.x),
                    P(b.particle.pos.y),
                    P(b.particle.velocity.x),
                    P(b.particle.velocity.y),
                    P(b.health),
                    b.dead_meat,
                    b.legs_animation.id as usize,
                    b.legs_animation.frame,
                    b.body_animation.id as usize,
                    b.body_animation.frame,
                    WeaponKind::values().iter().position(|&k| k == weapon).unwrap() + 1,
                    b.primary_weapon().ammo_count,
                    keys,
                    c.mouse_aim_x,
                    c.mouse_aim_y,
                    b.brain.as_ref().map_or(0, |brain| brain.current_waypoint),
                    p = precision,
                )
                .unwrap();
            }
            if detail {
                // sprite numbers as in Soldat: soldiers in join order from 1
                let num = |id: Option<SoldierId>| {
                    id.and_then(|id| world.soldiers.keys().position(|k| k == id))
                        .map_or(0, |i| i + 1)
                };
                for (i, &bid) in bots.iter().enumerate() {
                    let b = &world.soldiers[bid];
                    let brain = b.brain.as_ref().unwrap();
                    writeln!(
                        out,
                        "     bot {} force ({:.p$}, {:.p$}) ground {} collider {} target {} pissed {} next {} time {} place {}",
                        i + 1,
                        P(b.particle.force.x),
                        P(b.particle.force.y),
                        b.on_ground,
                        b.collider_distance,
                        num(brain.target),
                        num(brain.pissed_off),
                        brain.next_waypoint,
                        brain.waypoint_time,
                        brain.one_place_count,
                        p = precision,
                    )
                    .unwrap();
                }
            }

            if score_detail {
                let g = &world.game;
                let mut line = format!(
                    "     score kills {} deaths {} team {}",
                    s.kills, s.deaths, s.team as u8
                );
                if let Some((tid, _)) = target {
                    let t = &world.soldiers[tid];
                    line += &format!(
                        " target kills {} deaths {} team {}",
                        t.kills, t.deaths, t.team as u8
                    );
                }
                writeln!(
                    out,
                    "{line} teamscores {} {} {} {} timeleft {} mapchange {} wave {}/{}",
                    g.team_scores[1],
                    g.team_scores[2],
                    g.team_scores[3],
                    g.team_scores[4],
                    g.time_left,
                    g.map_change_counter,
                    g.wave_respawn_counter,
                    g.wave_respawn_time,
                )
                .unwrap();
            }

            if thing_detail {
                for (i, t) in world.things.iter().enumerate().filter(|(_, t)| t.active) {
                    let pos = |n: usize| {
                        t.skeleton
                            .particles()
                            .get(n - 1)
                            .map_or(Vec2::ZERO, |p| p.pos)
                    };
                    writeln!(
                        out,
                        "     thing {} style {} pos ({:.p$}, {:.p$}) ({:.p$}, {:.p$}) timeout {} static {}",
                        i + 1,
                        t.kind as u8,
                        P(pos(1).x),
                        P(pos(1).y),
                        P(pos(2).x),
                        P(pos(2).y),
                        t.timeout,
                        t.static_type,
                        p = precision,
                    )
                    .unwrap();
                }
            }

            if thing_old {
                for (i, t) in world.things.iter().enumerate().filter(|(_, t)| t.active) {
                    let get = |n: usize, old: bool| {
                        t.skeleton
                            .particles()
                            .get(n - 1)
                            .map_or(Vec2::ZERO, |p| iif(old, p.old_pos, p.pos))
                    };
                    let (a, b, c, d) = (get(1, true), get(2, true), get(3, false), get(4, false));
                    writeln!(
                        out,
                        "     thing {} old ({:.p$}, {:.p$}) ({:.p$}, {:.p$}) ({:.p$}, {:.p$}) ({:.p$}, {:.p$}) interest {}",
                        i + 1,
                        P(a.x), P(a.y), P(b.x), P(b.y), P(c.x), P(c.y), P(d.x), P(d.y),
                        t.interest,
                        p = precision,
                    )
                    .unwrap();
                }
            }

            if bullet_detail {
                for b in world.bullets.iter().filter(|b| b.active) {
                    writeln!(
                        out,
                        "     bullet pos ({:.p$}, {:.p$}) vel ({:.p$}, {:.p$}) timeout {}",
                        P(b.particle.pos.x),
                        P(b.particle.pos.y),
                        P(b.particle.velocity.x),
                        P(b.particle.velocity.y),
                        b.timeout,
                        p = precision,
                    )
                    .unwrap();
                }
            }
        }
    }

    writeln!(out, "bullets fired: {fired}").unwrap();
    out
}

/// A float printed like FreePascal's `Format('%.Nf')`: exact ties round away from zero
/// (Rust rounds them to even).
struct P(f32);

impl std::fmt::Display for P {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let prec = f.precision().unwrap_or(6);
        // f32 -> f64 is exact and `{:.60}` prints enough digits of its exact expansion
        let exact = format!("{:.60}", f64::from(self.0).abs());
        let point = exact.find('.').unwrap();
        let mut digits: Vec<u8> = exact.bytes().filter(|c| *c != b'.').collect();
        let keep = point + prec;
        if digits[keep] >= b'5' {
            let mut i = keep;
            loop {
                if i == 0 {
                    digits.insert(0, b'1');
                    break;
                }
                i -= 1;
                if digits[i] == b'9' {
                    digits[i] = b'0';
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
        let int_len = digits.len() - (exact.len() - 1 - point);
        let int = std::str::from_utf8(&digits[..int_len]).unwrap();
        let frac = std::str::from_utf8(&digits[int_len..int_len + prec]).unwrap();
        let sign = if self.0.is_sign_negative() { "-" } else { "" };
        let s = if prec > 0 {
            format!("{sign}{int}.{frac}")
        } else {
            format!("{sign}{int}")
        };
        write!(f, "{s:>w$}", w = f.width().unwrap_or(0))
    }
}

fn team_from_num(n: &str) -> Team {
    match n {
        "1" => Team::Alpha,
        "2" => Team::Bravo,
        "3" => Team::Charlie,
        "4" => Team::Delta,
        "5" => Team::Spectator,
        _ => Team::None,
    }
}

const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");

/// Makes trace lines comparable across Pascal and Rust number formatting: negative zeros
/// ("-0.00", from tiny negative values) lose their sign and runs of spaces collapse.
fn normalize_trace_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '-' && chars.get(i + 1) == Some(&'0') && chars.get(i + 2) == Some(&'.') {
            let zeros = chars[i + 3..].iter().take_while(|c| **c == '0').count();
            let next = chars.get(i + 3 + zeros);
            if zeros > 0 && next.is_none_or(|c| !c.is_ascii_digit()) {
                i += 1; // drop the sign
                continue;
            }
        }
        if chars[i] == ' ' && out.ends_with(' ') {
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    out.replace("( ", "(")
}

/// Golden scripts that don't fully match opensoldat yet, with the number of leading trace
/// lines that must match. Raise the numbers as mechanics get ported, never lower them;
/// every other script must match completely.
const KNOWN_DIVERGENCES: &[(&str, usize)] = &[];

/// Compares soldank against traces recorded from opensoldat itself.
#[test]
fn golden_traces_match_opensoldat() {
    let Some(vfs) = vfs() else { return };
    let mut report = String::new();
    let mut failed = false;

    // the traces aren't in the repository for now
    let Ok(entries) = std::fs::read_dir(GOLDEN_DIR) else {
        eprintln!("no golden traces in {GOLDEN_DIR}: skipped");
        return;
    };
    let mut scripts: Vec<_> = entries
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".script").map(str::to_owned)
        })
        .collect();
    scripts.sort();
    let data = Arc::new(GameData::load(&vfs).unwrap());

    for name in &scripts {
        let script = std::fs::read_to_string(format!("{GOLDEN_DIR}/{name}.script")).unwrap();
        let expected = std::fs::read_to_string(format!("{GOLDEN_DIR}/{name}.opensoldat.txt"))
            .unwrap_or_else(|_| {
                panic!("{name}.opensoldat.txt missing, run tools/opensoldat-trace/run.sh")
            });
        let actual = run_script_with(&vfs, &data, &script);

        let expected: Vec<String> = expected.lines().map(normalize_trace_line).collect();
        let actual: Vec<String> = actual.lines().map(normalize_trace_line).collect();
        let matching = expected
            .iter()
            .zip(&actual)
            .take_while(|(e, a)| e == a)
            .count();
        let complete = matching == expected.len() && actual.len() == expected.len();

        let required = KNOWN_DIVERGENCES
            .iter()
            .find(|(script, _)| script == name)
            .map_or(expected.len(), |&(_, required)| required);

        writeln!(report, "{name}: {matching}/{} lines match", expected.len()).unwrap();
        if let (Some(e), Some(a)) = (expected.get(matching), actual.get(matching)) {
            writeln!(report, "  opensoldat: {e}\n  soldank:    {a}").unwrap();
        }

        if matching < required || (required == expected.len() && !complete) {
            failed = true;
        } else if matching > required {
            writeln!(
                report,
                "  improved! raise KNOWN_DIVERGENCES for {name} to {matching}"
            )
            .unwrap();
        }
    }

    eprintln!("{report}");
    assert!(!failed, "golden traces diverged:\n{report}");
}

/// Regression snapshot of soldank's own behaviour for the golden scripts, including the
/// parts that don't match opensoldat yet, so every behaviour change shows up in review.
#[test]
fn soldier_trace_ctf_ash() {
    let Some(vfs) = vfs() else { return };
    let script = std::fs::read_to_string(format!("{GOLDEN_DIR}/basic_moves.script")).unwrap();
    insta::assert_snapshot!(run_script(&vfs, &script));
}

/// Debug helper: prints the soldank trace for any script, to diff against the opensoldat
/// harness output for the same script.
#[test]
#[ignore = "run with SOLDANK_TRACE_SCRIPT=<file> ... -- --ignored --nocapture"]
fn debug_trace() {
    let Some(vfs) = vfs() else { return };
    let path = std::env::var("SOLDANK_TRACE_SCRIPT").expect("SOLDANK_TRACE_SCRIPT not set");
    print!(
        "{}",
        run_script(&vfs, &std::fs::read_to_string(path).unwrap())
    );
}
