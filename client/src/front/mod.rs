//! The front end (egui): the menus with a local game, joining a server, demos and the
//! settings; and the settings over a game (the escape menu's "5 Settings"). The match's own
//! menus (team, weapons, escape) stay Soldat's.

pub mod preview;
pub mod theme;

use crate::app::Session;
use crate::platform;
use crate::settings::{self, Kind, PAGES, RESOLUTIONS};
use egui::{
    Align, Align2, Button, ComboBox, DragValue, Frame, Grid, Layout, Margin, RichText, ScrollArea,
    Slider, TextEdit, vec2,
};
use gfx2d::mq::window;
use soldank_core::assets::Vfs;
use soldank_core::config::Console;
use std::path::{Path, PathBuf};
use theme::{ACCENT, DIM, TEXT, WARNING};

/// What the menus ask the program for.
pub enum Action {
    Start(Session),
    Quit,
    /// The settings over the game close.
    CloseOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Local,
    Join,
    Demos,
    Settings,
    Credits,
}

/// Soldat's game modes by `sv_gamemode`: their names, score limits and maps' prefixes.
const MODES: [(&str, &str, &str); 7] = [
    ("Deathmatch", "sv_dm_limit", ""),
    ("Pointmatch", "sv_pm_limit", ""),
    ("Teammatch", "sv_tm_limit", ""),
    ("Capture the Flag", "sv_ctf_limit", "ctf_"),
    ("Rambomatch", "sv_rm_limit", ""),
    ("Infiltration", "sv_inf_limit", "inf_"),
    ("Hold the Flag", "sv_htf_limit", "htf_"),
];

/// What each mode's score limit counts.
const LIMIT_UNITS: [&str; 7] = [
    "kills", "points", "kills", "captures", "kills", "points", "points",
];

/// Servers joined lately, at most.
const RECENT_SERVERS: usize = 8;

/// `bots_difficulty`
const DIFFICULTIES: [(i64, &str); 5] = [
    (10, "Impossible"),
    (50, "Hard"),
    (100, "Normal"),
    (200, "Easy"),
    (300, "Stupid"),
];

/// A demo in the config directory's `demos/`.
struct DemoEntry {
    name: String,
    path: PathBuf,
    map: String,
    local: bool,
    date: u64,
    size: u64,
}

pub struct FrontEnd {
    pub screen: Screen,
    /// The settings over a game.
    pub overlay: bool,
    /// Why the last session couldn't start.
    pub error: Option<String>,
    /// A key binding being set: the command waiting for its key.
    pub capturing: Option<&'static str>,
    action: Option<Action>,
    /// Escape: back a screen at the next frame.
    back: bool,
    /// In the browser: no servers, no files.
    web: bool,
    settings_page: usize,
    /// Settings pages changed since they were saved, and the taunts.
    dirty: Vec<bool>,
    taunts_dirty: bool,
    maps: Vec<String>,
    map: String,
    map_filter: String,
    /// Only the maps made for the game mode.
    mode_maps: bool,
    address: String,
    password: String,
    favorites: Vec<String>,
    /// Servers joined lately, the latest first.
    recent: Vec<String>,
    /// A session is being made: what to show meanwhile.
    pub loading: Option<String>,
    demos: Vec<DemoEntry>,
    /// A demo to delete once more clicked.
    delete: Option<usize>,
    /// The soldier and map previews.
    pub previews: preview::Previews,
    /// The custom interfaces (`custom-interfaces/`), and a game's to load again (it changed).
    interfaces: Vec<String>,
    pub reload_interface: bool,
}

/// The game's maps, by name.
fn map_names(vfs: &Vfs) -> Vec<String> {
    let mut maps: Vec<String> = vfs
        .list_names("maps")
        .into_iter()
        .filter_map(|f| {
            let lower = f.to_ascii_lowercase();
            lower
                .ends_with(".pms")
                .then(|| f[..f.len() - 4].to_string())
        })
        .collect();
    maps.sort_by_key(|m| m.to_ascii_lowercase());
    maps.dedup();
    maps
}

/// The demos in `demos/`, newest first.
fn demo_entries(config_dir: &Path) -> Vec<DemoEntry> {
    let Ok(dir) = std::fs::read_dir(config_dir.join("demos")) else {
        return Vec::new();
    };
    let mut demos: Vec<DemoEntry> = dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == crate::demo::DEMO_EXTENSION).then_some(())?;
            let file = std::fs::File::open(&path).ok()?;
            let size = file.metadata().ok()?.len();
            let header =
                soldank_core::demo::read_demo_header(std::io::BufReader::new(file)).ok()?;
            Some(DemoEntry {
                name: path.file_stem()?.to_string_lossy().into_owned(),
                map: header.map,
                local: header.local,
                date: header.date,
                size,
                path,
            })
        })
        .collect();
    demos.sort_by_key(|d| std::cmp::Reverse(d.date));
    demos
}

fn favorites_file(config_dir: &Path) -> PathBuf {
    config_dir.join("favorites.txt")
}

fn recent_file(config_dir: &Path) -> PathBuf {
    config_dir.join("recent.txt")
}

/// A list of servers, one a line.
fn load_servers(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

impl FrontEnd {
    pub fn new(vfs: &Vfs, config_dir: &Path, web: bool) -> FrontEnd {
        let maps = map_names(vfs);
        let map = maps
            .iter()
            .find(|m| m.eq_ignore_ascii_case("ctf_Ash"))
            .or(maps.first())
            .cloned()
            .unwrap_or_default();
        FrontEnd {
            screen: Screen::Home,
            overlay: false,
            error: None,
            capturing: None,
            action: None,
            back: false,
            web,
            settings_page: 0,
            dirty: vec![false; PAGES.len()],
            taunts_dirty: false,
            maps,
            map,
            map_filter: String::new(),
            mode_maps: true,
            address: String::new(),
            password: String::new(),
            favorites: load_servers(&favorites_file(config_dir)),
            recent: load_servers(&recent_file(config_dir)),
            loading: None,
            demos: demo_entries(config_dir),
            delete: None,
            previews: preview::Previews::default(),
            interfaces: vfs.list_dirs("custom-interfaces"),
            reload_interface: false,
        }
    }

    /// Back from a session: the menus as they were, the lists again.
    pub fn back_from_session(&mut self, vfs: &Vfs, config_dir: &Path) {
        self.overlay = false;
        self.capturing = None;
        if self.screen == Screen::Settings {
            self.screen = Screen::Home;
        }
        self.maps = map_names(vfs);
        self.demos = demo_entries(config_dir);
        self.interfaces = vfs.list_dirs("custom-interfaces");
    }

    pub fn take_action(&mut self) -> Option<Action> {
        self.action.take()
    }

    /// The settings over the game.
    pub fn show_overlay(&mut self) {
        self.overlay = true;
        self.screen = Screen::Settings;
    }

    /// A settings page changed (outside the screen: a key bound).
    pub fn changed(&mut self, page: usize) {
        self.dirty[page] = true;
    }

    /// Escape: back a screen (or to the game).
    pub fn escape(&mut self) {
        self.back = true;
    }

    fn go_back(&mut self, console: &Console, config_dir: &Path) {
        if self.screen == Screen::Settings {
            self.save_settings(console, config_dir);
        }
        if self.overlay {
            self.overlay = false;
            self.action = Some(Action::CloseOverlay);
        } else {
            self.screen = Screen::Home;
        }
    }

    fn switch(&mut self, screen: Screen, console: &Console, config_dir: &Path) {
        if self.screen == Screen::Settings && screen != Screen::Settings {
            self.save_settings(console, config_dir);
        }
        if screen == Screen::Demos {
            self.demos = demo_entries(config_dir);
            self.delete = None;
        }
        self.screen = screen;
    }

    /// The changed settings pages to their files.
    fn save_settings(&mut self, console: &Console, config_dir: &Path) {
        for (page, dirty) in PAGES.iter().zip(&mut self.dirty) {
            if std::mem::take(dirty)
                && let Err(error) = settings::save_page(console, config_dir, page)
            {
                tracing::warn!(%error, file = page.file, "cannot save settings");
            }
        }
        if std::mem::take(&mut self.taunts_dirty)
            && let Err(error) = settings::save_taunts(console, config_dir)
        {
            tracing::warn!(%error, "cannot save the taunts");
        }
    }

    /// This frame's menus.
    pub fn ui(&mut self, ctx: &egui::Context, console: &mut Console, vfs: &Vfs, config_dir: &Path) {
        let (_, height) = window::screen_size();
        theme::scale(ctx, height, window::dpi_scale());
        if std::mem::take(&mut self.back) {
            self.go_back(console, config_dir);
        }
        // changed settings go to their files at once (a closed window keeps them)
        if self.taunts_dirty || self.dirty.iter().any(|dirty| *dirty) {
            self.save_settings(console, config_dir);
        }
        if self.overlay {
            self.overlay_ui(ctx, console, config_dir);
            return;
        }
        // the picked map's picture (home, the local game)
        self.previews.want_map = Some(self.map.clone());
        if let Some(text) = self.loading.clone() {
            egui::CentralPanel::default()
                .frame(Frame::new())
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new(text).size(34.0).color(TEXT));
                    });
                });
            return;
        }
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(250.0)
            .frame(
                Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(Margin::same(20)),
            )
            .show(ctx, |ui| self.nav(ui, console, config_dir));
        egui::CentralPanel::default()
            .frame(Frame::new().inner_margin(Margin::symmetric(32, 24)))
            .show(ctx, |ui| {
                if let Some(error) = self.error.clone() {
                    Frame::new()
                        .fill(egui::Color32::from_rgb(58, 30, 32))
                        .stroke(egui::Stroke::new(1.0_f32, WARNING))
                        .corner_radius(6)
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(error).color(TEXT));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("Dismiss").clicked() {
                                        self.error = None;
                                    }
                                });
                            });
                        });
                    ui.add_space(12.0);
                }
                match self.screen {
                    Screen::Home => self.home(ui, console),
                    Screen::Local => self.local(ui, console, vfs),
                    Screen::Join => self.join(ui, config_dir),
                    Screen::Demos => self.demos(ui),
                    Screen::Settings => {
                        ui.heading("Settings");
                        ui.add_space(6.0);
                        self.settings_ui(ui, console);
                    }
                    Screen::Credits => credits(ui),
                }
            });
    }

    fn nav(&mut self, ui: &mut egui::Ui, console: &Console, config_dir: &Path) {
        ui.add_space(12.0);
        ui.label(RichText::new("SOLDANK").size(46.0).strong().color(ACCENT));
        ui.add_space(40.0);
        for (screen, label) in [
            (Screen::Home, "Home"),
            (Screen::Local, "Local game"),
            (Screen::Join, "Join a server"),
            (Screen::Demos, "Demos"),
            (Screen::Settings, "Settings"),
        ] {
            let button = Button::new(RichText::new(label).size(21.0))
                .selected(self.screen == screen)
                .min_size(vec2(210.0, 42.0));
            if ui.add(button).clicked() {
                self.switch(screen, console, config_dir);
            }
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).color(DIM));
            if !self.web {
                let quit =
                    Button::new(RichText::new("Quit").size(21.0)).min_size(vec2(210.0, 42.0));
                if ui.add(quit).clicked() {
                    self.save_settings(console, config_dir);
                    self.action = Some(Action::Quit);
                }
            }
            let color = if self.screen == Screen::Credits {
                ACCENT
            } else {
                DIM
            };
            let credits =
                Button::new(RichText::new("Credits").size(17.0).color(color)).frame(false);
            if ui.add(credits).clicked() {
                self.switch(Screen::Credits, console, config_dir);
            }
        });
    }

    fn home(&mut self, ui: &mut egui::Ui, console: &Console) {
        let cvars = &console.cvars;
        ui.heading(format!("Welcome back, {}", cvars.string("cl_player_name")));
        ui.add_space(18.0);
        // the match set up last, ready to play
        let mode = cvars.int("sv_gamemode").clamp(0, 6) as usize;
        let width = ui.available_width().min(640.0);
        if let Some((Some((texture, size)), _)) = self.previews.map(&self.map) {
            let size = size * (width / size.x);
            Frame::new()
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgb(70, 76, 90),
                ))
                .corner_radius(8)
                .show(ui, |ui| {
                    ui.add(egui::Image::new((texture, size)).corner_radius(8));
                });
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(MODES[mode].0)
                    .size(24.0)
                    .strong()
                    .color(ACCENT),
            );
            ui.label(RichText::new(&self.map).size(24.0).color(TEXT));
        });
        let bots: i64 = match mode {
            2 => ["alpha", "bravo", "charlie", "delta"].as_slice(),
            3 | 5 | 6 => ["alpha", "bravo"].as_slice(),
            _ => ["noteam"].as_slice(),
        }
        .iter()
        .map(|team| cvars.int(&format!("bots_random_{team}")))
        .sum();
        let difficulty = DIFFICULTIES
            .iter()
            .find(|(v, _)| *v == cvars.int("bots_difficulty"))
            .map_or("Custom", |(_, name)| name);
        let bots = match bots {
            0 => "no bots".to_string(),
            1 => format!("1 bot ({difficulty})"),
            n => format!("{n} bots ({difficulty})"),
        };
        let summary = format!(
            "{} {} · {} minutes · {bots}",
            cvars.int(MODES[mode].1),
            LIMIT_UNITS[mode],
            cvars.int("sv_timelimit") / 3600
        );
        ui.label(RichText::new(summary).color(DIM));
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            let play = theme::primary("Play", 26.0).min_size(vec2(220.0, 52.0));
            if ui.add_enabled(!self.map.is_empty(), play).clicked() {
                self.action = Some(Action::Start(Session::Local {
                    map: self.map.clone(),
                }));
            }
            let change = Button::new(RichText::new("Change map and rules").size(19.0))
                .min_size(vec2(0.0, 52.0));
            if ui.add(change).clicked() {
                self.screen = Screen::Local;
            }
        });
    }

    fn local(&mut self, ui: &mut egui::Ui, console: &mut Console, vfs: &Vfs) {
        ui.heading("Local game");
        ui.add_space(6.0);
        let cvars = &mut console.cvars;
        let mode = cvars.int("sv_gamemode").clamp(0, 6) as usize;
        ui.columns(2, |cols| {
            // the map, and the maps
            let ui = &mut cols[0];
            self.previews.want_map = Some(self.map.clone());
            let width = ui.available_width();
            match self.previews.map(&self.map) {
                Some((texture, title)) => {
                    if let Some((texture, size)) = texture {
                        let size = size * (width / size.x).min(190.0 / size.y);
                        ui.image((texture, size));
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&self.map).size(20.0).color(TEXT));
                        if !title.is_empty() {
                            ui.label(RichText::new(title).small().color(DIM));
                        }
                    });
                }
                None => {
                    ui.add_space(220.0);
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.map_filter)
                        .hint_text("Find a map")
                        .desired_width(180.0),
                );
                ui.checkbox(&mut self.mode_maps, "For this mode");
            });
            let prefix = MODES[mode].2;
            let filter = self.map_filter.to_ascii_lowercase();
            ScrollArea::vertical()
                .id_salt("maps")
                .max_height(ui.available_height() - 10.0)
                .show(ui, |ui| {
                    for map in &self.maps {
                        let lower = map.to_ascii_lowercase();
                        let for_mode = match prefix {
                            "" => !["ctf_", "inf_", "htf_"]
                                .iter()
                                .any(|p| lower.starts_with(p)),
                            prefix => lower.starts_with(prefix),
                        };
                        if (self.mode_maps && !for_mode) || !lower.contains(&filter) {
                            continue;
                        }
                        let selected = *map == self.map;
                        if ui.selectable_label(selected, map).clicked() {
                            self.map = map.clone();
                        }
                    }
                });

            // the match
            let ui = &mut cols[1];
            Grid::new("local")
                .num_columns(2)
                .spacing([24.0, 12.0])
                .show(ui, |ui| {
                    ui.label("Mode");
                    let mut chosen = mode;
                    ComboBox::from_id_salt("mode")
                        .selected_text(MODES[mode].0)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (i, (name, ..)) in MODES.iter().enumerate() {
                                ui.selectable_value(&mut chosen, i, *name);
                            }
                        });
                    if chosen != mode {
                        let _ = cvars.set_now("sv_gamemode", &chosen.to_string());
                    }
                    ui.end_row();

                    let limit = MODES[chosen].1;
                    ui.label("Score limit");
                    let mut value = cvars.int(limit);
                    if ui.add(DragValue::new(&mut value).range(1..=999)).changed() {
                        let _ = cvars.set_now(limit, &value.to_string());
                    }
                    ui.end_row();

                    ui.label("Time limit (minutes)");
                    let mut minutes = cvars.int("sv_timelimit") / 3600;
                    if ui
                        .add(DragValue::new(&mut minutes).range(1..=120))
                        .changed()
                    {
                        let _ = cvars.set_now("sv_timelimit", &(minutes * 3600).to_string());
                    }
                    ui.end_row();

                    let teams: &[(&str, &str)] = match chosen {
                        2 => &[
                            ("Alpha bots", "bots_random_alpha"),
                            ("Bravo bots", "bots_random_bravo"),
                            ("Charlie bots", "bots_random_charlie"),
                            ("Delta bots", "bots_random_delta"),
                        ],
                        3 | 5 | 6 => &[
                            ("Alpha bots", "bots_random_alpha"),
                            ("Bravo bots", "bots_random_bravo"),
                        ],
                        _ => &[("Bots", "bots_random_noteam")],
                    };
                    for (label, cvar) in teams {
                        ui.label(*label);
                        let mut n = cvars.int(cvar);
                        if ui.add(DragValue::new(&mut n).range(0..=15)).changed() {
                            let _ = cvars.set_now(cvar, &n.to_string());
                        }
                        ui.end_row();
                    }

                    ui.label("Bot difficulty");
                    let now = cvars.int("bots_difficulty");
                    let mut chosen = now;
                    let name = DIFFICULTIES
                        .iter()
                        .find(|(v, _)| *v == now)
                        .map_or("Custom", |(_, n)| n);
                    ComboBox::from_id_salt("difficulty")
                        .selected_text(name)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (value, name) in DIFFICULTIES {
                                ui.selectable_value(&mut chosen, value, name);
                            }
                        });
                    if chosen != now {
                        let _ = cvars.set_now("bots_difficulty", &chosen.to_string());
                    }
                    ui.end_row();

                    for (label, cvar) in [
                        ("Realistic", "sv_realisticmode"),
                        ("Survival", "sv_survivalmode"),
                        ("Advance", "sv_advancemode"),
                        ("Friendly fire", "sv_friendlyfire"),
                    ] {
                        ui.label(label);
                        let mut on = cvars.bool(cvar);
                        if ui.checkbox(&mut on, "").changed() {
                            let _ = cvars.set_now(cvar, if on { "1" } else { "0" });
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(16.0);
            let start = theme::primary("Start", 24.0).min_size(vec2(200.0, 48.0));
            if ui.add_enabled(!self.map.is_empty(), start).clicked() {
                self.action = Some(Action::Start(Session::Local {
                    map: self.map.clone(),
                }));
            }
        });
        let _ = vfs;
    }

    fn join(&mut self, ui: &mut egui::Ui, config_dir: &Path) {
        ui.heading("Join a server");
        ui.add_space(6.0);
        if self.web {
            ui.label("Online play is in the desktop version of the game.");
            return;
        }
        let mut connect = None;
        Grid::new("join")
            .num_columns(2)
            .spacing([24.0, 12.0])
            .show(ui, |ui| {
                ui.label("Address");
                ui.add(
                    TextEdit::singleline(&mut self.address)
                        .hint_text("host or host:port")
                        .desired_width(280.0),
                );
                ui.end_row();
                ui.label("Password");
                ui.add(
                    TextEdit::singleline(&mut self.password)
                        .password(true)
                        .desired_width(280.0),
                );
                ui.end_row();
            });
        ui.add_space(8.0);
        let address = self.address.trim().to_string();
        ui.horizontal(|ui| {
            let button = theme::primary("Connect", 22.0).min_size(vec2(160.0, 44.0));
            if ui.add_enabled(!address.is_empty(), button).clicked() {
                connect = Some(address.clone());
            }
            let add = ui.add_enabled(
                !address.is_empty() && !self.favorites.contains(&address),
                Button::new("Add to favourites"),
            );
            if add.clicked() {
                self.favorites.push(address.clone());
                self.save_favorites(config_dir);
            }
        });
        ui.add_space(20.0);
        ui.label(RichText::new("Favourites").size(20.0).color(TEXT));
        if self.favorites.is_empty() {
            ui.label(RichText::new("Servers you add to your favourites show here.").color(DIM));
        }
        let mut remove = None;
        Grid::new("favorites")
            .num_columns(3)
            .striped(true)
            .spacing([20.0, 6.0])
            .show(ui, |ui| {
                for (i, favorite) in self.favorites.iter().enumerate() {
                    ui.label(favorite);
                    if ui.button("Connect").clicked() {
                        connect = Some(favorite.clone());
                    }
                    if ui.button("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some(i) = remove {
            self.favorites.remove(i);
            self.save_favorites(config_dir);
        }
        if !self.recent.is_empty() {
            ui.add_space(20.0);
            ui.label(RichText::new("Recent").size(20.0).color(TEXT));
            Grid::new("recent")
                .num_columns(2)
                .striped(true)
                .spacing([20.0, 6.0])
                .show(ui, |ui| {
                    for server in &self.recent {
                        ui.label(server);
                        if ui.button("Connect").clicked() {
                            connect = Some(server.clone());
                        }
                        ui.end_row();
                    }
                });
        }
        if let Some(address) = connect {
            // the latest first
            self.recent.retain(|server| *server != address);
            self.recent.insert(0, address.clone());
            self.recent.truncate(RECENT_SERVERS);
            save_servers(&recent_file(config_dir), &self.recent);
            self.action = Some(Action::Start(Session::Join {
                address,
                password: self.password.clone(),
            }));
        }
    }

    fn save_favorites(&self, config_dir: &Path) {
        save_servers(&favorites_file(config_dir), &self.favorites);
    }

    fn demos(&mut self, ui: &mut egui::Ui) {
        ui.heading("Demos");
        ui.add_space(6.0);
        if self.web {
            ui.label("Demos are in the desktop version of the game.");
            return;
        }
        if self.demos.is_empty() {
            ui.label(RichText::new("No demos yet.").color(TEXT));
            ui.label(
                RichText::new(
                    "Press F5 in a game to record one, or have every map recorded: Settings, \
                     Game, \"Record a demo of each map\".",
                )
                .color(DIM),
            );
            return;
        }
        let mut play = None;
        let mut delete = None;
        ScrollArea::vertical().id_salt("demos").show(ui, |ui| {
            Grid::new("demos")
                .num_columns(6)
                .striped(true)
                .spacing([22.0, 8.0])
                .show(ui, |ui| {
                    for header in ["Name", "Map", "Game", "Recorded", "Size", ""] {
                        ui.label(RichText::new(header).color(DIM));
                    }
                    ui.end_row();
                    for (i, demo) in self.demos.iter().enumerate() {
                        ui.label(&demo.name);
                        ui.label(&demo.map);
                        ui.label(if demo.local {
                            "single player"
                        } else {
                            "server"
                        });
                        ui.label(platform::format_time(demo.date, "%Y-%m-%d %H:%M"));
                        ui.label(format!("{} KB", demo.size.div_ceil(1024)));
                        ui.horizontal(|ui| {
                            if ui.button("Play").clicked() {
                                play = Some(demo.path.clone());
                            }
                            let sure = self.delete == Some(i);
                            let text = if sure { "Sure?" } else { "Delete" };
                            if ui.button(RichText::new(text).color(WARNING)).clicked() {
                                delete = Some(i);
                            }
                        });
                        ui.end_row();
                    }
                });
        });
        if let Some(path) = play {
            self.action = Some(Action::Start(Session::Demo { path }));
        }
        if let Some(i) = delete {
            if self.delete == Some(i) {
                let demo = self.demos.remove(i);
                if let Err(error) = std::fs::remove_file(&demo.path) {
                    tracing::warn!(%error, "cannot delete the demo");
                }
                self.delete = None;
            } else {
                self.delete = Some(i);
            }
        }
    }

    /// The settings over a game: a window, the game dimmed behind.
    fn overlay_ui(&mut self, ctx: &egui::Context, console: &mut Console, config_dir: &Path) {
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            screen,
            0.0,
            egui::Color32::from_black_alpha(150),
        );
        egui::Window::new("Settings")
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .fixed_size([820.0, 540.0])
            .show(ctx, |ui| {
                self.settings_ui(ui, console);
                ui.separator();
                if ui
                    .add(Button::new(RichText::new("Back to the game").size(20.0)))
                    .clicked()
                {
                    self.go_back(console, config_dir);
                }
            });
    }

    /// The settings' tabs and the page's settings.
    fn settings_ui(&mut self, ui: &mut egui::Ui, console: &mut Console) {
        ui.horizontal(|ui| {
            let names = PAGES.iter().map(|page| page.name).chain(["Taunts"]);
            for (i, name) in names.enumerate() {
                let tab = Button::new(RichText::new(name).size(19.0))
                    .selected(self.settings_page == i)
                    .min_size(vec2(118.0, 34.0));
                if ui.add(tab).clicked() {
                    self.settings_page = i;
                    self.capturing = None;
                }
            }
        });
        ui.add_space(10.0);
        if self.settings_page == PAGES.len() {
            self.taunts_dirty |= self.taunts_ui(ui, console);
            return;
        }
        let page = &PAGES[self.settings_page];
        let mut changed = false;
        // the player's soldier as the settings dress it
        let player = self.settings_page == 0;
        ui.horizontal_top(|ui| {
            ScrollArea::vertical()
                .id_salt("settings")
                .max_height(ui.available_height() - 50.0)
                .show(ui, |ui| {
                    Grid::new(page.name)
                        .num_columns(2)
                        .striped(true)
                        .spacing([40.0, 10.0])
                        .min_col_width(200.0)
                        .show(ui, |ui| {
                            for setting in page.settings {
                                ui.label(setting.label);
                                changed |= self.setting_widget(ui, console, setting);
                                ui.end_row();
                            }
                        });
                });
            if player {
                self.previews.want_soldier = true;
                ui.add_space(20.0);
                if let Some((texture, size)) = self.previews.soldier() {
                    Frame::new()
                        .fill(egui::Color32::from_rgb(16, 19, 24))
                        .corner_radius(6)
                        .show(ui, |ui| ui.image((texture, size * 0.8)));
                }
            }
        });
        if changed {
            self.dirty[self.settings_page] = true;
        }
    }

    /// The taunt keys: Alt with a digit or letter says a line or plays a taunt.
    fn taunts_ui(&mut self, ui: &mut egui::Ui, console: &mut Console) -> bool {
        use settings::Taunt;
        ui.label(RichText::new("In a game, hold Alt and press the key.").color(DIM));
        ui.add_space(4.0);
        let mut changed = false;
        ScrollArea::vertical()
            .id_salt("taunts")
            .max_height(ui.available_height() - 50.0)
            .show(ui, |ui| {
                Grid::new("taunts")
                    .num_columns(3)
                    .striped(true)
                    .spacing([24.0, 8.0])
                    .show(ui, |ui| {
                        for key in settings::taunt_keys() {
                            ui.label(settings::key_label(&key));
                            let taunt = settings::taunt_of(console, &key);
                            let mut chosen = taunt.clone();
                            let name = match &taunt {
                                Taunt::None => "Nothing",
                                Taunt::Say(_) => "Say",
                                Taunt::Action(action) => settings::TAUNT_ACTIONS
                                    .iter()
                                    .find(|(a, _)| a == action)
                                    .map_or("", |(_, name)| name),
                                Taunt::Command(_) => "Command",
                            };
                            ComboBox::from_id_salt(&key)
                                .selected_text(name)
                                .width(215.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut chosen, Taunt::None, "Nothing");
                                    let say = match &taunt {
                                        Taunt::Say(text) => Taunt::Say(text.clone()),
                                        _ => Taunt::Say(String::new()),
                                    };
                                    ui.selectable_value(&mut chosen, say, "Say");
                                    for (action, name) in settings::TAUNT_ACTIONS {
                                        ui.selectable_value(
                                            &mut chosen,
                                            Taunt::Action(action),
                                            name,
                                        );
                                    }
                                    if let Taunt::Command(command) = &taunt {
                                        let same = Taunt::Command(command.clone());
                                        ui.selectable_value(&mut chosen, same, "Command");
                                    }
                                });
                            match &mut chosen {
                                Taunt::Say(text) => {
                                    let edit = TextEdit::singleline(text)
                                        .char_limit(crate::chat::MAX_CHAT_TEXT)
                                        .hint_text("What to say")
                                        .desired_width(380.0);
                                    ui.add(edit);
                                }
                                Taunt::Command(command) => {
                                    ui.add(TextEdit::singleline(command).desired_width(380.0));
                                }
                                Taunt::None | Taunt::Action(_) => {
                                    ui.label("");
                                }
                            }
                            if chosen != taunt {
                                settings::set_taunt(console, &key, &chosen);
                                changed = true;
                            }
                            ui.end_row();
                        }
                    });
            });
        changed
    }

    /// A setting's widget: `true` if it changed.
    fn setting_widget(
        &mut self,
        ui: &mut egui::Ui,
        console: &mut Console,
        setting: &settings::Setting,
    ) -> bool {
        let cvar = setting.cvar;
        let set = |console: &mut Console, value: &str| console.cvars.set_now(cvar, value).is_ok();
        match setting.kind {
            Kind::Text(max) => {
                let mut text = console.cvars.string(cvar).to_string();
                let edit = TextEdit::singleline(&mut text)
                    .char_limit(max)
                    .desired_width(260.0);
                // (as typed: a space at the end is the next word's)
                ui.add(edit).changed() && !text.trim().is_empty() && set(console, &text)
            }
            Kind::Toggle => {
                let mut on = console.cvars.bool(cvar);
                ui.checkbox(&mut on, "").changed() && set(console, if on { "1" } else { "0" })
            }
            Kind::Range(min, max, step) => {
                let mut value = settings::number(console, cvar);
                let slider = Slider::new(&mut value, min..=max).step_by(step);
                ui.add(slider).changed() && {
                    let text = match step < 1.0 {
                        true => format!("{value:.2}"),
                        false => format!("{value:.0}"),
                    };
                    set(console, &text)
                }
            }
            Kind::Choice(choices) => {
                let now = console.cvars.get(cvar).map(|c| c.value.to_string());
                let mut chosen = now.clone().unwrap_or_default();
                let name = choices
                    .iter()
                    .find(|(v, _)| Some(*v) == now.as_deref())
                    .map_or(chosen.clone(), |(_, name)| name.to_string());
                ComboBox::from_id_salt(cvar)
                    .selected_text(name)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (value, name) in choices {
                            ui.selectable_value(&mut chosen, value.to_string(), *name);
                        }
                    });
                let changed = Some(&chosen) != now.as_ref() && set(console, &chosen);
                if changed && cvar == "r_fullscreen" {
                    window::set_fullscreen(chosen != "0");
                }
                changed
            }
            Kind::Color => {
                let (r, g, b) = console.cvars.color(cvar);
                let mut rgb = [r, g, b];
                egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed()
                    && set(
                        console,
                        &format!("${:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
                    )
            }
            Kind::Key(command) => {
                let text = if self.capturing == Some(command) {
                    "Press a key or button (Esc: cancel)".to_string()
                } else {
                    let keys = settings::keys_of(console, command);
                    match keys.is_empty() {
                        true => "-".to_string(),
                        false => keys
                            .iter()
                            .map(|k| settings::key_label(k))
                            .collect::<Vec<_>>()
                            .join(", "),
                    }
                };
                if ui
                    .add(Button::new(text).min_size(vec2(260.0, 26.0)))
                    .clicked()
                {
                    self.capturing = Some(command);
                }
                false
            }
            Kind::Interface => {
                let now = console.cvars.string(cvar).to_string();
                let mut chosen = now.clone();
                ComboBox::from_id_salt(cvar)
                    .selected_text(&now)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        let names = std::iter::once("Default")
                            .chain(self.interfaces.iter().map(String::as_str));
                        for name in names {
                            ui.selectable_value(&mut chosen, name.to_string(), name);
                        }
                    });
                let changed = chosen != now && set(console, &chosen);
                self.reload_interface |= changed;
                changed
            }
            Kind::Resolution => {
                let size = |name: &str, default: i64| match console.cvars.int(name) {
                    0 => default,
                    size => size,
                };
                let now = (size("r_screenwidth", 1280), size("r_screenheight", 720));
                let mut chosen = now;
                ComboBox::from_id_salt(cvar)
                    .selected_text(format!("{} x {}", now.0, now.1))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (w, h) in RESOLUTIONS {
                            ui.selectable_value(&mut chosen, (w, h), format!("{w} x {h}"));
                        }
                    });
                chosen != now
                    && set(console, &chosen.0.to_string())
                    && console
                        .cvars
                        .set_now("r_screenheight", &chosen.1.to_string())
                        .is_ok()
            }
        }
    }
}

/// Writes a list of servers, one a line.
fn save_servers(file: &Path, servers: &[String]) {
    let mut text = servers.join("\n");
    text.push('\n');
    if let Err(error) = std::fs::write(file, text) {
        tracing::warn!(%error, file = %file.display(), "cannot save the servers");
    }
}

/// Who made the game and its content, and their licences.
fn credits(ui: &mut egui::Ui) {
    ui.heading("Credits");
    ui.add_space(14.0);
    let section = |ui: &mut egui::Ui, title: &str| {
        ui.add_space(10.0);
        ui.label(RichText::new(title).size(21.0).color(ACCENT));
    };
    section(ui, "Soldank");
    ui.label("Created by Paweł Drzazga (helloer), with Mariano Cuatrin and urraka.");
    ui.label(concat!(
        "Version ",
        env!("CARGO_PKG_VERSION"),
        ". The game's code is under the MIT licence."
    ));
    section(ui, "Soldat");
    ui.label("Created by Michał Marcinkowski (Transhuman Design).");
    section(ui, "Game content");
    ui.label(
        "Graphics, sounds, maps, animations and the interface font from OpenSoldat's base \
         (release v0.4): Soldat's content by Michał Marcinkowski and the OpenSoldat \
         contributors.",
    );
    ui.horizontal_wrapped(|ui| {
        ui.hyperlink_to("opensoldat/base", "https://github.com/opensoldat/base");
        ui.label("·");
        ui.hyperlink_to(
            "Contributors",
            "https://github.com/opensoldat/base/blob/master/Credits.md",
        );
        ui.label("·");
        ui.hyperlink_to(
            "Licensed under CC BY 4.0",
            "https://creativecommons.org/licenses/by/4.0/",
        );
    });
}
