//! The performance budget on the server (PLAN.md, M8): 16 players (in-memory clients) and 16
//! bots on ctf_Ash; the server's part of each tick (the players' messages, the match, a
//! snapshot delta for each player) is timed over two minutes of game.
//!
//! `cargo bench -p soldank-server --bench tick` (the game assets from `SOLDANK_ASSETS`, else
//! `../assets`); it fails when the mean takes longer than `SOLDANK_TICK_BUDGET_MS` (1).

#[path = "../tests/common/mod.rs"]
mod common;

use common::*;
use soldank_core::net::team_from_num;
use soldank_core::*;
use std::time::Duration;

const HUMANS: usize = 16;
const BOTS: usize = 16;
const WARMUP_TICKS: usize = 600;
const TICKS: usize = 7200;

fn main() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[("sv_gamemode", "3")]) else {
        return;
    };
    let names: Vec<String> = game.game_bots();
    for i in 0..BOTS {
        let team = team_from_num(1 + (i % 2) as u8);
        game.game
            .add_bot(&mut game.server, &names[i % names.len()], team)
            .unwrap();
    }
    for i in 0..HUMANS {
        let mut hello = hello(&format!("Player {i}"));
        if let soldank_core::net::ClientMessage::Hello { team, .. } = &mut hello {
            *team = Some(1 + (i % 2) as u8);
        }
        game.join(hello, Link::perfect());
    }

    // the players run back and forth, shooting
    let input = |tick: usize, i: usize| {
        let right = (tick / 120 + i).is_multiple_of(2);
        Some(Input {
            buttons: Buttons::FIRE | if right { Buttons::RIGHT } else { Buttons::LEFT },
            aim: Vec2::new(if right { 300.0 } else { -300.0 }, -20.0),
        })
    };
    for tick in 0..WARMUP_TICKS {
        let inputs: Vec<_> = (0..HUMANS).map(|i| input(tick, i)).collect();
        game.step(&inputs);
    }
    let mut times = Vec::with_capacity(TICKS);
    for tick in 0..TICKS {
        let inputs: Vec<_> = (0..HUMANS).map(|i| input(tick, i)).collect();
        times.push(game.step_timed(&inputs));
    }

    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let mean = ms(times.iter().sum::<Duration>()) / TICKS as f64;
    let at = |q: f64| ms(times[((TICKS as f64 * q) as usize).min(TICKS - 1)]);
    println!(
        "server, {HUMANS} players and {BOTS} bots on ctf_Ash, {TICKS} ticks: mean {mean:.3} ms, \
         median {:.3} ms, p99 {:.3} ms, worst {:.3} ms ({} soldiers)",
        at(0.5),
        at(0.99),
        ms(times[TICKS - 1]),
        game.game.world.soldiers.len(),
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
