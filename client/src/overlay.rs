//! The dev overlay (`--features dev`, F12 or `debug`): cvars, a world inspector and debug
//! drawing switches, with egui. While it's open the cursor is free, and egui gets the input
//! it wants before the game.

use super::*;
use egui_miniquad::EguiMq;
use gfx2d::Gfx2dContext;
use render::debug::DebugDraw;
use std::collections::BTreeMap;

pub struct Overlay {
    egui: EguiMq,
    pub open: bool,
    filter: String,
    /// Cvar values being typed, by name.
    edits: BTreeMap<String, String>,
    selected: Option<SoldierId>,
}

impl Overlay {
    pub fn new(context: &mut Gfx2dContext) -> Overlay {
        Overlay {
            egui: EguiMq::new(&mut *context.ctx),
            open: false,
            filter: String::new(),
            edits: BTreeMap::new(),
            selected: None,
        }
    }

    /// Opens or closes it: an open overlay frees the cursor.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        window::set_cursor_grab(!self.open);
        window::show_mouse(self.open);
    }

    /// egui has the pointer (the game shouldn't see mouse input).
    pub fn wants_pointer(&self) -> bool {
        self.open && self.egui.egui_ctx().wants_pointer_input()
    }

    /// egui has the keyboard (a text field is being typed in).
    pub fn wants_keyboard(&self) -> bool {
        self.open && self.egui.egui_ctx().wants_keyboard_input()
    }

    pub fn mouse_motion(&mut self, x: f32, y: f32) {
        self.egui.mouse_motion_event(x, y);
    }

    pub fn mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.egui.mouse_wheel_event(dx, dy);
    }

    pub fn mouse_button(&mut self, button: MouseButton, x: f32, y: f32, down: bool) {
        if down {
            self.egui.mouse_button_down_event(button, x, y);
        } else {
            self.egui.mouse_button_up_event(button, x, y);
        }
    }

    pub fn char(&mut self, c: char) {
        self.egui.char_event(c);
    }

    pub fn key(&mut self, keycode: KeyCode, keymods: KeyMods, down: bool) {
        if down {
            self.egui.key_down_event(keycode, keymods);
        } else {
            self.egui.key_up_event(keycode, keymods);
        }
    }

    /// Runs and draws the overlay over the frame.
    pub fn draw(
        &mut self,
        context: &mut Gfx2dContext,
        world: &World,
        console: &mut Console,
        debug: &mut DebugDraw,
    ) {
        context.end_pass();
        let (filter, edits, selected) = (&mut self.filter, &mut self.edits, &mut self.selected);
        self.egui.run(&mut *context.ctx, |_, ctx| {
            world_window(ctx, world, selected);
            cvars_window(ctx, console, filter, edits);
            egui::Window::new("Draw").show(ctx, |ui| {
                ui.checkbox(&mut debug.polygons, "polygons");
                ui.checkbox(&mut debug.colliders, "colliders");
                ui.checkbox(&mut debug.waypoints, "waypoints");
                ui.checkbox(&mut debug.spawns, "spawn points");
                ui.checkbox(&mut debug.skeletons, "skeletons");
            });
        });
        self.egui.draw(&mut *context.ctx);
    }
}

/// The match, the soldiers, and the one picked.
fn world_window(ctx: &egui::Context, world: &World, selected: &mut Option<SoldierId>) {
    egui::Window::new("World").show(ctx, |ui| {
        let game = &world.game;
        ui.label(format!(
            "tick {}  {}  {:?}",
            world.tick, world.map.mapname, world.config.game_mode
        ));
        ui.label(format!(
            "time left {}s  team scores {:?}",
            game.time_left / 60,
            &game.team_scores[1..5]
        ));
        let bullets = world.bullets.iter().filter(|b| b.active).count();
        let things = world.things.iter().filter(|t| t.active).count();
        ui.label(format!("bullets {bullets}  things {things}"));
        ui.separator();

        egui::Grid::new("soldiers").striped(true).show(ui, |ui| {
            for title in ["name", "team", "health", "position", "weapon", "k/d"] {
                ui.strong(title);
            }
            ui.end_row();
            for (id, s) in world.soldiers.iter() {
                let name = if s.dead_meat {
                    format!("{} (dead)", s.name)
                } else {
                    s.name.clone()
                };
                if ui.selectable_label(*selected == Some(id), name).clicked() {
                    *selected = Some(id);
                }
                ui.label(format!("{:?}", s.team));
                ui.label(format!("{:.0}", s.health));
                ui.label(format!("{:.0}, {:.0}", s.particle.pos.x, s.particle.pos.y));
                ui.label(s.primary_weapon().name);
                ui.label(format!("{}/{}", s.kills, s.deaths));
                ui.end_row();
            }
        });

        let Some(s) = selected.and_then(|id| world.soldiers.get(id)) else {
            return;
        };
        ui.separator();
        ui.strong(&s.name);
        let weapon = s.primary_weapon();
        let secondary = s.secondary_weapon();
        let lines = [
            format!(
                "velocity {:.3}, {:.3}",
                s.particle.velocity.x, s.particle.velocity.y
            ),
            format!(
                "health {:.1}  vest {:.0}  jets {}  on ground {}",
                s.health, s.vest, s.jets_count, s.on_ground
            ),
            format!(
                "weapon {} {}/{}  secondary {} {}",
                weapon.name, weapon.ammo_count, weapon.ammo, secondary.name, secondary.ammo_count
            ),
            format!(
                "grenades {}  bonus {:?}  flags {}",
                s.tertiary_weapon().ammo_count,
                s.bonus_style,
                s.flags
            ),
            format!(
                "stance {}  legs {:?} {}  body {:?} {}",
                s.position,
                s.legs_animation.id,
                s.legs_animation.frame,
                s.body_animation.id,
                s.body_animation.frame
            ),
        ];
        for line in lines {
            ui.label(line);
        }
        if let Some(brain) = &s.brain {
            let name = |id: Option<SoldierId>| {
                id.and_then(|id| world.soldiers.get(id))
                    .map_or("-".to_string(), |t| t.name.clone())
            };
            ui.label(format!(
                "bot: target {}  pissed off {}  waypoint {}",
                name(brain.target),
                name(brain.pissed_off),
                brain.current_waypoint
            ));
        }
    });
}

/// Every cvar, filtered by name; values are set like the console does (Enter applies).
fn cvars_window(
    ctx: &egui::Context,
    console: &mut Console,
    filter: &mut String,
    edits: &mut BTreeMap<String, String>,
) {
    egui::Window::new("Cvars")
        .default_open(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("filter");
                ui.text_edit_singleline(filter);
            });
            let cvars: Vec<(String, String, String)> = console
                .cvars
                .iter()
                .filter(|c| c.name.contains(filter.as_str()))
                .map(|c| {
                    (
                        c.name.clone(),
                        c.value.to_string(),
                        c.description.to_string(),
                    )
                })
                .collect();
            let mut apply = None;
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    egui::Grid::new("cvars").striped(true).show(ui, |ui| {
                        for (name, value, description) in &cvars {
                            ui.label(name).on_hover_text(description);
                            let text = edits.entry(name.clone()).or_insert_with(|| value.clone());
                            let response = ui.text_edit_singleline(text);
                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                apply = Some((name.clone(), text.clone()));
                            } else if !response.has_focus() && text != value {
                                // changed elsewhere (console, configs)
                                *text = value.clone();
                            }
                            ui.end_row();
                        }
                    });
                });
            if let Some((name, value)) = apply {
                if let Err(error) = console.cvars.set(&name, &value) {
                    console.print(error.to_string());
                }
                edits.remove(&name);
            }
        });
}
