//! The frags menu (`RenderFragsMenuTexts` and its sprites in `RenderInterface`), the team
//! box with the team scores, the end of game texts, the weapon stats menu
//! (`RenderWeaponStatsTexts`), and the player order they share with the HUD
//! (`SortPlayers`).

use super::hud::{Hud, HudState};
use super::*;
use crate::stats::{SLOTS, WeaponStats};
use gfx::Interface;

/// `FRAGSMENU_PLAYER_HEIGHT`
const PLAYER_HEIGHT: f32 = 15.0;
/// `BACKGROUND_WIDTH`: the `back` sprite is scaled relative to this.
const BACKGROUND_WIDTH: f32 = 64.0;

/// A player in the scoreboard order.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Ranked {
    pub id: SoldierId,
    pub kills: i32,
    pub deaths: i32,
    pub flags: i32,
}

/// `SortPlayers`: by captures, then kills, then fewer deaths; spectators last.
pub fn sorted_players(world: &World) -> Vec<Ranked> {
    soldank_core::sort_players(&world.soldiers)
        .into_iter()
        .map(|id| {
            let s = &world.soldiers[id];
            Ranked {
                id,
                kills: s.kills,
                deaths: s.deaths,
                flags: s.flags,
            }
        })
        .collect()
}

/// `SortedTeamScore`: the four teams by score, with their colours.
fn sorted_team_scores(world: &World) -> [(Team, i32, u32); 4] {
    let mut teams = [
        (Team::Alpha, 0xD20F05),
        (Team::Bravo, 0x050FD2),
        (Team::Charlie, 0xD2D205),
        (Team::Delta, 0x05D205),
    ]
    .map(|(team, color)| (team, world.game.team_scores[team as usize], color));
    for i in 0..4 {
        for j in i + 1..4 {
            if teams[j].1 > teams[i].1 {
                teams.swap(i, j);
            }
        }
    }
    teams
}

/// `TeamPlayersNum`: players in each team (index 0 stays empty, as in the client).
fn team_players(world: &World) -> [usize; 6] {
    let mut counts = [0; 6];
    for soldier in world.soldiers.values().filter(|s| s.active) {
        if soldier.team != Team::None {
            counts[soldier.team as usize] += 1;
        }
    }
    counts
}

fn hex(c: u32, a: u8) -> Color {
    rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, a)
}

fn argb(c: u32) -> Color {
    hex(c, (c >> 24) as u8)
}

/// The team box, the frags menu (`frags`), the end of game texts and the weapon stats.
pub fn render_scoreboard(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    sprites: &[Vec<Sprite>],
    state: &HudState,
    frags: bool,
    stats: Option<&WeaponStats>,
) {
    let mut hud = Hud::new(batch, fonts, sprites, state.size);
    let world = state.world;

    if world.config.game_mode.is_team_game() && state.layout.shown.team {
        render_team_box(&mut hud, state);
    }
    if world.game.paused() {
        // fragx, fragy
        let x = (state.size.x / 2.0 - 300.0).floor() - 25.0;
        hud.text(
            FontStyle::Menu,
            "Game paused",
            vec2(197.0 + x, 24.0),
            rgb(185, 250, 138),
        );
    }
    if frags {
        render_frags_menu(&mut hud, state);
    } else if let Some(stats) = stats {
        render_weapon_stats(&mut hud, state, stats);
    }
}

/// The stats slots' weapon names (`Gun.Name`, which Soldat keeps out of weapons.ini) and
/// icons (`TextureID`).
const STAT_WEAPONS: [(&str, Interface); SLOTS] = [
    ("USSOCOM", Interface::GunsSocom),
    ("Desert Eagles", Interface::GunsDeagles),
    ("HK MP5", Interface::GunsMp5),
    ("Ak-74", Interface::GunsAk74),
    ("Steyr AUG", Interface::GunsSteyr),
    ("Spas-12", Interface::GunsSpas),
    ("Ruger 77", Interface::GunsRuger),
    ("M79", Interface::GunsM79),
    ("Barrett M82A1", Interface::GunsBarrett),
    ("FN Minimi", Interface::GunsMinimi),
    ("XM214 Minigun", Interface::GunsMinigun),
    ("Combat Knife", Interface::GunsKnife),
    ("Chainsaw", Interface::GunsChainsaw),
    ("LAW", Interface::GunsLaw),
    ("Flamer", Interface::GunsFlamer),
    ("Bow", Interface::GunsBow),
    ("Flame Bow", Interface::GunsBow),
    ("Hands", Interface::GunsFist),
    ("Grenade", Interface::Nade),
    ("Clusters", Interface::ClusterNade),
    ("Stationary gun", Interface::GunsM2),
];

/// `NumberFormat`: thousands separated by commas.
fn number_format(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The weapon stats menu: shots, hits, accuracy, kills and headshots of each weapon the
/// player fired, where the frags menu goes.
fn render_weapon_stats(hud: &mut Hud, state: &HudState, stats: &WeaponStats) {
    // fragx, fragy
    let x = (state.size.x / 2.0 - 300.0).floor() - 25.0;
    let y = 0.0;
    let rows = stats.shown().count();

    let back = hud.sprite(Interface::Back).width / BACKGROUND_WIDTH;
    hud.stretch(
        Interface::Back,
        vec2(25.0 + x, 5.0 + y),
        vec2(590.0, rows as f32 * 20.0 + 85.0) * back,
        rgba(255, 255, 255, (f32::from(state.alpha) * 0.56).round() as u8),
    );

    let legend = rgba(170, 160, 200, 230);
    hud.text(
        FontStyle::Small,
        "% = Accuracy",
        vec2(x + 465.0, y + 15.0),
        legend,
    );
    hud.text(
        FontStyle::Small,
        "HS = Headshots",
        vec2(x + 465.0, y + 25.0),
        legend,
    );

    let header = rgb(255, 255, 230);
    for (text, dx) in [
        ("Weapon:", 70.0),
        (" %", 240.0),
        ("Shots:", 290.0),
        ("Hits:", 390.0),
        ("Kills (HS):", 470.0),
    ] {
        hud.text(FontStyle::Menu, text, vec2(x + dx, y + 40.0), header);
    }

    let white = rgb(255, 255, 255);
    let mut row_y = y + 50.0;
    for (i, stat) in stats.shown() {
        row_y += 20.0;
        let (name, icon) = STAT_WEAPONS[i];
        hud.draw(icon, vec2(x + 30.0, row_y), 1.0, white, (0.0, 1.0));
        let columns = [
            (name.to_string(), 90.0),
            (format!("{}%", stat.accuracy()), 245.0),
            (number_format(stat.shots), 295.0),
            (number_format(stat.hits), 395.0),
            (format!("{} ({})", stat.kills, stat.headshots), 475.0),
        ];
        for (text, dx) in columns {
            hud.text(FontStyle::Small, &text, vec2(x + dx, row_y), white);
        }
    }

    hud.text(
        FontStyle::Small,
        "(Updated every 10 seconds)",
        vec2(x + 230.0, row_y + 20.0),
        rgba(255, 255, 230, 100),
    );
}

/// The team box with the flags taken and the team scores (`RenderTeamScoreTexts`).
fn render_team_box(hud: &mut Hud, state: &HudState) {
    let world = state.world;
    let mode = world.config.game_mode;
    let alpha = state.layout.alpha;
    let team_box = state.layout.team_box;
    let (x, y) = (team_box.0 * hud.iscale, team_box.1);

    let back = hud.sprite(Interface::Back).width / BACKGROUND_WIDTH;
    hud.stretch(
        Interface::Back,
        vec2(x, y),
        vec2(57.0, 88.0) * back,
        rgba(255, 255, 255, (f32::from(alpha) * 0.56).round() as u8),
    );

    let flag = |kind: ThingKind| world.things.iter().find(|t| t.active && t.kind == kind);
    let noflag = |hud: &mut Hud, pos: Vec2, color: u32| {
        hud.draw(Interface::Noflag, pos, 1.0, hex(color, alpha), (0.0, 1.0));
    };
    let middle = vec2(((team_box.0 + 19.0) * hud.iscale).floor(), team_box.1 + 3.0);
    match (mode, flag(ThingKind::AlphaFlag), flag(ThingKind::BravoFlag)) {
        (GameMode::CaptureTheFlag, Some(alpha_flag), Some(bravo_flag)) => {
            let pos = vec2(((team_box.0 + 4.0) * hud.iscale).floor(), team_box.1 + 5.0);
            if !alpha_flag.in_base {
                noflag(hud, pos, 0xFF0000);
            }
            if !bravo_flag.in_base {
                noflag(hud, pos + vec2(31.0, 0.0), 0x0000FF);
            }
        }
        (GameMode::Infiltration, Some(_), Some(bravo_flag)) => {
            if !bravo_flag.in_base {
                noflag(hud, middle, 0x0000FF);
            }
        }
        (GameMode::HoldTheFlag, ..) => {
            let holder = flag(ThingKind::PointmatchFlag)
                .and_then(|f| f.holding)
                .and_then(|id| world.soldiers.get(id));
            if let Some(holder) = holder {
                let color = if holder.team == Team::Alpha {
                    0xFF0000
                } else {
                    0x0000FF
                };
                noflag(hud, middle, color);
            }
        }
        _ => {}
    }

    // the scores, best team first
    let (count, spacing, top) = if mode == GameMode::Teammatch {
        (4, 24.0, y)
    } else {
        (2, 40.0, y + 25.0)
    };
    for (i, (_, score, color)) in sorted_team_scores(world).iter().take(count).enumerate() {
        hud.text(
            FontStyle::Menu,
            &score.to_string(),
            vec2(x + 2.0, top + spacing * i as f32),
            hex(*color, state.alpha),
        );
    }
}

/// `FragMenuBottom`: the frags menu's height.
fn frags_menu_bottom(world: &World, hide_spectators: bool) -> f32 {
    let counts = team_players(world);
    let mut players = world.soldiers.values().filter(|s| s.active).count();
    if hide_spectators {
        players -= counts[Team::Spectator as usize];
    }
    let teams_shown = counts.iter().filter(|&&n| n > 0).count();
    70.0 + (players + 1) as f32 * PLAYER_HEIGHT + 15.0 * teams_shown as f32
}

/// `FragsScrollMax`: steps of 20 the frags menu scrolls when it's taller than the screen.
pub fn frags_scroll_max(world: &World, hide_spectators: bool) -> u8 {
    let bottom = frags_menu_bottom(world, hide_spectators);
    if bottom > 480.0 - 80.0 {
        ((bottom - 480.0 + 80.0) / 20.0).round_ties_even() as u8
    } else {
        0
    }
}

/// The frags menu: players by team with kills, deaths and ping, the server and the time
/// left; at the end of a match the winner.
fn render_frags_menu(hud: &mut Hud, state: &HudState) {
    let world = state.world;
    let team_game = world.config.game_mode.is_team_game();
    let sorted = sorted_players(world);
    let counts = team_players(world);
    let players = sorted.len();
    // fragx, fragy, scrolled up by `FragsScrollLev`
    let x = (state.size.x / 2.0 - 300.0).floor() - 25.0;
    let y = -20.0 * f32::from(state.menus.frags_scroll);
    let white = |a: u8| rgba(255, 255, 255, a);

    // background
    let hide = state.cvars.bool("ui_hidespectators");
    let bottom = frags_menu_bottom(world, hide);
    let back = hud.sprite(Interface::Back).width / BACKGROUND_WIDTH;
    hud.stretch(
        Interface::Back,
        vec2(25.0 + x, 5.0 + y),
        vec2(590.0, bottom) * back,
        white((f32::from(state.alpha) * 0.56).round() as u8),
    );
    // the pulsing arrow of a list that scrolls
    if frags_scroll_max(world, hide) != 0 {
        let pulse = ((5.1 * state.elapsed).sin() * 255.0)
            .round_ties_even()
            .abs() as u8;
        let pos = vec2(580.0 + x, 240.0);
        hud.draw(Interface::Scroll, pos, 1.0, white(pulse), (0.0, 1.0));
    }

    // team captions and lines, in the order alpha, bravo, charlie, delta, no team
    // team captions and lines, in the order alpha, bravo, charlie, delta, no team,
    // spectators (`ui_hidespectators` leaves them out)
    let spectators = counts[Team::Spectator as usize];
    let mut lines = [None; 6];
    if team_game {
        let (mut step, mut above) = (0.0, 0);
        for team in [1, 2, 3, 4, 0, 5] {
            let shown = if team == 5 && hide { 0 } else { counts[team] };
            if shown > 0 {
                lines[team] = Some(vec2(
                    x + 35.0,
                    y + 50.0 + step + above as f32 * PLAYER_HEIGHT,
                ));
                step += 20.0;
                above += shown;
            }
        }
    } else {
        lines[0] = Some(vec2(x + 35.0, y + 40.0 + PLAYER_HEIGHT));
        let below = (players - spectators + 1) as f32 * PLAYER_HEIGHT;
        lines[5] = Some(vec2(x + 35.0, y + 40.0 + below));
    }
    let team_color = |team: usize| match team {
        1 => rgb(255, 0, 0),
        2 => rgb(0, 0, 255),
        3 => argb(0xFFDFDF53),
        4 => argb(0xFF53DF53),
        5 => rgb(129, 52, 118),
        _ => argb(0xF1C3C3C3),
    };
    if team_game {
        for (team, line) in lines.iter().enumerate() {
            if let Some(line) = line {
                hud.line(
                    *line + vec2(0.0, 15.0),
                    565.0,
                    team_color(team),
                    state.pixel,
                );
            }
        }
    }

    // columns
    let points = match world.config.game_mode {
        GameMode::Deathmatch | GameMode::Teammatch => "Kills:",
        _ => "Points:",
    };
    let header = rgb(255, 255, 230);
    hud.text(FontStyle::Menu, points, vec2(x + 280.0, y + 40.0), header);
    hud.text(
        FontStyle::Menu,
        "Deaths:",
        vec2(x + 390.0, y + 40.0),
        header,
    );
    hud.text(FontStyle::Menu, "Ping:", vec2(x + 530.0, y + 40.0), header);

    // server name and info, time left
    let cvars = state.cvars;
    hud.text(
        FontStyle::Small,
        cvars.string("sv_hostname"),
        vec2(x + 30.0, y + 15.0),
        rgb(233, 180, 12),
    );
    let left = world.game.time_left.max(0);
    let (min, sec) = (left / 3600, (left % 3600) / 60);
    hud.text(
        FontStyle::Small,
        &format!("Time {min:02}:{sec:02}"),
        vec2(x + 485.0, y + 15.0),
        rgba(170, 160, 200, 230),
    );
    let clock = crate::platform::local_time("%-I:%M:%S %p");
    hud.text(
        FontStyle::Small,
        &clock,
        vec2(x + 485.0, y + 30.0),
        rgba(170, 160, 200, 230),
    );
    hud.text(
        FontStyle::Small,
        cvars.string("sv_info"),
        vec2(x + 30.0, y + 30.0),
        rgb(200, 150, 0),
    );

    // players count
    hud.text(
        FontStyle::Small,
        "Players",
        vec2(x + 330.0, y + 15.0),
        rgba(200, 190, 180, 240),
    );
    if team_game {
        let teams = if world.config.game_mode == GameMode::Teammatch {
            4
        } else {
            2
        };
        let colors = [
            rgba(233, 0, 0, 240),
            rgba(0, 0, 233, 240),
            rgba(233, 233, 0, 240),
            rgba(0, 233, 0, 240),
        ];
        for i in 0..teams {
            hud.text(
                FontStyle::Small,
                &counts[i + 1].to_string(),
                vec2(
                    x + 440.0 + 20.0 * (i / 2) as f32,
                    y + 10.0 + 10.0 * (i % 2) as f32,
                ),
                colors[i],
            );
        }
    } else {
        hud.text(
            FontStyle::Small,
            &players.to_string(),
            vec2(x + 450.0, y + 15.0),
            rgba(200, 190, 180, 240),
        );
    }

    // players
    let mut rows = [0usize; 6];
    let mut team_kills = [0; 6];
    for ranked in &sorted {
        let soldier = &world.soldiers[ranked.id];
        let spectator = soldier.is_spectator();
        if spectator && hide {
            continue;
        }
        let k = match () {
            _ if spectator => 5,
            _ if team_game => soldier.team as usize,
            _ => 0,
        };
        let Some(line) = lines[k] else { continue };
        let py = line.y + 20.0 + PLAYER_HEIGHT * rows[k] as f32;
        rows[k] += 1;
        team_kills[k] += soldier.kills;

        // the sprites sit a pixel lower without teams
        let sy = py + if team_game { 0.0 } else { 1.0 };
        let icon_alpha = white(state.alpha);
        if soldier.dead_meat && !spectator {
            hud.draw(
                Interface::Deaddot,
                vec2(32.0 + x, sy + 1.0),
                1.0,
                icon_alpha,
                (0.0, 1.0),
            );
        }
        if Some(ranked.id) == state.local_player() {
            hud.draw(
                Interface::Smalldot,
                vec2(31.0 + x, sy + 1.0),
                1.0,
                icon_alpha,
                (0.0, 1.0),
            );
        }
        if soldier.flags > 0 {
            let pos = vec2(x + 337.0, sy - 1.0);
            hud.draw(Interface::Flag, pos, 1.0, icon_alpha, (0.0, 1.0));
        }
        if soldier.brain.is_some() {
            let pos = vec2(x + 534.0, sy);
            hud.draw(Interface::Bot, pos, 1.0, icon_alpha, (0.0, 1.0));
        }

        let color = if spectator {
            rgba(220, 50, 200, 113)
        } else {
            hex(soldier.looks.shirt, 255)
        };
        hud.text(FontStyle::Small, &soldier.name, vec2(x + 44.0, py), color);
        hud.text(
            FontStyle::Small,
            &soldier.kills.to_string(),
            vec2(x + 284.0, py),
            color,
        );
        hud.text(
            FontStyle::Small,
            &soldier.deaths.to_string(),
            vec2(x + 394.0, py),
            color,
        );
        if soldier.flags > 0 {
            let flags = format!("x{}", soldier.flags);
            hud.text(FontStyle::Small, &flags, vec2(x + 348.0, py), color);
        }
        if soldier.brain.is_none() {
            let ping = soldier.ping.to_string();
            hud.text(FontStyle::Small, &ping, vec2(x + 534.0, py), color);
        }
    }

    // team names and their kills
    if team_game {
        let names = ["Player", "Alpha", "Bravo", "Charlie", "Delta", "Spectator"];
        let kills_colors = [
            None,
            Some(0xD20F05),
            Some(0x151FD9),
            Some(0xD2D205),
            Some(0x05D205),
            None,
        ];
        for (team, line) in lines.iter().enumerate() {
            let Some(line) = *line else { continue };
            hud.text(FontStyle::Small, names[team], line, team_color(team));
            if let Some(color) = kills_colors[team] {
                hud.text(
                    FontStyle::Small,
                    &team_kills[team].to_string(),
                    vec2(x + 284.0, line.y + 3.0),
                    hex(color, 0xDD),
                );
            }
        }
    }

    // RenderEndGameTexts
    if world.game.ended() && !world.game.paused() && players > 1 {
        if team_game {
            let scores = sorted_team_scores(world);
            let (text, color, dx) = if scores[0].1 == scores[1].1 {
                ("It's a tie", rgb(245, 245, 245), 137.0)
            } else {
                match scores[0].0 {
                    Team::Alpha => ("Alpha team wins", rgb(210, 15, 5), 50.0),
                    Team::Bravo => ("Bravo team wins", rgb(5, 15, 205), 50.0),
                    Team::Charlie => ("Charlie team wins", rgb(210, 210, 5), 50.0),
                    _ => ("Delta team wins", rgb(5, 210, 5), 50.0),
                }
            };
            // bottom aligned
            let height = hud.fonts.measure(FontStyle::Menu, text).y;
            let pos = vec2(x + dx, bottom + y - height);
            hud.text(FontStyle::Menu, text, pos, color);
        } else if let Some(best) = sorted.first()
            && best.kills > 0
        {
            let text = format!("{} wins", world.soldiers[best.id].name);
            let pos = vec2(x + 107.0, y + 24.0);
            hud.text(FontStyle::Menu, &text, pos, rgb(185, 250, 138));
        }
    }
}
