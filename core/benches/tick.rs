//! The performance budget (PLAN.md, M8): a full match, 32 bots fighting on ctf_Ash, should
//! cost well under a millisecond a tick. Two minutes of game are timed tick by tick after ten
//! seconds of warming up.
//!
//! `cargo bench -p soldank-core --bench tick` (the game assets from `SOLDANK_ASSETS`, else
//! `../assets`); it fails when the mean tick takes longer than `SOLDANK_TICK_BUDGET_MS` (1).

use soldank_core::assets::Vfs;
use soldank_core::config::Cvars;
use soldank_core::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const PLAYERS: usize = 32;
const WARMUP_TICKS: usize = 600;
const TICKS: usize = 7200;

fn main() {
    let path = std::env::var_os("SOLDANK_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets"));
    if !path.exists() {
        eprintln!("no game assets at {}: nothing to measure", path.display());
        return;
    }
    let mut vfs = Vfs::new();
    vfs.mount(&path).unwrap();
    let data = Arc::new(GameData::load(&vfs).unwrap());
    let mut cvars = Cvars::new();
    register_cvars(&mut cvars);
    cvars.set("sv_gamemode", "3").unwrap();
    let map = MapFile::load(&vfs, "ctf_Ash").unwrap();
    let mut world = World::new(data.clone(), map, WorldConfig::from_cvars(&cvars, &data));
    world.spawn_mode_things();
    world.spawn_kits();

    // the game's bots in turn, half a team each
    let names: Vec<String> = vfs
        .list("configs/bots")
        .into_iter()
        .filter_map(|f| f.strip_suffix(".bot").map(str::to_string))
        .filter(|name| !name.eq_ignore_ascii_case("dummy"))
        .collect();
    for i in 0..PLAYERS {
        let name = &names[i % names.len()];
        let profile = BotProfile::load(&vfs, name, &world.config.weapons).unwrap();
        let team = if i.is_multiple_of(2) {
            Team::Alpha
        } else {
            Team::Bravo
        };
        world.spawn_bot(&profile, team);
    }

    for _ in 0..WARMUP_TICKS {
        world.step(&[]);
    }
    let mut times = Vec::with_capacity(TICKS);
    let (mut kills, mut shots) = (0, 0);
    for _ in 0..TICKS {
        let start = Instant::now();
        let events = world.step(&[]);
        times.push(start.elapsed());
        kills += events
            .iter()
            .filter(|e| matches!(e, GameEvent::Killed { .. }))
            .count();
        shots += events
            .iter()
            .filter(|e| matches!(e, GameEvent::BulletFired { .. }))
            .count();
    }

    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let mean = ms(times.iter().sum::<Duration>()) / TICKS as f64;
    let at = |q: f64| ms(times[((TICKS as f64 * q) as usize).min(TICKS - 1)]);
    println!(
        "{PLAYERS} bots on ctf_Ash, {TICKS} ticks: mean {mean:.3} ms, median {:.3} ms, \
         p99 {:.3} ms, worst {:.3} ms ({kills} kills, {shots} shots, {} soldiers)",
        at(0.5),
        at(0.99),
        ms(times[TICKS - 1]),
        world.soldiers.len(),
    );

    let budget: f64 = std::env::var("SOLDANK_TICK_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    if mean > budget {
        eprintln!("over the budget of {budget} ms a tick");
        std::process::exit(1);
    }
}
